//! Throwing an object out.
//!
//! The other half of the fault path, and the half that did not exist
//! until M3 step 008: until a policy could decide what to throw away, a
//! full arena refused rather than choosing, and `Arena::place` bumped a
//! pointer that never came back.
//!
//! Its own module because `mlos-objman` was at four when it arrived.
//! That is also why the arena itself is now a sibling crate -- it knows
//! nothing about objects, which made it the separable half.

use mlos_abi::{Error, ObjectId, Result};
use mlos_events::Event;

use crate::Manager;

impl<const N: usize> Manager<'_, N> {
    /// Throws `id` out of the arena, giving its bytes back.
    ///
    /// The other half of the fault path, and the one that did not exist
    /// until M3 step 008. Everything here has to happen together or the
    /// system disagrees with itself: the bytes go back to the arena, the
    /// table stops calling the object resident, the residency counter
    /// comes down, and the event stream says so.
    ///
    /// Reads as `evicted` afterwards rather than `never`, and the
    /// difference matters: both have `resident_at == 0` and only the use
    /// count separates them -- an object thrown away is one a decision was
    /// made about, and one never wanted is not.
    ///
    /// `NoBudget` from the arena means the free list is full, and NOTHING
    /// is changed: the object stays resident rather than becoming memory
    /// the accounting has lost.
    pub fn evict(&mut self, id: ObjectId) -> Result<u32> {
        let meta = *self.table.get(id).ok_or(Error::BadObject)?;
        if meta.resident_at == 0 {
            return Err(Error::NotResident);
        }
        self.arena.release(meta.resident_at, meta.size)?;
        self.counters.resident(-i64::from(meta.size));
        self.events.record(Event::evicted(id, meta.owner, &meta));

        let gone = self.table.get_mut(id).ok_or(Error::BadObject)?;
        gone.resident_at = 0;
        gone.tier = meta.home;
        Ok(meta.size)
    }
}
