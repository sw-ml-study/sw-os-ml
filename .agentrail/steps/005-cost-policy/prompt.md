The cost-aware policy, `docs/architecture.md` s.5: four eviction verbs,
not one -- DEMOTE, DISCARD (recompute later), QUANTIZE, DROP -- chosen by
`min(reload_cost, recompute_cost, quantize_cost)` against size, never past
the owner's quality floor. Known-next-use already divides by recovery
cost; this is the step where the verb is chosen too, and where
quantization becomes an eviction outcome rather than a rung walked by
hand. Simulator first, with the G4 and G5 workloads as the baselines it
must not lose to; kernel to the integer.
