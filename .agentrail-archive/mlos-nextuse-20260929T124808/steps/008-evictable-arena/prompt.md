The arena stops being a bump allocator.

Until it can choose a victim and give memory back, no policy can run in the
kernel at all -- and that is not a gap in a plan, it is the current state:
`Arena::place` bumps a pointer and a full arena returns `NoBudget`. Its own
documentation says a general allocator was refused on purpose, because it would
have let the manager quietly succeed at the point where the interesting
question -- what should have been evicted -- is the one being dodged. Step 006
answers that question. This step makes it possible to act on the answer.

What eviction has to do that placement did not: free a region, coalesce with its
neighbours so the arena does not fragment into unusable gaps, and update the
object table so the evicted object reads as evicted rather than resident.
`mlos-events` already has `Kind::Evicted` reserved and nothing emits it; this is
where it starts to.

Fragmentation is the real risk and it is worth measuring rather than assuming.
Objects here are mostly uniform -- 1 KiB tiles, 2 KiB activations -- which makes
the easy case easy; KV blocks that grow will not be. Report what fraction of the
arena is unusable after a long run, because a policy that evicts perfectly into
an arena that cannot reuse the space has not helped.

The residency counters and both layout emitters must stay correct across an
eviction. `Counters::resident` is already signed for exactly this reason. The
runtime layout document should show an evicted object as `evicted`, not
`never` -- the two are the same `resident_at == 0` and only the use count tells
them apart, which is why `mlos_spaces::state` distinguishes them.
