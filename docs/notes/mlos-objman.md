# mlos-objman

The model object manager: owns the object table, the arena resident
objects live in, and the fault path between them. Objects layer; where a
conventional kernel has a VM subsystem, MLOS has this. Linked from
`crates/mlos-objman/src/lib.rs`, `fault.rs`, `evict.rs` and `lease.rs`.

## The fast path is the design constraint

A hit must be a table lookup and nothing else: no allocation, no call out
to a policy, no lock. A fault that costs an IPC round trip before it even
knows where to look is a fault too expensive to have, which is why the
table lives in the kernel while policy does not (`docs/architecture.md`
s.4). `Manager::acquire` is that fast path: a lookup, a few counter
updates and an event; everything else is `service`.

## What a model fault says

A page fault says an address was not mapped. That is all a page table
knows, and it is why paging has to guess: the only thing an ordinary
kernel can do with a fault is note the address and hope recency predicts
the future.

A model fault says which layer's weights, for which model, on behalf of
which session, wanted by when. Everything in `docs/PRD.md` (choosing a
provider, choosing what to evict, deciding whether to recompute instead of
fetch) is a decision this structure makes possible and an address does
not. `ModelFault::class` is the axis every metric in `docs/PRD.md` s.5.2
is broken down by: "40,000 faults" is meaningless, "38,000 of them cold KV
blocks" is a diagnosis.

`ModelFault::cost` is carried in the fault rather than looked up later
because it is what makes the fault comparable: a policy asked to free
space needs to weigh this against what it would have to throw away.

`ModelFault::new` returns `None` for an id that does not decode (an
all-zero word, or an undefined class). A fault on something that is not an
object is a bug in the caller, not a residency problem, so `service`
reports it as `BadClass` rather than trying to serve it.

## The order of `service`

Describe, find the provider, make room, fetch, record. The fault is
described before anything is attempted, so a failure to find room still
leaves a record of what was wanted: the difference between a system that
can explain why it refused and one that just refuses.

The event is recorded after the outcome, so one event says what actually
happened rather than a hope amended later. Its offset is arena-relative
rather than the absolute address the lease carries: that is what the
layout document's `dram` regions use, and an event has to be replayable
onto one, which an absolute address would put outside the space it
belongs to.

`provider_for` returns `NoProvider`, a distinct failure from `BadObject`:
one means nobody registered the object, the other that nobody attached the
thing that could fetch it. The reference it returns borrows the manager's
lifetime `'a`, not the borrow of `self`; otherwise holding a provider
would lock the table it is about to place into.

## `place` fetches before it records residency

`place` is separate from `service` because its failure means something
different: no room is a residency decision nobody has made yet, and it is
the error a session's admission contract exists to prevent.

It fetches before recording residency. An object the table calls resident
but whose bytes never arrived is worse than a miss: the next acquire is a
hit, and returns whatever was in the arena beforehand. `Arena::place`
returning the address and the room to fill in one call is what makes
this ordering natural rather than a discipline.

## The stream is asked once per acquire

`acquire` asks `Stream::next_after` once, before it knows whether the
acquire is a hit or a miss, and the answer is written into whichever path
serves it. A position stays true until the object is acquired again, so
nothing revisits it when the stream advances. `Manager::stream` is empty
until something declares one, and an empty stream answers `Never` for
everything, which is exactly what the table said before streams existed,
so nothing changes for a workload that does not declare one.

## Eviction

`make_room` asks the policy one victim at a time, so a policy never has to
know how much more room is needed, only which single object it would give
up next. This is the same shape `mlos-sim` uses, deliberately: the two
have to make the same sequence of decisions or their counts cannot match.

It stops on a refusal, on an eviction that fails, or when the largest free
run is big enough, not when the total free is. After evictions those
differ, and placing needs a contiguous run. `Occupancy::fits` is that
test, and `mlos-sim` asks the same one.

`Manager::policy` being `None` is demand paging, which is what MLOS did
until M3 step 009: a full arena refuses rather than choosing. Attaching a
policy is what made the kernel able to act on the answer step 006
measured.

