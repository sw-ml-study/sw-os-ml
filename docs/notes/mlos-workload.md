# mlos-workload

A decode loop, generated as an access trace: several sessions sweeping a
model's weights and re-reading their own KV cache. Host-side measurement
layer; the workload the M3 comparison is decided on. Linked from
`crates/mlos-workload/src/lib.rs`, `model.rs`, `real.rs` and `shape.rs`.

## Why a generated workload at all

The recorded trace cannot decide anything. A dense sweep re-reads nothing
within a pass, so LRU evicts precisely what it is about to need and misses
everything; known-next-use is optimal on it by construction. Both facts
follow from the shape of the workload before any policy is written, and a
result that could not have come out otherwise is not a measurement.

A real decode loop has two halves that pull in opposite directions:

- **Weights are swept cyclically.** Every token reads every tile, in the
  same order. This is LRU's worst case, and worth understanding rather
  than just asserting: in a cycle longer than the budget, the tile LRU
  just touched is the one it will want last, so it evicts exactly what it
  is about to need.
- **KV accumulates and is re-read.** Each token appends a block per layer
  and reads every earlier block for that layer.

## Why round-robin over sessions of different lengths

The first design was one session re-reading its whole prefix every token,
and it did not work. Re-reading the whole prefix is itself a cyclic sweep:
every block is read once per round, in order, so after one pass LRU's
recency order is exactly insertion order and it evicts what FIFO evicts.
Measured, they tied at 192 reads each on the KV half alone.

**A purely cyclic workload can never distinguish FIFO from LRU**, and that
is a property rather than an accident. The two differ only when something
placed early is used recently, which a uniform scan never produces.

What produces it in real serving is concurrency. Several sessions decode
at once, they sit at different positions, and they do not all finish
together. A session that has stopped generating still holds KV that will
never be read again; an active session holds early blocks it reads every
round. LRU distinguishes those and FIFO cannot: it evicts the active
session's early blocks while a finished session's later blocks survive
purely for being younger.

So the workload is round-robin over sessions of different lengths.
`Decode::length` spreads them evenly, so the shortest stops after a
fraction of the run and the longest lasts all of it. That is also nearer
the system MLOS is for: the premise in `docs/PRD.md` is many sessions
wanting the same weights, and a single-session trace could not express
it.

## The access order follows from the arithmetic

Within a session's turn: every weight tile, layer by layer, then that
session's KV prefix for the layer, because the projections come from the
weights and attention reads the cache. An order chosen for convenience
would be a different workload wearing this one's name.

`Decode::trace` returns a `Vec` rather than filling a caller's slice,
unlike `mlos-trace`: nothing replays a generated trace in the kernel, and
a length the caller had to predict is a length that can disagree with the
loop that fills it.

## This is a model of a decode loop, not a recording of one

Weaker provenance than the recorded step-001 trace, and it has to be said.
What keeps it honest is that the access order follows from the structure
rather than from anyone's preference, and that `Real` replaces the
synthetic sizes with a real model's.

## The model half, and one generator

A trace names objects and says nothing about their size or cost, because
those are properties of the model rather than of the workload. `model.rs`
is the half the trace deliberately does not carry: the `Model` impls the
simulator pairs with the trace, whose header names which one it must be.

`Decode::meta` answers for weight tiles from the synthetic model and for
KV blocks from this workload, which is what produced them. Anything else
is `None` rather than a guess: a trace naming an object this workload
never generates means the two are not a matched pair, and the simulator
counts that separately from a residency decision for exactly that reason.

`Decode::text` renders the workload once, on the host, to be written where
the guest will read it, so the kernel replays the same accesses the
simulator measured rather than its own idea of them. Two generators
agreeing is a thing to be checked; one generator and a file is a thing
that cannot disagree. It sits beside the `Model` impl because both are
this workload as another crate consumes it.

## `Real`: the same loop over a real shape

`Decode` is the loop over the 8x16 synthetic model, and everything it
argues about sessions and reuse holds for `Real`. What changes is where
the objects come from: not `mlos-synth`'s uniform kilobyte tiles but a
`Shape` read from emufpga's sidecar -- real tensors, real sizes, and the
model's own declaration of which are swept per token and which are read
once. That is the difference between a trace shaped like inference and a
trace shaped like a guess, which `docs/architecture.md` s.12 names as the
risk.

