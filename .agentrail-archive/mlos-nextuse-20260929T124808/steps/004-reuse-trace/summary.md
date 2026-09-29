mlos-workload generates a decode loop: N sessions sharing one model round-robin, each sweeping every weight tile per token and re-reading its own KV prefix. 4 sessions x 40 rounds = 25,200 accesses, 12,800 weights and 12,400 KV -- neither half dominating.

TWO GUESSES WERE WRONG, and they are worth more than the step's output.

1. RE-READING A KV PREFIX IS ITSELF A CYCLIC SWEEP. The step prompt expected KV to be where recency pays. It is not: reading blocks 0..t every round touches each once per round in order, so after one pass LRU's recency ordering IS insertion ordering and it evicts what FIFO evicts. Measured: tied at 192 reads each. GENERAL PROPERTY: a purely cyclic workload can never distinguish FIFO from LRU, because the two differ only when something placed EARLY is used RECENTLY and a uniform scan never produces that.

2. WHAT MAKES RECENCY INFORMATIVE IS SESSIONS FINISHING. A stopped session holds KV nobody reads again; an active one holds early blocks read every round. At 192 KiB: 1 session -> FIFO 1,100 vs LRU 3,696 (LRU structurally loses); 4 sessions finishing at staggered times -> FIFO 12,067 vs LRU 11,942 (LRU wins). Tested directly in session_count_is_the_mechanism.

THE TUNING RISK, named in the plan, handled explicitly: what changed was the workload becoming MULTI-SESSION, which is more realistic (real serving runs concurrent sessions starting and finishing at different times; PRD's premise is many sessions wanting the same weights) rather than more convenient. Fairness is a consequence of realism, not the reason for it. The knob is measured by a test so step 006 cannot quietly pick a favourable column.

POLICY ONLY MATTERS INSIDE A BAND -- the most useful thing measured here. Below ~128 KiB the cyclic weight sweep misses everything whatever is evicted; above ~384 KiB the working set fits and nothing is evicted. Every policy is identical outside. 'Which policy is better' is the wrong question; step 006 MUST report the band's shape, not one row. tests/sweep.rs prints it (--ignored --nocapture).

DEMAND'S READ COUNT IS NOT COMPARABLE: 384 reads vs LRU's 11,942, bought by refusing 6,896 of 25,200 where LRU refuses none. Step 006 must print refusals beside reads.

KV blocks live in mlos-workload, not mlos-synth: mlos-synth describes what a model IS (fixed inventory, same every run); KV is produced by running it, grows with the conversation, belongs to one session. Moves to the kernel when M4 makes sessions real.

RATIO SENSITIVITY as asked: weights:KV is 1.03:1 at 4x40. Longer runs shift toward KV (it grows quadratically, weights linearly); more sessions shift toward weights. The band and the winner both move with session count, which is the finding.

sw-checklist 177/0/4 -- same four, none new.