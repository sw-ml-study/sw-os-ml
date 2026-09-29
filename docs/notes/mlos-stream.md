# mlos-stream

A declared sequence of objects and where the workload is in it: the
mechanism by which a transformer hands the operating system its own
future. Objects layer. Linked from `crates/mlos-stream/src/lib.rs` and
`chain.rs`.

## Why it exists

`docs/PRD.md` s.1 claims that most ML state is read in an order known in
advance. A stream is that order, written down, and it is what finally
writes `ObjectMeta::next_use`, a field that had existed since M2 step 001
with nothing to set it.

## Finite, because a decode loop is not actually cyclic

A declaration is a finite sequence, and running off the end of it answers
`Never`: nothing has declared a future beyond what was declared. A
workload that really does loop declares the loop it will run.

`Never` is an honest answer rather than a maximum. It means nothing has
declared a future for the object, not that it will not be wanted, and a
policy must be free to treat that differently from "wanted very far
away". That is why `NEVER` is a distinct sentinel and not a large number
standing in for infinity.

## Borrowed, because the declaration is as long as the workload

One period of a sweep fitted in a fixed array. A decode loop's whole
access sequence does not, and a kernel with no allocator cannot grow one.
The declarer owns the storage, statics in the kernel and `Vec`s in the
simulator, and a stream borrows it. `Stream::declare` replaces whatever
was declared before and resets the cursor: a new declaration is a new
workload, and carrying a position across would point into a sequence that
no longer exists.

## Advancing is one increment, and the type is why

`NextUse::At` holds a position rather than a distance, which is what makes
`Stream::advance` a single addition whatever the table holds. A distance
is measured from somewhere: advancing a stream by one step would make
every resident object's distance wrong, and the kernel would have to walk
the object table to correct them, a per-token cost over the very structure
it would be walking. A position does not move when the cursor does. The
subtraction happens once, in the policy, for the handful of objects it
actually compares.

## `next_after`: strictly after the cursor, one-based

Ticks are one-based (the first acquire happens at tick 1) because that is
how `ObjectMeta::used_tick` has counted since M2, so position `p` in the
declaration is tick `p + 1`. Mixing the two bases is not a cosmetic
error: an object wanted by the very next access reads as `Never` and
becomes the most evictable thing in the table.

The answer is strictly after the cursor because it is asked while the
object is being acquired: the use happening now is not the one a policy
needs to know about. When the workload is running what it declared, which
is the case every measurement covers, the object is the one on the cursor
and the answer is one indexed read of the chain. Otherwise it is a scan of
what remains, which is the honest general answer.

## The chain: one pass, precomputed, shared

`chain` makes one pass over a declared sequence and produces, for every
position, the position of the next access to the same object. That is the
whole of what a policy needs to know about the future, precomputed once,
so asking costs an indexed read rather than a search.

It walks backwards because that is the direction the answer falls out in:
walking from the end, the last position seen for an object is its next
use from any earlier point.

No allocation, no map. The caller supplies the output and a scratch table
of objects seen so far, which is linear-searched: a decode loop touches a
few hundred distinct objects against thousands of accesses, so the scan is
short and a hash table would be machinery the kernel would have to
justify. `seen` needs one slot per distinct object, not per access.

Both buffers are refused with `NoBudget` if too small, rather than
truncated, because a partial chain produces confident wrong numbers.

`Stream::declare` takes the chain as an argument rather than computing it,
because the kernel has nowhere to put it that a stream could own, and
passing it is what forces both sides to build it with the same code.

## What it cannot yet express

`NextUse::Probability`, an MoE router's distribution, is never produced
here, because nothing routes yet. The distinction is preserved rather than
collapsed: a declared stream says exactly when, and a router will say only
how likely, and a policy may act on the first with certainty and weight by
the second. Collapsing them would licence evicting something the system
merely guessed about as though it knew.

## Lessons

**A decode loop has no period.** An earlier version declared one period
and ran the cursor past the end of it, on the reasoning that every token
reads the same objects in the same order. That is true of the weights and
false of everything else: a KV cache accumulates, so token 17 reads
sixteen blocks that token 1 could not have named, and a sequence with a
growing tail has no period. Under the cyclic model the kernel answered
`Never` for every KV block, making the most-reused objects in the workload
look like the most evictable, while the simulator, reading the whole
trace, knew better. The two disagreed by 27 reads out of 3758, which is
the sort of gap that looks like rounding and is not. Declarations are now
finite, and a workload that loops declares the loop.

**One implementation of "when is this next wanted".** The simulator used
to compute the chain itself, from the whole trace, with its own copy of
the algorithm; the kernel scanned a declaration instead. Two
implementations is two answers, and step 009 measured the difference at
those 27 reads. There is now one `chain` and both sides call it,
`mlos-sim` wrapping it in `Vec`s it can afford and the kernel in statics
it can. Any future change to how next-use is computed goes in `chain`, and
nowhere else.
