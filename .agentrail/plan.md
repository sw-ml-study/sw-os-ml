# MLOS Next-Use (M3)

Vision: prove the thesis, or find out it is wrong.

Belady's optimal replacement is unimplementable in general because it
needs the future. A dense transformer HAS the future: it will read layer
0's tiles, then layer 1's, in that order, every token. The claim
`docs/PRD.md` makes is that an operating system which accepts that gift
beats one that guesses. M3 is where the claim becomes a measured table or
stops being made.

Gate G4: known-next-use replacement reduces provider reads against LRU
and FIFO, under an identical residency budget, on a trace taken from a
real model's shape.

## Three things this saga must not do

**Do not make it easy.** A pure dense sweep is the degenerate case: LRU is
pessimal on it and next-use is optimal, and the separation is guaranteed
before a line is written. A result that could not have come out otherwise
measures nothing. The trace has to carry real reuse structure -- a KV
cache that accumulates across tokens and is re-read, weights re-swept per
token -- so that LRU has a fair chance to be right sometimes.

**Do not hand-write the trace.** `docs/architecture.md` s.12 names this as
a risk and it is the obvious way to cheat without meaning to. The model's
shape comes from emufpga's `.spm` sidecar, which is real: it lists every
tensor with rows, cols and element count, and it declares which streams
are swept once per operation and rewound and which are read once. That is
an access pattern from a real checkpoint, not a guess at one.

**Do not let the kernel and the simulator drift.** They must link the same
policy crates and produce the same numbers on the same trace. A policy
that behaves differently in the two is a policy neither result describes.

## Where this starts from

M2 left: an object table, a fault path, three tiers, two layout emitters
and a residency event stream. It left NO decision-making at all -- the
arena is a bump allocator and a full arena returns `NoBudget` rather than
choosing a victim. Every policy in this saga is therefore new, and the
kernel cannot run any of them until the arena can evict.

A naming collision to clear first: M2's `mlos-trace` crate records
residency EVENTS (placed/hit/refused). This saga's `mlos-trace` is an
access trace -- what the workload asked for, independent of what any
policy did. Step 1 renames the former to `mlos-events`, which is what it
actually is, and takes the name back.

## Steps

1. **trace-format** -- `mlos-trace`: a recorded access sequence, with
   record and replay, host-side. Derive the first one from an existing
   `runtime-events.jsonl`, so even the bootstrap trace comes from a real
   run rather than a literal. Rename M2's `mlos-trace` to `mlos-events`.
2. **sim-harness** -- `mlos-sim`: replay a trace against a policy under a
   fixed residency budget, counting provider reads and bytes moved.
   Identical budget across policies or the comparison says nothing.
3. **baselines** -- `mlos-policy`: demand, FIFO, LRU. The numbers to beat,
   and the ones that tell you the harness works: LRU must beat FIFO on a
   workload with reuse, and if it does not, the harness is wrong.
4. **generative-trace** -- a trace with the reuse structure a decode loop
   actually has: weights re-swept per token, KV blocks accumulating and
   being re-read. Shape from emufpga's `.spm` sidecar.
5. **stream-syscalls** -- `ml_stream_declare` / `ml_stream_advance`: the
   calls by which userspace hands the kernel its own future, and the thing
   that finally writes `ObjectMeta::next_use`.
6. **nextuse-policy** -- known-next-use, in the simulator first. Distance
   acted on with certainty, probability only as a hint -- the two are
   different kinds of knowledge and the policy must not collapse them.
7. **evictable-arena** -- the arena stops being a bump allocator. Until it
   can choose a victim and give memory back, no policy can run in the
   kernel at all.
8. **in-kernel** -- the same policy crates, in-kernel under TCG, producing
   the same numbers as the simulator on the same trace.
9. **g4-report** -- the measured comparison table, with the trace, the
   budget and the method stated so someone else can dispute it. Gate G4.

## What would make this fail honestly

If known-next-use does not separate from LRU on a real generative trace,
say so and stop before M4. `docs/PRD.md` s.9 open questions Q1 and Q2 are
answered by this measurement whichever way it comes out, and a negative
result is a finding rather than a failure -- it would mean the gift is not
worth what it costs to accept, and the architecture should change before
four more milestones are built on it.

## Known blocker, from the outset

Step 4 needs a real checkpoint to extract. emufpga has the importer and
the format, but the only `.spm` in the tree is a `tiny.spm` test fixture.
Obtaining a checkpoint (`scripts/extract-checkpoint` takes a `.pt`; the
project targets a GGUF `DeepSeek` R1 quant) is a prerequisite nobody has
done yet. Steps 1-3 and 5-9 do not depend on it; step 4 does, and a
smaller real model is an acceptable substitute for it.
