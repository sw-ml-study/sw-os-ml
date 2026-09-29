OUTCOME 1: known-next-use separates clearly. The saga continues.

28 configurations (4 session counts x 7 budgets). Next-use does 32-71% fewer provider reads than the best baseline and NEVER loses. Where the working set fits the budget, every policy converges as it must.

  4 sessions, 25,200 accesses:
   96 KiB  fifo 25200  lru 25200  next-use 15791  +38%
  128 KiB  fifo 25200  lru 25200  next-use 12599  +51%
  160 KiB  fifo 19526  lru 17692  next-use  7729  +57%
  192 KiB  fifo 12067  lru 11942  next-use  3480  +71%
  224 KiB  fifo  2917  lru  5112  next-use  1312  +56%
  256 KiB  fifo  1448  lru   928  next-use   928   +0% (fits)

Bytes fall further than reads (a fifth of LRU's at 128 KiB against half the reads) because the policy keeps expensive objects and evicts cheap ones -- the cost term earning its place.

HARNESS CHECKED FIRST, as required. LRU beats FIFO at 4 and 8 sessions through most of the band. It does NOT at 1-2 sessions, where the workload is near a pure cyclic scan -- step 004's finding holding, not a harness fault. Next-use never loses; that invariant found the two step-005 bugs and is pinned by tests/optimal.rs.

PRD Q1 (tile vs tensor) ANSWERED: neither, and this workload cannot settle it. Priced identically (3 ms latency + 1 GB/s for both), tiles move 29% fewer bytes at the tightest budget; tensors cost 6% less TIME because a 3 ms seek dwarfs a 16 KiB transfer. Which matters depends on the tier. Decisively: the workload sweeps every tile of a layer consecutively, so a coarse object never brings an unwanted byte -- the case tiles exist for (partial access under MoE routing) is absent. Keep tiles (ObjectId already carries the field, costs nothing), revisit at M5.

PRD Q2 (table in kernel or service) HOLDS, with numbers. At 192 KiB: 86% of accesses are hits, 12.3% cause an eviction. Hits crossing IPC would dominate -> table stays in kernel. An eviction already sits behind a 42 us fault, so a few-microsecond service round trip adds ~12% to fault cost -> policy in a service is affordable. The step-002 policy interface was built pure for exactly this and can move without the table.

DOCUMENT: docs/m3-verdict.md, findable without reading a commit message, regenerable by the command at its top.

WHAT IT DOES NOT SETTLE (verdict s.5): (1) THE SIMULATOR SUPPLIES THE FORESIGHT by holding the whole trace -- the claim is that a transformer hands the OS that knowledge via a declared stream, and nothing has built or measured that; step 007 does, and if streams cannot deliver next-use cheaply the result does not transfer. (2) the workload models a decode loop rather than recording one. (3) costs are modelled. (4) the arena still cannot evict at all. (5) it is NOT evidence MLOS beats Linux -- only that a policy knowing next-use beats policies that do not.

GATE G4 IS NOT MET. G4 needs the comparison from a running kernel: step 008 (evictable arena) and step 009 (in-kernel), and 009 must reproduce these counts EXACTLY.