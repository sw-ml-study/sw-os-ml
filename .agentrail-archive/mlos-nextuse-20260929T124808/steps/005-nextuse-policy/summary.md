Known-next-use: evict what is wanted furthest away, per unit of what getting it back would cost. Measured on the step-004 workload (4 sessions, 40 rounds, 25,200 accesses):

  budget   fifo    lru   next-use
  128 KiB 25200  25200     12599  +51%
  160 KiB 19526  17692      7729  +57%
  192 KiB 12067  11942      3480  +71%
  256 KiB  1448    928       928   +0% (everything fits)

Total recovery cost at 128 KiB: LRU 56,160 ms vs 5,856 ms -- about tenfold.

THE RESULT WAS INVERTED UNTIL TWO BUGS WERE FOUND, and they were found by a RULE rather than a test: a policy with strictly more information than LRU cannot honestly lose to it. It was losing 3x, so the policy was wrong, not the thesis.

BUG 1 -- coordinate mismatch. Foresight::after answers in trace INDICES; foresee was handed a one-based tick and compared them directly. An object whose next use was the very NEXT access failed 'at > now', fell through to NextUse::Never, and became the MOST evictable thing in the table. Belady was being told to evict exactly what it was about to need. DIAGNOSIS METHOD worth repeating: ran pure Belady (no cost term) beside the cost-weighted policy; when BOTH lost, the cost term was exonerated and the foresight was the only suspect left.

BUG 2 -- integer truncation. Recovery costs are nanoseconds in the millions; scaling distance by 1000 before dividing made every weight tile nearer than 4000 steps score exactly 0, so the policy broke ties by table position. With SCALE=1e6 the cost term does what it was meant to: a few more reads than pure Belady, and lower total recovery cost. tests/optimal.rs now holds that as an invariant (within 10% of the Belady read ceiling, and never costlier).

CORRECTION TO STEP 004: it concluded policy only matters in a band, and below 128 KiB everything misses whatever is evicted. True of FIFO/LRU, FALSE of next-use -- where both baselines score 25,200 of 25,200, next-use serves half. The cliff was a property of the baselines, not the workload.

THE THREE RULES are pinned individually in mlos-policy/tests/decides.rs: furthest-away goes first; Never (nothing declared a future) goes before any known distance; cheaper-to-recover goes first at equal distance; distance still beats cost when large enough (the truncation regression); and a Probability horizon is HALVED so a hint must be twice as good as knowledge before it wins.

NOT THE VERDICT. Step 006 must weigh: the result moves with session count (step 004); the SIMULATOR supplies foresight that step 007's syscalls would have to deliver in a kernel; the workload is a model of a decode loop, not a recording.

sw-checklist 176/0/5 -- new one is mlos-sim at 5 modules (lib, foresight, resident, run, view); collapsing any pair trades it for a function-count warning.