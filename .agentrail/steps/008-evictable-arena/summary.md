The arena stops being a bump allocator: first fit over a sorted free list, coalescing on release. Manager::evict gives bytes back, drops residency, restores the home tier, and emits Kind::Evicted -- reserved since M2 step 010 and emitted by nothing until now.

PROVED IN A GUEST, not just in tests. 4 KiB arena holding four tiles:
  evict 0 0 -> evicted 1024 B; evict 0 1 -> evicted 1024 B
  arena     -> 2 of 4 KiB used, room for 2 more tiles, LARGEST RUN 2048 B
Two adjacent holes became one 2 KiB run. get 0 0 afterwards costs the same 3001 us the first fetch did, which is the proof the bytes really went. objs shows evicted tiles as 'Cold 0x0 used 1' against never-touched 'used 0' -- the evicted/never distinction mlos_spaces::state draws.

FRAGMENTATION MEASURED, as asked, because 'free bytes' is the number that hides it:
- UNIFORM objects do not fragment. 1 KiB tiles into 32 KiB: evict alternate ones and every hole is exactly one tile wide and reusable; evict the rest and it is one run again.
- RAGGED objects do. 256 B to 1792 B mixed, half released: 49,664 B free, largest run 34,304 B -> 69% reachable in one piece. KV blocks growing with context are this case.
The shell's  verb now reports the largest run beside the total, because a policy evicting perfectly into memory it cannot hand out has not helped.

A release the fixed free list cannot record is REFUSED and nothing changes -- the object stays resident rather than becoming bytes the accounting has lost. That is a worse failure than declining to evict.

ObjectMeta gained :  MOVES (faulted-in becomes Warm), so eviction needs somewhere to put the object back, and guessing Cold would send a recomputable activation to a block store that never held it.

STRUCTURAL: mlos-arena is now its own crate. mlos-objman was at four modules when eviction arrived and AGENTS.md says make the sibling crate rather than push a fifth concern in. The arena was the separable half -- it hands out runs of bytes and knows nothing about objects. That left room for evict.rs beside fault.rs, which is where throwing something out belongs. Also moved Kind out of mlos-events/event.rs to keep both under the gate. Net: zero new warnings (5, unchanged).

NOTHING DRIVES IT YET. A full arena still returns NoBudget rather than asking a policy for a victim. Wiring the M3 policy into the fault path is step 009. The mechanism exists; the decision does not.

Verdict numbers unchanged: 12,599 at 128 KiB, 3,480 at 192 KiB.