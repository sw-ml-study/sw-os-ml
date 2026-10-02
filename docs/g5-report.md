# Gate G5: four sessions, one read

Status: MEASURED, 2026-10-02, saga `mlos-parameter-major` step 006.

**G5 is met.** On a decode loop shaped by a real 1.08-billion-parameter
checkpoint, four sessions needing the same layer cause one provider read,
not four: under parameter-major scheduling the weight reads fall from
once per session per token to once per token, and with a policy that
knows the future the total reads fall sixteenfold (44,309 to 2,787 at
1536 MiB). `Ps`, parameter applications per parameter read, goes from 8.8
to 77.4. The same scheduler crate runs in the kernel over its session
table and produces the same integers as the simulator, on aarch64 and on
x86-64 (section 5).

Two things cut the other way and are reported as findings: the gain is
bounded by the share of accesses that are a session's own cache, which
no scheduler can share, and lockstep costs a session latency, which a
contracted session buys back at a price that rises steeply once it runs
a token ahead of the others.

## 1. What was compared

### The schedules

Both are `mlos-sched`, `no_std`, shared by the simulator and the kernel.
A scheduler sees each session's position (token, index within it, how
long it has waited, its latency ceiling) and what it wants next, and
names the session to serve.

| schedule | rule | source |
| --- | --- | --- |
| process-major | one session from the start of a token to its end, then the next live session, round-robin. The order M3 measured on | `process.rs` |
| parameter-major | the lowest `(token, index)`, ties to the object the most sessions want next, then the lowest session; a session past its latency ceiling is served first. A token barrier plus lockstep: the model passes memory once per token | `parameter.rs` |

### The policies, budget and costs

As in [g4-report.md](g4-report.md): demand, FIFO, LRU and known-next-use
from `mlos-policy`, with ties broken on object id; the budget a real
first-fit arena (`mlos-arena`) of exactly the stated bytes; weights at
3 ms plus one nanosecond per byte to reload, KV at 0.4 ms. Next-use
divides recovery cost by `share_count`, the live holds on an object, so
an object k sessions hold is k times dearer to evict (step 002).

### The traces

Per-session lanes, each a list of tokens: `Decode::tokens(session)` over
the synthetic 8x16 model and `Real::tokens(session)` over MiniCPM5-1B
(the shape and provenance are in the G4 report). Sessions finish at
different times, as in M3. A schedule merges the lanes into one trace
(`mlos_sched::merge`); the process-major merge is byte for byte the trace
G4 was measured on, held to its integers by `tests/baseline.rs`. Every
number below is reads from a provider, lower being better, with the KV
share of accesses beside it because that share bounds the gain.

## 2. The tables

`cargo test -p mlos-workload --test g5 -- --ignored --nocapture
--test-threads=1` prints them with timings.

### MiniCPM5-1B at F16, 40 rounds, no prompt

| sessions, budget | KV share | LRU process | LRU parameter | gain | next-use process | next-use parameter | gain | Ps next-use: process / parameter |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1, 1536 MiB | 74% | 26,490 | 26,490 | +0% | 22,632 | 22,632 | +0% | 7,653 / 7,653 |
| 2, 1024 MiB | 70% | 34,910 | 31,573 | +10% | 32,085 | 10,869 | +67% | |
| 2, 1536 MiB | 70% | 34,910 | 31,530 | +10% | 29,004 | **1,726** | **+95%** | 8,256 / 46,551 |
| 4, 1024 MiB | 68% | 54,150 | 44,053 | +19% | 49,555 | 11,664 | +77% | |
| 4, 1536 MiB | 68% | 54,150 | 44,010 | +19% | 44,309 | **2,787** | **+94%** | 8,801 / 77,433 |
| 4, 1664 MiB | 68% | 54,150 | 44,010 | +19% | 40,655 | 2,664 | +94% | |
| 8, 1024 MiB | 67% | 93,830 | 70,213 | +26% | 85,381 | 32,448 | +62% | |
| 8, 1536 MiB | 67% | 93,830 | 70,170 | +26% | 76,207 | **4,875** | **+94%** | 9,196 / 139,197 |

### MiniCPM5-1B at F16, 40 rounds, 2,048-token prompts, 16-token KV blocks, 1792 MiB

