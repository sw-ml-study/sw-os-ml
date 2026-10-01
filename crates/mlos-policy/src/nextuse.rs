//! Known-next-use: evict what is wanted furthest away per unit of
//! recovery cost.
//!
//! Invariant: a probabilistic horizon is discounted against a known one,
//! and `Never` is a chosen horizon, not infinity. Design and history:
//! docs/notes/mlos-policy.md.

use mlos_abi::ObjectId;
use mlos_objtab::{NextUse, ObjectMeta};

use crate::{Policy, Residency};

/// Evicts whatever is wanted furthest away per unit of recovery cost.
pub struct KnownNextUse;

/// The policy, as a value to pass around beside `FIFO` and `LRU`.
pub const NEXT_USE: KnownNextUse = KnownNextUse;

/// The horizon given to an object nothing has declared a future for.
/// Outranks any real distance; `UNDECLARED * SCALE` must not overflow.
const UNDECLARED: u64 = 1 << 40;

/// How much a probabilistic horizon is discounted against a known one.
const HINT: u64 = 2;

/// Fixed-point scale for the distance-per-cost ratio. Must exceed the
/// recovery costs in play (nanoseconds, millions) or the ratio truncates
/// to zero for every expensive object.
const SCALE: u64 = 1_000_000;

impl Policy for KnownNextUse {
    fn name(&self) -> &'static str {
        "next-use"
    }

    fn victim(&self, resident: &dyn Residency, _wanting: &ObjectMeta) -> Option<ObjectId> {
        let (mut best, now): (Option<(ObjectId, u64)>, u32) = (None, resident.now());
        for index in 0..resident.len() {
            let Some((id, meta)) = resident.at(index) else {
                continue;
            };
            let score = evictability(&meta, now);
            // Ties go to the lower id, so the answer does not depend on
            // the order the residents were offered in.
            if best.is_none_or(|(held_id, held)| score > held || (score == held && id < held_id)) {
                best = Some((id, score));
            }
        }
        best.map(|(id, _)| id)
    }
}

/// How good a victim this is: higher is more evictable. Distance per unit
/// of recovery cost, where recovery is the cheaper of reload and
/// recompute.
fn evictability(meta: &ObjectMeta, now: u32) -> u64 {
    let (reload, recompute) = (meta.reload_cost.0, meta.recompute_cost.0);
    let recovery = match reload.min(recompute) {
        0 => 1,
        cost => u64::from(cost),
    };
    // Evicting an object k sessions hold costs k reloads, so it is k times
    // dearer to give up. Zero holders count as one.
    let holders = u64::from(meta.share_count.max(1));
    horizon(meta.next_use, now).saturating_mul(SCALE) / recovery.saturating_mul(holders)
}

/// How far away the next use is, in steps, with a hint discounted.
fn horizon(next: NextUse, now: u32) -> u64 {
    match next {
        NextUse::Never => UNDECLARED,
        // Saturating: a position already passed means wanted now, which
        // is a distance of zero and the worst possible victim.
        NextUse::At(position) => u64::from(position.saturating_sub(now)),
        // Expected about 1/p steps away, then discounted: a hint is not
        // knowledge.
        NextUse::Probability(chance) => match chance {
            0 => UNDECLARED / HINT,
            odds => u64::from(u16::MAX) / u64::from(odds) / HINT,
        },
    }
}
