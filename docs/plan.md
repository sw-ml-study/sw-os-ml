# MLOS -- Plan

Status: current. Updated at the end of every saga.
See [PRD.md](PRD.md) for the gates this plan drives toward,
[status.md](status.md) for where we actually are.

---

## 1. Shape of the plan

Six milestones, each ending at a demonstrable result rather than at a
quantity of code. Each milestone is one agentrail saga. Sagas are
sequential; the parked list at the bottom stays parked until something
on the critical path needs it.

```
  M0  foundations        docs + saga discipline            <- DONE
  M1  it boots           aarch64 under QEMU/HVF, console, timer
  M2  it holds objects   object table, providers, model fault
  M3  it knows better    next_use beats LRU on a real trace       (G4)  <- DONE
  --  it is portable     x86-64 HAL; both guests on Mac and Linux  (no gate)
  M4  it shares          parameter-major scheduling               (G5)  <- DONE
  M5  it degrades        sessions, contracts, admission, ladder   (G6)
  M6  it crosses PCIe    x86-64 + VFIO GPU placement, ML-MMU Gen 0 (G7,G8)
```

**M1 through M5 require no GPU at all.** Gates G1--G6 are OS semantics
-- boot, object table, model fault, next-use versus LRU, one read
serving N sessions, degradation under pressure -- and every one of them
is demonstrated with synthetic tensors and recorded traces on the CPU.
The GPU first appears at G7, on the Linux/NVIDIA host. That is what
makes the whole plan viable on a Mac whose GPU we cannot reach
([architecture.md s.7.1](architecture.md#option-c----no-gpu-on-the-mac)).

**Added 2026-09-26: the x86-64 port is its own pair of sagas, not a
step of M6.** M6 was recut away from PCIe passthrough (below) and the
x86-64 HAL it used to carry was left homeless. It is also needed sooner
than M6 for a reason that has nothing to do with GPUs: since the CI
runner was deleted (M3 step 003) nothing checks that MLOS builds and
boots anywhere but one Mac. Two sagas -- `mlos-x86-64` and
`mlos-two-hosts`, s.3 below -- give the kernel a second architecture and
the project a second machine, so that every later result can be
reproduced on hardware the author does not own. They carry no PoC gate
and can run on a branch alongside the tail of M3, which they barely
touch.

The ordering is not arbitrary. M3 and M4 are the two results that
decide whether "ML OS" is a real architecture or a repackaging of
framework tricks (`docs/research.txt` closing paragraph). They come
before the expensive hardware work on purpose: if `next_use` does not
separate from LRU on real traces, we want to know that before buying
IOMMU-clean hardware.

## 2. Milestones

### M0 -- Foundations (complete)

Saga `ml-os-foundations`. This document set, the agentrail discipline,
the conformance gates. No code.

### M1 -- It boots (PoC gate G1)

**Result:** MLOS prints to a console and services a timer interrupt,
booted as an aarch64 guest under QEMU/HVF on the Mac, and under
Virtualization.framework as the second hypervisor.

Scope:
- `mlos-abi`, `mlos-hal`, `mlos-hal-aarch64`, minimal `mlos-kernel`.
- `_start` at EL1, DTB parse, page tables, MMU on, stack switch.
- GICv3 + generic timer. PL011 console.
- `mlos-cli`: `mlos build`, `mlos run --host hvf|tcg|vz`, `mlos doctor`.
- CI: the same binary boots under TCG, deterministically.

Deliberately *not* in M1: x86-64 (M6), userspace, scheduling beyond a
single kernel thread, any ML concept whatsoever.

**Risk to watch:** the UEFI path. Direct `-kernel` boot will work
quickly and tempt us to skip UEFI, but requirement N2 needs
Virtualization.framework working, and that means UEFI plus an
uncompressed raw image ([design.md](design.md#31-aarch64-under-qemu-virt--hvf)).
Do both in M1 or the second hypervisor never happens.

### M2 -- It holds objects (PoC gates G2, G3)

**Result:** a synthetic transformer's weights are registered as
`ML_OBJECT`s across three tiers; touching a non-resident one raises a
`MODEL_FAULT` that the provider layer services; faults are counted per
class.

Scope:
- `mlos-objtab`: `ObjectId`, `ObjectMeta`, the open-addressed table.
- `mlos-provider` + `dram`, `block`, `hostfile` (virtio-fs).
- `mlos-provider-virtio`: blk, fs, console.
- Fault path: `ml_acquire` -> miss -> `MODEL_FAULT` -> provider ->
  place -> lease -> resume. Fast path never crosses IPC.
- `mlos-metrics`: `Pf`, `Rm`, `Bt` counters.
- Syscalls: the object and lease groups.
- First userspace process, and the syscall boundary that implies.
- `mlsh`: the in-guest object/session inspector over virtio-console
  ([design.md s.9](design.md#9-how-you-interact-with-mlos)). This is
  the interface you actually use MLOS through, so it arrives with the
  first objects worth inspecting.

`hostfile` over virtio-fs matters more than it looks: it means model
files live on the host and the dev loop does not rebuild a disk image
to change a weight file.

### M3 -- It knows better (PoC gate G4)

**Result:** a table, from measurement, showing known-next-use
replacement reducing provider reads against LRU and FIFO under an
identical residency budget, on a trace taken from a real model.

Scope:
- `mlos-trace`: trace format, record and replay.
- Traces from real models via emufpga's importers -- **not
  hand-written** (architecture s.12 risk).
- `mlos-sim`: host-side replay harness running the same
  `mlos-policy-*` crates the kernel links.
- `mlos-policy` (demand, fifo, lru), `mlos-policy-nextuse`.
- `ml_stream_declare` / `ml_stream_advance`: the calls by which
  userspace hands the kernel the future.
- The same comparison run in-kernel under TCG, matching the simulator.

**This is the milestone that justifies the project.** Belady's optimal
algorithm is normally unimplementable; a dense transformer hands you
the future. If the separation is not there, stop and reconsider before
M4.

**Result, 2026-09-27: G4 met.** [g4-report.md](g4-report.md). On a
1.08B-parameter Llama's shape, 42--77% fewer provider reads than the
better baseline in the band where the budget is near the per-token
working set, and no difference outside it; the kernel reproduces the
simulator to the integer on aarch64 and x86-64. The traces are derived
from a real checkpoint's inventory, not recorded from inference, and the
report says so. Two findings feed forward: state budgets as a fraction of
the working set and report the band, and paged KV blocks are the
granularity M4 and M5 inherit.

### M4 -- It shares (PoC gate G5)

**Result:** four concurrent sessions needing the same layer cause one
provider read, not four, measured against a process-major baseline on
the same trace.

Scope:
- Session objects; `share_count` maintained on the object table.
- Parameter-major scheduler: schedulable unit is
  `(model-state chunk, session set)`.
- `Ps` (scan productivity) and `Ss` (sessions per GB) counters.
- The latency escape hatch: a latency-contracted session buys a private
  read rather than waiting its turn in the batch.

This is the direct continuation of the emufpga SPM result, promoted
from a tensor-stream property to an OS scheduling policy.

**Result, 2026-10-02: G5 met.** [g5-report.md](g5-report.md). On the real
1B model's shape, parameter-major scheduling reads each weight once per
token for every session: LRU's weight reads fall from once per session per
token to once per token, and next-use reads fall sixteenfold (44,309 to
2,787 with four sessions at 1536 MiB; `Ps` 8.8 to 77.4). The gain is
bounded by the share of accesses that are a session's own cache, and
lockstep costs a session about 11% in period, which a ceiling buys back
until it is a quarter of a token, below which the group pays eightfold.
The same `mlos-sched` crate schedules the kernel's replay on aarch64 and
x86-64 to the simulator's integers.

### M5 -- It degrades (PoC gate G6)

**Result:** under a shrinking residency budget, MLOS walks the
degradation ladder and reports the quality and latency it delivered,
instead of killing a session.

Scope:
- `Contract { quality_floor, latency_ceiling, resident_ceiling }`.
- Admission control: `ml_session_create` refusing is a normal outcome.
- The eight-rung ladder (architecture s.5), including KV quantization
  and context truncation.
- `mlos-policy-cost`: reload vs recompute vs quantize, and the
  `recompute` provider that makes `DISCARD` legitimate.
- `mlos-policy-router`: MoE expert prefetch from a router distribution.
- `Rc`, `Ph`, `Ks` counters.

### M6 -- It controls real host resources (PoC gate G7)

**Recut 2026-09-17.** M6 was "it crosses PCIe": an x86-64 HAL, PCI
enumeration, VFIO passthrough and a driver for a real GPU. That is the
wrong destination. Passing a GPU through and driving it would make MLOS a
GPU operating system; the goal is an ML *resource* operating system, and
the host already has a better NVIDIA driver than this project will ever
write. See [architecture.md s.7.4](architecture.md#74-the-hostguest-boundary-what-mlos-does-not-own).

**Result:** MLOS, as a guest, places real objects in real host resources
it does not drive -- host memory, host storage, and host GPU memory --
through narrow virtio interfaces, and the M3 policy governs the
placement.

Scope:
- `virtio-ml-storage`: an object provider backed by a host file, served
  with `mmap`, `io_uring` or NVMe as the host prefers. MLOS never learns
  which.
- `virtio-ml-compute`: "make this object resident in device memory",
  implemented host-side with CUDA or Metal. The host does the transfer;
  MLOS decides that it should happen.
- The policy from M3 driving both, so the comparison extends from a
  simulated tier to a real one.
- What crossing the boundary costs, measured per object, per layer and
  per token -- because the granularity at which the guest/host hop is
  worth making is a finding, not an assumption.

Passthrough stays on the table as an experiment, not a destination: it is
the only way to measure what the boundary costs, and that number is worth
having. It is no longer what M6 is for.

### M7 -- Another machine is a provider (PoC gate G8)

Planned in detail in [clustering.md](clustering.md): simulate a cost
graph first, then two emulated guests on one Mac joined by a host relay,
then `virtio-net` and a second machine. Homogeneous before heterogeneous,
so that any difference the cluster makes is attributable.

**Result:** an object resident in node B's RAM is fetched by node A over
Ethernet, and the policy chooses between that and node A's own SSD on
cost rather than on a tier ordinal.

This is where `TIER` stops being a ladder and becomes the cost graph
[architecture.md s.7.5](architecture.md#75-tiers-are-a-cost-graph-not-a-ladder)
describes. Node B's RAM may be cheaper to reach than node A's disk, and
no total ordering can express that.

Scope: `virtio-ml-net` as a remote object provider; placement as a set
per object rather than one tier; cost per edge; the policy asking "what
is the cheapest way to satisfy this before its deadline".

### M8 -- Heterogeneity

**Result:** an ugly deliberate cluster -- mismatched GPUs, mismatched
RAM, SAS arrays beside NVMe, a slow link -- serving a model far larger
than any one node's VRAM, with MLOS placing state across all of it.

Old hardware is an asset here rather than a nuisance: older PCIe
generations make bad residency decisions *more* expensive, which makes
them easier to measure.

### M9 -- Global scheduling

**Result:** placement, compute and network scheduled together beat the
same nodes deciding independently. The measurement is tokens/sec, TTFT,
p50/p95 latency, bytes moved at each level, GPU idle time, redundant
loads and expert hit rate, against a baseline of independent per-node
inference runtimes.

This is the end-state experiment, and it is where a distributed control
plane either earns its existence or does not.

### M10 -- ML-MMU

**Result:** the mechanisms that turned out to matter in software, in
gateware, against the
[register contract](design.md#8-the-ml-mmu-register-contract) that
already exists. Deliberately last: hardware should accelerate what has
been shown to work, not what was hoped would.

## 3. Follow-on sagas

Each milestone is one saga. Proposed names, plans and first steps, so
the next session can `agentrail init` without redesigning:

### Saga `mlos-boot` (M1)

> Vision: one kernel binary that reaches a console and a timer tick as
> an aarch64 guest, under two different hypervisors, from one source
> tree. Nothing ML-shaped exists yet; this saga earns the right to
> write the interesting parts.

1. `workspace` -- Cargo workspace, Rust 2024, both bare targets
   building an empty kernel. `sw-checklist` green.
2. `abi-skeleton` -- `mlos-abi`: error codes, `ObjectId` layout with
   its layout test. Written now because the ML-MMU contract depends on
   these bits and changing them later is expensive.
3. `hal-trait` -- `mlos-hal`: the four-method trait, `BootInfo`.
4. `aarch64-entry` -- `_start`, DTB parse, page tables, MMU on, stack.
5. `console-timer` -- PL011, GICv3, generic timer. First printed line.
6. `cli-run` -- `mlos build` / `mlos run --host hvf|tcg` / `mlos doctor`.
7. `uefi-and-vz` -- UEFI image path; boot under
   Virtualization.framework. Requirement N2 satisfied.
8. `ci-tcg` -- deterministic TCG boot test in CI.

### Saga `mlos-objects` (M2)

> Vision: the kernel stops being a boot exercise and starts being an
> ML OS. An object table, providers, and a model fault that carries
> enough to make a good decision.

1. `objtab` -- `ObjectId`, `ObjectMeta`, open-addressed table, host
   unit tests.
2. `provider-trait` -- `mlos-provider`, `dram` provider, resolution.
3. `virtio` -- virtio-mmio transport, console, blk, fs.
4. `hostfile-provider` -- model files on the host, no image rebuild.
5. `userspace` -- first process, syscall entry, capability check.
6. `model-fault` -- the fault path end to end, fast path IPC-free.
7. `metrics-pf` -- per-class fault counters; `Rm`, `Bt`.
8. `mlsh` -- the inspector shell: `objs`, `sessions`, `faults`.
9. `synthetic-model` -- 8 layers x 16 tiles registered and swept.
   Gates G2 and G3.

Then, added 2026-09-11, the layout emitters -- MLOS as the second
producer of the cross-repo visualization contract:

10. `layout-static` -- `build/storage-layout.json`: the disk image, the
    arena reservation and the physical map, in the columnar contract.
11. `layout-runtime` -- `build/runtime-layout.json`: the same contract
    for the running system, so residency is visible rather than
    asserted. Brought with it the boot-script channel
    (`mlsh.run=` in `/chosen/bootargs`), without which no headless
    capture can drive the shell at all.
12. `layout-events` -- residency transitions streamed as they happen,
    so a viewer can animate churn instead of diffing snapshots.
    Recording costs 4.3 ns an event on native hardware against 42 us a
    fault, which is why it is left on.
13. `layout-coordinate` -- sample artifacts, checksums, and the
    vocabulary handed to the three sibling repos.

The contract is sw-mlpl's
(`../sw-mlpl/docs/storage-layout-viz.md`), already emitted by sw-tos
and already parsed by sw-mlpl's interpreter. MLOS adopts it unchanged
and extends it only through columns the contract permits -- tier,
class, residency state, next-use -- which is the whole argument for a
shared format: the things an ML object store knows that a flash image
does not show up as *columns*, not as a fork.

### Saga `mlos-nextuse` (M3)

> Vision: prove the thesis, or find out it is wrong. A transformer hands
> the OS its own future; show that an OS which accepts the gift beats one
> that guesses.

Reordered 2026-09-16 to put the measurement first. The original order
reached a number at step 8 of 8; this one reaches it at step 6 of 11,
and the four steps before it need no emulator and nothing from another
repository. If the answer is no, everything after step 6 is work not
worth doing, and finding that out early is the point.

1. `trace-format` -- `mlos-trace`, record and replay. Renames M2's
   `mlos-trace` (residency events) to `mlos-events` and takes the name
   back for what the plan always meant by it: an access trace. **Done.**
2. `sim-harness` -- `mlos-sim`, identical budget across policies,
   enforced rather than intended.
3. `baselines` -- demand, FIFO, LRU. Also the harness's own test: LRU
   must beat FIFO on a workload with reuse, or the harness is wrong.
4. `reuse-trace` -- weights re-swept cyclically and KV blocks
   accumulating, so LRU has a fair chance to be right. From the
   synthetic model, which needs `KvBlock` objects to express it.
   **Not blocked:** what the measurement needs first is fairness, and
   fairness does not require a real checkpoint.
5. `nextuse-policy` -- known-next-use. Distance acted on with certainty,
   probability only as a hint.
6. `verdict` -- **stop and look at the table.** Four policies, several
   budgets, and a decision about whether the rest is worth doing.
7. `stream-syscalls` -- `ml_stream_declare` / `advance`. The first thing
   ever to write `ObjectMeta::next_use`. Not needed before step 6: a
   simulator has the whole trace and can compute next-use directly,
   which is what makes it a simulator.
8. `evictable-arena` -- the arena stops being a bump allocator. Nothing
   can run a policy in the kernel until it can give memory back.
9. `in-kernel` -- same policy crates in-kernel under TCG; the numbers
   match the simulator exactly, not approximately.
10. `generative-trace` -- realism: real tensor sizes and the real
    consumption order, from emufpga's `.spm` sidecar. The blocker
    ([external-asks.md](external-asks.md) emufpga A1) was cleared by a
    checkpoint already on the machine and a header-only importer mode.
    **Done.**
11. `g4-report` -- the measured comparison table, at more than one
    budget, written so it can be disputed. Gate G4. **Done; met.**

Saga complete 2026-09-27, eleven steps.

Three things this saga must not do, restated from its own plan because
they are the ways it would fail without noticing: do not make the
workload easy (a dense sweep guarantees the answer before any code is
written), do not hand-write a trace ([architecture.md](architecture.md)
s.12), and do not let the kernel and the simulator drift.

If the separation is not there on a real generative trace, say so and
stop before M4. `docs/PRD.md` s.9 Q1 and Q2 are answered by the
measurement whichever way it comes out.

### Saga `mlos-x86-64` (portability, no gate)

> Vision: one kernel, two architectures, one table. The same `mlos-kernel`
> source boots as an x86-64 guest, reaches `mlsh`, faults a model in from
> virtio-blk, and replays the M3 comparison to the SAME integers the
> aarch64 guest and the simulator produce. Every crate above the HAL is
> already architecture-neutral and built for `x86_64-unknown-none` on
> every run; this saga is the HAL beneath them and the proof that neutral
> meant neutral.

Where it starts from: `mlos-kernel` has an x86-64 `_start` that is a
`hlt` loop, kept so the target cannot rot. Five crates are aarch64-only
and say so with `#![cfg(target_arch = "aarch64")]`: `mlos-hal-aarch64`
(entry, timer), `mlos-gic-aarch64`, `mlos-mmu-aarch64`,
`mlos-trap-aarch64`, and `mlos-pl011` beside them. `mlos-fdt` is the
discovery mechanism and x86-64 has no device tree. `mlos-cli` hardcodes
the aarch64 target triple, `qemu-system-aarch64`, and `hvf|tcg|vz`.

Decisions taken up front, so the saga does not relitigate them:

- **QEMU `microvm`, not `q35`.** `microvm` puts virtio devices on
  virtio-mmio, which MLOS already speaks; `q35` puts them on PCI, which
  is `mlos-pci` and belongs to M6. The console is a 16550 at COM1 over
  port I/O, the interrupt controller is LAPIC + IOAPIC, the timer is the
  LAPIC timer or TSC-deadline. That is the whole device set, and it is
  small on purpose.
- **PVH direct boot, not UEFI.** QEMU loads an ELF carrying the PVH
  entry note straight into 32-bit protected mode with a `hvm_start_info`
  in `%ebx` -- memory map, command line, module list. That is the x86-64
  twin of `-kernel Image` with the device tree in `x0`, and it is the
  same trade M1 made: the direct path for the dev loop, UEFI parked until
  something needs firmware services (`docs/design.md` s.3.2 still
  describes the OVMF path; it is not wrong, it is later).
- **The command line plays the device tree's part.** `microvm` announces
  each virtio-mmio slot as `virtio_mmio.device=SIZE@ADDR:IRQ` on the
  kernel command line, and PVH hands over the memory map directly. So
  `BootInfo` is filled from two sources on x86-64 where it was filled
  from one on aarch64, and `mlos-lab::set_slots` -- which was written
  to be told rather than to parse -- needs no change at all.
- **1 GiB identity map, as on aarch64.** PML4 and PDPT with gigabyte
  pages, `EFER.LME`, `CR0.PG`, far jump. If the emulated CPU lacks
  `pdpe1gb`, 2 MiB pages; the HAL reports which it used, because a
  difference in page size is a difference someone measuring `sweep` will
  eventually need to know about.
- **TCG is the reference, on both architectures.** An x86-64 guest on an
  Apple Silicon Mac is TCG or nothing, and the numbers this saga has to
  reproduce are TCG numbers already. KVM on a Linux host is
  `mlos-two-hosts`.

Steps:

1. `x86-entry` -- `mlos-hal-x86-64`: PVH note, 32-bit entry, long mode,
   1 GiB identity map, stack, `.bss` cleared, into `mlos_main` with the
   `hvm_start_info` pointer. `mlos build --arch x86-64` and `mlos run
   --arch x86-64 tcg` in `mlos-cli`, both architectures selectable and
   neither the default of the other. A `hlt` is no longer the entry.
2. `x86-console` -- `mlos-uart16550`: COM1 over port I/O, transmit then
   receive. First printed line: the banner, with the architecture in it.
   `unsafe` confined to the driver crate, every block with its `SAFETY:`.
3. `x86-bootinfo` -- memory map from the PVH table, virtio-mmio slots
   parsed from the command line, `mlsh.run=` honoured from the same
   string. `mem` and `dev` in the shell report what was found rather than
   what was assumed. Nothing from `mlos-fdt` is linked.
4. `x86-traps` -- `mlos-trap-x86-64`: IDT, exception entry, a fault
   report naming vector, error code, `RIP` and `CR2` -- the same shape
   the aarch64 report gives for `ESR`/`ELR`/`FAR`. Provoked on purpose
   from the shell and read back.
5. `x86-interrupts` -- `mlos-apic-x86-64`: LAPIC and IOAPIC, the LAPIC
   timer at the 2 Hz tick the shell already counts, the serial receive
   interrupt, and a nanosecond clock for `sweep` from the TSC with its
   rate read from `CPUID.15H` where QEMU offers it and calibrated
   against the ACPI PM timer where it does not. Which of the two it was
   is printed, because a calibrated clock is not a read one.
6. `x86-virtio-blk` -- the existing `mlos-virtio-blk` over virtio-mmio on
   `microvm`, the model disk attached, `model` reporting `weights from
   virtio-blk`, and the `0xA0` provenance nibble read back. Zero new
   driver code is the expected result; a line of it is a finding.
7. `x86-replay` -- `model 32; replay demand|fifo|lru|next-use` under
   x86-64 TCG, and a boot test asserting each line equals the aarch64
   guest's and the simulator's, exactly. `docs/status.md` gains the
   architecture as a column. This is the step the saga exists for.
8. `x86-gate` -- `kbuild-x86` and `kclippy-x86` already run; the boot
   tests now run both architectures whenever both QEMU binaries exist,
   and `mlos doctor` says which are missing. `sw-checklist` at the same
   count it started at.

Not in this saga: SMP, PCI, ACPI beyond the PM timer, UEFI, KVM.

### Saga `mlos-two-hosts` (portability, no gate)

> Vision: a Linux machine and a Mac, each running the whole gate and
> booting both guests, so that nothing about MLOS depends on the machine
> it was written on. The CI runner that was deleted at M3 step 003 was
> the only thing checking this, and its one real find -- QEMU installed
> without ROM blobs -- was exactly the kind of tooling failure that a
> second machine surfaces and a second architecture does not.

The matrix, and the accelerator in each cell:

```
                        aarch64 guest        x86-64 guest
  Mac (Apple Silicon)   HVF, TCG, VZ         TCG
  Linux (x86-64)        TCG                  KVM, TCG
```

Each host accelerates its own architecture and emulates the other. TCG
is in every cell, which is what makes the replay numbers comparable
across all four.

Steps:

1. `host-detect` -- `mlos run` chooses `hvf` on macOS, `kvm` on Linux
   when `/dev/kvm` is writable, `tcg` otherwise, and says which. A named
   accelerator that the host cannot provide is refused with the reason,
   not passed to QEMU to fail in its own words.
2. `linux-doctor` -- `mlos doctor` on Linux: both `qemu-system-*`
   binaries, the ROM blobs (the ten-day failure, checked for by name),
   `/dev/kvm` and group membership, the two bare targets in `rustup`.
   A `scripts/provision-linux.sh` that installs what is missing, or
   prints what to install where it cannot.
3. `kvm-boot` -- the x86-64 guest under KVM: the console, the timer, the
   model disk. Timing under KVM is real timing, so `sweep` gets its first
   number from a hardware-virtualized x86-64 core, recorded beside the
   HVF number.
4. `gate-on-linux` -- the full pre-commit gate from AGENTS.md run on the
   Linux host, every failure fixed or recorded. Path assumptions,
   `objcopy` vs `llvm-objcopy`, case-sensitive filesystems, `sed -i`
   flags: this is where they surface.
5. `boot-matrix` -- `cargo test -p mlos-cli -- --ignored` runs every cell
   of the matrix the host can offer and skips the rest by name, so the
   same test file is green on both machines and says what it did not
   run. The replay comparison asserted in every TCG cell.
6. `portability-report` -- `docs/status.md`: what each machine boots,
   which accelerators, which cells reproduced the M3 table, and the
   tooling differences found. Replaces the lament in AGENTS.md about the
   lost runner with what replaced it.

### Saga `mlos-parameter-major` (M4)

> Vision: invert the scheduler. Stop dragging the model past memory
> once per session; drag it past once and serve everyone waiting.

Steps: `session-objects`, `share-count`, `scheduler-inversion`,
`latency-escape`, `ps-ss-metrics`, `g5-report`.

Started 2026-09-29 as saga `mlos-parameter-major`, from this outline;
each step wrote its own reasoning into its commit and `docs/status.md`.
**Complete 2026-10-02, six steps, gate G5 met.** M3's rules carry over: measure in the band near the
per-token working set and say where it is, keep the KV cache paged, and
keep the kernel and the simulator agreeing to the integer.

### Saga `mlos-degradation` (M5)

> Vision: make the system able to say "I served you at Q4 with a 4K
> context" instead of "killed".

Steps: `contracts`, `admission-control`, `ladder`, `recompute-provider`,
`cost-policy`, `router-prefetch`, `g6-report`.

Started 2026-10-02 as saga `mlos-degradation`, from this outline; each
step writes its own reasoning into its commit and `docs/status.md` as it
is worked. Carried in from the G5 report: admission must know what a
latency ceiling costs the other sessions, and rungs that shrink KV raise
the share of accesses a scheduler can share.

### Saga `mlos-pcie` (M6)

> Vision: reach a real GPU across a real PCIe bus, and give the ML-MMU
> something to be emulated against.

Steps: `acpi`, `pci-ecam`, `bar-mapping`, `vfio-host-setup`,
`device-provider`, `mlmmu-qemu-device`, `mlmmu-provider`, `g7-g8-report`.
(`x86-64-hal` moved to saga `mlos-x86-64` on 2026-09-26; by the time M6
starts, the x86-64 guest already boots.)

## 4. Cross-repo dependencies

Each of these is written up as an ask -- what, where, why, and what MLOS
does if the answer is no -- in [external-asks.md](external-asks.md).

| Needed from | What | Needed by |
| --- | --- | --- |
| `emufpga` | The `.spm` sidecar: real tensor inventory, and which streams rotate per operation | M3 step 10 |
| a real checkpoint | Nobody has extracted one; only `tiny.spm` exists | M3 step 10, **blocked** |
| a host compute service | CUDA or Metal behind `virtio-ml-compute` | M6 |
| a Linux host | Any x86-64 Linux box with KVM; no GPU needed | saga `mlos-two-hosts` |
| a second machine | Any Linux box with a GPU, on the same LAN | M7 |
| `emufpga` | ML-MMU gateware, Gen 1+ | after M6 |
| `demo-memory` | Eviction and retrieval policy candidates | M3, M5 |
| `sw-mlpl` | Array language as eventual userspace | after M6 |
| `sw-mlpl` | The columnar layout contract + the viz library | M2 layout steps |
| `demo-extensions` | native3d: boxes, picking, labels, camera | M2 layout steps |
| `sw-tos` | First producer of the same contract; vocabulary precedent | M2 layout steps |

MLOS owes emufpga the ML-MMU register contract
([design.md](design.md#8-the-ml-mmu-register-contract)); that is the
one artifact another repo is blocked on, and it exists now.

## 5. Hardware to acquire

Nothing before M6. When M6 approaches, the Linux/NVIDIA host needs:

- VT-d or AMD-Vi, and a motherboard whose IOMMU groups isolate the GPU
  (GPU and its audio function alone, plus bridges -- no NIC, no NVMe).
- Above-4G decoding and **resizable BAR** in firmware. ReBAR matters
  more here than for a gaming VM: an object manager placing tensors
  across PCIe wants the whole VRAM aperture, not a 256 MB window.
- A GPU that is not the host's display adapter, so it can be bound to
  `vfio-pci` early.
- Enough RAM for hugepages plus a residency budget worth measuring.

MIG-capable hardware (A100/H100/RTX PRO class) becomes interesting only
if multi-tenant residency across hardware partitions is pursued, which
is post-M6 and not currently planned.

## 6. Parked

Not "rejected" -- deliberately not now, with the condition that would
unpark them:

| Parked | Unpark when |
| --- | --- |
| `virtio-mlaccel` on the Mac -- Apple GPU compute via a Metal host backend | Post-M6, and only if we want the Mac to run a real model. Nothing before M6 needs it |
| A Venus guest encoder | Never. Tens of thousands of lines of Mesa protocol code; `virtio-mlaccel` gets the same result for a fraction of it |
| Lifting Asahi's AGX driver | Never. Bare-metal only, Linux-DRM-bound, GPL. Useful as documentation, not as code |
| Training state: gradients, optimizer, checkpoints | M5 lands and mutable state at LoRA scale is well understood |
| The AI/agent operating environment (durable agent jobs, virtual context, tools as capabilities) | It is a *sibling project*; unpark as its own repo, never inside this one |
| Real FPGA gateware | M6 lands and the Gen 0 emulated device has proven the contract |
| Bare-metal boot | Never, per PRD s.7.1 |
| RISC-V target | GPU support in the RISC-V ecosystem matures. The HAL keeps this additive |
| Native NVIDIA driver via the GSP RPC path | Not planned; recorded as conceivable, not intended |
| RAG semantic paging | After M5. It is the broadest and least well-posed of the object classes |
| MIG-backed multi-tenant residency | Post-M6, and only with the hardware for it |
| `sw-mlpl` as the MLOS shell | Post-M6 |
