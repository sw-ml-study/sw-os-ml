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
| `mlos_main` | `mlos-kernel-x86-64`: checks `EFER.LMA`, the start-info magic and the page size, and reports them through `isa-debug-exit` |
| Console | none yet -- step `x86-console` |
| CLI | `mlos --arch x86-64 build`, `mlos --arch x86-64 run [tcg] [--capture S]`; no `--arch` is aarch64, unchanged |

## Verified

`cargo test -p mlos-cli -- --ignored` boots both paths (1 GiB and
2 MiB) under TCG on QEMU 8.2.2, Ubuntu 24.04. KVM: not attempted --
saga `mlos-two-hosts`.

## Known gaps, to fold in when the lanes merge

- `mlos --help` does not mention `--arch`; `mlos --arch x86-64 help` does.
  Left so the shared `USAGE` text is not edited by both lanes.
- `mlos doctor` does not check `qemu-system-x86_64` (step `x86-gate`).
- This file becomes a column of `status.md`.
