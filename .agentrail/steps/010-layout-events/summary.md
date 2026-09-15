Residency transitions stream as JSON Lines behind an '@ev ' marker, one per line, joined to the snapshot by region id. Kinds: placed / hit / refused (evicted reserved for M3, nothing emits it). mlos runtime now writes build/runtime-events.jsonl beside build/runtime-layout.json FROM ONE BOOT, so the snapshot and the events that produced it describe the same run.

DESIGN: recording does not format. The fault path pushes a Copy Event into a 256-entry Ring owned by the Manager and returns; JSON happens later in the shell. The ring is a field rather than a global so tests get their own.

MEASURED OVERHEAD (the step asked for a number): HVF native +4.3 ns/event against 42 us/fault; TCG +129 ns/event against 91 us/fault. One ten-thousandth of a fault, so it is on by default. Method matters: the FIRST attempt measured sweeps that fault and found nothing -- 32 virtio round trips per sweep swamped the signal and run-to-run spread exceeded the effect. The real number comes from sweeps where residency is already established (32 hits + 1 refusal, no device I/O), six repetitions each way in both orders with a warm-up. Needed the generic timer: sweep now reports elapsed ns at 62.5 MHz because the 2 Hz tick cannot resolve anything the object manager does.

REPLAY EQUIVALENCE, the test that keeps the two emitters honest: snapshot before a sweep + replay the stream = snapshot after. Mutation-checked -- wrong placement offset, dropped placement, unrecorded hit each fail it. Nothing else could catch drift; the emitters are in different crates reading different data and each passes its own tests alone.

TWO REAL BUGS FOUND:
1. Silent stack overflow. Manager is built on the stack and moved into a static; a 512-entry table plus the new ring is >40 KiB in debug, and the boot stack was 64 KiB. No symptom: memory is identity-mapped in 1 GiB blocks so overflow corrupts .bss instead of faulting -- the guest just stopped printing after 'model'. Stack raised to 256 KiB in linker/aarch64.ld. THE REAL FIX IS STILL OPEN: stop building a 40 KiB value on the stack. Anything that grows Manager again will hit this.
2. mlos_machine::setting split boot args on whitespace, so mlsh.run=model;trace off;sweep silently became model;trace -- valid, shorter, wrong. setting is GONE, replaced by rest() = everything after the key, caller narrows. Consequence: a boot setting whose value has spaces must come LAST, and mlos runtime now writes mlos.rev= before mlsh.run=.

ALSO FIXED, my own: the five ignored boot tests each ran mlos layout/runtime, which write fixed build/ paths, so they clobbered each other in parallel -- passed serially, failed together. Now emitted once through a OnceLock and shared. Two boots instead of seven.

EVENT SHAPE for step 011 to hand over: {seq, event, region, object, offset, bytes, cost, tier, why}, every key on every line. seq is monotonic and exact; there is NO wall clock -- duration is 'cost', the modelled provider figure, which is the axis M3's comparison uses. The dropped count is always emitted as a final line, never omitted when zero.

Samples at examples/viz/, all from clean tree 8a118259b0e9:
  storage-layout.json  sha256 2f60a8368d2b4b70876abd4dbb485abbd57acd7451339972f6b330257a227f8f
  runtime-layout.json  sha256 eaf309cf83d980e7c8ac2def2a4af156d3e9bef0af9b37744ff0216afcb92653
  runtime-events.jsonl sha256 69f4274c243445ce2763c4f0e1692a3cc051d27e8ba2ff7b3e5816fa0f44b581

sw-checklist 157/0/4 -- same four as after step 009, none new; three this step introduced were retired in it.