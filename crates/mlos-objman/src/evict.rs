//! Throwing an object out: the other half of the fault path.
//!
//! Invariant: the arena, the table, the counters and the event ring
//! change together or not at all. Design and history:
//! docs/notes/mlos-objman.md.

use mlos_abi::{Error, ObjectId, Result};
use mlos_events::Event;
use mlos_objtab::ObjectMeta;
use mlos_policy::Residency;

use crate::Manager;

/// The kernel's object table, as a policy is allowed to see it: a scan
/// of every slot, which is all an open-addressed table can offer.
pub struct Held<'a, const N: usize> {
    /// The table being looked at.
    pub table: &'a mlos_objtab::Table<N>,
    /// The acquire clock, which is what `now` means to a policy.
    pub clock: u32,
}

impl<const N: usize> Residency for Held<'_, N> {
    /// Every slot, not every object; a vacant one answers `None` at `at`.
    fn len(&self) -> usize {
        N
    }

    fn at(&self, index: usize) -> Option<(ObjectId, ObjectMeta)> {
        match self.table.at(index) {
            Some((id, meta)) if meta.resident_at != 0 => Some((id, meta)),
            _ => None,
        }
    }

    /// The acquire clock, not the stream cursor: `next_use` and
    /// `used_tick` are one-based ticks on this counter, and a zero-based
    /// cursor would put every distance off by one.
    fn now(&self) -> u32 {
        self.clock
    }
}

impl<const N: usize> Manager<'_, N> {
    /// Evicts one victim at a time until `wanting.size` fits in a single
    /// free run (`Occupancy::fits`, the same test `mlos-sim` asks), or
    /// until the policy stops naming victims.
    pub(crate) fn make_room(&mut self, wanting: &ObjectMeta) {
        while !self.arena.occupancy().fits(wanting.size) {
            let held = Held {
                table: &self.table,
                clock: self.clock,
            };
            let Some(policy) = self.policy else {
                return; // demand paging: refuse rather than choose
            };
            let Some(victim) = policy.victim(&held, wanting) else {
                return;
            };
            if self.evict(victim).is_err() {
                return; // nothing more can be given back
            }
        }
    }

    /// Throws `id` out of the arena, giving its bytes back and restoring
    /// its home tier. `NotResident` if it was not here. `NoBudget` if the
    /// free list is full, and then nothing changes: the object stays
    /// resident.
    pub fn evict(&mut self, id: ObjectId) -> Result<u32> {
        let meta = *self.table.get(id).ok_or(Error::BadObject)?;
        if meta.resident_at == 0 {
            return Err(Error::NotResident);
        }
        self.arena.release(meta.resident_at, meta.size)?;
        self.counters.resident(-i64::from(meta.size));
        self.evictions += 1;
        self.events.record(Event::evicted(id, meta.owner, &meta));

        let gone = self.table.get_mut(id).ok_or(Error::BadObject)?;
        gone.resident_at = 0;
        gone.tier = meta.home;
        Ok(meta.size)
    }
}
