# x86-64 status

What works on the x86-64 guest today (saga `mlos-x86-64`, `lanes/x86/`).
Kept apart from [status.md](status.md) while the aarch64 lane is editing
that file; the two merge when both lanes have, and the architecture
becomes a column there (step `x86-replay`).

## Boot

| | State |
|---|---|
| Machine | QEMU `microvm`, TCG, `-cpu max` |
| Boot protocol | PVH: ELF with a `XEN_ELFNOTE_PHYS32_ENTRY` note, loaded by `-kernel`, entered in 32-bit protected mode with `hvm_start_info` in `%ebx` |
| Entry | `mlos-hal-x86-64` `_start`: `.bss` zeroed, 64 KiB stack, 1 GiB identity map, long mode, `mlos_main(start_info)` |
| Identity map | 1 GiB pages when the CPU has `pdpe1gb` (`-cpu max`); 2 MiB pages otherwise (QEMU's default `qemu64` lacks it). The guest reports which |
| `mlos_main` | `mlos-kernel-x86-64`: opens COM1, prints the banner (`MLOS x86-64`) and what the entry found -- `EFER.LMA`, start-info magic, page size -- then echoes input until `Ctrl-D` and exits through `isa-debug-exit` with those facts as bits |
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
- `mlos doctor` does not check `qemu-system-x86_64` (step `x86-gate`).
- This file becomes a column of `status.md`.
