//! The free list: runs of bytes nothing is using.
//!
//! Invariant: sorted by address, and merged with its neighbours the
//! moment a run is returned. Design: docs/notes/mlos-arena.md.

use crate::{Arena, Occupancy};

impl Occupancy {
    /// Whether a placement of `size` bytes would succeed right now,
    /// rounded as [`Arena::place`] rounds. The one test both the kernel's
    /// and the simulator's eviction loops stop on.
    #[must_use]
    pub const fn fits(&self, size: u32) -> bool {
        self.largest >= size.next_multiple_of(Arena::ALIGN) as u64
    }
}

/// A run of free bytes.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Hole {
    /// Offset from the region's base.
    pub at: usize,
    /// How many bytes.
    pub len: usize,
}

/// Merges the hole at `index` with whichever neighbours it touches,
/// later neighbour first so the index stays valid.
pub fn merge(free: &mut [Hole], holes: &mut usize, index: usize) {
    let touches_next = index + 1 < *holes && free[index].at + free[index].len == free[index + 1].at;
    if touches_next {
        free[index].len += free[index + 1].len;
        free.copy_within(index + 2..*holes, index + 1);
        *holes -= 1;
    }
    let touches_previous = index > 0 && free[index - 1].at + free[index - 1].len == free[index].at;
    if touches_previous {
        free[index - 1].len += free[index].len;
        free.copy_within(index + 1..*holes, index);
        *holes -= 1;
    }
}
