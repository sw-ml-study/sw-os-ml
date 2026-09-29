# mlos-synth

The synthetic model: its shape, the ids and metadata of its objects, and
the tiers they come from. Objects layer, `no_std`. Linked from
`crates/mlos-synth/src/lib.rs`, `model.rs`, `disk.rs`, `kv.rs` and
`tiers.rs`.

## Definition, not instance

This crate is definition only. The one live instance (the manager holding
it, the arena it faults into, the device it reads from) is `mlos-lab`. The
split is the difference between "a transformer has eight layers of sixteen
tiles" and "this machine currently has seven of them resident", and
keeping them apart means the shape can be described without a running
kernel to describe it on.

## A transformer without arithmetic

Eight layers of sixteen tiles, plus a scale and an activation each. No
arithmetic happens: the point is the access pattern and the residency
pressure, which is what the object manager is being asked about.
`docs/plan.md` M1 milestone 33 makes the case: a compelling demonstration
of an ML operating system needs no neural network in it.

`MODEL` names the model in anything that has to name it. A trace carries
an `ObjectId` per access and no sizes; the sizes come from the model the
ids were taken against. Naming it in the trace is what makes replaying
against the wrong one a caught error rather than a table of numbers about
nothing.

## Weights and activations are mirror images

Weights are immutable, backed by storage, and impossible to recompute.
That last part is the interesting field: there is no computation that
produces a trained weight, so eviction may demote these and must never
discard them, which is a decision the table can only make because the
distinction is recorded.

Activations are transient, and cheaper to rebuild than to store. They are
the mirror image of weights, and the reason both cost fields exist. An
ordinary kernel must find somewhere to put a page it evicts; this one can
decide the object was never worth keeping.

## KV blocks

KV blocks are the half of a decode loop that accumulates. They live here
rather than in `mlos-workload` because the kernel replays the same trace
as the simulator, so it has to be able to register a KV block when one is
first asked for, and it cannot do that from a `std` crate it does not
link. What is a property of the workload is the decode loop, which stays
in `mlos-workload`; what a KV block is belongs beside the model whose
objects it sits among.

The class is session-scoped: `ObjectClass::is_session_scoped` says so,
and it means `Fields::model` carries a session id rather than a model id.
Getting that wrong would let one session's KV alias another's, which is
why the ABI made it a function rather than a convention.

`BYTES` (256) is small beside a weight tile on purpose. One block is one
position's keys and values for one layer; a tile is a slab of a weight
matrix. The interesting pressure comes from how many blocks accumulate,
not from any one of them being large.

A block's metadata is the opposite of a weight tile's in every respect a
policy cares about. A tile is immutable, shared by every session, and
cannot be recomputed at any price. A block is mutable, owned by one
session, and can be recomputed, by re-running attention over the prefix,
which is expensive and gets more expensive the further back it sits. Its
tier is `Warm` rather than `Cold`: a block does not come from storage the
way a weight does, it comes from having done the work once. The reload
cost (400 us) models spilling to storage and reading it back, which is
what a real serving engine does under pressure; the recompute cost (2 ms)
is dearer, which is why a cost-aware policy would spill rather than drop.

## Tiers: one type, described by cost

`tiers::Tier` is one type, not two. An earlier version had a `Backing`
provider and a `Recompute` provider with identical bodies and different
constants, which is duplication wearing a costume: what distinguishes a
block store from a recomputation, as far as the object manager is
concerned, is entirely the cost.

`BACKING` is NVMe-shaped: three milliseconds to the first byte, then about
a gigabyte a second. The accuracy matters less than the shape: latency
dominates for a small object, transfer for a large one, and that crossover
is what an eviction policy has to find.

`RECOMPUTE` is the tier an ordinary operating system does not have. An
activation reaches it by being thrown away and comes back by being
calculated again, which is why `recompute_cost` sits beside `reload_cost`
in the object table. No seek, but real work: cheaper than storage for a
small object and worse for a large one.

Neither pattern-filling tier touches a device. `read` fills with a
pattern derived from the object so a reader can tell whose bytes it got,
and notice if it got nobody's.

## The disk

`Disk` replaces the pattern-filling stub for weights when a block device
is present. The bytes are the same either way, which is the point: the
test is that they now cross a virtqueue to get here, so "three tiers"
stops being a claim about the table and becomes a fact about where the
data is. Its cost numbers are unchanged from the stub it replaces, so a
comparison of policies is not confounded by the tier getting cheaper
underneath it.

The layout is by id rather than by a table on disk: layer and tensor give
a position directly, so there is no index to read before the first read.
A real model file would need one; a synthetic one should not pretend to.

The trace shares the one device, starting at `TRACE_AT`, the sector the
weights end at. A second virtio-blk would have meant teaching `probe` to
tell two block devices apart and pick the right one, which is machinery
in service of a layout decision, and the layout decision is free: the
model occupies a known, fixed extent, so everything after it is spare.

`Disk::block` is public because `Disk` adds a layout and a cost and
nothing else; a constructor would be a formality around a single field.
