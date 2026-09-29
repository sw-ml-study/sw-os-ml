# mlos-mmu-aarch64

aarch64 boot translation tables: an identity map from a single level-1
table of 1 GiB blocks, then the MMU switched on. Platform layer. Linked
from `crates/mlos-mmu-aarch64/src/lib.rs`, `descriptor.rs` and `regs.rs`.

## Why the crate is empty on other architectures

`#![cfg(target_arch = "aarch64")]` makes the crate compile to nothing
elsewhere, so the workspace-wide gate can sweep every crate without a
hand-maintained exclude list. The crate says where it applies; a list in
`.cargo/config.toml` would say it somewhere else and then drift, which is
what happened before the line existed.

## Why an identity map

Virtual address equals physical because the code doing the switching
must keep running across it: with any other mapping, the instruction
after `isb` is somewhere else.

## Why this is not a `PageTable` implementation

`mlos_hal::PageTable` describes arbitrary byte-range mapping, which needs
a multi-level walker, table allocation and block splitting. Nothing needs
that yet (the object manager is M2, `docs/plan.md`), and writing a
general mapper before there is a caller to shape it means writing the
wrong one. What exists here is what boot needs and no more.

## The shape of QEMU `virt`

Everything below 1 GiB is device memory on `virt` (flash, GIC, PL011,
virtio-mmio, PCIe ECAM); RAM starts at `0x4000_0000`. The map reflects
that: block 0 is device, and RAM blocks come from the device tree rather
than from an assumption about how much there is. Blocks not covered by
any region are left invalid, so a stray access to memory the device tree
never claimed faults instead of quietly succeeding against nothing.

## One level, one size

Every entry maps a 1 GiB block, which is why there is no table-walking
code: 512 entries of 1 GiB cover the 512 GiB of a 39-bit address space,
and `virt` fits inside the first four. `T0SZ` of 25 in `TCR_EL1` gives
that 39-bit space, whose initial lookup is at level 1, which is exactly
why there is no level-0 table. Finer granularity arrives when something
needs it.

## The table is an `UnsafeCell`, not a `static mut`

`TTBR0_EL1` requires 4 KiB alignment, hence `#[repr(C, align(4096))]`.
`UnsafeCell` rather than `static mut` because the 2024 edition rejects
references to the latter, correctly, since nothing else stops two of them
existing. The `Sync` impl is sound because the table is written exactly
once, on the boot core before any other core leaves its park loop, and is
read-only to hardware afterwards.

## Descriptor bits

The access flag `AF` is not optional. With it clear, the first access
through the entry takes an access-flag fault, and at boot there is no
handler: the machine simply stops with no way to say why.

Device blocks are Device-nGnRE and non-executable. Mapping MMIO as normal
memory would let the core reorder, merge or speculatively issue accesses
to it, which is the difference between a driver that works and one that
appears to.

Normal blocks are cacheable, inner shareable, and executable, because the
kernel image lives in one of them and we are running out of it. Narrowing
execute permission to the image's own range is a job for whoever first
has a reason to care.

`MAIR_EL1` attribute 0 is Device-nGnRE (`0x04`) and attribute 1 is Normal
write-back read/write-allocate (`0xff`). `descriptor::ATTR_*` index into
this, so the two must agree.

## `TCR_EL1`

`IPS` is read from `ID_AA64MMFR0_EL1.PARange` rather than assumed.
Programming an intermediate physical size the implementation does not
support is architecturally unpredictable, and "unpredictable" on the
instruction after the MMU comes on is not a failure anyone can debug.
`TG0` is left zero because `0b00` is the 4 KiB granule; the field is
absent from the expression rather than forgotten.

## The barriers are the substance

In `install`, `dsb ishst` publishes the table writes before anything can
walk them; the `isb` makes the new control registers visible to
instruction fetch; the `tlbi vmalle1` removes anything cached from before
we existed. Dropping any of them yields a machine that boots on one host
and hangs on another. The sequence follows the Arm ARM's requirements for
programming the EL1&0 regime.

`is_enabled` reads `SCTLR_EL1.M` back so a claim that the MMU is on can
be checked rather than inferred from the fact that we are still running.
