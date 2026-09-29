//! A fixed-capacity physical memory map.
//!
//! Invariant: built before there is an allocator, so it never grows; a
//! region that does not fit is dropped, never silently kept. Design:
//! docs/notes/mlos-machine.md.

use mlos_hal::{MemoryKind, MemoryRegion};

/// How many regions the map can hold.
pub const MAX_REGIONS: usize = 16;

/// The physical memory map.
#[derive(Clone, Copy)]
pub struct Regions {
    items: [MemoryRegion; MAX_REGIONS],
    len: usize,
}

impl Regions {
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

    /// Appends every `(base, len)` a `reg` decoder yields, as usable
    /// memory, stopping at the first pair it cannot produce or the first
    /// that does not fit.
    pub fn extend_usable(&mut self, pair: &impl Fn(usize) -> Option<(u64, u64)>) {
        for index in 0.. {
            let Some((base, len)) = pair(index) else {
                return;
            };
            if !self.push(MemoryRegion {
                base,
                len,
                kind: MemoryKind::Usable,
            }) {
                return; // map full; keep the regions we already have
            }
        }
    }

    /// The regions recorded so far.
    #[must_use]
    pub fn as_slice(&self) -> &[MemoryRegion] {
        &self.items[..self.len]
    }
}

impl Default for Regions {
    /// An empty map.
    fn default() -> Self {
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
}
