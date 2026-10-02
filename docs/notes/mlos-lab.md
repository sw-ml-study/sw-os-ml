# mlos-lab

The one live object manager: a synthetic model held in a static arena,
read from whatever device this machine turned out to have, swept and
replayed from the shell. Kernel layer; one of the crates allowed to hold
`unsafe`. Linked from `crates/mlos-lab/src/lib.rs`, `state.rs`,
`devices.rs`, `replay.rs` and `sweep.rs`.

## Why it exists

`mlos-synth` says what a synthetic model is; this holds one. It is what
makes `docs/PRD.md` gates G2 and G3 observable rather than merely tested.

The arena is deliberately smaller than the model: `ARENA_BYTES` is a
quarter of the model's weights, chosen so a sweep runs out. That is not a
limitation to apologise for; it is the entire premise. RAM is the scarce
resource, the model does not fit, and what the system does about that is
the subject. A demonstration where everything fits demonstrates nothing;
the interesting number is how far it got. `CAPACITY` (512 objects) is
comfortably more than the model needs, so a full table is never what a
sweep runs into first.

`register` clamps the budget to `[TILE_BYTES, ARENA_BYTES]`: a budget
below one tile can hold nothing, and one above the static buffer does not
exist. Both are user input.

## One accessor, `with`

`with` runs a closure against the manager rather than wrapping every
question. The shell wants to ask things nobody has thought of yet (what
tier is this in, how often has it been used, what would evicting it save),
and a crate that answers only the questions it anticipated is one the
shell has to be extended through every time it wants a new one.

## The same policy crate as the simulator

`choose` attaches one of `mlos-policy`'s policies by name; `None` is
demand paging, which is what MLOS did before M3 step 009. The policies are
the same crate `mlos-sim` links, which is the whole point: two
implementations would be two results.

## Statics, and where the `unsafe` lives

The manager, its arena and the replay buffers are statics because the
kernel has no allocator and the manager must outlive every command that
touches it. They are in `state.rs` by themselves because that is where the
`unsafe` is: everything else in this crate is ordinary code, and keeping
the two apart is what makes "where is the unsafe" answerable by looking at
a file name.

The soundness argument, repeated at every `unsafe impl Sync` and every
access, is that these are touched only from the shell loop, on the boot
core, with interrupts enabled but no handler reaching any of them (the
console interrupt queues a byte and returns). The arena is additionally
handed to exactly one manager at a time: `register` replaces the manager
that held it in the same breath.

The replay buffers are far too large for a stack that already nearly lost
to a 40 KiB `Manager`. They are also the storage a `Stream` borrows: the
declaration is as long as the workload, so it cannot live inside the
manager and it cannot be grown. `replay_room` hands all five out at once
rather than through five accessors, because a replay needs all of them and
handing them out separately would let one be borrowed while the others
were not, which is the shape of exactly the aliasing this module exists to
keep in one place.

## Devices are probed lazily, from slots the kernel names

Nothing needs a disk until a model is registered, and a machine without
one should not pay for the search or be told about it, so the block device
is probed on first use and remembered. The virtio-mmio slots are set at
boot by the kernel (`set_slots`) rather than discovered here, because the
device tree is the kernel's to read and this crate has no business parsing
one.

The model is registered against whichever provider is present, disk or
stub, so the same commands work either way and the difference shows up
only in where bytes came from.

## Replay: the kernel and the simulator run the same trace

The point of M3 step 009: `mlos-sim` and the kernel link the same
`mlos-policy` crate and replay the same trace under the same budget, and
the counts have to match exactly. A policy that behaves differently in the
two is a policy neither result describes, and the simulator would have
been measuring something that never ran.

The trace arrives on the model disk, after the weights, because
`/chosen/bootargs` is far too small to carry one and the block transport
already exists. `mlos_synth::disk::TRACE_AT` is the sector both sides
agree on. It is stored as an eight-byte length then the text. `Block::
read_at` accepts sector-aligned reads only, which is why the length is
read as part of the first sector rather than as eight bytes of its own,
and the text sliced out afterwards. A length longer than the buffer is
refused rather than truncated: a shortened trace is a different workload,
and the counts it produced would be confidently wrong rather than
obviously absent.

