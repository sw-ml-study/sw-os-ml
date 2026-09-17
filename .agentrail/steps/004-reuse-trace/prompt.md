A workload with reuse in it, so the comparison can come out either way.

**This step takes over the job the `generative-trace` step was written for.**
That step now covers realism only -- real tensor sizes from a real checkpoint --
and it is blocked on emufpga. This one is not blocked on anything, because what
the measurement needs first is not realism, it is FAIRNESS.

The trace from step 001 is a dense sweep: every tile read once, in order, never
re-read. On it, LRU is pessimal by construction and known-next-use is optimal by
construction. A comparison run against it would separate beautifully and mean
nothing, because it could not have come out any other way.

What a decode loop actually does, and why it is the fair test:

- **Weights are swept cyclically.** Every token reads every tile, in the same
  order. That is LRU's WORST case and it is worth understanding why: the tile
  LRU just used is the one that will not be wanted again until the next token
  comes round, so LRU evicts exactly the tiles it is about to need.
- **KV blocks accumulate and are re-read.** Each token appends a block per layer
  and re-reads every earlier block for that layer. That is LRU's BEST case:
  recently-written KV really is about to be used again, and LRU keeps it.

A real decode loop has both at once. That is what gives LRU a fair chance to be
right about half the workload, and it is the only way beating it means anything.
Weight the two so neither trivially dominates, and say in the summary what the
ratio was and how sensitive the result is to it -- if the answer swings on that
knob, the knob is the finding.

`mlos-synth` needs KV blocks to express this. `ObjectClass::KvBlock` exists in
the ABI and nothing registers one. They are session-scoped -- the class reuses
the model field as a session id, see `ObjectClass::is_session_scoped` -- mutable,
and they grow with context, which is the opposite of a weight tile in every
respect that matters to a policy. Registering them is most of this step.

Generate the trace with the existing `mlos-trace` writer, from the model
definition rather than by hand. It is derived rather than recorded, which is
weaker than step 001's provenance, and the summary must say so: it is a model of
a decode loop, not a recording of one. What keeps it honest is that the access
ORDER follows from the model's structure rather than from anyone's preference,
and that the step-010 realism work replaces the sizes with real ones later.

Say how big it gets. Step 001 measured 20.2 bytes an access; a thousand tokens
of a model this shape will be a large multiple of 66 accesses.
