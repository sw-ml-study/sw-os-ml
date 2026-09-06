//! Level-1 block descriptors.
//!
//! One level, one size. Every entry maps a 1 GiB block, which is why there
//! is no table-walking code here: 512 entries of 1 GiB cover the 512 GiB
//! of a 39-bit address space, and QEMU `virt` fits inside the first four.
//! Finer granularity arrives when something needs it (`lib.rs`).

/// Descriptor type: a block, not a table pointer or a page.
const BLOCK: u64 = 0b01;

/// Access flag. **Not optional.** With `AF` clear, the first access
/// through the entry takes an access-flag fault, and at boot there is no
/// handler -- the machine simply stops with no way to say why.
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

/// A block of memory-mapped device registers.
///
/// Device-nGnRE and non-executable. Mapping MMIO as normal memory would
/// let the core reorder, merge or speculatively issue accesses to it,
/// which is the difference between a driver that works and one that
/// appears to.
#[must_use]
pub const fn device(physical: u64) -> u64 {
    (physical & ADDR_MASK) | BLOCK | AF | ATTR_DEVICE | XN
}

/// A block of ordinary RAM: cacheable, shareable, executable.
///
/// Executable because the kernel image lives in one of these and we are
/// running out of it. Narrowing that to the image's own range is a job for
/// whoever first has a reason to care.
#[must_use]
pub const fn normal(physical: u64) -> u64 {
    (physical & ADDR_MASK) | BLOCK | AF | ATTR_NORMAL | SH_INNER
}
