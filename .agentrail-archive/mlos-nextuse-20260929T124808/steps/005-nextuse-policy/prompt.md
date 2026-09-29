Known-next-use replacement, in the simulator.

The policy the project exists to test. When the budget is full, evict the object
whose next use is furthest away -- Belady's rule, which is normally
unimplementable because it needs the future, and which a declared stream hands
over for free.

What it must get right that a naive reading would not:

- **Distance and probability are different kinds of knowledge.** Act on
  `Distance` with certainty; treat `Probability` as a weight, never as a
  distance. An expert the router gives a 5% chance is not "far away", it is
  uncertain, and evicting it costs its reload multiplied by that chance.
- **Cost is part of the decision, not just distance.** `ObjectMeta` carries
  `reload_cost` and `recompute_cost` because they differ by orders of magnitude
  and by class. The furthest-away object is the wrong victim if it is also the
  only one that cannot be recomputed. Weights have `CostNs::IMPOSSIBLE` for
  recompute; activations are cheaper to rebuild than to store. A policy that
  looks only at distance throws away the other half of what the table knows.
- **`Never` is not infinity.** An object nothing has declared a future for is
  the obvious victim, but it is obvious because nothing KNOWS about it, not
  because nothing will want it.

Against the baselines on both traces, with the table in the summary. The
step-001 dense sweep should show a large separation and prove little; the
step-004 generative trace is the one that counts.

If the separation on the generative trace is not there, say so. `docs/PRD.md`
s.9 Q1 and Q2 are answered by this measurement whichever way it comes out, and a
negative result means the architecture should change before four more milestones
are built on it.
