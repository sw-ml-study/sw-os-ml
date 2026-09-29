# mlos-hal-x86-64

The x86-64 entry point, and the few facts about the CPU only the
architecture can answer. Platform layer, the twin of `mlos-hal-aarch64`.
Linked from `crates/mlos-hal-x86-64/src/lib.rs`, `boot.rs`, `phys.rs`,
`port.rs` and `entry.s`.

## Why its own crate

`unsafe` is confined to the HAL and driver crates (AGENTS.md, "Hard
constraints"), and this is the x86-64 HAL: it owns the entry, the page
tables the entry builds, port I/O, MSRs, and the one place a
loader-supplied physical address becomes a reference. Everything above it
is safe code. The aarch64 twin has the same shape and the same `extent`
contract, so the kernel's two halves can be compared line for line.

The crate is empty anywhere but a bare x86-64 target. The `cfg` names
`target_os = "none"` as well as the architecture because a Linux x86-64
host is also `target_arch = "x86_64"`, and a 32-bit entry stub has no
business in a host build.

## PVH rather than firmware

The guest is QEMU `microvm` booted by PVH (`docs/plan.md`, saga
`mlos-x86-64`). PVH is the x86-64 counterpart of `-kernel Image` on
arm64: QEMU finds a `XEN_ELFNOTE_PHYS32_ENTRY` note in the ELF, loads the
segments at their physical addresses, and jumps to the note's address in
32-bit protected mode with paging off and an `hvm_start_info` pointer in
`%ebx`. No firmware and no image conversion, the same trade M1 made on
arm64. Everything a device tree tells the aarch64 kernel is read from
`start_info` later, in Rust; the entry reads nothing else.

## The boot map

`entry.s` zeroes `.bss` first, because the page tables and the stack live
there, then builds an identity map in two pieces. The first 1 GiB is RAM.
3 to 4 GiB is the device window, mapped with `PCD|PWT` so device
registers are never cached: the LAPIC (`0xfee00000`), the IOAPIC
(`0xfec00000`) and the virtio-mmio slots (`0xfeb00000`) live there. 1 to 3
GiB stays unmapped on purpose, so a stray pointer there still faults
rather than reading through to something.

The map uses 1 GiB pages when the CPU has `pdpe1gb`
(`CPUID.80000001h:EDX[26]`) and falls back to 512 2 MiB pages per
gigabyte otherwise. Which it was is reported (`gigabyte_pages`) rather
than hidden: page size changes TLB reach, and someone comparing `sweep`
timings across CPUs will eventually need to know. QEMU's default CPU lacks
`pdpe1gb`; `-cpu max` has it.

`long_mode` reads `EFER.LMA` back from the hardware rather than assuming
it from having got this far, for the same reason the banner reports it.

## Physical memory as slices

The PVH start info, its memory map and the command line are at physical
addresses the loader chose. Below 1 GiB an address is a pointer, because
the identity map says so; above it nothing is mapped, and `bytes` and
`c_str` return empty rather than fault. Parsing what is there is
`mlos-pvh`'s job, in safe code, and keeping the address-to-slice step here
is what lets every parsing rule be tested on the host.

`MAX_CMDLINE` (4096) is only a guard against a missing terminator walking
off through memory; QEMU's own limit is lower.

`peek` is a safe `fn` although it reads any address, because every
outcome is defined by the machine: mapped, it returns what is there;
unmapped, the CPU raises #PF and the trap reporter takes over. It exists
for `mem peek`, which is both a debugging read and the shell's way to
provoke a page fault on purpose, so the fault is the intended, reported
outcome and not undefined behaviour. The read is volatile and unaligned
so the compiler neither elides it nor assumes alignment.

## Port I/O is `unsafe fn`

`inb`, `outb` and `rdmsr` are `unsafe fn` because a port or MSR write can
do anything the hardware behind it does: reset the machine, remap memory.
The caller names the register and owns the consequence. `rdtsc` is safe:
at CPL 0 it only reads. Contrast `mlos-uart16550`, whose own port module
is safe because it is only ever handed a UART's registers.

## Sleeping without losing a wakeup

`wait_unless` makes the check of `pending` and the `hlt` atomic with
respect to interrupts: `cli` before the check, then `sti; hlt`. `sti`
holds interrupts off for exactly one more instruction, so an interrupt
that arrives during the check is delivered only once the CPU is already
in `hlt`, and wakes it. Without that shadow an interrupt landing between
the check and the `hlt` would be serviced and then slept through, and a
keystroke would wait for the next timer tick.

## Exiting with an answer

`qemu_exit` writes to QEMU's `isa-debug-exit` device at port `0xf4`, and
QEMU exits with status `(code << 1) | 1`. That is how a boot test gets a
real answer before there is a console to read: the kernel's report bits
come back as a process exit status. Without the device the write goes
nowhere and the CPU parks, which a test sees as a timeout. `mlos run`
attaches the device.

## Lessons

**Far jumps through memory.** The switch to long mode jumps through a
memory far pointer (`jmp fword ptr [mlos_long_mode_ptr]`) rather than
`push offset sym; retf`: LLVM's Intel-syntax parser encodes
`push offset sym` with a 16-bit immediate, which cannot hold an address
above 64 KiB. Found in `spikes/x86-skeleton`.
