//! What to throw away when memory runs out.
//!
//! `no_std` and pure, which `docs/design.md` s.2 makes a structural rule:
//! a policy takes the object table's view and returns a decision. It
//! performs no I/O and **holds no state the table does not own**. That is
//! the whole reason the same code can run in the kernel and in `mlos-sim`
//! against a recorded trace, and it is why every method here takes
//! `&self`.
//!
//! The rule has a price, paid deliberately. An LRU that cannot keep its
//! own list has to find the least-recently-used object by looking, which
//! is a scan of the resident set per eviction. A kernel can afford that at
//! these sizes and could not at a million objects. Known-next-use has to
//! scan as well -- it is looking for a maximum -- so the cost is the same
//! for the policy being tested and for the ones it is tested against,
//! which is what keeps the comparison fair even though it is not what
//! keeps it fast.
//!
//! What a policy reads is [`ObjectMeta`], and every field it needs is
//! already there: `used_tick` for recency, `placed_tick` for insertion
//! order, `next_use` for the future, `reload_cost` and `recompute_cost`
//! for what being wrong would cost.

#![no_std]
#![forbid(unsafe_code)]

mod baselines;
mod nextuse;
mod residency;

use mlos_abi::ObjectId;
use mlos_objtab::ObjectMeta;

pub use baselines::{Demand, FIFO, LRU, Oldest};
pub use nextuse::{KnownNextUse, NEXT_USE};
pub use residency::Residency;

/// How to choose what to throw away.
pub trait Policy {
    /// What to call it in a report.
    fn name(&self) -> &'static str;

    /// Which resident object to evict to make room for `wanting`.
    ///
    /// `None` means decline -- and declining is a legitimate answer, not a
    /// failure. Demand paging never evicts anything, and the cost of that
    /// is exactly what the comparison is measuring. A policy may also
    /// decline because every candidate is worse than refusing: an object
    /// that cannot be recomputed and is wanted next is not a victim at any
    /// price.
    ///
    /// Called repeatedly while there is still not enough room, so a policy
    /// naming one victim at a time is normal and does not need to know how
    /// much more is needed.
    fn victim(&self, resident: &dyn Residency, wanting: &ObjectMeta) -> Option<ObjectId>;
}
