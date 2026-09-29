Demand, FIFO and LRU in mlos-policy, and the first real table.

FIFO AND LRU ARE ONE TYPE reading different clocks -- both evict the smallest tick, differing only in whether that tick is arrival or last use. Written as one, the comparison between them is exactly a comparison of which clock matters. A test confirms they are indistinguishable when nothing is reused, because then the two clocks are the same clock.

THE HARNESS'S OWN TEST PASSES: on a workload where one object is wanted repeatedly, LRU beats FIFO. Had it not, mlos-sim would have been wrong and every later number worthless.

THE FIRST TABLE, on examples/viz/runtime.trace (real recording, 32 KiB budget, 144 KiB model):
  demand  32 reads   0 evicted  2 refused  484 hits/1000
  fifo    66 reads  34 evicted  0 refused    0 hits/1000
  lru     66 reads  34 evicted  0 refused    0 hits/1000

DOING NOTHING WINS and FIFO/LRU are identical. Both are facts about the WORKLOAD: a dense sweep re-reads nothing within a pass, so either baseline evicts precisely what it is about to need and misses every access; demand refuses twice, keeps its 32 tiles, and the second sweep hits them. This is the step-004 argument in numbers rather than prose, and it is recorded rather than skipped because the first fair result needs something to compare against.

UNDESIGNED FINDING: refusing beat replacing. That is PRD F4's admission-control argument arriving uninvited, in a measurement built to show something else.

No policy keeps a queue or list -- design.md s.2 forbids state the table does not own -- so each scans for a minimum, as the kernel would over its own table. Known-next-use will scan for a maximum. Equal cost to the policy under test and its baselines: fair, explicitly not fast.

PROCESS NOTE: the policy sources went in with the CI-removal commit because git add -A swept them. Two concerns in one commit; said rather than rewritten, since it was pushed.