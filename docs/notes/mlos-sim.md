# mlos-sim

Replays an access trace against a policy under a budget, and counts what
it cost. Host-side measurement layer: the harness every number in M3
comes out of. Linked from `crates/mlos-sim/src/lib.rs`, `foresight.rs`,
`resident.rs`, `run.rs` and `view.rs`.

## Why the harness's own correctness comes first

Every M3 number comes out of this crate, which makes its correctness the
first thing to worry about. Two properties carry that weight:

**The budget is a property of the run, not of the policy.** A comparison
where one policy got more memory is not a comparison, and it is the
easiest mistake to make and the hardest to see afterwards. `compare`
takes one budget and applies it to every policy, so the mistake is not
expressible rather than merely discouraged.

**The same trace and policy give the same counts every time.** Nothing
here consults a clock, a hash seed or an iteration order that could vary.
A test says so, because a harness that were nondeterministic would make
every later number arguable and there would be no way to tell from the
numbers themselves. The one `HashMap` in the crate is an index used only
for the simulator's own lookups; nothing a policy sees is enumerated
through it.

`docs/design.md` s.10 calls this the host-side level of the test pyramid.
It is `std`, and the policies it runs are not: they are the `no_std`
crates the kernel links, which is what lets the kernel replay the same
trace (M3 step 009) and be required to match the counts.

## The model is separate from the trace

A trace names objects and says nothing about how big they are, on
purpose: size is a property of the model, not of the workload. The `Model`
trait is where sizes and costs come from, and the trace header names which
one it must be. An access naming an object the model does not have is
counted in `Outcome::mismatched`, its own counter rather than a refusal,
because it means something completely different: a refusal is a residency
decision, and a mismatch is the two files not being about the same thing.
Counting it as an ordinary refusal would hide a mismatched pair behind a
plausible number.

## The resident set is a `Vec` with linear scans

Deliberately, because that is what the kernel's open-addressed table can
offer a policy, and the point of the simulator is to run the code the
kernel will run. A `HashMap` here would let a policy be written that the
kernel cannot host, and the discovery would come at the kernel-replay step
after every number had been produced.

The policy's view is that `Vec`; the simulator's own lookups are not.
Finding an object on a hit and finding it to remove it go through an
index, because a real model's cache is tens of thousands of blocks and a
linear `find` on every access made a trace that size take minutes to
replay, while the kernel, whose table is hashed, would have taken none of
that. What the simulator must share with the kernel is the policy's cost,
not its own bookkeeping.

Removal swaps the last element into the hole, so the order a policy sees
residents in changes as they leave. That is allowed since step 009: every
policy breaks ties on object id, so no answer depends on enumeration
order, and the kernel's hash-slot order was never insertion order anyway.

`Resident::touch` finds and touches in one call because they are one
decision: separating them leaves a caller holding an index into a
collection it is about to mutate, which is the shape of a bug rather than
of an API.

## The budget is an arena, not a number

It was a number, resident bytes against a ceiling, until step 009 put the
kernel beside the simulator on one trace and they disagreed by thirteen
reads under next-use, and by none under FIFO or LRU. The baselines evict
in roughly the order things were placed, so their holes coalesce and a
byte count is a fair model of them. Next-use evicts whatever is furthest
away, wherever it sits, and a 1 KiB tile does not fit in four 256-byte
holes. The kernel paid for that; the simulator did not know it existed.

So the budget is `mlos_arena::Arena`, the same crate the kernel places
into, over a buffer the size of the budget. What fragmentation costs a
policy is part of what the policy costs, and a simulator that left it out
would be measuring a kernel nobody has. Two consequences follow:

- `NoBudget` from `Resident::insert` after room has been made means what
  it means in the kernel: the free bytes exist and no single run of them
  is large enough. It is counted as a refusal, because that is what the
  kernel reports for it.
- A release the arena refuses (its free list is full) leaves the object
  where it was, exactly as `Manager::evict` does. Nothing is changed, so
  the accounting cannot say bytes are free that are not.

## `make_room` is the kernel's loop

