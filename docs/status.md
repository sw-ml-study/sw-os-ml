# MLOS -- Status

**Ground truth.** If it is not in this file, it does not work.
Updated in the same commit as the work it describes.

Last updated: 2026-10-05, during saga `mlos-degradation` (M5), after step 003.

---

## Where we are

**M1 to M4 complete, and the kernel boots on two architectures. Five of
eight gates met.** Gate G4 is [g4-report.md](g4-report.md): known-next-use
does 42--77% fewer provider reads than the better baseline on a real 1B
model's shape in the band where any policy can differ. Gate G5 is
[g5-report.md](g5-report.md): four sessions needing the same layer cause
one read, not four -- parameter-major scheduling cuts next-use reads
sixteenfold on the real shape -- and the same scheduler runs in the kernel
on both architectures, matching the simulator to the integer.

The kernel boots to a shell as a native aarch64 guest and as an x86-64
guest, holds an object table, services a model fault from a real block
device, and chooses what to evict with the same policy crates the
simulator runs, producing the same counts to the integer. The comparison
has been run on a trace shaped by a real checkpoint, a 1.08-billion-
parameter Llama, and it separates where the budget is near the model's
per-token working set and collapses where it is not. That is where the
thesis in [PRD.md](PRD.md) was tested, and it held -- with the caveat,
stated in the report rather than buried, that the traces are derived from
a real checkpoint's inventory and the forward pass, not recorded from an
inference engine.

## PoC gates

