# MLOS Degradation (M5)

Vision: make the system able to say "I served you at Q4 with a 4K
context" instead of "killed". (`docs/plan.md`, saga `mlos-degradation`.)

Gate G6 (`docs/PRD.md` s.5.1): under a shrinking residency budget the
system walks a declared degradation ladder (quantize cold KV, shrink
context, drop candidates) and reports the quality and latency it
delivered, rather than OOM-killing a session.

Steps, from `docs/plan.md`: contracts, admission-control, ladder,
recompute-provider, cost-policy, router-prefetch, g6-report. Each step
expands its own reasoning when it is worked; this plan is the outline.

Rules carried from M3 and M4: every decision runs in the simulator first
and the kernel must match it to the integer; measure in the band near the
per-token working set and say where it is; the KV cache stays paged;
negative rows are findings. From the G5 report: admission must know what
a latency ceiling costs the other sessions in reads, and rungs that shrink
KV raise the share of accesses a scheduler can share.
