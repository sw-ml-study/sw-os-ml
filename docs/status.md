# MLOS -- Status

**Ground truth.** If it is not in this file, it does not work.
Updated in the same commit as the work it describes.

Last updated: 2026-09-18, during saga `mlos-nextuse`, after step 007.

---

## Where we are

**M1 complete, M2 well under way. Three of eight gates met.**

The kernel boots to a shell as a native aarch64 guest, holds an object
table, and services a model fault from a real block device. What it does
not have is a policy: nothing in it yet decides what to keep. That is
M3, and M3 is where the thesis in [PRD.md](PRD.md) is actually tested.

## PoC gates

From [PRD.md](PRD.md#51-the-proof-of-concept-gate-the-thing-we-are-building-toward).

| Gate | State | Milestone |
| --- | --- | --- |
| G1 -- it boots, reaches a shell | **done** | M1 |
| G2 -- it holds an object table | **done** | M2 |
| G3 -- it faults | **done** | M2 |
| G4 -- known-next-use beats LRU | not started | M3 |
| G5 -- one read serves N sessions | not started | M4 |
| G6 -- degrades instead of dying | not started | M5 |
| G7 -- controls a real host resource | not started | M6 |
| G8 -- another machine is a provider | not started | M7 |

## Milestones

| Milestone | State |
| --- | --- |
| M0 foundations | **complete** -- saga `ml-os-foundations`, 7 steps |
| M1 it boots | **17 of 18 steps, 1 parked** -- saga `mlos-boot`. Gate G1 met. Virtio console and CI done; `efi-stub` parked |
| M2 it holds objects | **complete** -- saga `mlos-objects`, 11 steps. Gates G2 and G3 met |
| M3 it knows better | **7 of 11 steps** -- saga `mlos-nextuse`. [The verdict](m3-verdict.md) is in: known-next-use separates clearly, 32--71% fewer provider reads. Gate G4 is not met until the same numbers come out of the kernel |
| M4 it shares | not started |
| M5 it degrades | not started |
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
| Shell | `mlsh`: `help`, `mem`, `dev`, `ticks`, `model`, `objs`, `get L T`, `sweep`, `faults`, `arena`, `layout`, `trace`, `stream` |
| Streams | `ml_stream_declare` / `ml_stream_advance` as `mlos-stream`. A declared cyclic order, and a cursor. **The kernel now writes `ObjectMeta::next_use`** -- the field that existed from M2 step 001 with nothing to set it |
| Layout | `mlos layout` writes `build/storage-layout.json`: three spaces (disk, arena, guest RAM), 140 regions, in sw-mlpl's columnar `system-layout` contract |
| Snapshot | `mlos runtime` boots, sweeps and writes `build/runtime-layout.json` from the live object table -- residency, reuse, cost and `backs` edges from stored tile to arena placement |
| Events | The same boot writes `build/runtime-events.jsonl`: one JSON line per residency transition (`placed` / `hit` / `refused`), joined to the snapshot by region id. `trace` prints them; `trace on\|off` switches recording |
| Traces | And `build/runtime.trace`: the access sequence those events record -- session and `ObjectId` per acquire, and nothing about what the system did. What M3 replays policies against |
| Simulator | `mlos-sim` replays a trace against a policy under a fixed residency budget and counts hits, provider reads, bytes, evictions and refusals. `compare` takes one budget for every policy, so an unequal comparison cannot be expressed |
| Policy interface | `mlos-policy`: `no_std` and pure, so the same code runs in the kernel and the simulator. A policy reads `ObjectMeta` and names a victim; it holds no state the table does not own |
| Boot script | `/chosen/bootargs` carries `mlsh.run=model;sweep;layout`, so a headless capture can drive the shell. A log file is not a terminal, so nothing else could |
| Tooling | `mlos build` / `run [hvf\|tcg\|vz]` / `run --capture N` / `run --debug` / `doctor` / `layout` / `runtime` |
| Timing | `sweep` reports elapsed nanoseconds from the ARM generic timer, not the 2 Hz tick -- which is what makes any claim about what the fault path costs measurable. The rate is read from `CNTFRQ_EL0` rather than assumed: 24 MHz under HVF, which is Apple Silicon's own counter passed through, and 62.5 MHz under TCG, which is QEMU's |
| Tests | 30 fast test binaries plus eight TCG boot tests (`cargo test -p mlos-cli -- --ignored`). Local only, by choice -- see [AGENTS.md](../AGENTS.md); there is no CI and the local gate is the stricter of the two |

## What does not exist yet

No residency policy: the arena is a bump allocator and eviction is
unimplemented, so a full arena reports `NoBudget` rather than choosing a
victim. No userspace, no scheduler beyond a single kernel thread, no
leases, no sessions, no sharing, no degradation ladder, no GPU and no
ML-MMU. `next_use` is recorded and read by nothing -- which is exactly
the gap M3 closes, and the reason M3 is the milestone that matters.

The only trace that exists is a dense sweep, and it is the EASY case. LRU
is pessimal on it by construction and known-next-use is optimal by
construction, so any comparison run against it proves nothing. Step 004
brings the trace with real reuse structure in it, and until then no
number from this saga should be quoted.

No eviction, so no `evicted` event. `Kind::Evicted` exists in the event
vocabulary and nothing emits it: until M3 has a policy, running out of
arena produces a `refused` and the object that would have been thrown away
stays. The most interesting line in a residency film is the one that is
not there yet.

The runtime document has no `sysram` space. A running kernel has no symbol
table and cannot say where its own `.text` ended, so guest RAM appears only
in the static document, drawn from the linked image. The two share `disk`
and `dram`, with the same region ids, which is what lets a consumer join
them.

`region_next_use` is `never` until a stream is declared. The kernel writes
it from M3 step 007 onwards -- `stream` in `mlsh` declares the model's
sweep and `stream N` advances it -- so a layout document taken after that
carries real positions. Nothing declares one automatically, because
nothing yet decides anything with it: the kernel has no policy and no
eviction, which is steps 008 and 009.

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

Saga `mlos-nextuse` is open, nine steps, plan in
[plan.md](plan.md#saga-mlos-nextuse-m3). M3 is the milestone the project
exists for: a transformer hands the operating system its own future, and
the claim is that an OS which accepts the gift beats one that guesses.
Everything built so far is mechanism -- a table, a fault, three tiers, two
emitters, an event stream. Nothing has decided anything yet.

Step 006 `verdict`: stop and look at the table. Four policies, several
budgets, several session counts, and a decision about whether the rest of
the saga is worth doing. The plan commits to the other outcome too -- if
the separation is not there, say so and stop before M4.

The saga was reordered on 2026-09-16 to reach a number sooner. Steps 002
to 005 build the harness, the baselines, a workload with real reuse in
it, and the known-next-use policy; step 006 stops and looks at the
table. None of those four needs an emulator or anything from another
repository. Everything after step 006 is work that is only worth doing
if the answer is yes.

The emufpga blocker ([external-asks.md](external-asks.md) A1) now sits at
step 010, where it belongs: a real checkpoint buys REALISM -- real tensor
sizes and the real consumption order -- and what the measurement needs
first is FAIRNESS, which the synthetic model can express on its own.

Install QEMU before starting it.
