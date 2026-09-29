//! Level-1 block descriptors: one level, one 1 GiB size.
//!
//! Invariant: `ATTR_*` index `regs::MAIR`, so the two must agree.
//! Design: docs/notes/mlos-mmu-aarch64.md.

/// Descriptor type: a block, not a table pointer or a page.
const BLOCK: u64 = 0b01;

/// Access flag. Must be set: with it clear the first access faults, and
/// at boot there is no handler.
const AF: u64 = 1 << 10;

/// Inner shareable, for normal memory.
const SH_INNER: u64 = 0b11 << 8;

/// `AttrIndx` 0: the `MAIR_EL1` slot holding Device-nGnRE.
const ATTR_DEVICE: u64 = 0 << 2;

/// `AttrIndx` 1: the `MAIR_EL1` slot holding Normal write-back.
const ATTR_NORMAL: u64 = 1 << 2;

/// Never execute, at either privilege level.
const XN: u64 = (1 << 53) | (1 << 54);

/// Output address field of a level-1 block: bits 47:30.
const ADDR_MASK: u64 = 0x0000_FFFF_C000_0000;

/// Size of one level-1 block.
pub const BLOCK_SIZE: u64 = 1 << 30;

/// A block of memory-mapped device registers: Device-nGnRE and
/// non-executable.
#[must_use]
pub const fn device(physical: u64) -> u64 {
    (physical & ADDR_MASK) | BLOCK | AF | ATTR_DEVICE | XN
}

/// A block of ordinary RAM: cacheable, shareable, executable.
#[must_use]
pub const fn normal(physical: u64) -> u64 {
    (physical & ADDR_MASK) | BLOCK | AF | ATTR_NORMAL | SH_INNER
}
