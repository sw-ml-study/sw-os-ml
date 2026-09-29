# mlos-hal-aarch64

aarch64 platform support: the entry point, the `Image` header, the image
extent, and the generic timer. Platform layer, below `mlos-hal`. Linked
from `crates/mlos-hal-aarch64/src/lib.rs`, `boot.rs` and `timer.rs`.

## Why `unsafe` lives here

This is the first code in the repo that must be right on real silicon,
and AGENTS.md ("Hard constraints") confines `unsafe` to the HAL and driver
crates. Every block names the invariant it relies on.

## Why the crate is empty on other architectures

`#![cfg(target_arch = "aarch64")]` makes the crate compile to nothing
elsewhere, so the workspace-wide gate (`cargo kclippy-x86`, `cargo
kbuild-x86`) can sweep every crate without a hand-maintained exclude
list. The crate says where it applies; a list in `.cargo/config.toml`
would say it somewhere else and then drift, which is exactly what
happened before this line existed: four crates were built for the bare
targets and never linted there.

## The `Image` header

The arm64 Linux `Image` header at offset 0 is not decoration. Step 004
found `x0` arriving as zero: QEMU jumps straight to an ELF's entry point,
skipping the arm64 boot protocol that puts the device tree pointer there.
It does place a device tree in RAM regardless (a probe found `d00dfeed`
at `0x4000_0000`), but finding it by scanning for a magic number is a
hack we would carry forever. The header is the documented way to be
handed it. It is wanted anyway: `Virtualization.framework` boots an
uncompressed raw arm64 image and nothing else (`docs/architecture.md`
s.8.1).

The layout is fixed by the protocol:

```text
  0  code0        branch past the header
  4  code1        0
  8  text_offset  where to load, relative to the base of DRAM
 16  image_size   memory footprint, including .bss and the stack
 24  flags        bit 0 endianness, bits 1-2 page size, bit 3 placement
 32  res2 res3 res4
 56  magic        "ARM\x64"
 60  res5         PE/COFF offset, unused
```

`text_offset` is `0x20_0000` and the placement flag is clear, so the
loader must put us at DRAM base + 2 MiB, which is the `0x4020_0000` the
linker script links for. Setting the "load me anywhere" flag instead
would let the loader pick, and position-dependent code linked at a fixed
address would then land somewhere it was not linked for.

## The entry point is naked

`primary_entry` is naked because a compiler-emitted prologue would push
to a stack that does not exist yet. In order it parks every core but the
boot core, installs the boot stack, zeroes `.bss`, and branches to
`mlos_main`. `x0` holds the device tree pointer and is never clobbered;
only `x1` and `x2` are scratch.

Secondary cores park (`wfe` loop) rather than spin into the kernel: MLOS
is single-core until there is a scheduler for them to enter, and a core
running the boot path twice corrupts what the first one built.

## The image extent

`extent` lives in `lib.rs` rather than its own module: it is one
function, and the crate is at the `sw-checklist` module gate (AGENTS.md:
design to the gate, not the line).

It exists because the device tree describes what the machine has and
knows nothing about what a loader put into it; the fact that the bottom
of RAM is occupied by us has to come from the linker script that placed
us there. It includes `.bss` and the boot stack, which occupy RAM but no
file bytes. Reporting only the file's worth would leave the stack looking
allocatable, which is a subtle way to hand out the ground you are
standing on.

The linker symbols are read with `&raw const` rather than a reference:
they have no valid value, only an address, and forming a `&u8` to them
would claim otherwise.

## `wait_for_interrupt` lives with the architecture

The architecture's answer to "nothing to do" is `wfi` here and `hlt` on
x86-64. It lives with the architecture rather than with the idle loop
that calls it, so the loop does not have to know which one it is on.

## The generic timer

`TIMER_PPI` is a fixed number from the Arm architecture rather than from
the device tree. The tree's `interrupts` property describes the same
thing, but PPI 30 is architectural for the non-secure EL1 physical timer,
and parsing three cells of interrupt specifier to rediscover it would be
ceremony, not portability.

`CNTP_TVAL_EL0` is a countdown, so writing it is both "when" and "start
counting". It is also how the interrupt is cleared: the timer asserts its
output for as long as the count is negative, so a handler that does not
rearm is called again immediately, forever.

The counter frequency is read from `CNTFRQ_EL0` rather than assumed. On
QEMU `virt` it is 62.5 MHz, but a hard-coded rate is a kernel that keeps
time on one board only.

## Lessons

**A prologue faults before any code retires.** Step 001 established the
naked entry empirically: the compiler's pushed `str x30, [sp, #-0x10]!`
faulted into the zero vector table at `VBAR_EL1 + 0x200` before any of
our code ran, under both TCG and HVF. Nothing at the entry may touch the
stack until `sp` is set.