Two static buffers hold the text as it comes off the device and the parsed
accesses. `mlos-trace` parses into a caller-provided slice for exactly
this reason; it was written in step 001 knowing this step was coming.
`DISTINCT` (the chain's scratch) is far smaller than `ACCESSES` because
that is the point: a decode loop touches a few hundred objects thousands
of times. Too small is refused rather than truncated.

**The declaration is the whole trace, and that is not cheating.** The
kernel is being told what it will acquire, which is exactly what
`ml_stream_declare` is for and exactly what a decode loop can honestly say
about itself. What it must not do is read the trace the way the simulator
does, computing an answer from accesses nobody declared, and it does not:
`Stream` sees a sequence handed to it, and the cursor advances one step
per acquire like a running workload's would.

Every counter in `Replayed` comes from what the manager actually did (the
fault count and the eviction count either side of the acquire) rather
than from what the replay expected. That is the only way the comparison
means anything.

`known` registers an object the table has not heard of, and that is not a
convenience for the replay: a KV block comes into existence when a session
first writes one, and a kernel that could only serve objects somebody had
declared in advance could not run a decode loop at all. `mlos-sim` behaves
the same way for a different reason (its `Model` answers for any id), and
the two have to agree or the counts cannot.

## Sweep and declare

The sweep walks the model in order because that is what a dense
transformer does, and the whole argument of `docs/PRD.md` is that the
order is knowable in advance. It is the sweep whose numbers M3 has to
improve on.

`declare` is `ml_stream_declare` with the declaration derived from the
model rather than supplied by a caller: there is no userspace to supply it
and `docs/plan.md` defers one deliberately. What matters is that the
kernel is told the order rather than inferring it, and a declaration built
from `model::tile` is told in exactly the sense a process would tell it.

It declares one sweep, not a repeating one. A stream is a finite sequence,
so a tile behind the cursor reads `Never`, which is the truth: the kernel
has been told about one pass and nothing beyond it. The shell's `stream`
verb exists to watch that being written, and watching it run out is part
of what there is to see. It borrows the replay's declaration buffers
because they are the storage this kernel has for a declared sequence and
a stream borrows rather than owns.

## Lessons

**Declare the workload, not its period.** Declaring the trace rather than
a 128-tile weight sweep is what closed the last disagreement with the
simulator. A sweep is periodic and a decode loop is not: its KV cache
accumulates, so every KV block read as `Never`, the most evictable thing
in the table, and next-use spent 27 reads evicting exactly what it was
about to want. See also `docs/notes/mlos-stream.md`.

**Count what the manager did, not what the replay expected.** Any new
counter in `Replayed` is derived by differencing the manager's own
counters around the acquire. A counter the replay computes for itself is a
second implementation of the thing being measured.

## Lanes: the scheduler runs in the kernel (M4 step 006)

`lanes.rs` rebuilds per-session lanes from the disk trace in static
buffers: the trace's objects grouped by session, each access's token and
index within it, and one lane per live session in slot order, which is
the order the trace first named them. A token begins wherever a session
touches the model's first tile, which is how the generator starts every
token and the only boundary the trace carries. `Lanes` implements
`mlos_sched::Waiting` over those buffers with a cursor per lane, and
`order` runs any `Schedule` over it, writing the merged order and its
sessions into the room. `replay(parameter)` then declares that order as
the stream and replays it, so `next_use` positions are the order the
scheduler chose, as the simulator's foresight is over its merged trace.

The process-major order rebuilt this way is the disk trace itself, which
is why `replay POLICY process` prints the lines it always did, and the
parameter-major order equals `mlos_sched::merge(ParameterMajor, tokens)`
on the host because both sides run the same `pick` over the same lanes
with the same ceilings (zero: the sessions are adopted under
`Contract::NONE`). The boot tests compare all eight lines, headline
numbers included, as strings.

Two more static arrays were the cost: the grouped objects and their
shape (12k entries each), plus the merged order's sessions. The lane
list itself is sixteen entries, one per possible session.
