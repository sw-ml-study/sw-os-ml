//! What the loader and firmware told us about this machine.
//!
//! Invariant: produced once by architecture-specific code and immutable
//! afterwards; the kernel never learns which firmware described it.
//! Design: docs/notes/mlos-hal.md.

/// What a region of physical memory is good for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MemoryKind {
    /// Free for the kernel to allocate from.
    Usable,
    /// Holds the kernel image. Not allocatable, not reclaimable.
    Kernel,
    /// Firmware structures, the device tree, loader scratch. Reclaimable
    /// once parsed.
    Reclaimable,
    /// Memory-mapped device registers. Never allocate, never cache.
    Device,
    /// Present but unusable.
    Reserved,
}

/// One contiguous run of physical memory.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MemoryRegion {
    /// First byte.
    pub base: u64,
    /// Length in bytes.
    pub len: u64,
    /// What it is good for.
    pub kind: MemoryKind,
}

/// The machine, as described at boot. Borrows its map: there is no
/// allocator yet to own one.
#[derive(Clone, Copy, Debug)]
pub struct BootInfo<'a> {
    /// The physical memory map, in ascending address order.
    pub regions: &'a [MemoryRegion],
    /// How many CPUs the platform reported.
    pub cpu_count: u32,
}

impl<'a> BootInfo<'a> {
    /// The machine, from a memory map and a CPU count.
    #[must_use]
    pub const fn new(regions: &'a [MemoryRegion], cpu_count: u32) -> Self {
        Self { regions, cpu_count }
    }

    /// Total allocatable bytes: the sum of every `Usable` region.
    #[must_use]
    pub fn usable_bytes(&self) -> u64 {
        self.regions
            .iter()
            .filter(|r| r.kind == MemoryKind::Usable)
            .map(|r| r.len)
            .sum()
    }
}
