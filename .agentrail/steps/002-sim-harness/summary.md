mlos-sim replays an access trace under a fixed residency budget and reports hits, provider reads, bytes moved, evictions and refusals. mlos-policy is the interface: no_std and pure per design.md s.2, so the same code runs in the kernel and the simulator.

THE BUDGET IS ENFORCED BY THE SIGNATURE. compare(trace, model, budget, policies) takes ONE budget for all of them, so a comparison where policies had different budgets cannot be expressed. That is the easiest mistake here and the hardest to see afterwards.

THE PURITY RULE HAS A PRICE, paid deliberately: a policy that cannot keep its own list finds the LRU object by scanning the resident set, O(n) per eviction. Known-next-use scans too (it wants a maximum), so the cost falls equally on the policy under test and its baselines. Fair, explicitly not fast. Revisit at a million objects.

TWO FIELDS ADDED TO ObjectMeta: placed_tick (insertion order, all FIFO knows) and used_tick (recency, all LRU knows). A policy forbidden its own state cannot derive either. The KERNEL keeps them too, via a new Manager::clock incremented per acquire -- a field the simulator fills and the kernel does not is a field they will disagree about, and step 009 requires exact agreement.

MISMATCHED TRACE/MODEL GETS ITS OWN COUNTER, not a refusal. A refusal is a residency decision; a trace naming objects the model lacks means the two files are not about the same thing, and hiding that behind a plausible number is how a comparison ends up meaningless.

Residency trait is indexed (len/at) rather than iterable, and  returns a copy: the kernel must satisfy it over an open-addressed table with tombstones and cannot hand out borrows. The simulator uses a Vec with linear scans for the same reason -- a HashMap would let a policy be written the kernel cannot host, and that would surface at step 009 after every number was produced.

MUTATION-CHECKED: budget check off by one object fails 3 tests; uncounted refusal fails exactly the demand-paging test; mismatch reported as refusal fails exactly the mismatch test. Determinism is tested directly.

The aliases test caught mlos-sim (std) missing from the bare-target exclusions before the cross build did.

sw-checklist 172/0/4 -- same four, none new; four introduced this step were retired in it.