| sessions | KV share | LRU process | LRU parameter | gain | next-use process | next-use parameter | gain | time |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 4 | 94% | 217,251 | 224,997 | -3% | 79,254 | **13,438** | **+84%** | 166 s |
| 8 | 94% | 512,427 | 521,633 | -1% | 301,800 | **25,680** | **+92%** | 507 s |

### Synthetic 8x16 model, 40 rounds

| sessions, budget | KV share | LRU process | LRU parameter | gain | next-use process | next-use parameter | gain |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 2, 96 KiB | 51% | 15,920 | 13,360 | +17% | 10,392 | 9,721 | +7% |
| 2, 192 KiB | 51% | 4,960 | 5,580 | -12% | 928 | 928 | +0% |
| 4, 96 KiB | 49% | 25,200 | 17,520 | +31% | 15,927 | 13,811 | +14% |
| 4, 128 KiB | 49% | 25,200 | 17,520 | +31% | 12,722 | 12,455 | +3% |
| 4, 160 KiB | 49% | 17,692 | 16,912 | +5% | 7,729 | 7,736 | +0% |
| 4, 192 KiB | 49% | 11,942 | 15,246 | -27% | 3,480 | 3,480 | +0% |
| 8, 96 KiB | 47% | 44,160 | 26,240 | +41% | 27,390 | 22,531 | +18% |
| 8, 256 KiB | 47% | 16,253 | 20,194 | -24% | 4,448 | 4,448 | +0% |

## 3. What the tables say

**Four sessions, one read.** On the real shape under LRU, parameter-major
takes the weight reads from once per session per token to once per
token: 16,900 to 6,760 across the run with four sessions, and nothing
else moves, because every KV access still misses a pure cycle. That is
the gate's sentence, measured. Under next-use the weights are shared and
the cache is kept as well, and the reads fall sixteenfold.

**The gain is bounded by the cache.** A session's KV is its own.
Parameter-major removes at most `(N - 1) / N` of the weight reads and
none of the KV reads, so the KV share of accesses caps the gain for a
policy that cannot keep the cache: LRU gains 19% with four sessions when
KV is 68% of accesses, and loses 1 to 3% when it is 94%, because
lockstep interleaves the sessions' cache phases and spoils recency. The
synthetic model shows the other end: its 128 KiB of weights fit in every
budget from 128 KiB up, so there is nothing to share, next-use already
served the weights once for everyone, and parameter-major changes its
reads by at most 18% at 96 KiB and nothing above. LRU loses 12 to 27% at
loose budgets on that model for the same lockstep reason. The negative
rows are findings.

**`Ps` is the headline.** Weight bytes applied per thousand weight bytes
read. Next-use alone gets 7,653 to 9,196 on the real shape, because the
weights that fit stay; parameter-major with four sessions gets 77,433,
with eight 139,197, and the number keeps growing with sessions because
every session after the first is a hit on a read already made. Where the
weights fit entirely it is the same under both schedules and equals the
uses per weight over the run, the ceiling.

## 4. Latency, and the price of leaving the lockstep

Period is what a session's user waits for each token: acquires from the
first access of one token to the first access of the next. Real shape,
four sessions, 1536 MiB, next-use; session 1 carries the ceiling.
`cargo test -p mlos-workload --test latency -- --ignored --nocapture`.

| schedule | session 1 ceiling | reads | session 1 period mean / worst | session 4 period mean / worst |
| --- | --- | --- | --- | --- |
| process-major | none | 44,309 | 1,086 / 1,540 | 1,338 / 1,923 |
| parameter-major | none | 2,787 | 1,208 / 1,633 | 1,353 / 1,946 |
| parameter-major | 1024 | 2,787 | 1,169 / 1,540 | 1,353 / 1,946 |
| parameter-major | 512 | 3,206 | 817 / 964 | 1,353 / 1,946 |
| parameter-major | 256 | 2,922 | 561 / 753 | 1,353 / 1,950 |
| parameter-major | 128 | 22,709 | 433 / 513 | 1,353 / 2,336 |
| parameter-major | 32 | 23,137 | 337 / 417 | 1,353 / 3,638 |

