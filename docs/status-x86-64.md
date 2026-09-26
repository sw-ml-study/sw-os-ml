# x86-64 status

What works on the x86-64 guest today (saga `mlos-x86-64`, `lanes/x86/`).
Kept apart from [status.md](status.md) while the aarch64 lane is editing
that file; the two merge when both lanes have, and the architecture
becomes a column there (step `x86-replay`).

## Boot

| | State |
|---|---|
| Machine | QEMU `microvm,acpi=off`, TCG, `-cpu max`. `acpi=off` because with ACPI on, `microvm` describes virtio-mmio slots in the DSDT and leaves them off the command line |
| Boot protocol | PVH: ELF with a `XEN_ELFNOTE_PHYS32_ENTRY` note, loaded by `-kernel`, entered in 32-bit protected mode with `hvm_start_info` in `%ebx` |
| Entry | `mlos-hal-x86-64` `_start`: `.bss` zeroed, 64 KiB stack, 1 GiB identity map, long mode, `mlos_main(start_info)` |
| Identity map | 1 GiB pages when the CPU has `pdpe1gb` (`-cpu max`); 2 MiB pages otherwise (QEMU's default `qemu64` lacks it). The guest reports which |
| `mlos_main` | `mlos-kernel-x86-64`: opens COM1, prints the banner (`MLOS x86-64`) and what was found, then runs `mlsh`. `Ctrl-D` ends the guest through `isa-debug-exit` with the entry's findings as bits |
| Boot info | `mlos-pvh` (host-tested, no `unsafe`): memory map from `hvm_start_info` (e820: RAM usable, ACPI reclaimable, anything else reserved), virtio-mmio slots from `virtio_mmio.device=` on the command line, `mlsh.run=` from the same string with QEMU's appended entries cut off. The kernel image is carved out with the same `reserve` aarch64 uses; the loader's structures only when they sit in usable RAM (with ACPI off QEMU puts them in the BIOS area, already reserved). Map overflow past `MAX_REGIONS` is reported, not dropped silently. Nothing from `mlos-fdt` is in the image (`nm`); it is still a build dependency through `mlos-machine` |
| Shell | `mlsh`, fed by polling COM1 into the same queue the aarch64 receive interrupt fills. `mem` and `dev` report the PVH map, the 16550, `timer none`, `gic none`. CPU count is 1 (the boot CPU; SMP is not in this saga). `sweep` has no clock yet (step `x86-interrupts`) |
| Traps | `mlos-trap-x86-64`: a 32-gate IDT, one stub per vector (so the vector is known without asking), a uniform frame, and a report in the aarch64 layout: `!! trap N (name)`, then error code, `RIP`, `CR2`, `RFLAGS`. Fatal path only -- nothing is saved for a return. Installed right after COM1, before anything else can fault. The reporter prints on COM1 and exits with the boot bits plus `0x08` (trapped). `mem peek ADDR` in `mlsh` reads 8 bytes through `Facts.peek` (x86-64 supplies it; aarch64 passes `None` and says so); on an unmapped address it is how a fault is provoked on purpose. No IST or TSS yet, so a fault that overflows the stack becomes a triple fault |
| Interrupts | `mlos-apic-x86-64`. The 8259s are remapped to 0xe0+ and masked. The LAPIC base is read from `IA32_APIC_BASE`; the IOAPIC is at 0xfec00000 by PC convention (no ACPI to say otherwise). LAPIC periodic timer at 2 Hz on vector 0x30, its rate measured (divide-by-16; QEMU's 1 GHz APIC clock shows as ~62.5 M counts/s). COM1 IRQ 4 routed by the IOAPIC to vector 0x34, edge, active high; the UART's receive interrupt wakes the shell. `mlos-trap-x86-64` has a 256-gate IDT: 32-255 save the caller-saved registers and return. Idle is `cli; check; sti; hlt`, so a keystroke cannot land between the check and the sleep. Measured: an idle guest costs 0.18 s of host CPU over 6 s, against 5.9 s when it polled |
| Clock | The TSC. Its rate is read from `CPUID.15H` when the CPU reports one, otherwise calibrated over 50 ms of PIT channel 0; which is printed (under TCG it is calibrated, ~2.1 GHz here). Shifted right if it exceeds a `u32` of Hz, which `Facts.clock` takes. `sweep` reports real microseconds |
| Device window | 3-4 GiB is identity-mapped uncached (PCD|PWT): LAPIC, IOAPIC, virtio-mmio. 1-3 GiB stays unmapped, so `mem peek 0x40000000` still faults |
| Boot stack | 256 KiB, reserved in `linker/x86_64.ld` like aarch64's. It was 64 KiB and `model` overflowed it through the page tables -- a triple fault, since the fault handler's own fetches faulted -- the same bug aarch64 hit in its step 010. Page tables now sit at the far end of `.bss`. No guard page: that needs 4 KiB mappings |
| Console | `mlos-uart16550`: COM1 over port I/O, 115200 8N1, polled transmit and receive. FIFO deliberately left off so input sent before the kernel looks is not discarded. Receive interrupt: step `x86-interrupts` |
| CLI | `mlos [--arch x86-64] build`, `mlos [--arch x86-64] run [tcg] [--capture S]`. Without `--arch`, `build` and `run` mean the host's architecture (x86-64 on a Linux PC, aarch64 on Apple Silicon); `doctor`, `layout`, `runtime` keep the shared path |

## Verified

`cargo test -p mlos-cli -- --ignored` boots the x86-64 guest four ways
under TCG on QEMU 8.2.2, Ubuntu 24.04: banner through `mlos run`, echo of
scripted input with the exit bits checked, the 2 MiB fallback on QEMU's
default CPU, and `build` defaulting to the host architecture. KVM: not
attempted -- saga `mlos-two-hosts`.

## Known gaps, to fold in when the lanes merge

- `mlos --help` does not mention `--arch` or the host default;
  `mlos --arch x86-64 help` does. Left so the shared `USAGE` text is not
  edited by both lanes.
- `mlos layout` and `mlos runtime` still build the aarch64 image on any
  host.
- `mlsh`'s `dev` is now driven by `Facts` alone: the console kind, a
  timer name (`Facts.timer`: `generic, irq` on aarch64, `lapic, vector`
  on x86-64), and an interrupt controller named by `Facts.irqchip` where
  there is no GIC. aarch64's output is unchanged; its `boot.rs` sets the
  two new fields (two lines in that lane's file).
- The plan named the ACPI PM timer for calibration; `microvm` runs with
  ACPI off (step `x86-bootinfo`), so there is none, and the PIT is used.
- `mlos-machine` (which `mlsh` and `mlos-snapshot` use only for `rest()`)
  depends on `mlos-fdt`. Moving `rest()` somewhere neutral would drop the
  build dependency too.

- `mem peek` is x86-64 only: aarch64's `boot.rs` passes `peek: None`
  (one line in that lane's file). The aarch64 lane can supply one.

## For step `x86-virtio-blk`

- The virtio-mmio slots at `0xfeb00000`+ are now mapped, uncached, in
  the device window (step `x86-interrupts`).
- `mlos doctor` does not check `qemu-system-x86_64` (step `x86-gate`).
- This file becomes a column of `status.md`.
