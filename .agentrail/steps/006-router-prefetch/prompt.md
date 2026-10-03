Router prefetch, `docs/architecture.md` s.3.5 and s.5: an MoE router
hands the kernel a probability distribution over experts per token, which
is `NextUse::Probability` -- the variant nothing has produced since M2.
A prefetch is a `Speculative` lease; one never upgraded is a prefetch
miss, which is how `Ph` is counted. Build the router-driven workload (an
expert set per layer, a distribution per token), the prefetch policy, and
`Ph`, in the simulator; say what it does to `Ps` and to the band.
Knowledge and guesses stay different kinds: a hint is discounted, never
acted on as a distance.
