//! One replay in progress: the per-access step.
//!
//! Invariant: `make_room` is the kernel's loop, stopping on the same
//! `Occupancy::fits` for the same three reasons. Design:
//! docs/notes/mlos-sim.md.

use mlos_abi::{ObjectClass, ObjectId};
use mlos_objtab::{NextUse, ObjectMeta};
use mlos_policy::Policy;

use crate::{Model, Outcome, Resident};

/// What does not change across a replay.
pub struct Run<'a> {
    /// Where sizes and costs come from.
    pub model: &'a dyn Model,
    /// The policy under test.
    pub policy: &'a dyn Policy,
    /// Where each object is next wanted.
    pub seen: &'a crate::Foresight,
}

impl Run<'_> {
    /// One access.
    pub fn step(&self, resident: &mut Resident<'_>, outcome: &mut Outcome, id: ObjectId, now: u32) {
        let Some(meta) = self.model.meta(id) else {
            // Not a refusal: the trace and the model disagree.
            outcome.mismatched += 1;
            return;
        };
        let next = crate::wanted_at(self.seen, now);
        let weight = id.class() == Some(ObjectClass::WeightTile);
        if resident.touch(id, now, next) {
            outcome.hits += 1;
            outcome.weights_applied += u64::from(meta.size) * u64::from(weight);
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
        // Attempted however the loop ended; a failed placement is the
        // refusal.
        if resident.insert(id, meta, now, next).is_err() {
            out.refused += 1;
            return;
        }
        out.reads += 1;
        out.bytes += u64::from(meta.size);
        out.cost += u64::from(meta.reload_cost.0);
        if id.class() == Some(ObjectClass::WeightTile) {
            out.weights_applied += u64::from(meta.size);
            out.weights_read += u64::from(meta.size);
        }
    }

    /// Evicts one victim at a time until `wanting` would fit, the policy
    /// names no victim, or the arena refuses a release.
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
