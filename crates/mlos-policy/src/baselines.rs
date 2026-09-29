//! The three baselines an ordinary operating system would bring: demand
//! paging, FIFO and LRU.
//!
//! Invariant: none keeps a queue or a list; each scans the table's own
//! ticks for a minimum. Design: docs/notes/mlos-policy.md.

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

/// First in, first out: evict whatever arrived earliest, by `placed_tick`.
pub const FIFO: Oldest = Oldest {
    name: "fifo",
    tick: |meta| meta.placed_tick,
};

/// Least recently used: evict whatever has gone longest unwanted, by
/// `used_tick`.
pub const LRU: Oldest = Oldest {
    name: "lru",
    tick: |meta| meta.used_tick,
};

impl Policy for Oldest {
    fn name(&self) -> &'static str {
        self.name
    }

    /// The smallest tick in the resident set; ties go to the lower id.
    fn victim(&self, resident: &dyn Residency, _wanting: &ObjectMeta) -> Option<ObjectId> {
        let mut oldest: Option<(ObjectId, u32)> = None;
        for index in 0..resident.len() {
            let Some((id, meta)) = resident.at(index) else {
                continue;
            };
            let tick = (self.tick)(&meta);
            // Ticks are unique, so the tie-break never fires; it is written
            // so every policy answers independently of enumeration order.
            if oldest.is_none_or(|(held_id, held)| tick < held || (tick == held && id < held_id)) {
                oldest = Some((id, tick));
            }
        }
        oldest.map(|(id, _)| id)
    }
}
