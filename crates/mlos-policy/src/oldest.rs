//! FIFO and LRU: the same policy, reading different clocks.
//!
//! One type rather than two, because that is what they are. Both evict
//! the resident object with the smallest tick; they differ only in which
//! tick they look at -- when the object arrived, or when it was last
//! wanted. Writing them as one makes the comparison between them exactly
//! a comparison of which clock matters, which is the only interesting
//! thing about it.
//!
//! Neither keeps a queue or a list. `docs/design.md` s.2 forbids a policy
//! state the table does not own, so both find their victim by scanning
//! for a minimum -- which is what the kernel would have to do over its own
//! table, and what known-next-use will do when it scans for a maximum.

use mlos_abi::ObjectId;
use mlos_objtab::ObjectMeta;

use crate::{Policy, Residency};

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
