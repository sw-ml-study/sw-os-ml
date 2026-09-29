//! What to throw away when memory runs out.
//!
//! Invariant: a policy performs no I/O and holds no state the table does
//! not own, so the same code runs in the kernel and in `mlos-sim`. Design
//! and history: docs/notes/mlos-policy.md.

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

    /// Which resident object to evict to make room for `wanting`. `None`
    /// declines, which is an answer, not a failure. Called once per victim
    /// until enough room exists.
    fn victim(&self, resident: &dyn Residency, wanting: &ObjectMeta) -> Option<ObjectId>;
}
