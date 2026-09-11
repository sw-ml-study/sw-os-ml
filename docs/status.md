# MLOS -- Status

**Ground truth.** If it is not in this file, it does not work.
Updated in the same commit as the work it describes.

Last updated: 2026-09-11, during saga `mlos-objects`, after step 008.

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
| G7 -- touches a real GPU | not started | M6 |
| G8 -- ML-MMU emulated | not started | M6 |

## Milestones

| Milestone | State |
| --- | --- |
| M0 foundations | **complete** -- saga `ml-os-foundations`, 7 steps |
| M1 it boots | **17 of 18 steps, 1 parked** -- saga `mlos-boot`. Gate G1 met. Virtio console and CI done; `efi-stub` parked |
| M2 it holds objects | **8 of 11 steps** -- saga `mlos-objects`. Gates G2 and G3 met. Steps 009--011 finish the layout emitters |
| M3 it knows better | not started |
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
| Shell | `mlsh`: `help`, `mem`, `dev`, `ticks`, `model`, `objs`, `get L T`, `sweep`, `faults`, `arena`, `list` |
| Layout | `mlos layout` writes `build/storage-layout.json`: three spaces (disk, arena, guest RAM), 140 regions, in sw-mlpl's columnar `system-layout` contract |
| Tooling | `mlos build` / `run [hvf\|tcg\|vz]` / `run --capture N` / `run --debug` / `doctor` / `image disk` / `layout` |
| Tests | 29 fast test binaries plus three TCG boot tests (`cargo test -p mlos-cli -- --ignored`); CI runs the lot on an aarch64 Linux runner |

## What does not exist yet

No residency policy: the arena is a bump allocator and eviction is
unimplemented, so a full arena reports `NoBudget` rather than choosing a
victim. No userspace, no scheduler beyond a single kernel thread, no
leases, no sessions, no sharing, no degradation ladder, no GPU and no
ML-MMU. `next_use` is recorded and read by nothing -- which is exactly
the gap M3 closes, and the reason M3 is the milestone that matters.

The layout emitter is static only. `mlos layout` describes what the build
produced -- where each weight tile sits on disk, how the kernel image
divides RAM, how big the arena is -- and every object in it reads
`"state": "never"`, because nothing has run. What is actually resident
needs step 009, and the disk-to-arena edges that make an "explain this
object" view possible arrive with it. A picture drawn from today's file is
a picture of a build, not of a running system.

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

Saga `mlos-objects` step 009 `layout-runtime`: the same contract emitted
from the running system, through an `mlsh layout` verb and
`mlos run --capture`, so the arena shows what is resident and the edge
table can join a stored tile to the bytes it became.

Install QEMU before starting it.
