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
use mlos_objtab::ObjectMeta;
use mlos_policy::Residency;

use crate::Manager;

/// The kernel's object table, as a policy is allowed to see it.
///
/// A scan of every slot, because that is what an open-addressed table can
/// offer: there is no cheaper way to enumerate what is resident, and a
/// policy that wanted one would be a policy the kernel could not host.
/// `mlos-sim` scans a `Vec` for the same reason -- the simulator was
/// built to the kernel's constraints rather than the other way round, so
/// that step 009 could not discover the interface was unimplementable
/// after every number existed.
pub struct Held<'a, const N: usize> {
    /// The table being looked at.
    pub table: &'a mlos_objtab::Table<N>,
    /// The acquire clock, which is what `now` means to a policy.
    pub clock: u32,
}

impl<const N: usize> Residency for Held<'_, N> {
    /// Every slot, not every object. A vacant one answers `None` and the
    /// policy skips it, which costs a compare and saves counting.
    fn len(&self) -> usize {
        N
    }

    fn at(&self, index: usize) -> Option<(ObjectId, ObjectMeta)> {
        match self.table.at(index) {
            Some((id, meta)) if meta.resident_at != 0 => Some((id, meta)),
            _ => None,
        }
    }

    /// The acquire clock, NOT the stream cursor.
    ///
    /// They are the same counter while a workload runs what it declared,
    /// and they are not while a shell pokes at objects out of order. The
    /// clock is the one that has to be here: `next_use` positions are
    /// one-based ticks, `used_tick` is a one-based tick, and a policy
    /// computing `at - now` against a zero-based cursor is off by one for
    /// every candidate. A uniform shift sounds harmless and is not --
    /// `evictability` divides by a per-object recovery cost, so shifting
    /// every distance by one reorders candidates with different costs.
    fn now(&self) -> u32 {
        self.clock
    }
}

impl<const N: usize> Manager<'_, N> {
    /// Evicts until `size` bytes fit, or until the policy stops naming
    /// victims.
    ///
    /// Asked one victim at a time, so a policy never has to know how much
    /// more room is needed -- only which single object it would give up
    /// next. The same shape `mlos-sim` uses, deliberately: the two have to
    /// make the same sequence of decisions or their counts cannot match.
    ///
    /// Stops on a refusal, on an eviction that fails, or when the
    /// LARGEST FREE RUN is big enough -- not when the total free is. After
    /// evictions those differ, and placing needs a contiguous run.
    /// `Occupancy::fits` is that test, and `mlos-sim` asks the same one.
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
        self.evictions += 1;
        self.events.record(Event::evicted(id, meta.owner, &meta));

        let gone = self.table.get_mut(id).ok_or(Error::BadObject)?;
        gone.resident_at = 0;
        gone.tier = meta.home;
        Ok(meta.size)
    }
}
