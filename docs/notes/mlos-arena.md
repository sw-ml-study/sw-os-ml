# mlos-arena

A fixed region with a coalescing free list: where a faulted-in object is
put, and taken out of. Objects layer. Linked from
`crates/mlos-arena/src/lib.rs` and `holes.rs`.

## Why its own crate

It knows nothing about objects: it hands out runs of bytes and takes them
back, and `mlos-objman` decides which object goes in one. `mlos-objman`
was at four modules when eviction arrived, and AGENTS.md says to make the
sibling crate rather than push a fifth concern into a crate that is full.

## Why it was a bump allocator first

Until M3 step 008 the arena could not free anything, deliberately. Until
a policy could decide what to throw away, the honest behaviour when
memory ran out was to say so; a general allocator would have let the
manager quietly succeed at exactly the point where the interesting
question, what should have been evicted, was being dodged. Step 006
answered that question, so step 008 is where acting on it became
possible.

## A sorted free list, coalesced on release

Free extents are kept sorted by address and merged with their neighbours
the moment one is returned. Merging on release rather than on demand is
what keeps the list short: a thousand evictions of adjacent tiles leave
one hole, not a thousand, and a list that grew instead would run out of
its fixed capacity on a workload that never fragmented.

The capacity is fixed (`HOLES`, 64) because there is no allocator to grow
it. A release that would need a sixty-fifth extent is refused rather than
silently leaking the memory, and the bytes stay the caller's: the object
is still resident. That is the failure mode worth being loud about.
Memory that has been evicted and cannot be reused is worse than memory
that was never freed, because the accounting says it is available.
Sixty-four is generous for uniform tiles, which coalesce into a handful
of runs; a workload of ragged sizes could exhaust it, and `release` says
so.

`merge` takes the later neighbour first: merging with the earlier one
shifts the array down, and doing that before looking right would leave
the right index pointing at a different hole.

## First fit, not best fit

First fit is what a kernel can afford and it is not obviously worse. Best
fit leaves a trail of slivers too small for anything; first fit leaves
larger fragments nearer the end. `tests/fragmentation.rs` measures what
actually happens rather than trusting either story: uniform 1 KiB tiles
do not fragment at all, and a ragged 256 B to 1792 B mix leaves 69% of
its free space reachable in one run.

## Place and room are one call

`place` reserves the bytes and returns the slice to fill in one call,
because they are one decision. Separating them is what once let the
fault path allocate and then forget to fetch.

`NoBudget` from `place` means no single run is large enough. After
evictions that is "not enough contiguous room", a different thing from
"not enough room", and the reason `Occupancy::largest` exists.
`Occupancy` is one call rather than four accessors because every caller
that wants one of its numbers wants at least two, and a `largest` that
disagreed with the `used` it was read beside would be worse than either
alone. `largest` is the fragmentation number: when it is far below
`capacity - used`, the arena has free memory it cannot give to anything,
and a policy evicting perfectly into it has not helped.

## Alignment

Placements are rounded up to `ALIGN` (16 bytes): enough for anything a
device will DMA into, and it keeps one object's tail out of the next
one's cache line. It is public because a layout emitter draws the arena
in these units, and two statements of the same granularity would be one
too many. `Occupancy::fits` rounds the same way, because a run that fits
the size and not its alignment is a run the placement would still
refuse.

## Lessons

**The simulator must fragment too.** `mlos-sim` used to count bytes and
evict until the count fitted. The kernel and the simulator then disagreed
by thirteen reads on a next-use replay, every one of them an eviction the
kernel made because the free bytes it had were not contiguous. The arena
took a lifetime parameter so a `Vec` could back it on the host, and the
simulator now places into a real one. A simulator that does not fragment
is not simulating this arena.

**One question, answered once.** `Occupancy::fits` exists so that the
kernel's `make_room` and the simulator's eviction loop stop on the same
test. Two loops with two conditions is two answers.

**Safe, and worth saying.** `Arena::new` looks like it should be `unsafe`
and is not: the address it hands out is the address of memory it holds
an exclusive borrow of, so it cannot name anything it does not own.
