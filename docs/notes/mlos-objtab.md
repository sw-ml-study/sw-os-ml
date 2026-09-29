# mlos-objtab

The ML object table: a fixed-capacity map from `ObjectId` to what the
kernel knows about that object. Objects layer; a pure data structure with
no `unsafe`, no allocation and no I/O. Linked from
`crates/mlos-objtab/src/lib.rs`, `meta.rs`, `probe.rs` and `table.rs`.

## Why it exists

Where a conventional kernel has a page table, MLOS has this. A page table
entry answers "where is this page and is it dirty". An entry here answers
"what would getting this back cost, when is it next wanted, and how many
sessions are waiting on it", which are the questions every decision in
`docs/PRD.md` turns on. Every field in `ObjectMeta` exists to answer a
question a page-based operating system cannot ask.

It is a pure data structure on purpose: providers fetch, policies decide,
this only remembers. That is what makes it testable on the host without a
VM, and what lets one table type serve both the kernel and `mlos-sim`.

## Fixed capacity, constant-initialised

The table is fixed-size because it lives in the kernel and there is no
allocator, and because a table that can grow can grow on the fault path,
which is the one place that must not allocate. `Table::EMPTY` is a
constant rather than a constructor so it can initialise a `static`: the
kernel's table has to exist before there is anything to allocate it with.

## Open addressing with linear probing

A tree would be the obvious alternative and is the wrong one. The fast
path is an exact-match lookup on the model-fault path, and it must be a
couple of cache lines rather than a traversal. Linear probing keeps a
miss in the same cache line as the hit it displaced.

`ObjectId` is structured (class, model, layer, tensor, tile), so its low
bits are anything but random: a dense layer sweep walks consecutive
tensors, and masking the raw value would pile a whole layer into adjacent
slots. `probe::start` uses Fibonacci hashing (multiply by the golden-ratio
constant, take the high bits) to mix the high bits down, which is what
makes a sweep spread out instead of collide.

`Slot::Removed` is distinct from `Slot::Vacant` because linear probing
walks until it finds a vacancy. Turning a removed slot into a vacancy
would cut the chain and hide every object that probed past it. `insert`
reuses the first `Removed` slot it passes, but only after confirming the
id is not already live further along the chain.

`insert` returns `false` when the table is full, dropping the
registration rather than evicting something the caller did not ask to
lose. Eviction is a policy decision, and the table makes none.

## `at`: slot order is arbitrary but stable

`Table::at` exposes slot order, which is hash order and therefore
arbitrary. What matters is that it is stable: a policy asked twice about
an unchanged table must name the same victim, or a replay stops being
deterministic and every kernel/simulator comparison becomes an argument.

It is indexed rather than iterable because that is the shape
`mlos-policy`'s `Residency` asks for, and it asks for it because a kernel
cannot hand out a borrow into a table it is about to mutate.

## `NextUse::At` is a position, not a distance

`NextUse` is the field that does not exist in any page-based operating
system, and the one that makes this whole design worth building. It is
three-way rather than a number because the two kinds of knowledge differ
in kind, not degree: a dense layer sweep yields an exact distance, an MoE
router yields a distribution. A policy may act on `At` with certainty and
on `Probability` only as a hint, and collapsing them to one number would
silently license the wrong decision.

`At` holds a position, and the difference from a distance is the whole
reason `ml_stream_advance` can be O(1). A distance is measured from
somewhere, so advancing a stream by one step would make every resident
object's distance wrong and the kernel would have to walk the table to fix
them, a per-token cost over the very structure it would be walking. A
position is measured from the stream's origin and does not move when the
cursor does, so advancing is a single increment and the subtraction
happens once, in the policy, for the handful of objects it actually
compares.

## `home` and `handle` are distinct from `tier` and `resident_at`

`tier` moves: an object faulted in becomes `Warm`. Evicting it needs
somewhere to put it back, and guessing `Cold` would send a recomputable
activation to a block store that never had it. So `home` is set once, when
the object is registered, and never changed.

`handle` is where the object's home is, as its provider understands
"where": a DRAM address, a block number, a recipe for recomputing it. It
is opaque on purpose. The table records which provider to ask and what to
tell it; only the provider knows what the number means. `resident_at` is
where the object happens to be now, or zero. The two are both needed:
collapsing them would mean an object could only be fetched once.

## Precision is a choice, not decoration

`Precision` is recorded because precision is a choice the kernel can make.
Demoting cold KV from FP16 to Q4 is rung 2 of the degradation ladder, and
it is the reason an ML workload can be made cheaper under pressure where
an ordinary process cannot.

## `Tier::Stream`

The tier a page-based system cannot express. A streamed object is not "in
memory" in a way you could point at; it is a scheduled flow that compute
is arranged around.

## `CostNs::IMPOSSIBLE`

Weights have no recompute cost: there is no computation that produces
them. Eviction must be able to tell "expensive" from "impossible", because
it may choose the first and never the second. `u32::MAX` is that value.

## `placed_tick` and `used_tick` live in the table

`placed_tick` is insertion order, which is the only thing FIFO knows;
`used_tick` is recency, which is what LRU knows and all it knows. They are
distinct for exactly the case the two disagree about: an object placed
early and used recently.

They are in the table rather than inside a policy because `docs/design.md`
s.2 makes it a rule that a policy holds no state the table does not own.
A FIFO keeping its own queue could not be one piece of code running both
in the kernel and in the simulator, and that sameness is the only thing
making the comparison worth anything.

## `share_count`

The number parameter-major scheduling is built on: four sessions waiting
on one layer should cause one read, not four.

## `Mutability::CowOverlay`

Shared until written, then private. How a model fork stays cheap:
sessions share one copy of the weights until one of them writes.
