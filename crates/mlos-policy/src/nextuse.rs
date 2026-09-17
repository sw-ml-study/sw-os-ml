//! Known-next-use: evict what is wanted furthest away.
//!
//! The policy this project exists to test. Belady's rule is normally
//! unimplementable because it needs the future; a dense transformer hands
//! it over, because the order it will read its own weights follows from
//! its own structure.
//!
//! Three things it must get right that a naive reading would not.
//!
//! **Cost is half the decision.** Belady's rule assumes every miss costs
//! the same, which is true for pages and false for everything MLOS holds.
//! A weight tile cannot be recomputed at any price; an activation is
//! cheaper to rebuild than to keep. The furthest-away object is the wrong
//! victim if it is also the dearest to get back, so what is maximised is
//! **distance per unit of recovery cost** rather than distance.
//!
//! **A guess is not knowledge.** `NextUse::Distance` is exact -- a
//! declared stream said so. `NextUse::Probability` is a router's
//! distribution, and acting on it as though it were a distance would
//! licence evicting something the system was merely unsure about as
//! though it knew. Both are turned into a horizon, and the probabilistic
//! one is then HALVED, so a hint has to be twice as good as knowledge
//! before it wins. That is the discount made explicit rather than left in
//! a comment.
//!
//! **`Never` is not infinity.** An object nothing has declared a future
//! for is the obvious victim, and it is obvious because nothing KNOWS
//! about it -- not because nothing will want it. It gets the furthest
//! horizon, and that is a decision with a reason rather than a maximum.

use mlos_abi::ObjectId;
use mlos_objtab::{NextUse, ObjectMeta};

use crate::{Policy, Residency};

/// Evicts whatever is wanted furthest away per unit of recovery cost.
pub struct KnownNextUse;

/// The policy, as a value to pass around beside `FIFO` and `LRU`.
pub const NEXT_USE: KnownNextUse = KnownNextUse;

/// The horizon given to an object nothing has declared a future for.
///
/// Large enough to outrank any real distance in this workload and far
/// enough from `u32::MAX` that multiplying by a scale cannot overflow.
const UNDECLARED: u64 = 1 << 40;

/// How much a probabilistic horizon is discounted against a known one.
const HINT: u64 = 2;

/// Fixed-point scale for the distance-per-cost ratio.
///
/// Recovery costs are nanoseconds and run to millions, so a smaller scale
/// truncates the whole ratio to zero for anything expensive: the first
/// version used a thousand, every weight tile nearer than four thousand
/// steps scored exactly zero, and the policy was choosing between ties by
/// table position. Large enough that the distance still resolves after
/// the division, small enough that the multiply cannot overflow.
const SCALE: u64 = 1_000_000;

impl Policy for KnownNextUse {
    fn name(&self) -> &'static str {
        "next-use"
    }

    fn victim(&self, resident: &dyn Residency, _wanting: &ObjectMeta) -> Option<ObjectId> {
        let mut best: Option<(ObjectId, u64)> = None;
        for index in 0..resident.len() {
            let Some((id, meta)) = resident.at(index) else {
                continue;
            };
            let score = evictability(&meta);
            if best.is_none_or(|(_, held)| score > held) {
                best = Some((id, score));
            }
        }
        best.map(|(id, _)| id)
    }
}

/// How good a victim this is: higher is more evictable.
///
/// Distance per unit of recovery cost, so an object that is far away but
/// ruinous to get back can still be worth keeping over one that is nearer
/// and cheap.
///
/// The cost is the CHEAPER of fetching it again and rebuilding it.
/// `CostNs::IMPOSSIBLE` is what weights carry for recompute -- there is no
/// computation that produces a trained weight -- so taking the minimum is
/// what makes an activation, cheap to rebuild and expensive to store, a
/// better victim than a tile at the same distance.
fn evictability(meta: &ObjectMeta) -> u64 {
    let (reload, recompute) = (meta.reload_cost.0, meta.recompute_cost.0);
    let recovery = match reload.min(recompute) {
        0 => 1,
        cost => u64::from(cost),
    };
    horizon(meta.next_use).saturating_mul(SCALE) / recovery
}

/// How far away the next use is, in steps, with a hint discounted.
fn horizon(next: NextUse) -> u64 {
    match next {
        NextUse::Never => UNDECLARED,
        NextUse::Distance(steps) => steps as u64,
        // A router's distribution, not a distance. An expert wanted with
        // probability p each step is expected about 1/p steps away -- and
        // then halved, because being unsure is not the same as knowing.
        NextUse::Probability(chance) => match chance {
            0 => UNDECLARED / HINT,
            odds => u64::from(u16::MAX) / u64::from(odds) / HINT,
        },
    }
}