`Run::make_room` is the same loop as `Manager::make_room`, asking the same
arena the same question (`Occupancy::fits`) and stopping for the same
three reasons: the policy names no victim, the arena will not take one
back, or a run large enough has opened up. One victim at a time, so a
policy never has to know how much more room is needed, only which single
object it would give up next.

The placement is then attempted regardless of how the loop ended: an
arena nothing was evicted from may still fit, and a failed placement is
the refusal, which is what the kernel reports for a placement that could
not be made.

## Foresight: knowing the future because the whole trace is here

This is what makes a simulator a simulator. Belady's rule is normally
unimplementable because it needs to know when an object will next be
wanted; a replay harness holds the entire access sequence, so it can
simply look.

That is not cheating, and it is not the experiment either. The claim
`docs/PRD.md` makes is that a transformer hands the operating system this
knowledge, a declared stream says which objects come next, in order, so
the kernel can have it without a trace. `ml_stream_declare` (M3 step 007)
is where that arrives. Until then the simulator supplies it, and the
numbers say what a policy would do given knowledge the kernel is about to
be given.

The chain itself is `mlos_stream::chain`, which the kernel also calls.
What remains in `foresight.rs` is the part that is genuinely the
simulator's: allocating buffers the size of the trace, which is exactly
what the kernel cannot do. The buffers are sized from the trace so they
cannot be too small, which is the only failure `chain` has; the kernel
sizes its statics by guess instead, and finds out loudly when the guess is
wrong.

No object's next use is looked up by identity. `ObjectMeta` already
records `used_tick`, when it was last wanted, and an object that has not
been wanted since is still sitting at that point in the trace. So the next
occurrence after any resident object's last use is a single indexed read,
and the map from object to position that would otherwise be needed does
not have to exist.

`wanted_at` is one lookup, at acquire time, never revisited: the object
is not wanted again before the position it returns, so the answer stays
true until the next acquire rewrites it. `next_use` is therefore written
once, in `touch` or `insert`, which is the whole point of storing a
position rather than a distance.

## The policy's view is a separate module

`view.rs` is apart from `resident.rs` because it is a different audience.
The inherent methods on `Resident` are the simulator's: put this in, take
that out, how full are we. `Residency` is the narrow, read-only window a
policy gets, and it is the same window the kernel opens over its own
table.

`at` hands back a copy rather than a reference on purpose: a policy
holding a borrow into the resident set could not be called while the
simulator mutates it, and the kernel could not offer one at all over an
open-addressed table with tombstones.

## Why `hit_per_mille`

The ratio to compare policies on. A raw read count is only comparable
between runs of the same length, and traces of different lengths are
exactly what the workload generator produces. `refused` is in the
denominator and is not a failure: demand paging refuses by design and the
cost of refusing is part of what the comparison measures. A policy with
many refusals and few reads has not won.

## Lessons

**Two implementations of "when is this next wanted" are two answers.**
The chain used to be a second implementation living in this crate. Step
009 put the two side by side on one trace and they disagreed by 27 reads.
It is now `mlos_stream::chain` on both sides.

**Positions, not distances.** An earlier version stored next-use
distances and had to walk every resident object on every access to keep
them current; a kernel could not afford that, and the step that said so is
the reason `NextUse` carries a position.

**Indices are zero-based and ticks are one-based.** `Foresight::after`
answers in trace indices and the replay counts one-based ticks, so the
position a policy compares against `Residency::now` is the index plus one.
Mixing the two is what made the first version wrong, and wrong in the
worst available way: an object whose next use was the very next access
fell through to `Never` and became the most evictable thing in the table.
Belady was being told to throw away exactly what it was about to need, and
it lost to LRU by three times. Getting this wrong is invisible in every
test but the measurement.

**A simulator that does not fragment is not simulating this arena.** See
"The budget is an arena, not a number" above; the thirteen-read
disagreement is the reason `mlos-arena` takes a lifetime parameter.

## Sessions exist for the replay

M4 step 001. `replay` adopts every session the trace names before the
first access and reports the count in `Outcome::sessions`, as the kernel's
replay does with its manager. Nothing is served differently yet: the
table exists so that step 003's scheduler has sessions to schedule, and so
the simulator and the kernel already agree on who exists when.
