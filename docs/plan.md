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
  M3  it knows better    next_use beats LRU on a real trace       (G4)
  M4  it shares          parameter-major scheduling               (G5)
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

### M6 -- It crosses PCIe (PoC gates G7, G8)

**Result:** on the Linux/NVIDIA host, MLOS as a guest enumerates a
passed-through GPU, maps its BARs, and moves a tensor into device
memory under object-manager control. Separately, the emulated ML-MMU
device services descriptors through the `mlmmu` provider.

Scope:
- `mlos-hal-x86-64`: long mode, ACPI (MADT, MCFG), APIC, page tables.
- `mlos-pci`: ECAM enumeration, BAR mapping (**resizable BAR
  required**), MSI-X.
- `mlos-provider-device`: GPU VRAM as a tier, DMA in and out.
- QEMU device model implementing the
  [ML-MMU register contract](design.md#8-the-ml-mmu-register-contract).
- `mlos-provider-mlmmu` driving it, `CAPS`-gated.
- Host setup documented and checked by `mlos doctor`: IOMMU groups,
  early `vfio-pci` binding, Above-4G decoding, ReBAR, hugepages.

M6 is where the second machine becomes necessary. Everything before it
runs on the Mac.

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
    consumption order, from emufpga's `.spm` sidecar. **Blocked on a real
    checkpoint** ([external-asks.md](external-asks.md) emufpga A1).
11. `g4-report` -- the measured comparison table, at more than one
    budget, written so it can be disputed. Gate G4.

Three things this saga must not do, restated from its own plan because
they are the ways it would fail without noticing: do not make the
workload easy (a dense sweep guarantees the answer before any code is
written), do not hand-write a trace ([architecture.md](architecture.md)
s.12), and do not let the kernel and the simulator drift.

If the separation is not there on a real generative trace, say so and
stop before M4. `docs/PRD.md` s.9 Q1 and Q2 are answered by the
measurement whichever way it comes out.

### Saga `mlos-parameter-major` (M4)

> Vision: invert the scheduler. Stop dragging the model past memory
> once per session; drag it past once and serve everyone waiting.

Steps: `session-objects`, `share-count`, `scheduler-inversion`,
`latency-escape`, `ps-ss-metrics`, `g5-report`.

### Saga `mlos-degradation` (M5)

> Vision: make the system able to say "I served you at Q4 with a 4K
> context" instead of "killed".

Steps: `contracts`, `admission-control`, `ladder`, `recompute-provider`,
`cost-policy`, `router-prefetch`, `g6-report`.

### Saga `mlos-pcie` (M6)

> Vision: reach a real GPU across a real PCIe bus, and give the ML-MMU
> something to be emulated against.

Steps: `x86-64-hal`, `acpi`, `pci-ecam`, `bar-mapping`, `vfio-host-setup`,
`device-provider`, `mlmmu-qemu-device`, `mlmmu-provider`, `g7-g8-report`.

## 4. Cross-repo dependencies

Each of these is written up as an ask -- what, where, why, and what MLOS
does if the answer is no -- in [external-asks.md](external-asks.md).

| Needed from | What | Needed by |
| --- | --- | --- |
| `emufpga` | The `.spm` sidecar: real tensor inventory, and which streams rotate per operation | M3 step 4 |
| a real checkpoint | Nobody has extracted one; only `tiny.spm` exists | M3 step 4, **blocked** |
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
