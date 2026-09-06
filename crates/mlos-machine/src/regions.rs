//! A fixed-capacity physical memory map.
//!
//! Fixed because this is built before there is an allocator -- the map is
//! what the allocator gets built *from*.

use mlos_hal::{MemoryKind, MemoryRegion};

/// How many regions the map can hold.
///
/// QEMU `virt` reports one DRAM region, and carving the kernel image and
/// the device tree blob out of it can turn that one into five. Sixteen
/// leaves room for a machine with a split map without pretending this is
/// a growable collection.
pub const MAX_REGIONS: usize = 16;

/// The physical memory map.
#[derive(Clone, Copy)]
pub struct Regions {
    items: [MemoryRegion; MAX_REGIONS],
    len: usize,
}

impl Regions {
    /// An empty map.
    #[must_use]
    pub const fn new() -> Self {
        let empty = MemoryRegion {
            base: 0,
            len: 0,
            kind: MemoryKind::Reserved,
        };
        Self {
            items: [empty; MAX_REGIONS],
            len: 0,
        }
    }

    /// Appends a region. Returns `false` if the map is full, which drops
    /// the region rather than growing silently or panicking at boot.
    pub fn push(&mut self, region: MemoryRegion) -> bool {
        let Some(slot) = self.items.get_mut(self.len) else {
            return false;
        };
        *slot = region;
        self.len += 1;
        true
    }

    /// The regions recorded so far.
    #[must_use]
    pub fn as_slice(&self) -> &[MemoryRegion] {
        &self.items[..self.len]
    }
}

impl Default for Regions {
    fn default() -> Self {
        Self::new()
    }
}