`evict` does four things that have to happen together or the system
disagrees with itself: the bytes go back to the arena, the table stops
calling the object resident, the residency counter comes down, and the
event stream says so. The tier is restored to `home`, not guessed. An
evicted object reads as `evicted` afterwards rather than `never`, and the
difference matters: both have `resident_at == 0` and only the use count
separates them. An object thrown away is one a decision was made about;
one never wanted is not.

`NoBudget` from `Arena::release` means the free list is full, and then
nothing is changed: the object stays resident rather than becoming memory
the accounting has lost.

## `Held`: the table as a policy may see it

`Held` presents the table to a policy as a scan of every slot, because
that is what an open-addressed table can offer. There is no cheaper way to
enumerate what is resident, and a policy that wanted one would be a policy
the kernel could not host. `mlos-sim` scans a `Vec` for the same reason:
the simulator was built to the kernel's constraints rather than the other
way round, so that step 009 could not discover the interface was
unimplementable after every number existed.

`Residency::len` is every slot, not every object. A vacant one answers
`None` and the policy skips it, which costs a compare and saves counting.

## Leases, not reference counts

A count says only how many; a lease says what kind. A policy that cannot
tell a pinned object from a speculatively prefetched one has to treat the
prefetch as sacred, which defeats the point of prefetching.

- `Pin`: resident until released; counts against the session's budget.
- `Borrow`: resident for one operation; may be revoked between operations.
- `Streaming`: consumed in order, once. The holder promises not to look
  back, which is what lets the object be dropped as it passes.
- `Speculative`: fetched on a guess. Free to drop, and a speculative lease
  that is never upgraded is exactly what a prefetch miss is, which is how
  `Ph` gets counted.

`Handle::address` is valid while the lease is held and means nothing
afterwards. A consumer that stores it past a release has assumed the
thing this design exists to prevent: that where an object lives is a
property of the object.

## Why the manager counts what it counts

`counters` are kept here rather than by a caller because this is where the
events are: a fault that the manager serviced and a caller forgot to count
is a fault that did not happen, as far as any measurement is concerned.

`evictions` is counted here rather than derived, because a replay has to
attribute evictions to the acquire that caused them and the event ring is
a fixed size that a long run overflows.

`clock` is the monotonic acquire count behind `ObjectMeta::placed_tick`
and `used_tick`. The kernel keeps them even though nothing in the kernel
reads them yet: a field the simulator fills and the kernel does not is a
field the two disagree about, and step 009 replays the same trace in both
and requires the counts to match exactly.

`events` records what happened, in order; `counters` say how much. A
viewer animating residency needs the sequence, not the totals, and the
totals can be rebuilt from the sequence where the reverse is not true. The
field is `events` and the crate is `mlos-events`; the shell verb that
prints them is still `trace`, because that is what someone types and it
is still what it does. The name `mlos-trace` belongs to the M3 access
trace, which is a different thing: what the workload asked for, rather
than what the manager did about it.

## Lessons

**Residency is `resident_at`, not the tier.** The tier says where an
object lives; only the address says whether it is here. The two agreed for
weight tiles, which start `Cold` and become `Warm` when placed, and
disagreed the moment a KV block arrived: KV is produced by compute rather
than storage, so it starts `Warm`, and a never-fetched one read as a hit
at address zero. Found by the kernel and the simulator disagreeing about a
replay they were meant to agree on exactly. Every residency test in this
crate is `resident_at != 0`, and a new one must be too.

**`now` is the acquire clock, not the stream cursor.** They are the same
counter while a workload runs what it declared, and they are not while a
shell pokes at objects out of order. The clock is the one a policy must
see: `next_use` positions are one-based ticks, `used_tick` is a one-based
tick, and a policy computing `at - now` against a zero-based cursor is off
by one for every candidate. A uniform shift sounds harmless and is not:
`evictability` divides by a per-object recovery cost, so shifting every
distance by one reorders candidates with different costs.

**Split the arena out before the fifth module.** `evict` arrived when
`mlos-objman` was at four modules. The arena knows nothing about objects,
which made it the separable half, and it became the sibling crate
`mlos-arena` rather than a fifth module here.
