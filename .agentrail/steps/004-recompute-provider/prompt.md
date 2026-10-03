The recompute provider, which makes `DISCARD` legitimate
(`docs/architecture.md` s.5: an activation cheaper to rebuild than to
keep is discarded, not demoted). `mlos-synth::tiers` has a recompute tier
with a cost and nothing behind it; give it a provider that produces an
object from what it depends on, charges `recompute_cost`, and counts `Rc`
(recomputed byte-equivalents per token, `docs/PRD.md` s.5.2). KV blocks
can be recomputed from the prefix; weights cannot, and the provider must
refuse them rather than fabricate.
