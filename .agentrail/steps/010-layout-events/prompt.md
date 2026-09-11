Stream residency events as they happen, so the viewer can animate rather than
diff two snapshots. This is MLOS's distinguishing contribution to the shared
demo: sw-tos's storage is mostly static once built, whereas the interesting
thing about an ML object store is churn -- what came in, what it cost, what had
to go to make room.

A compact event line per transition, emitted on the console under a verb or a
flag (`trace on`), carrying the stable region_id from step 008 and no more than
it needs: what happened (fault / fetched / placed / hit / refused, and evicted
once M3 has a policy), which object, which tier it came from, how many bytes,
what it cost, and a monotonic tick. One line per event, self-delimited, so a
consumer can read a truncated stream without waiting for a closing brace.

It must be cheap enough to leave on: the fault path is the thing being measured
and an emitter that dominates its own measurement is worthless. Measure the
overhead with `ticks` and record the number in the summary -- if it is not small
relative to a fault, say so plainly rather than shipping it on by default.

Events and snapshots must agree: replaying the event stream onto the step-009
snapshot taken before a sweep must reproduce the snapshot taken after it. That
is the test, and it is the only thing that keeps the two emitters from drifting
into two different stories about the same run.

`mlos run --capture` should be able to separate the event stream from shell
output cleanly -- a consumer should not have to parse mlsh's prose.
