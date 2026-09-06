//! Address translation, described without naming a page size.
//!
//! Sizes and level counts are the architecture's business. This interface
//! speaks in byte ranges; an implementation is free to satisfy a range
//! with 4 KiB leaves, 2 MiB blocks, or whatever its tables offer.

/// A physical address.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub struct PhysAddr(pub u64);

/// A virtual address.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub struct VirtAddr(pub u64);

/// What a mapping permits.
///
/// Plain booleans rather than a bitfield: the bit encodings differ per
/// architecture, so encoding them here would put architecture above the
/// HAL, which is exactly what this crate exists to prevent.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct PageFlags {
    /// Readable.
    pub read: bool,
    /// Writable.
    pub write: bool,
    /// Executable.
    pub execute: bool,
    /// Device memory: uncached, and not speculatively accessed. Wrong
    /// memory type on an MMIO range is the classic way a driver appears
    /// to work until it does not.
    pub device: bool,
}

/// Why a mapping request could not be satisfied.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MapError {
    /// The address or length is not aligned to anything the tables can
    /// express.
    Misaligned,
    /// The range overlaps an existing mapping.
    Occupied,
    /// No memory left to allocate a table level from.
    OutOfTables,
}

/// The architecture's page tables.
pub trait PageTable {
    /// Maps `len` bytes at `va` onto physical memory at `pa`.
    ///
    /// # Safety
    ///
    /// The caller guarantees the physical range is owned by whoever is
    /// asking, and that creating this alias does not violate an
    /// invariant some other mapping relies on. Nothing here can check
    /// that.
    unsafe fn map(
        &mut self,
        va: VirtAddr,
        pa: PhysAddr,
        len: usize,
        flags: PageFlags,
    ) -> Result<(), MapError>;

    /// Removes a mapping and invalidates whatever caching the
    /// architecture does for it.
    ///
    /// # Safety
    ///
    /// The caller guarantees nothing holds a reference into the range.
    unsafe fn unmap(&mut self, va: VirtAddr, len: usize) -> Result<(), MapError>;

    /// Installs these tables as the ones the CPU translates through.
    ///
    /// # Safety
    ///
    /// The tables must map, at minimum, the currently executing code and
    /// its stack. Getting this wrong faults on the instruction after the
    /// switch, with no way to report it.
    unsafe fn activate(&self);
}
