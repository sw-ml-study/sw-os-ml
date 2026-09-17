//! The three an ordinary operating system would bring.
//!
//! Together because they are one thing: the numbers known-next-use has to
//! beat. Demand paging is the floor -- it never evicts, it refuses, and
//! what refusing costs is part of what the comparison measures. FIFO and
//! LRU are the two a page-based kernel actually ships.
//!
//! FIFO and LRU are ONE TYPE reading different clocks, because that is
//! what they are. Both evict the resident object with the smallest tick;
//! they differ only in whether that tick is when the object arrived or
//! when it was last wanted. Written as one, the comparison between them
//! is exactly a comparison of which clock matters -- and they come out
//! identical whenever nothing is reused, because then the two clocks are
//! the same clock.
//!
//! None of them keeps a queue or a list. `docs/design.md` s.2 forbids
//! policy state the table does not own, so each finds its victim by
//! scanning for a minimum -- which is what the kernel would have to do
//! over its own table, and what known-next-use does when it scans for a
//! maximum.

use mlos_abi::ObjectId;
use mlos_objtab::ObjectMeta;

use crate::{Policy, Residency};

/// Evicts nothing, ever.
pub struct Demand;

impl Policy for Demand {
    fn name(&self) -> &'static str {
        "demand"
    }

    fn victim(&self, _resident: &dyn Residency, _wanting: &ObjectMeta) -> Option<ObjectId> {
        None
    }
}

/// Evicts whichever resident object has the smallest tick.
pub struct Oldest {
    /// What to call it.
    name: &'static str,
    /// Which clock to read.
    tick: fn(&ObjectMeta) -> u32,
}

/// First in, first out: evict whatever arrived earliest.
///
/// Knows nothing about use. An object fetched once at the start and
/// wanted on every access since is still the first thing it throws away,
/// which is the failure LRU exists to fix.
pub const FIFO: Oldest = Oldest {
    name: "fifo",
    tick: |meta| meta.placed_tick,
};

/// Least recently used: evict whatever has gone longest unwanted.
///
/// The one an ordinary operating system brings, and a good guess whenever
/// the recent past predicts the near future. A cyclic sweep is where that
/// stops being true: the object LRU just used is the one that will not be
/// wanted again until the cycle comes round, so it evicts precisely what
/// it is about to need.
pub const LRU: Oldest = Oldest {
    name: "lru",
    tick: |meta| meta.used_tick,
};

impl Policy for Oldest {
    fn name(&self) -> &'static str {
        self.name
    }

    /// The smallest tick in the resident set.
    ///
    /// Ties go to the earlier index, which makes the choice deterministic
    /// without making it meaningful: two objects with the same tick are
    /// genuinely indistinguishable to this policy, and a replay still has
    /// to give the same answer twice.
    fn victim(&self, resident: &dyn Residency, _wanting: &ObjectMeta) -> Option<ObjectId> {
        let mut oldest: Option<(ObjectId, u32)> = None;
        for index in 0..resident.len() {
            let Some((id, meta)) = resident.at(index) else {
                continue;
            };
            let tick = (self.tick)(&meta);
            if oldest.is_none_or(|(_, held)| tick < held) {
                oldest = Some((id, tick));
            }
        }
        oldest.map(|(id, _)| id)
    }
}
