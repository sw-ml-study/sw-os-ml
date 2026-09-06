//! What the loader and firmware told us about this machine.
//!
//! Produced once, by architecture-specific code, from whatever the
//! platform offers -- a device tree on aarch64, ACPI tables on x86-64 --
//! and immutable afterwards. The kernel never learns which it was.

/// What a region of physical memory is good for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MemoryKind {
    /// Free for the kernel to allocate from.
    Usable,
    /// Holds the kernel image. Not allocatable, but not reclaimable
    /// either -- we are running out of it.
    Kernel,
    /// Firmware structures, the device tree, loader scratch. Reclaimable
    /// once parsed, which is worth doing: on a machine sized by its
    /// residency budget, tens of megabytes is real.
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

/// The machine, as described at boot.
///
/// `regions` is a borrowed slice rather than an owned collection because
/// there is no allocator yet when this is built -- the memory map is what
/// the allocator is built *from*.
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

    /// Total allocatable bytes.
    ///
    /// The number every residency budget in `docs/PRD.md` is a fraction
    /// of, so it is computed once from the map rather than guessed.
    #[must_use]
    pub fn usable_bytes(&self) -> u64 {
        self.regions
            .iter()
            .filter(|r| r.kind == MemoryKind::Usable)
            .map(|r| r.len)
            .sum()
    }
}