One token, one session, in this workload:

1. Per layer, the rotating streams in declared order -- q, k, v, then that
   session's KV blocks for the layer so far, then o, gate, up, down.
   Attention reads the cache between the projections that feed it and the
   one that consumes it, so that is where the blocks go.
2. Then the rotating streams outside any layer: the output head.

The resident streams (embeddings, norms) are read once, at the start, by
the first session, which is exactly what the sidecar declares them to be:
"read once into RAM". A norm is touched every layer in the arithmetic, but
the sidecar's author put it in RAM for good and this workload does not
second-guess a declaration it was given. Whether those resident bytes then
survive is the policy's problem, as it would be in a real system.

### Costs are per byte

`Real::meta` prices weights and KV by the byte: three milliseconds to the
first byte, then a gigabyte a second, the same figure `mlos-synth`'s disk
tier charges, applied per object. A precomputed scalar sized for a
kilobyte tile would let a 400 MB head arrive for the price of one, which
is the unit error `tests/verdict.rs` had to correct.

## Context, and why it is a knob

Measured without one, the real shape is a weight sweep with a rounding
error of KV on it: MiniCPM5-1B's two KV heads make one token's cache
24 KiB across all layers, against 1.68 GB of weights swept to produce it.
Forty tokens of four sessions is 3.8 MB of cache. On that trace FIFO and
LRU tie exactly -- a pure cycle cannot distinguish them -- and next-use
wins by construction, which is the degenerate case the M3 plan said to
avoid, arriving from a real model rather than a synthetic one.

What makes KV matter is context: sessions that arrive with a long prompt
already cached, and cache blocks coarse enough that a real context is
thousands of objects rather than millions. `Context` is both numbers.
`Context::DECODE_ONLY` is the bare loop, for comparing with the synthetic
model; a prefix in the thousands with sixteen or thirty-two tokens per
block is what a serving engine actually holds, and is where recency has
something to be right about. One token per block is a block per position,
as `mlos-synth` defines it; a serving engine pages the cache in sixteen or
thirty-two, and so does a table that has to hold it.

## The shape is read, never invented

`shape.rs` reads the sidecar `emufpga import` writes beside a `.spm`: one
line per stream with rows, columns and element count, and a
`rotating-streams` line saying how many of them are swept once per
operation and rewound. Everything after that boundary is read once into
RAM. That is an access pattern declared by the model's own forward pass,
and the module only reads it. The sidecar is kilobytes of text; the
weights themselves are never opened here, and never will be. What a
residency policy needs to know about a model is what it is, not its bytes.

Sidecar order is consumption order for the rotating region, because that
is what the order file is; `Shape::streams` keeps it.

`bytes_per_element` is the caller's to say: the sidecar counts elements
and does not know what they will be stored as. Two for the checkpoint's
own F16; a policy comparison at another precision changes that one number
and nothing else.

Streams outside any layer (embeddings, final norm, output head) are filed
under one past the last real layer, so an `ObjectId` can name them without
a class of their own.

`Shape::kv_block_bytes` is read off the model rather than assumed: the key
and value projections' output widths are the per-token cache row, so the
number is `rows(k_proj) + rows(v_proj)` elements of layer zero. Zero for a
shape with no attention in it, which a caller should treat as "this is not
a decoder".

## Tokens, and one generator

M4 step 003. `Decode::tokens(session)` and `Real::tokens(session)` give
one session's stream as a list of tokens, each the objects one step asks
for in the arithmetic's order. `trace()` on both is now
`mlos_sched::merge(ProcessMajor, tokens)`: the hand-written round-robin
loops are gone, and `tests/baseline.rs` holds the merged order to the G4
report's integers (25,200 accesses, 12,722 and 3,480 next-use reads;
54,150 and 44,309 on the real shape; the kernel's 4,448-access trace at
3,748) so that one generator cannot drift from what was measured. The
real shape's resident streams are the first thing in session zero's
first token, which is where the old loop put them.
