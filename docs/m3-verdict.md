# M3 verdict: does knowing the future beat guessing?

Status: MEASURED, 2026-09-17, saga `mlos-nextuse` step 006.

Reproduce with:

```sh
cargo test -p mlos-workload --test verdict -- --ignored --nocapture --test-threads=1
```

## The answer

**Known-next-use separates clearly, at every budget where any policy can
differ, at every session count tested.** Twenty-eight configurations; it
never loses to a baseline, and where the working set fits in the budget
every policy converges as it must.

The saga continues. What that means and does not mean is in
[§5](#5-what-this-is-not).

## 1. The comparison

Provider reads, lower being better. The workload is a decode loop --
sessions sharing one model round-robin, each sweeping every weight tile
per token and re-reading its own KV prefix -- against the synthetic
8x16 model. Demand paging is shown as `reads+refusals`: its read count is
bought by declining to serve, and is not comparable with policies that
served every access.

### Four sessions, 40 rounds, 25,200 accesses

| budget | demand | FIFO | LRU | next-use | vs best baseline |
| --- | --- | --- | --- | --- | --- |
| 96 KiB | 102+15720 | 25200 | 25200 | **15791** | +38% |
| 128 KiB | 134+12520 | 25200 | 25200 | **12599** | +51% |
| 160 KiB | 256+9392 | 19526 | 17692 | **7729** | +57% |
| 192 KiB | 384+6896 | 12067 | 11942 | **3480** | +71% |
| 224 KiB | 512+4696 | 2917 | 5112 | **1312** | +56% |
| 256 KiB | 640+2672 | 1448 | 928 | **928** | +0% |
| 384 KiB | 928+0 | 928 | 928 | **928** | +0% |

### The same, by session count

| sessions | best gain | where | converges at |
| --- | --- | --- | --- |
| 1 | +44% | 128 KiB | 224 KiB |
| 2 | +67% | 160 KiB | 224 KiB |
| 4 | +71% | 192 KiB | 256 KiB |
| 8 | +59% | 224 KiB | 384 KiB |

The gain is 32--71% across the whole matrix and never negative. The
budget at which the baselines catch up is simply the budget at which the
working set fits; past it nothing is ever evicted and no policy can
differ from another.

### Bytes and cost, four sessions

| budget | policy | reads | KiB moved | evicted | refused | cost (ms) |
| --- | --- | --- | --- | --- | --- | --- |
| 128 KiB | demand | 134 | 128 | 0 | 12520 | 507 |
| 128 KiB | fifo | 25200 | 15900 | 24922 | 0 | 56160 |
| 128 KiB | lru | 25200 | 15900 | 24922 | 0 | 56160 |
| 128 KiB | next-use | 12599 | 3320 | 12468 | 0 | **5856** |
| 192 KiB | fifo | 12067 | 5512 | 11635 | 0 | 16807 |
| 192 KiB | lru | 11942 | 3837 | 11510 | 0 | 8866 |
| 192 KiB | next-use | 3480 | 966 | 3096 | 0 | **1852** |

Bytes moved fall further than reads do -- a fifth of LRU's at 128 KiB
against half the reads -- because next-use keeps the expensive objects
and evicts the cheap ones. That is the cost term earning its place.

## 2. The harness, checked before the result

Two things had to be true before any of the above meant anything.

**LRU must beat FIFO where recency predicts reuse.** It does, at four and
eight sessions through most of the band (17,692 against 19,526 at 160 KiB;
11,942 against 12,067 at 192 KiB).

**It does not beat FIFO everywhere, and that is not a harness bug.** At
one and two sessions FIFO wins in part of the band -- 1,100 against 3,696
at 192 KiB with one session. A single session re-reading its whole KV
prefix each round is a pure cyclic scan, and on a cycle longer than the
budget LRU evicts precisely what it is about to need. This was measured in
step 004 and is a property of cyclic access, not of the simulator.

**Known-next-use must never lose**, because it has strictly more
information than either baseline. It never does, and enforcing that found
two real bugs in step 005 -- a coordinate mismatch that made the object
wanted *next* look like the best victim, and an integer truncation that
flattened the scoring to ties. Both were invisible to every other test.
`crates/mlos-workload/tests/optimal.rs` holds the invariant.

## 3. PRD Q1: tile or whole tensor?

> Is the object granularity a weight *tile* or a whole tensor? Tiles give
> better residency control and a larger table. Start with tensor-granular
> objects and a tile sub-index; revisit after G4.

Measured, with both granularities priced identically -- three
milliseconds to the first byte, then a gigabyte a second, which is what
`mlos-synth`'s backing tier charges:

| budget | granularity | objects | KiB moved | cost (ms) |
| --- | --- | --- | --- | --- |
| 128 KiB | tile (1 KiB) | 928 | **3319** | 37788 |
| 128 KiB | tensor (16 KiB) | 808 | 4675 | **35692** |
| 192 KiB | tile (1 KiB) | 928 | 966 | 10446 |
| 192 KiB | tensor (16 KiB) | 808 | 966 | **10086** |

**Answer: no strong preference, and this workload cannot settle it.**
Tiles move 29% fewer bytes at the tightest budget; tensors cost 6% less
time, because a three-millisecond seek dwarfs a sixteen-kibibyte
transfer. Which matters depends entirely on the tier: against storage,
latency dominates and coarse wins; against a peer's RAM over a fast link
it would invert.

More importantly, this workload sweeps every tile of a layer
consecutively, so a coarse object never brings a byte that is not wanted.
**The case tiles exist for -- partial access, as in MoE expert selection
or sparse attention -- is not in this workload at all.** Keeping tiles
costs nothing (`ObjectId` already carries the field) and the question
should be revisited at M5, when expert routing makes access partial.

## 4. PRD Q2: table in the kernel or in a service?

> Does the model object table belong in the kernel or in a privileged
> service? Provisional answer: table in kernel, *policy* in a service,
> with the policy consulted on eviction and prefetch only.

**The provisional answer holds, and now has numbers behind it.** At
192 KiB with four sessions, next-use serves 25,200 accesses with 3,480
reads: **86% of accesses are hits**, and 12.3% cause an eviction.

- A hit is a table lookup. If hits crossed an IPC boundary they would
  dominate everything -- 86% of accesses paying microseconds for a
  lookup that costs nanoseconds. **The table must be in the kernel.**
- An eviction is 12.3% of accesses, and each one already sits behind a
  fault that costs about 42 us of real time (measured under HVF,
  [status.md](status.md)). A round trip to a service at a few
  microseconds would add roughly 12% to the cost of a fault. **Consulting
  a policy service on eviction is affordable.**

The policy interface built in step 002 was designed for this: `no_std`,
pure, holding no state the table does not own, so it can be moved to a
service without the table following it.

## 5. What this is not

The separation is real and the caveats are not small.

1. **The simulator supplies the foresight.** It holds the entire trace
   and can simply look up when an object is next wanted. The claim is
   that a transformer HANDS an operating system that knowledge through a
   declared stream, and nothing has built or measured that yet. Step 007
   does. If streams cannot deliver next-use cheaply in a kernel, this
   result does not transfer.
2. **The workload is a model of a decode loop, not a recording of one.**
   Its access ORDER follows from the model's structure rather than from
   anyone's preference, which is what keeps it honest, but step 010
   replaces the sizes with a real checkpoint's and that number is the one
   to trust.
3. **Costs are modelled.** The tiers are NVMe-shaped figures, not
   measurements of a device.
4. **Nothing has run in the kernel.** The arena is still a bump allocator
   and cannot evict at all; step 008 is what makes any of this executable
   by MLOS rather than about it.
5. **This is not evidence that MLOS beats Linux.** It is evidence that a
   policy which knows when objects are next wanted beats policies that do
   not, under identical budgets, on a workload shaped like inference.
   Whether that justifies an operating system is a larger question, and
   the remaining milestones are what answer it.

## 6. What happens next

Outcome 1 of the three the plan named: it separates clearly, so the saga
continues. The numbers above are the baseline the later ones are compared
against -- step 008's in-kernel run must reproduce them exactly, not
approximately, or the two are not measuring the same thing.
