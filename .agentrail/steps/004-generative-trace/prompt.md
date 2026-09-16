A trace with the reuse structure a decode loop actually has.

This is the step that decides whether M3 measures anything. The step-001 trace
is a dense sweep: every tile read once, in order, never re-read. LRU is pessimal
on it by construction and known-next-use is optimal by construction, so the
separation is guaranteed before any code is written -- and a result that could
not have come out otherwise is not a measurement.

What a generative decode loop actually does: re-sweep every weight tile once per
token, and accumulate KV blocks that are written once and then re-read by every
subsequent token. That gives LRU a fair chance to be right -- recently-used KV
really is about to be used again -- which is the only way beating it means
anything.

Shape from a real model, via emufpga's `.spm` sidecar. It lists every tensor
with rows, cols and element count, and it declares `rotating-streams`: which
streams are swept once per operation and rewound, and which are read once into
RAM. That is an access pattern taken from a real checkpoint rather than a guess
at one, and it is what `docs/architecture.md` s.12 means by not hand-writing
traces.

BLOCKED, and it was blocked before this saga started: the only `.spm` in
emufpga is a `tiny.spm` test fixture. Extracting a real one needs a checkpoint
(`scripts/extract-checkpoint` takes a `.pt`; the project targets a GGUF
DeepSeek R1 quant) and nobody has done it. A smaller real model is an acceptable
substitute -- what matters is that the tensor inventory and the rotating
boundary are read from a real file rather than invented. If no checkpoint can be
had, say so plainly, use `tiny.spm`, and mark every number that follows as
resting on a fixture rather than a model.

Note in the summary how large the trace is, and whether the simulator can still
replay it in a time anyone will wait for.
