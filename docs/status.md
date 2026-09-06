# MLOS -- Status

**Ground truth.** If it is not in this file, it does not work.
Updated in the same commit as the work it describes.

Last updated: 2026-09-06, saga `ml-os-foundations` + step 006 (GPU path correction).

---

## Where we are

**M0 complete. No code exists.**

The architecture is written and the conformance discipline is in place.
The kernel has not been started. There is no `Cargo.toml` in this repo
yet, which is why `sw-checklist` reports "No Cargo.toml files found"
rather than a passing score.

## PoC gates

From [PRD.md](PRD.md#51-the-proof-of-concept-gate-the-thing-we-are-building-toward).

| Gate | State | Milestone |
| --- | --- | --- |
| G1 -- it boots | not started | M1 |
| G2 -- it holds an object table | not started | M2 |
| G3 -- it faults | not started | M2 |
| G4 -- known-next-use beats LRU | not started | M3 |
| G5 -- one read serves N sessions | not started | M4 |
| G6 -- degrades instead of dying | not started | M5 |
| G7 -- touches a real GPU | not started | M6 |
| G8 -- ML-MMU emulated | not started | M6 |

## Milestones

| Milestone | State |
| --- | --- |
| M0 foundations | **complete** -- saga `ml-os-foundations`, 5 steps |
| M1 it boots | not started -- saga `mlos-boot` proposed in [plan.md](plan.md#saga-mlos-boot-m1) |
| M2 it holds objects | not started |
| M3 it knows better | not started |
| M4 it shares | not started |
| M5 it degrades | not started |
| M6 it crosses PCIe | not started |

## What exists in this repo

```
  AGENTS.md          agentrail briefing + sw-os-ml policy
  CLAUDE.md          -> AGENTS.md
  README.md          document map
  docs/PRD.md        what MLOS is for, 8 gates, non-goals
  docs/architecture.md   kernel concepts + platform/hypervisor/GPU survey
  docs/design.md     crates, syscalls, object table, ML-MMU contract
  docs/plan.md       6 milestones, 6 follow-on sagas
  docs/status.md     this file
  docs/research.txt  raw source material (input, not specification)
  .agentrail/        saga ml-os-foundations
```

No `crates/`. No `scripts/`. No workspace.

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
| `BootInfo` marks the whole DRAM region `Usable`, including the range the kernel image occupies (`0x4020_0000` upward). `usable_bytes()` therefore overstates by the image size. | A future allocator that trusts it would hand out the kernel's own memory. Nothing consumes it yet. | 008-memory-map |
| `MemoryKind::Kernel` and `Reclaimable` are defined but never produced. | The map has less structure than its type suggests. | 008-memory-map |
| A device tree with more than 8 memory regions silently keeps the first 8. | Not reachable on QEMU `virt`, which reports one. | When a machine needs it |
| The identity map is 1 GiB blocks and the whole of RAM is executable. | No W^X, no per-page permissions. Nothing runs but the kernel yet. | When the object manager needs finer granularity (M2) |
| `mlos_hal::PageTable` is declared but not implemented. | The `Platform` associated type has no concrete impl. | Same -- a general mapper written before it has a caller is the wrong mapper |

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

`agentrail init --name mlos-boot`, with the plan from
[plan.md](plan.md#saga-mlos-boot-m1). First step is `workspace`: a
Cargo workspace on Rust 2024 that builds an empty kernel for both bare
targets with `sw-checklist` green.

Install QEMU before starting it.
