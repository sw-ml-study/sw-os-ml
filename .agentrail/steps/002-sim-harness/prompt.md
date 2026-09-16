`mlos-sim`: replay a trace against a policy under a fixed residency budget, and
count what it cost.

The harness the whole milestone is measured with, so its own correctness is the
thing to worry about first. It replays a trace, asks a policy what to evict when
the budget is full, and reports provider reads and bytes moved -- the numbers
`docs/PRD.md` s.5.2 says matter, not wall-clock, because the tier costs are
modelled and a wall-clock comparison would be measuring the host.

**Identical budget across policies, enforced rather than intended.** A
comparison where one policy got more memory is not a comparison, and it is the
easiest mistake to make and the hardest to see afterwards. Make the budget a
property of the run, not of the policy, and have the harness refuse to report a
comparison where the budgets differ.

The policy interface belongs here and it must be the one the kernel can
implement: given the table and the object that needs room, name a victim or
decline. No allocation, no iteration the kernel could not afford. A trait the
simulator can satisfy but the kernel cannot is a trait that will have to be
rewritten at step 008, after every number has already been produced against it.

Deterministic: the same trace and the same policy give the same counts every
run, and a test says so. Any nondeterminism here would make every later number
arguable.

`docs/design.md` s.10 calls this the host-side level of the test pyramid; this
is that level existing.
