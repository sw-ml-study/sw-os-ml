The parameter-major inversion, `docs/architecture.md` s.6. A scheduler
that sees N sessions' declared streams and decides whose turn it is: the
model passes memory once, and every session waiting on the object is
served before the next one is read. Shared `no_std` code in the simulator
first, with the process-major round-robin as the baseline that must
reproduce G4's numbers exactly before anything new is measured. Then the
gate: reads at 1, 2, 4 and 8 sessions, process-major against
parameter-major, in the band near the working set, under both LRU and
next-use. The KV cache is per session and is not shared; report the
bound that puts on the gain.
