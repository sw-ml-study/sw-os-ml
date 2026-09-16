The measured comparison table. PoC gate G4.

The artifact this milestone exists to produce: known-next-use against demand,
FIFO and LRU, on the generative trace, under an identical residency budget, with
provider reads and bytes moved for each. In `docs/` where someone can find it,
and in `docs/status.md` as ground truth.

Write it so it can be disputed. State the trace and where it came from, the
budget, the model shape, the policy rules and the exact commands that reproduce
the numbers. A result nobody else can re-run is an assertion, not a measurement,
and this one is the whole argument for the project.

Report the separation at more than one budget. A policy that wins only at the
budget it was tuned for has not shown anything, and the shape of the curve --
where the advantage appears and where it disappears -- says more than any single
row. The budget where LRU catches up is a real finding.

Answer `docs/PRD.md` s.9 Q1 and Q2 explicitly, by name, with the numbers that
answer them.

**If the separation is not there, say so and stop.** Do not soften it, do not
pick the trace that makes it look better, and do not proceed to M4. A negative
result here means the gift is not worth what it costs to accept, and the
architecture should be reconsidered before four more milestones are built on it.
That outcome is a finding and the project is better for having measured it than
for having assumed it.

Update `docs/status.md`: G4 met or not met, what the numbers were, and what
changed about the plan as a result.