From [PRD.md](PRD.md#51-the-proof-of-concept-gate-the-thing-we-are-building-toward).

| Gate | State | Milestone |
| --- | --- | --- |
| G1 -- it boots, reaches a shell | **done** | M1 |
| G2 -- it holds an object table | **done** | M2 |
| G3 -- it faults | **done** | M2 |
| G4 -- known-next-use beats LRU | **done** -- [g4-report.md](g4-report.md). 42--77% fewer reads in the band on a real model's shape, 37--71% on the synthetic one; kernel = simulator to the integer on both architectures. Traces derived, not recorded, and the report says so | M3 |
| G5 -- one read serves N sessions | **done** -- [g5-report.md](g5-report.md). Parameter-major scheduling: weight reads once per token for all sessions; next-use reads 44,309 -> 2,787 with four sessions on a real 1B model; the same `mlos-sched` crate in the kernel on both architectures matches the simulator to the integer | M4 |
| G6 -- degrades instead of dying | in progress -- the ladder walks under a shrinking budget in the kernel on both architectures (step 003); the report is step 007 | M5 |
| G7 -- controls a real host resource | not started | M6 |
| G8 -- another machine is a provider | not started | M7 |

## Milestones

| Milestone | State |
| --- | --- |
| M0 foundations | **complete** -- saga `ml-os-foundations`, 7 steps |
| M1 it boots | **17 of 18 steps, 1 parked** -- saga `mlos-boot`. Gate G1 met. Virtio console and CI done; `efi-stub` parked |
| M2 it holds objects | **complete** -- saga `mlos-objects`, 11 steps. Gates G2 and G3 met |
| M3 it knows better | **complete** -- saga `mlos-nextuse`, 11 steps. Gate G4 met: [g4-report.md](g4-report.md). Known-next-use separates in the band where the budget is near the per-token working set (42--77% on a real 1B model, 37--71% synthetic) and nowhere else; the band's location is a property of the model and the serving regime, which is the milestone's second finding |
| M4 it shares | **complete** -- saga `mlos-parameter-major`, 6 steps. Gate G5 met: [g5-report.md](g5-report.md). Sessions as kernel objects, `share_count` live, a `no_std` scheduler shared by kernel and simulator, the latency escape, `Ps`/`Ks`/`Ss`, and the kernel scheduling the replay on both architectures |
| M5 it degrades | **in progress** -- saga `mlos-degradation`, steps 001 (contracts), 002 (admission control) and 003 (the ladder) done of seven |
| M6 it crosses PCIe | not started |

Read that honestly: M1 is the part any small operating system has to do,
and M2 so far is mechanism -- a table, a fault, three tiers, a shell that
can poke at them. Nothing in `docs/PRD.md`'s thesis is tested until M3,
because the thesis is a claim about *decisions* and no decision has been
made yet. Three gates of eight are met.

## What works today

Booted as a native aarch64 guest on Apple Silicon -- nothing emulates
x86-64 anywhere in this repo.

| | |
| --- | --- |
| Boot | arm64 `Image` header, so the loader applies the Linux boot protocol and hands us the device tree in `x0` |
| Hosts | QEMU/HVF (native), QEMU/TCG (deterministic), Virtualization.framework via vfkit (starts, no output yet) |
| Discovery | Memory map, CPU count, PL011 base and IRQ, GICv3 distributor and redistributor -- all read from the device tree, none hardcoded |
| Memory | Identity map, 1 GiB blocks, `SCTLR_EL1.M` read back to prove it. Kernel image and blob carved out: 510 MiB usable of 512 |
| Faults | Vector table installed; a fault reports `ESR`/`ELR`/`FAR`/`SPSR` and which of the sixteen vectors fired |
| Interrupts | GICv3 + generic timer at 2 Hz, tracking wall clock; PL011 receive on a shared interrupt |
| Shell | `mlsh` with `help`, `mem`, `dev`, `ticks`, line editing |
| Consoles | PL011, or a virtio console over virtio-mmio, chosen from `/chosen/bootargs` |
| Objects | `ObjectId` (class/model/layer/tensor/tile), an open-addressed table, `ObjectMeta` carrying tier, cost, reuse and next-use |
| Faults | `ml_acquire` -> miss -> `MODEL_FAULT` -> provider read -> arena placement -> resident. Counted per class |
| Tiers | Three, with genuinely different costs: a virtio-blk disk, a recompute tier, and DRAM |
| Model | A synthetic 8x16 transformer, 136 objects, 144 KiB, registered and sweepable from the shell |
| Shell | `mlsh`: `help`, `mem`, `dev`, `ticks`, `model [KIB]`, `objs`, `get L T`, `sweep`, `faults`, `arena`, `layout`, `trace`, `stream`, `replay POLICY` |
| Streams | `ml_stream_declare` / `ml_stream_advance` as `mlos-stream`. A declared cyclic order, and a cursor. **The kernel now writes `ObjectMeta::next_use`** -- the field that existed from M2 step 001 with nothing to set it |
| Eviction | `mlos-arena` is a real allocator: first fit over a sorted free list, coalescing on release. `Manager::evict` gives bytes back, drops residency, and emits `Kind::Evicted`. `evict L T` from the shell. A full arena now asks the policy for victims (`Manager::make_room`) until the largest free run fits, rather than refusing |
| Policy in kernel | `Manager::policy`: demand (refuse), FIFO, LRU or known-next-use -- the same `mlos-policy` crate `mlos-sim` links, chosen with `replay POLICY`. Verified against the simulator on the same trace and budget: identical integers for all four |
| Replay | The M3 workload rides on the model disk after the weights (`mlos_synth::disk::TRACE_AT`: an eight-byte length, then trace text). The guest parses it into statics, declares it as a stream, replays it, and prints what the manager actually did. `mlos run tcg --capture 120 --run 'model 32;replay lru'` drives it headless |
| Sessions | `mlos-session`: a session is a record with an id, a contract and a resident-byte account, held by the manager (sixteen at most; a seventeenth is refused). `resident_ceiling` is the one contract field enforced: an acquire that would pass it is refused before any victim is chosen. Ending a session evicts what it owned and forgets it. `session`, `session new [KIB]`, `session end ID` in the shell; both replays adopt the trace's sessions first and destroy them last. Sessions without processes: nothing but the shell and the replay drives them yet |
| Contracts | `Contract { quality_floor: Precision, latency_ceiling, resident_ceiling }`, and per session a `Delivered { coarsest, worst_period, peak_resident }` that only gets worse: the coarsest precision served and the peak are recorded as bytes are charged, the worst token period by both schedule drivers identically. `session` lists promised against delivered; `session new [KIB] [WAIT] [FLOOR]` sets the three promises. Since step 003 `coarsest` moves when the ladder requantizes a block, and `Delivered` also records the deepest rung and the bytes read to degrade |
| Admission | `mlos-admit`: `ml_session_create` refusing is a normal outcome with a reason. A `Capacity { bytes, reserved, kv_per_token, token }` admits a contract when the KV its declared `context` will hold fits what the budget has not promised to the live sessions (after the weights' per-token working set is reserved), when its own resident ceiling can hold that context, and when its latency ceiling is at least a quarter token (the G5 break-even) and every contracted session's ceiling still covers the others running ahead of it. `Ss` is now the sessions the budget admits at the live mix, the live count when nothing declared a context. `session new [KIB] [WAIT] [FLOOR] [CTX]` prints `refused: <why>`; the kernel's answers equal the host's on both architectures (`tests/admission.rs`) |
| Ladder | `mlos-ladder`: the eight rungs of architecture s.5 as one function, `Shape::fate`, saying what every KV block is on every rung -- cold KV to Q8 (L1) and Q4 (L2), the oldest cold span summarised into one Q4 block (L3), half the remaining cold span dropped as candidates (L4), the context truncated to its hot window (L5) -- with `need` and `cost` (bytes read, `Rc`) summed from it. `squeeze` sets a budget and walks: every session deepened one rung at a time together, never past its quality floor (Q8 stops at L1, Fp16 never moves, context rungs need "anything goes"); when none can move, L6 (admission has no room) and L7 (terminate the highest id). Resident blocks shrink in place -- the arena takes the tail back -- and dropped ones leave the table. Summarising and RAG are declared and accounted, not performed. A session with a context now arrives with it cached (`arrive`). `session squeeze KIB` in the shell; `session` shows each session's rung and bytes read. The kernel's lines equal the host's on aarch64 and x86-64 (`tests/ladder.rs`) |
| Leases | `share_count` is the number of leases holding an object: Pin, Borrow and Streaming count, Speculative does not. `acquire` raises it, `release` (`ml_release`) lowers it, `evict` zeroes it. Known-next-use divides recovery cost by it, so an object two sessions hold outlives one held by one (tested). Replays and sweeps `consume`: acquire and release in one, so every M3 count is unchanged. `release L T` in the shell |
| Scheduler | `mlos-sched`, `no_std`: `ProcessMajor` serves one session's whole token then the next (the G4 order, held to its integers by a test); `ParameterMajor` keeps every session on the lowest token and the furthest behind within it, so weights are read once per token and each session's KV phase runs privately. `merge` (host, behind `alloc`) drives either over per-session lanes into a trace; `Decode::tokens` and `Real::tokens` are those lanes and `trace()` is now the process-major merge. A session whose wait (other sessions' acquires since its token became due) reaches its `latency_ceiling` is served out of turn and reads alone: the escape hatch. **In the kernel too**: `replay POLICY [process\|parameter]` rebuilds per-session lanes from the disk trace in static buffers, runs the same scheduler over them and replays the order it chose; eight count lines match the simulator's on aarch64 and x86-64 |
| Metrics | `mlos-metrics` counts faults, fetched bytes, hit bytes and resident bytes per class, and computes the PRD s.5.2 headline numbers in one place: `Ps` (weight bytes applied per thousand read), `Ks` (KV bytes per live session), `Ss` (sessions per GiB of budget, in thousandths; measured at a fixed budget until M5's admission control). The kernel's replay line and the simulator's carry all three, computed by the same function, and the boot test compares them as strings. `metrics` in the shell prints `Rm`, `Ps`, `Ks`, `Ss` |
| Layout | `mlos layout` writes `build/storage-layout.json`: three spaces (disk, arena, guest RAM), 140 regions, in sw-mlpl's columnar `system-layout` contract |
| Snapshot | `mlos runtime` boots, sweeps and writes `build/runtime-layout.json` from the live object table -- residency, reuse, cost and `backs` edges from stored tile to arena placement |
| Events | The same boot writes `build/runtime-events.jsonl`: one JSON line per residency transition (`placed` / `hit` / `refused`), joined to the snapshot by region id. `trace` prints them; `trace on\|off` switches recording |
| Traces | And `build/runtime.trace`: the access sequence those events record -- session and `ObjectId` per acquire, and nothing about what the system did. What M3 replays policies against |
| Simulator | `mlos-sim` replays a trace against a policy under a fixed residency budget and counts hits, provider reads, bytes, evictions and refusals. `compare` takes one budget for every policy, so an unequal comparison cannot be expressed. The budget is a real `mlos-arena` -- the kernel's allocator -- so what fragmentation costs a policy is counted |
| Policy interface | `mlos-policy`: `no_std` and pure, so the same code runs in the kernel and the simulator. A policy reads `ObjectMeta` and names a victim; it holds no state the table does not own |
| Boot script | `/chosen/bootargs` carries `mlsh.run=model;sweep;layout`, so a headless capture can drive the shell. A log file is not a terminal, so nothing else could |
| Tooling | `mlos build` / `run [hvf\|tcg\|vz]` / `run --capture N` / `run --debug` / `doctor` / `layout` / `runtime` |
| Timing | `sweep` reports elapsed nanoseconds from the ARM generic timer, not the 2 Hz tick -- which is what makes any claim about what the fault path costs measurable. The rate is read from `CNTFRQ_EL0` rather than assumed: 24 MHz under HVF, which is Apple Silicon's own counter passed through, and 62.5 MHz under TCG, which is QEMU's |
| Tests | 40 fast test binaries plus nine TCG boot tests (`cargo test -p mlos-cli -- --ignored`), one of which boots the kernel, replays four policies, and asserts each count line equals the simulator's. Local only, by choice -- see [AGENTS.md](../AGENTS.md); there is no CI and the local gate is the stricter of the two |

## What does not exist yet

No userspace, no scheduler of threads (one kernel thread; the scheduler
that exists decides whose acquire is served, not who runs), no GPU and no
ML-MMU. The degradation ladder exists but performs no arithmetic: a rung
changes sizes and precisions in the table and says what the transform
would have read. Leases are counted but not enforced:
`share_count` is the live holds on an object and next-use weighs it, but
nothing yet refuses to evict a pinned object or revokes a borrow between
operations. Sessions exist as kernel records but nothing creates one
except the shell and the replay: there is no process to own a session,
and `get` acts as session 1. The real shape's traces run in the
simulator only; the kernel schedules and replays the synthetic
4,448-access trace, where it proves the simulator faithful.

The trace from a real model's shape (step 010) is a model OF a decode
loop over real tensors, not a recording of one: the tensor inventory and
the rotating boundary come from the checkpoint through emufpga's
importer, and the loop that reads them follows from the architecture.
No trace here was recorded from running inference, because nothing here
runs inference. What the kernel replays in-kernel is still the synthetic
8x16 model: the guest parses a trace into static arrays and a real
shape's trace is half a million accesses.

The in-kernel comparison runs on a 2x16 replay trace (4,448 accesses),
not the verdict's 4x40. The guest parses the whole trace into static
arrays, and 25,200 accesses would be 400 KiB of `.bss` plus half a
megabyte read off a virtual disk under TCG. Exactness is what step 009
needed, and a shorter trace shows it as well as a longer one; the report
(step 011) decides what the kernel is asked to run.

The runtime document has no `sysram` space. A running kernel has no symbol
table and cannot say where its own `.text` ended, so guest RAM appears only
in the static document, drawn from the linked image. The two share `disk`
and `dram`, with the same region ids, which is what lets a consumer join
them.

`region_next_use` is `never` until a stream is declared. The kernel writes
it from M3 step 007 onwards -- `stream` in `mlsh` declares the model's
sweep and `stream N` advances it, and `replay` declares the whole trace
it is about to run -- so a layout document taken after either carries
real positions. Nothing declares one at boot, because nothing but those
two is a workload yet.

`NextUse::Probability` is still produced by nothing. A declared stream
says exactly WHEN; only a router says how LIKELY, and nothing routes until
M5. The distinction is preserved rather than collapsed.

Events carry no wall clock. The only clock the shell had when they were
designed is the 2 Hz tick, which cannot resolve a fault, and elapsed time
under TCG is not the timing of any real machine. Ordering comes from `seq`,
which is exact; duration comes from `cost`, the modelled figure a provider
charges -- the same axis M3's comparison is measured on. A real timestamp
per event is now possible (the generic timer is wired up for `sweep`) and
has not been done.

## Environment as verified on this machine

Checked 2026-09-06 on the primary development Mac:

| Thing | State |
| --- | --- |
| Host | macOS 26.5 (25F71), arm64 |
| Rust | 1.96.0 -- 2024 edition available (needs >= 1.85) |
| `agentrail` | installed |
| `sw-checklist` | installed |
| **QEMU** | **not installed** -- M1 step 1 blocker |
| **EDK2 / AAVMF firmware** | **not present** |
| **libkrun / krunkit** | not installed -- **and not needed**; QEMU 9.2+ covers it |
| Bare targets | `aarch64-unknown-none-softfloat`, `x86_64-unknown-none` not yet added via rustup |
| Linux/NVIDIA host | not yet provisioned -- not needed before M6 |

The first three rows are what `mlos doctor` will check once it exists.
QEMU is the immediate prerequisite for M1.

## What MLOS is for, restated 2026-09-17

Not a self-sufficient operating system, and never going to be. MLOS is a
**distributed guest control plane** for ML state: it runs in a VM, the
host OS keeps owning the NVIDIA driver, CUDA, Metal, filesystems, NVMe
and the network stack, and MLOS owns what none of them can express --
which object should be where, when, at what precision and at what cost,
across GPU VRAM, CPU cores, RAM, SSDs, disks and the network.

That reframing recut M6 onward (see [plan.md](plan.md)): host providers
over narrow virtio interfaces rather than PCI passthrough and a GPU
driver, then distribution, heterogeneity, global scheduling, and the
ML-MMU last. M1--M5 were already right and did not move.

It makes M3 more important rather than less. `next_use` is not a better
cache eviction heuristic; it is the first small test of whether semantic
knowledge about inference is worth moving resource decisions out of the
host OS at all. If it is not, a distributed control plane has no reason
to exist.

## Is this picture of a real system?

A fair question to ask of any rendering drawn from
[layout-handoff.md](layout-handoff.md)'s data, and the answer differs by
file.

- `storage-layout.json` describes a **build**. Every object in it reads
  `"state": "never"`, because nothing has run.
- `runtime-layout.json` and `runtime-events.jsonl` describe a **running
  system** -- a real kernel, a real virtio-blk device, real residency --
  **managing a synthetic model**. The eight layers of sixteen tiles have
  no arithmetic in them, and the tier costs are NVMe-shaped figures rather
  than measurements. The access pattern and the residency pressure are the
  subject; a demonstration of an ML operating system needs no neural
  network in it.

## A workload that can come out either way

`mlos-workload` generates a decode loop: several sessions sharing one
model, round-robin, each sweeping every weight tile per token and
re-reading its own KV prefix. Four sessions over forty rounds is 25,200
accesses, 12,800 of them weights and 12,400 KV -- neither half dominating.

Two things were measured rather than assumed, and the first guess was
wrong both times.

**Re-reading a KV prefix is itself a cyclic sweep.** The plan expected
KV to be where recency pays: recently-written blocks are about to be
wanted again. It is not. Reading blocks `0..t` every round touches every
block once per round in order, so after one pass LRU's recency order IS
insertion order and it evicts exactly what FIFO evicts. They tied at 192
reads each. **A purely cyclic workload can never distinguish FIFO from
LRU**, which is a property worth knowing rather than an accident.

**What makes recency informative is sessions finishing.** A session that
has stopped holds KV nobody will read again; an active one holds early
blocks it reads every round. Measured at a 192 KiB budget:

| sessions | FIFO reads | LRU reads |
| --- | --- | --- |
| 1 | 1,100 | 3,696 |
| 4 (finishing at different times) | 12,067 | **11,942** |

One session is a pure cycle and LRU is structurally pessimal on it. Add
sessions that finish at different times and recency starts to mean
something. That is the knob this workload has, and the answer moves with
it -- which step 006 must report rather than pick a column from.

**Policy matters in a band -- and that was only true of the baselines.**
Step 004 concluded that below about 128 KiB the cyclic weight sweep
misses everything whatever is evicted. That holds for FIFO and LRU and
not for known-next-use, which is the correction step 005 made: where both
baselines score 25,200 reads out of 25,200 accesses, next-use serves half
of them. Above about 384 KiB the working set fits and every policy is
identical, which is still true of all of them. `cargo test -p
mlos-workload --test sweep -- --ignored --nocapture` prints the shape.

**Demand paging's read count is not comparable.** At 192 KiB it does 384
reads against LRU's 11,942 -- and refuses 6,896 of 25,200 accesses to get
there, where LRU refuses none. Reporting those two numbers side by side
without the refusals would be the most misleading row in any table.

## What fragmentation costs

Measured rather than assumed, in `mlos-arena/tests/fragmentation.rs`.

**Uniform objects do not fragment.** The workload MLOS runs today is
1 KiB tiles into a 32 KiB arena: evict every other one and the arena is
half free in 1 KiB holes, every one of which fits another tile. Evict the
rest and it is a single run again.

**Ragged objects do, and the arena says by how much.** A mix of 256 B to
1792 B objects, half of them released: 49,664 B free and the largest
single run 34,304 B, so **69% of the free space is reachable in one
piece**. `arena` in the shell reports the largest run beside the total,
because a policy evicting perfectly into memory it cannot hand out has
not helped -- and KV blocks growing with context are the ragged case.

A release the fixed free list cannot record is REFUSED and nothing
changes: the object stays resident rather than becoming bytes the
accounting has lost.

## The verdict is in

[m3-verdict.md](m3-verdict.md) is the document; this is the sentence.

**Known-next-use beats every baseline at every budget where any policy
can differ, at every session count tested** -- twenty-eight
configurations, 32% to 71% fewer provider reads, never a loss. Bytes moved
fall further than reads do, because it keeps the expensive objects and
evicts the cheap ones.

What that does NOT settle is in
[s.5 of the verdict](m3-verdict.md#5-what-this-is-not), and the first
item is the one that matters: **the simulator supplies the foresight.**
It holds the whole trace and looks the answer up. The claim is that a
transformer hands an operating system that knowledge through a declared
stream, and nothing has built or measured that. Step 007 does, and if
streams cannot deliver next-use cheaply in a kernel then this result does
not transfer.

PRD s.9 Q1 and Q2 are answered by name in the verdict, with numbers.
Gate G4 is **not** met by this: G4 asks for the comparison from a running
kernel, which needs an evictable arena (step 008) and the in-kernel port
(step 009).

## Known-next-use, measured

Four sessions, forty rounds, 25,200 accesses. Provider reads, lower being
better. Demand is shown as reads+refusals because its read count is
bought by not serving the workload.

| budget | demand | FIFO | LRU | next-use | vs best baseline |
| --- | --- | --- | --- | --- | --- |
| 32 KiB | 35+22060 | 25200 | 25200 | 22127 | +13% |
| 64 KiB | 67+18860 | 25200 | 25200 | 18959 | +25% |
| 96 KiB | 102+15720 | 25200 | 25200 | 15791 | +38% |
| 128 KiB | 134+12520 | 25200 | 25200 | **12599** | **+51%** |
| 160 KiB | 256+9392 | 19526 | 17692 | **7729** | **+57%** |
| 192 KiB | 384+6896 | 12067 | 11942 | **3480** | **+71%** |
| 256 KiB | 640+2672 | 1448 | 928 | 928 | +0% |
| 384 KiB | 928+0 | 928 | 928 | 928 | +0% |

Total recovery cost at 128 KiB: LRU 56,160 ms against next-use 5,856 ms,
about a tenfold reduction.

**This is not the verdict.** Step 006 is, and it has to weigh three
things this table does not show: the result moves with session count
(step 004), the simulator SUPPLIES the foresight that step 007's syscalls
would have to deliver in a kernel, and the workload is a model of a
decode loop rather than a recording of one.

Two bugs were found by the rule that a policy with strictly more
information cannot lose. Both are in
[the commit](https://github.com/sw-ml-study/sw-os-ml/commits/main) and
both were invisible to every other test.

## The first policy table, and why it proves nothing

Three baselines replayed against `examples/viz/runtime.trace` -- a real
recording of two sweeps on a real kernel -- under a 32 KiB budget against
a 144 KiB model.

| policy | provider reads | bytes | evicted | refused | hits per 1000 |
| --- | --- | --- | --- | --- | --- |
| demand | **32** | 32 KiB | 0 | 2 | **484** |
| fifo | 66 | 66 KiB | 34 | 0 | 0 |
| lru | 66 | 66 KiB | 34 | 0 | 0 |

**Doing nothing wins**, and FIFO and LRU are indistinguishable. Both facts
are properties of the workload rather than of the policies. A dense sweep
re-reads nothing within a pass, so the object either baseline has just
touched is the one it will want last: they evict precisely what they are
about to need and miss every single access. Demand refuses twice, keeps
the 32 tiles it already had, and the second sweep hits them.

So this table is not evidence about residency policy. It is evidence that
the trace is the degenerate case, which is why step 004 builds a workload
with real reuse in it before step 006 compares anything that matters.

One thing in it is worth keeping, because nobody designed the measurement
to show it: refusing beat replacing. That is `docs/PRD.md` F4's argument
for admission control turning up uninvited.

## What tracing costs

Measured, not asserted. Two identical sweeps, one with `trace off` and one
with `trace on`, repeated six times each in both orders with a warm-up
first, on sweeps that only HIT -- no virtio round trips, so what is left is
the table lookup and the ring store.

| | per sweep (33 events) | per event | a fault, for scale |
| --- | --- | --- | --- |
| QEMU/HVF, native | +0.14 us | **~4 ns** | 42 us |
| QEMU/TCG | +4.3 us | 129 ns | 91 us |

The HVF figure is near its own measurement floor and is quoted loosely
for that reason: Apple's counter runs at 24 MHz, so one tick is 41.7 ns
and the +0.14 us delta is about three ticks. Twelve runs either side of
the switch is what makes it a number rather than a rounding error.

Recording costs about one ten-thousandth of a fault on native hardware, so
it is on by default. The first attempt to measure it used sweeps that
faulted, and found nothing: 32 virtio transactions per sweep swamped the
signal and the run-to-run spread was larger than the effect. The number
above is from the sweeps where residency is already established.

## The kernel and the simulator agree

Step 009. Four policies, one trace (2 sessions x 16 rounds, 4,448
accesses), one budget (32 KiB), replayed in `mlos-sim` on the host and in
`mlos-kernel` under QEMU/TCG. The lines the guest prints and the lines the
simulator computes are compared as strings by
`crates/mlos-cli/tests/replay.rs`, and they are equal. The x86-64 guest
(saga `mlos-x86-64`) prints the same lines: `tests/replay_x86.rs` boots
both guests side by side and asserts x86-64 = aarch64 = simulator.

| policy | reads | hits | bytes | evicted | refused | simulator | aarch64 guest | x86-64 guest |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| demand | 35 | 741 | 32,768 | 0 | 3,672 | = | = | = |
| fifo | 4,448 | 0 | 3,497,984 | 4,392 | 0 | = | = | = |
| lru | 4,448 | 0 | 3,497,984 | 4,392 | 0 | = | = | = |
| next-use | 3,748 | 700 | 2,781,184 | 3,713 | 0 | = | = | = |

Four disagreements were found on the way. Each was a bug, each side was
self-consistent, and only the comparison could have found any of them:

- **Residency was read from the tier.** `acquire` called an object
  resident when its tier was `Warm`. Weight tiles start `Cold` and
  agreed; KV blocks start `Warm` -- they come from compute, not storage
  -- and a never-fetched one read as a hit at address zero. Residency is
  now `resident_at != 0`.
- **Streams were cyclic.** One declared period, cursor running past the
  end, on the reasoning that every token reads the same objects in the
  same order. True of weights, false of a KV cache, which accumulates.
  Every KV block read as `Never`, the most evictable thing in the table,
  and next-use threw away exactly what it was about to want: 27 reads. A
  declaration is now a finite sequence, borrowed from the declarer, and
  both sides build its next-use chain with the one function
  `mlos_stream::chain`.
- **The simulator's budget was a number.** It evicted until a byte count
  fitted; the kernel evicts until a contiguous run fits in a first-fit
  arena. FIFO and LRU evict in roughly placement order, so their holes
  coalesce and the two models agree; next-use evicts whatever is
  furthest away, wherever it sits, and paid for fragmentation the
  simulator could not see: 8 reads. `mlos-sim` now links `mlos-arena`
  and places into a real one, and both sides ask `Occupancy::fits`.
- **Ties were broken by enumeration order.** The kernel offers residents
  in hash-slot order, the simulator in insertion order, and two `Never`
  blocks of the same session score equally. Which one goes changes the
  hole geometry: 5 reads. Both policies now break ties on object id, so
  the answer no longer depends on who is asking.

Under the arena-aware simulator the verdict's numbers move by at most a
percentage point -- next-use at 128 KiB with four sessions is 12,722
reads rather than 12,599, +50% over LRU rather than +51% -- and the
baselines do not move at all, because their evictions never fragmented.
The separation stands. [m3-verdict.md](m3-verdict.md) keeps step 006's
table as the record of what was measured then; step 011's report is the
one to quote.

**How the trace reaches the guest.** On the model disk, after the
weights. The model occupies a fixed extent, so the sector after it is
spare, and one device is one probe: a second virtio-blk would have meant
teaching `probe` to tell two block devices apart, in service of a layout
decision that is free. `/chosen/bootargs` carries the script in
(`mlsh.run=model 32;replay lru`) and the console carries the counts out.

## A real model's shape

Step 010. The tensor inventory of a real checkpoint --
`ewinregirgojr/MiniCPM5-1B-Agentic-Tooluse-Merged-FP16`, a 24-layer
Llama, 1,080,632,832 parameters, 219 tensors, stored F16 -- read from
its safetensors headers by emufpga's importer and written as a sidecar
(`emufpga import --sidecar-only`): 169 rotating streams swept once per
token (seven matrices per layer and the untied 200M-weight output head),
50 resident (the embedding and the norms). The order file that produced
it is the Llama forward pass, and emufpga's `spm-order` refuses it if it
disagrees with the checkpoint. Copies are in
`crates/mlos-workload/shapes/`; the source of truth is emufpga's
`layouts/` (external ask A1, delivered). No weight was read by anything.

`mlos-workload::Real` runs the same decode loop over that shape as
`Decode` runs over the synthetic model: per token, the rotating streams
in declared order, the session's KV blocks for the layer between the
value projection and the output projection, the resident streams read
once at the start. Weights are 2,061 MiB, of which 1,678 MiB are swept
per token; one token's KV is 24 KiB across all layers, read off the key
and value projections' widths.

**At short contexts this shape is a weight sweep, and the comparison
collapses.** Four sessions, forty tokens, no prompt: 54,150 accesses,
3.8 MB of cache. FIFO and LRU tie exactly at every budget -- a pure
cycle cannot distinguish them -- and next-use wins by the fraction of
the sweep the budget holds:

| budget | demand | FIFO | LRU | next-use | vs best baseline |
| --- | --- | --- | --- | --- | --- |
| 512 MiB | 436+43,679 | 54,150 | 54,150 | 53,259 | +2% |
| 1024 MiB | 244+43,083 | 54,150 | 54,150 | 49,555 | +9% |
| 1536 MiB | 2,605+1,400 | 54,150 | 54,150 | 44,309 | +19% |
| 1792 MiB | 2,618+100 | 2,619 | 2,619 | 2,619 | +0% |

That is the degenerate case the M3 plan said not to construct, arriving
from a real model. It is a property of the model, not of the trace:
MiniCPM's two KV heads make its cache tiny beside its weights, and the
cache only rivals the sweep at tens of thousands of resident tokens.

**With context, the band appears.** Eight sessions arriving with 2,048
cached tokens each, forty decode steps, the cache paged sixteen tokens
to a block: 589,694 accesses, 389 MiB of cache at peak. Below 1.7 GiB
the per-token working set still exceeds the budget and every baseline
access misses; between 1.7 and 2.1 GiB LRU beats FIFO -- recency has
something to be right about, which is the fairness the plan asked for
-- and next-use beats LRU:

| budget | demand | FIFO | LRU | next-use | vs best baseline |
| --- | --- | --- | --- | --- | --- |
| 1536 MiB | 3,067+548,834 | 589,694 | 589,694 | 571,925 | +4% |
| 1792 MiB | 7,472+497,034 | 521,669 | 512,427 | **301,800** | **+42%** |
| 2048 MiB | 23,856+47,324 | 124,969 | 124,227 | **29,639** | **+77%** |
| 2560 MiB | 25,155+0 | 25,155 | 25,155 | 25,155 | +0% |

The same shape of result as the synthetic verdict, at the same place:
where the budget is a large fraction of the working set. Outside that
band no policy matters, and saying so is part of the measurement.

**Trace size, and whether anyone will wait.** The decode-only trace
replays in 2 to 4 seconds per budget for all four policies. The
long-context trace takes 90 to 380 seconds per budget -- ten minutes for
the table -- because a victim scan is linear in the resident set and the
resident set is twenty-five thousand KV blocks. Finding an object on a
hit was linear too until this step; `mlos-sim` now indexes its resident
set, which is bookkeeping the kernel's hashed table never paid for and
the simulator had no business charging. The scan is the policy's cost
and stays. `cargo test -p mlos-workload --test real -- --ignored
--nocapture` prints both tables with timings.

Not done here: replaying this trace in the kernel. The guest holds a
trace in static arrays sized for the synthetic workload, and a real
shape's weights are two gigabytes against a 32 KiB arena; the in-kernel
replay stays on the synthetic model, where it proves the simulator
faithful, and the simulator carries the real shape.

## Parameter-major, in the simulator

M4 step 003. Provider reads, process-major against parameter-major, same
trace of per-session streams, same budget, same policy. `cargo test -p
mlos-workload --test g5 -- --ignored --nocapture --test-threads=1` prints
the full tables.

**On the real shape the inversion is the whole story.** MiniCPM5-1B at
F16, forty rounds, no prompt: the per-token weight sweep (1,678 MiB)
exceeds every budget below the working set, so process-major re-reads it
once per session per token and parameter-major reads it once per token
for everyone.

| sessions, budget | KV share | LRU process | LRU parameter | gain | next-use process | next-use parameter | gain |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 2, 1536 MiB | 70% | 34,910 | 31,530 | +10% | 29,004 | **1,726** | **+95%** |
| 4, 1536 MiB | 68% | 54,150 | 44,010 | +19% | 44,309 | **2,787** | **+94%** |
| 8, 1536 MiB | 67% | 93,830 | 70,170 | +26% | 76,207 | **4,875** | **+94%** |
| 8, 1024 MiB | 67% | 93,830 | 70,213 | +26% | 85,381 | 32,448 | +62% |

Four sessions needing the same layer cause one read, not four: under LRU
the weight reads fall from 16,900 to 6,760 (once per token-round) and
nothing else moves, because every KV access still misses a pure cycle;
under next-use the weights are shared and the cache is kept, and reads
fall sixteenfold. With 2,048-token prompts and 16-token KV blocks (94%
of accesses are KV) at 1,792 MiB: next-use 301,800 to 25,680 with eight
sessions (+92%), 79,254 to 13,438 with four (+84%); LRU 1 to 3% worse,
because lockstep interleaves the sessions' cache phases and spoils
recency. Those two rows took 166 s and 507 s to replay.

**On the synthetic model there is little to share.** Its 128 KiB of
weights fit in every budget from 128 KiB up, so next-use already served
them once for everyone and parameter-major changes its reads by at most
18% at 96 KiB and nothing above 160 KiB. LRU gains 17 to 41% at tight
budgets and loses 12 to 27% at loose ones (two sessions at 192 KiB: 4,960
to 5,580; four at 192 KiB: 11,942 to 15,246), for the same reason as
above: lockstep is good for weights and bad for recency over KV. A
negative row is a finding, not an omission.

**The bound.** Parameter-major shares weights and cannot share a
session's cache, so its best case removes `(N - 1) / N` of the weight
reads and none of the KV reads. The KV share column is why the gain
under LRU tops out where it does, and why the long-context rows are
where the inversion matters least for a recency policy and most for a
policy that knows the future.

**What this is not yet.** The simulator's scheduler over host-side
lanes. The kernel replays one merged trace and does not schedule; that is
step 006, where the same crate runs over the session table and the counts
must match these to the integer.

## The latency escape

M4 step 004. Period is what a session's user waits for each token:
acquires from the first access of one token to the first of the next.
Real shape, four sessions, forty rounds, 1536 MiB, next-use; session 1
is the one with a ceiling. `cargo test -p mlos-workload --test latency --
--ignored --nocapture` prints it.

| schedule | session 1 ceiling | reads | session 1 period mean / worst | session 4 period mean / worst |
| --- | --- | --- | --- | --- |
| process-major | none | 44,309 | 1,086 / 1,540 | 1,338 / 1,923 |
| parameter-major | none | 2,787 | 1,208 / 1,633 | 1,353 / 1,946 |
| parameter-major | 1024 | 2,787 | 1,169 / 1,540 | 1,353 / 1,946 |
| parameter-major | 512 | 3,206 | 817 / 964 | 1,353 / 1,946 |
| parameter-major | 256 | 2,922 | 561 / 753 | 1,353 / 1,950 |
| parameter-major | 128 | 22,709 | 433 / 513 | 1,353 / 2,336 |
| parameter-major | 32 | 23,137 | 337 / 417 | 1,353 / 3,638 |

Lockstep costs session 1 about 11% in period against process-major, for
sixteen times fewer reads. A ceiling of 256 acquires halves its period
for 5% more reads; 512 for 15%; at 128 and below the session runs a full
token ahead, every weight it reads alone is one the others then miss, and
the group pays eight times the reads. The break-even for this workload is
a ceiling near a quarter of a token. The wait is other sessions' acquires
since the session's token became due, not the gap between two of its own
accesses, because under lockstep that gap is a handful while the token
takes four times longer; the first version of the escape measured the gap
and never fired.



## The ladder, measured

M5 step 003. What each rung buys in resident bytes and costs in `Rc` and
`Ks`, on the real shape, in the band: MiniCPM5-1B at F16, four sessions
with 2,048-token prompts (16-token blocks, the newest 128 tokens hot),
forty rounds, 1,792 MiB, next-use, parameter-major. `cargo test --release
-p mlos-workload --test ladder -- --ignored --nocapture`, three seconds.

| rung | prompt KV per session | reads | bytes read | refused | Rc per token | Ks at end |
| --- | --- | --- | --- | --- | --- | --- |
| L0 | 48.0 MiB | 13,438 | 1,885 MiB | 100 | 0 B | 48.7 MiB |
| L1 | 25.5 MiB | 12,699 | 2,166 MiB | 0 | 1,887,436 B | 26.2 MiB |
| L2 | 14.2 MiB | 12,699 | 2,121 MiB | 0 | 2,831,155 B | 15.0 MiB |
| L3 | 8.7 MiB | 7,035 | 2,099 MiB | 0 | 3,067,084 B | 9.5 MiB |
| L4 | 5.9 MiB | 4,155 | 2,088 MiB | 0 | 3,067,084 B | 6.7 MiB |
| L5 | 3.0 MiB | 1,179 | 2,076 MiB | 0 | 3,067,084 B | 3.8 MiB |

L0 is G5's row to the read. It reads the fewest bytes only because it
refuses 100 accesses the budget cannot place; L1 is the first rung that
serves everything, and bytes rise 281 MiB because those accesses are now
served. Quantizing (L1, L2) buys bytes and not reads -- the same objects,
smaller; summarising, cutting candidates and truncating (L3--L5) cut the
accesses themselves. `Rc` is paid once per transform, here amortised over
100 tokens; L4 and L5 read nothing, since dropping is free. As the room
for KV shrinks the walk stops at L0 down to 192 MiB, L1 at 128, L2 at 96
and 64, L3 at 48, L4 at 32 and 24, L5 at 16, and terminates below that.

In the kernel, on the synthetic model: a Q8 session and an anything-goes
one, squeezed to 30, 25 and 22 KiB, reach L1, then L4 with the Q8
session held at its floor, then L7 with the newer one terminated; every
rung, every byte read and every byte held equals the host's on both
architectures.

## The headline numbers

M4 step 005. `Ps` is the one the project is named for: weight bytes
applied per thousand weight bytes read from a provider, so one thousand is
one use per read and N thousand is N sessions sharing each read. From the
G5 tables under next-use, process-major against parameter-major:

| workload | sessions, budget | Ps process-major | Ps parameter-major |
| --- | --- | --- | --- |
| MiniCPM5-1B, no prompt | 1, 1536 MiB | 7,653 | 7,653 |
| MiniCPM5-1B, no prompt | 2, 1536 MiB | 8,256 | 46,551 |
| MiniCPM5-1B, no prompt | 4, 1536 MiB | 8,801 | 77,433 |
| MiniCPM5-1B, no prompt | 8, 1536 MiB | 9,196 | 139,197 |
| synthetic 8x16 | 4, 128 KiB | 38,787 | 76,646 |
| synthetic 8x16 | 4, 192 KiB | 100,000 | 100,000 |

Next-use alone already gets eight applications per read on the real shape
(the weights that fit stay); parameter-major with four sessions gets
seventy-seven, and the number keeps growing with sessions because every
session after the first is a hit on a read already made. Where the
weights fit entirely (the synthetic model at 192 KiB) `Ps` is the same
under both schedules and equals the number of times each weight is used
over the run, which is the ceiling.

`Ss` is sessions per GiB of resident budget. Through M4 it was measured at
a fixed budget -- live sessions over the arena's capacity -- and four
sessions in 1,536 MiB is 2.667. Since M5 step 002 the kernel reports the
sessions the budget *admits* at the live mix (`mlos-admit`), which is the
live count when, as in every replay, nothing declared a context. `Ks` is KV bytes resident per
live session. All three are computed by `mlos_metrics::Headline::of` from
raw counts, by the kernel from its counters and by the simulator from its
outcome, and the kernel's replay line equals the simulator's as a string.

## Known gaps

Things that are wrong or missing on purpose, recorded so they are not
discovered by something trusting them.

| Gap | Consequence | Closes in |
| --- | --- | --- |
| The identity map is 1 GiB blocks, so all of RAM is executable and writable. | No W^X and no per-page permissions. Nothing runs but the kernel yet. | When the object manager needs finer granularity (M2) |
| `mlos_hal::PageTable` is declared but not implemented. | `Platform`'s associated type has no concrete impl. | Same. A general mapper written before it has a caller is the wrong mapper |
| A device tree with more than 16 memory regions silently keeps the first 16. | Not reachable on QEMU `virt`, which reports one that carving turns into five. | When a machine needs it |
| No console under Virtualization.framework. VZ puts its virtio devices on the **PCI** bus, not MMIO -- a Linux guest there needs `CONFIG_VIRTIO_PCI`. MLOS speaks virtio-mmio, which VZ does not offer. | Requirement N2 is not met. `mlos run vz` starts the VM; nothing comes out. Closing it needs PCI ECAM enumeration and virtio-pci -- which is `mlos-pci`, already required by M6 for GPU passthrough. | M6, when PCI lands. Deliberately not sooner: writing PCI twice to save a milestone is the wrong trade |
| The virtio console is transmit-only. | Nothing can be typed at MLOS over virtio; the PL011 is the only input. Not reachable under QEMU, which has both. | When something needs it |
| `017-efi-stub` is **parked**, not scheduled. | No UEFI boot and no `VZEFIBootLoader`. Neither is needed while `VZLinuxBootLoader` takes the raw image, and VZ needs a PCI console before a boot path matters. | Unpark when something actually needs firmware services |
| The kernel image is not a PE/COFF EFI application, so real UEFI firmware will not load it (`Image type X64 can't be loaded`). | No UEFI boot, and no `VZEFIBootLoader`. Neither is needed while `VZLinuxBootLoader` takes the raw image. | 016-efi-stub |
| The console is the first `pl011@` node, not the one `/chosen/stdout-path` names. | A machine whose console is not its first UART would print where nobody is reading. Correct for every tree QEMU emits; tested against a two-UART blob. | When a machine needs it -- `/chosen` comes after the UARTs, so it needs candidates resolved at the end of the walk rather than one field |
| The whole 1 MiB the device tree blob declares is reserved, though its content is ~8.5 KiB. | ~1 MiB unavailable until something reclaims it. It is marked `Reclaimable`, so it can be. | When there is an allocator to reclaim into |

## Decisions made, and what would reverse them

Recorded so a future session does not relitigate them by accident.

| Decision | Rationale | Would reverse if |
| --- | --- | --- |
| New kernel, not a Linux/BSD fork | The thesis is about the abstraction the kernel presents; inheriting a page-based VM subsystem defeats it | -- |
| Rust 2024, `no_std` kernel | `unsafe_op_in_unsafe_fn` and unsafe `extern` make kernel hazards individually justified | -- |
| Boot in a VM, never bare metal | Hardware bring-up buys nothing the hypervisor does not give us | -- |
| QEMU + HVF `-M virt` is the primary Apple Silicon target | Needs a gdb stub and custom emulated devices (the ML-MMU); Virtualization.framework offers neither | QEMU's HVF support regresses badly |
| Virtualization.framework kept as second hypervisor | Requirement N2: no single hypervisor's quirks become load-bearing | -- |
| QEMU/KVM + VFIO on Linux | Only stack combining passthrough, custom devices, and a debugger. Firecracker has no PCIe at all | Cloud Hypervisor grows a usable custom-device path |
| GPU gate G7 is *placement*, not compute | Stops MLOS becoming a driver project | -- |
| No GPU on the Mac; GPU work happens on the Linux box | M1--M5 need no GPU at all, so Apple's GPU is not on the critical path | -- |
| If the Mac ever needs GPU compute: a custom `virtio-mlaccel` device with a Metal host backend, not Venus | Venus's guest encoder is tens of thousands of lines of Mesa; a typed op ring is hundreds | Someone ports a Venus encoder to `no_std` |
| Asahi Linux's GPU driver is a reference, never a dependency | Bare-metal only (the GPU coprocessor can reach all physical memory, so there is no boundary to virtualize), Linux-DRM-bound, GPL | Apple ships GPU virtualization |
| One VMM on the Mac (QEMU), not two | Corrected: QEMU has carried virtio-gpu Venus since 9.2, so libkrun was never needed | -- |
| Object table in kernel, policy in a service | A fault that costs an IPC round trip before it knows where to look is too expensive | Measurement shows the service hop is free |
| x86-64 and aarch64 first, RISC-V later | GPU support on RISC-V is not mature enough for G7 | RISC-V GPU support matures |

Open questions Q1--Q4 are in [PRD.md](PRD.md#9-open-questions) and are
answered by measurement at M3 (Q1, Q2) and M6 (Q3).

## Next action

Saga `mlos-parameter-major` (M4) is complete and gate G5 is met;
[g5-report.md](g5-report.md) is the artifact. Three things follow into
the plan (its section 8): M5's admission control should admit a
latency-contracted session knowing what its ceiling costs the others in
reads; degradation rungs that shrink KV raise the share of accesses a
scheduler can share; and two milestones have built session machinery with
nothing but the shell to drive it -- a recorded trace from a real engine
with MLOS under it is the gap both G4 and G5 name, and it wants a step
before M6.

Next, per [plan.md](plan.md): M5 `mlos-degradation` (gate G6) and saga
`mlos-two-hosts`, which can run in parallel. Either starts with
`agentrail init` from its section of the plan.
