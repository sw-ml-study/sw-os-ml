The ladder, `docs/architecture.md` s.5: L0 full precision and context, L1
cold KV to Q8, L2 cold KV to Q4, L3 summarise the oldest span, L4 fewer
RAG candidates, L5 truncate the context window, L6 refuse new sessions,
L7 terminate the lowest priority. Under a shrinking budget the manager
walks it rung by rung instead of refusing, never past a session's quality
floor, and records which rung each session is on. Rungs that need
arithmetic MLOS does not have (summarising, RAG) are declared and
accounted, not performed: the object changes size and precision in the
table and the trace says what the workload would have read. Measure what
each rung buys in resident bytes and costs in `Rc` and `Ks`, on the real
shape, in the band.
