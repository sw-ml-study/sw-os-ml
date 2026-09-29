mlos-trace is an access trace: a session and an ObjectId per acquire, in order, and nothing else. No tiers, sizes, costs or residency -- those are properties of the system under test, and baking one in would score each policy against a different question.

RENAME: M2's mlos-trace (residency events) is now mlos-events. Manager::trace -> Manager::events. The shell verb stays 'trace' because that is what someone types.

THE FIRST TRACE IS A RECORDING. mlos runtime derives it from the event stream it already emits -- every placed/hit/refused is one acquire that really happened on a real kernel reading a real device, in order. Three artifacts from one boot; no second recorder to disagree. architecture.md s.12 names hand-written traces as the risk and this avoids it.

'refused' belongs in the trace: the workload asked, a policy declined, and under another policy it might have succeeded. That is exactly why the trace must not record what the system did.

A stream that dropped events is REFUSED, not truncated: a trace with an invisible hole is a different workload and nothing downstream could tell.

Events now carry a session. One session exists, so assuming it would have been right -- and would have stopped being right at M4, silently, in a file being measured from.

no_std and allocation-free: parse fills a caller-provided slice, render writes to fmt::Write, because step 009 replays the same traces in the kernel. File I/O is the host's, three lines.

The model name is in the header. An ObjectId does not say how big it is; sizes come from the model the trace was taken against. Replaying against the wrong one is now catchable.

SIZE, for later steps: 20.2 bytes/access. A 24-layer model at ~170 streams/token is 3.3 KiB/token, 3.3 MiB for 1000 tokens. Text, and staying text.

THE TRACE THAT EXISTS IS THE EASY CASE and status.md says so: a dense sweep has no reuse, LRU is pessimal on it by construction and next-use optimal by construction. No number from this saga should be quoted until step 004.

Committed: examples/viz/runtime.trace at clean tree c87f56c7eb4d.

sw-checklist 162/0/4 -- same four, none new; three introduced this step were retired in it.