//! Demand paging: fetch on miss, never throw anything away.
//!
//! What MLOS does today, and the floor the comparison is measured
//! against. It is in the table not because anyone would choose it but
//! because refusing has a cost, and the only way to see what that cost is
//! is to measure a system that only ever refuses.
//!
//! Its shape is worth noticing: it never thrashes. A policy that evicts
//! can be made to do arbitrarily much work by a workload that defeats it;
//! this one simply stops serving. Whether that is better depends entirely
//! on what the caller does with a refusal, which is the argument
//! `docs/PRD.md` F4 makes for admission control.

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
