//! One replay in progress.
//!
//! The per-access step, which is where the whole subject lives: an
//! acquire either finds its object, or has to make room for it, or cannot
//! be served at all. Everything the comparison measures is decided here.

use mlos_abi::ObjectId;
use mlos_objtab::{NextUse, ObjectMeta};
use mlos_policy::Policy;

use crate::{Model, Outcome, Resident};

/// What does not change across a replay.
pub struct Run<'a> {
    /// Where sizes and costs come from.
    pub model: &'a dyn Model,
    /// The policy under test.
    pub policy: &'a dyn Policy,
    /// Where each object is next wanted, which a declared stream would
    /// tell a kernel and a whole trace tells a simulator.
    pub seen: &'a crate::Foresight,
}

impl Run<'_> {
    /// One access.
    pub fn step(&self, resident: &mut Resident<'_>, outcome: &mut Outcome, id: ObjectId, now: u32) {
        let Some(meta) = self.model.meta(id) else {
            // The trace names an object this model does not have, so they
            // are not the same model. Counting it as an ordinary refusal
            // would hide a mismatched pair behind a plausible number.
            outcome.mismatched += 1;
            return;
        };
        let next = crate::wanted_at(self.seen, now);
        if resident.touch(id, now, next) {
            outcome.hits += 1;
            return;
        }
        self.fetch(resident, outcome, (id, meta), (now, next));
    }

    /// A miss: make room if the policy will give any, then place.
    fn fetch(
        &self,
        resident: &mut Resident<'_>,
        out: &mut Outcome,
        want: (ObjectId, ObjectMeta),
        when: (u32, NextUse),
    ) {
        let (id, meta) = want;
        let (now, next) = when;
        self.make_room(resident, out, &meta);
        // Attempted regardless of how the loop ended -- an arena nothing
        // was evicted from may still fit -- and failing IS the refusal,
        // which is what the kernel reports for a placement that could not
        // be made.
        if resident.insert(id, meta, now, next).is_err() {
            out.refused += 1;
            return;
        }
        out.reads += 1;
        out.bytes += u64::from(meta.size);
        out.cost += u64::from(meta.reload_cost.0);
    }

    /// Evicts until `wanting` would fit, or until it cannot go on.
    ///
    /// The same loop as `Manager::make_room`, asking the same arena the
    /// same question, and stopping for the same three reasons: the policy
    /// names no victim, the arena will not take one back, or a run large
    /// enough has opened up. One victim at a time, so a policy never has
    /// to know how much more room is needed -- only which single object
    /// it would give up next.
    fn make_room(&self, resident: &mut Resident<'_>, out: &mut Outcome, wanting: &ObjectMeta) {
        while !resident.arena.occupancy().fits(wanting.size) {
            let Some(victim) = self.policy.victim(resident, wanting) else {
                return;
            };
            if resident.remove(victim).is_err() {
                return;
            }
            out.evicted += 1;
        }
    }
}
