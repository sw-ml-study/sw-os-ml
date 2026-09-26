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
- `mlsh`'s `dev` now prints the console kind from `Facts` instead of a
  literal `pl011`, and `timer none` when there is no timer irq. The
  timer line still says `generic` when there is one; x86-64's timer
  needs a name in `Facts` (step `x86-interrupts`, or the refactor).
- `mlos-machine` (which `mlsh` and `mlos-snapshot` use only for `rest()`)
  depends on `mlos-fdt`. Moving `rest()` somewhere neutral would drop the
  build dependency too.

## For step `x86-virtio-blk`

- The virtio-mmio slots are at `0xfeb00000`+, far above the 1 GiB
  identity map. They must be mapped (uncached) before the driver touches
  them, or the first access faults.
- `mlos doctor` does not check `qemu-system-x86_64` (step `x86-gate`).
- This file becomes a column of `status.md`.
