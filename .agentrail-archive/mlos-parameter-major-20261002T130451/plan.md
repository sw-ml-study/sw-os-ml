# MLOS Parameter-Major (M4)

Vision: invert the scheduler. Stop dragging the model past memory once
per session; drag it past once and serve everyone waiting.
(`docs/plan.md`, saga `mlos-parameter-major`.)

Gate G5 (`docs/PRD.md` s.5.1): four concurrent sessions needing the same
layer cause one provider read, not four, measured against a
process-major baseline on the same trace under the same budget.

Steps, from `docs/plan.md`: session-objects, share-count,
scheduler-inversion, latency-escape, ps-ss-metrics, g5-report. Each step
expands its own reasoning when it is worked; this plan is deliberately
the outline only.

M3's standing rules carry over: measure in the band near the per-token
working set and say where it is; keep the KV cache paged; the kernel and
the simulator agree to the integer.
