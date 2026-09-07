//! The ML object table.
//!
//! Where a conventional kernel has a page table, MLOS has this. A page
//! table entry answers "where is this page and is it dirty". An entry here
//! answers "what would getting this back cost, when is it next wanted, and
//! how many sessions are waiting on it" -- which are the questions every
//! decision in `docs/PRD.md` turns on.
//!
//! Pure data structure: no `unsafe`, no allocation, no I/O, and therefore
//! testable on the host without a VM. Providers fetch; policies decide;
//! this only remembers.

#![no_std]
#![forbid(unsafe_code)]

mod meta;
mod probe;
mod table;

pub use meta::{CostNs, Mutability, NextUse, ObjectMeta, Precision, ProviderId, SessionId, Tier};
pub use table::Table;

impl<const N: usize> Default for Table<N> {
    fn default() -> Self {
        Self::EMPTY
    }
}
