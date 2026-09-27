# MLOS x86-64 (portability, no gate)

Source: docs/plan.md, saga `mlos-x86-64` (commit 99deb14). This lane runs
in `lanes/x86/` via `agentrail --saga lanes/x86` so it never touches the
root `.agentrail/`, which belongs to the aarch64 lane's active saga.

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
