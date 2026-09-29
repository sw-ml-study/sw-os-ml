`mlos-trace`: a recorded access sequence, with record and replay, host-side.

First, take the name back. M2's `mlos-trace` records residency EVENTS --
placed, hit, refused -- which is what the object manager DID. This saga's
`mlos-trace` is an access trace: what the workload ASKED FOR, independent of
what any policy did about it. They are different things and the second one is
what the plan and `docs/architecture.md` have always meant by "trace". Rename
the M2 crate to `mlos-events`, which is what it actually is: lib.rs, event.rs,
line.rs, its `Manager::trace` field, and the handful of callers. Mechanical, and
`cargo kbuild-arm` plus the boot tests will tell you if you missed one.

Then the format. An access trace is a sequence of acquires: which session
wanted which `ObjectId`, in order. That is all a policy needs to be replayed
against, and deliberately nothing about tiers, costs or residency -- those are
properties of the SYSTEM under test, not of the workload, and baking them into
the trace would mean every policy was scored against a different question.

Record and replay both, and make them exact inverses: a trace written and read
back must be the same sequence, checked by a test rather than by eye.

The first trace must not be a literal. Derive it from an existing
`examples/viz/runtime-events.jsonl`: every `placed` and every `hit` is exactly
one acquire that really happened, in the order it happened, on a real kernel
reading a real device. That makes the bootstrap trace a recording rather than
an invention, and it is the discipline `docs/architecture.md` s.12 asks for --
the risk being that a hand-written trace quietly encodes the answer you were
hoping to measure. Note in the summary that this particular trace is a dense
sweep and therefore the EASY case; step 004 brings the one with real reuse
structure in it.

Host-side and `no_std`-compatible where it costs nothing: step 008 runs the
same policies in the kernel, and a trace type the kernel cannot name would mean
two formats. The file format itself is host-side.

Say in the summary how big a trace gets per token of a real model, because
step 004 will multiply it.
