//! The free list: runs of bytes nothing is using.
//!
//! Sorted by address and merged the moment one is returned, which is what
//! keeps it short -- a thousand evictions of adjacent tiles leave one
//! hole rather than a thousand, and a list that grew instead would run
//! out of its fixed capacity on a workload that never fragmented.

use crate::{Arena, Occupancy};

impl Occupancy {
    /// Whether a placement of `size` bytes would succeed right now.
    ///
    /// THE question a caller making room has to ask, and the one place
    /// it is answered: the kernel's `make_room` and the simulator's
    /// eviction loop both stop when this says yes, so the two cannot
    /// disagree about when enough has been thrown away. Rounded the way
    /// [`Arena::place`] rounds, because a run that fits the size and not
    /// its alignment is a run the placement would still refuse.
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

/// Merges the hole at `index` with whichever neighbours it touches.
///
/// The later neighbour first: merging with the earlier one shifts the
/// array down, and doing that before looking right would leave the right
/// index pointing at a different hole.
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