Lockstep costs session 1 about 11% in period for sixteen times fewer
reads. The escape, `docs/architecture.md` s.6's latency-contracted
session buying its way out of the batch, is a ceiling on the acquires a
session will wait since its token became due: once overdue it is served
first and runs its token alone. A ceiling of 256 halves the period for 5%
more reads; 512 for 15%; at 128 and below the session runs a full token
ahead, every weight it reads alone is one the others then miss, and the
group pays eight times the reads. The break-even for this workload is a
ceiling near a quarter of a token. The unit is acquires, the kernel's own
clock and the unit `next_use` is in.

## 5. In the kernel

The same `mlos-sched` crate runs in the guest. `replay POLICY
[process|parameter]` reads the trace off the model disk, adopts its
sessions into the manager's session table, cuts it into per-session
lanes in static buffers (a new token wherever a session touches the
model's first tile), runs the scheduler over them to produce the order,
declares that order as the stream, and replays it. Every count comes from
what the manager did. The boot tests `crates/mlos-cli/tests/replay.rs`
and `replay_x86.rs` boot the aarch64 guest, and then both guests side by
side, under TCG, and compare each of eight lines (four policies, two
schedules) as a string with the simulator's, headline numbers included.

**They match.** On 2026-10-02 both tests passed: the aarch64 guest's
eight lines equal the simulator's, and the x86-64 guest's equal both. The
kernel's parameter-major numbers on its 4,448-access trace at 32 KiB are
the simulator's exactly, reads, hits, bytes, evictions, refusals, `Ps`,
`Ks` and `Ss`. The process-major lines are the ones the kernel has
printed since M3 step 009: the lanes rebuilt from the trace and
scheduled process-major reproduce the trace itself.

## 6. What this does not show

1. **Sessions without processes.** A session is a kernel record the shell
   and the replay create; there is no userspace to own one, and the
   traces are derived from a real checkpoint's inventory, not recorded
   from an inference engine. The gap G4 named is still open.
2. **One access at a time.** The simulator and the guest serve accesses
   serially, so a period is a count of acquires, not a time. Parallel
   service, where lockstep would also save wall-clock by batching compute,
   is not modelled; what is measured is the reads and the ordering, which
   is what residency can be measured on.
3. **The real shape runs in the simulator only.** The kernel schedules
   and replays the synthetic 4,448-access trace, where it proves the
   simulator faithful; the real shape's numbers are the simulator's.
4. **Leases are counted, not enforced.** `share_count` weighs the policy;
   nothing refuses to evict a pinned object or revokes a borrow between
   operations.
5. **`Ss` is measured, not admitted.** Sessions per GiB of resident budget
   is the live count over the arena's capacity until M5's admission
   control gives the budget a say.
6. **One model, one serving regime.** MiniCPM5-1B has aggressive
   grouped-query attention and a small cache. A model with more KV heads
   sits further toward the long-context rows, where the inversion matters
   less for a recency policy and most for one that knows the future.

## 7. Reproduce it

```sh
# The three G5 tables (two fast, the long-context one about 12 minutes).
cargo test -p mlos-workload --test g5 -- --ignored --nocapture --test-threads=1

# Periods and the escape's price.
cargo test -p mlos-workload --test latency -- --ignored --nocapture

# The process-major merge equals the G4 order, to the integer.
cargo test -p mlos-workload --test baseline

# The kernel, both architectures, eight lines each, against the simulator.
cargo test -p mlos-cli --test replay -- --ignored
cargo test -p mlos-cli --test replay_x86 -- --ignored
```

Every number in this document was pasted from those commands' output on
2026-10-02.

## 8. What changes in the plan

G5 is met, so M5 (`mlos-degradation`, gate G6) is worth doing. Three
findings feed forward:

- **Lockstep is a latency contract's problem.** M5's `Contract` already
  has the field; its admission control should admit a session with a
  latency ceiling knowing what that ceiling costs the others in reads.
- **The cache bounds everything.** Degradation rungs that shrink KV
  (quantize cold cache, truncate context) raise the share of accesses a
  scheduler can share; that is a reason to prefer them that this
  measurement puts a number on.
- **Sessions need processes.** Two milestones have now built session
  machinery with nothing to drive it but the shell. The gap G4 named, a
  recorded trace from a real engine with MLOS under it, is the same gap,
  and the plan should give it a step before M6.
