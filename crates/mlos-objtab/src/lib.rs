//! The ML object table: what the kernel knows about each piece of model
//! state, where a conventional kernel has a page table.
//!
//! Invariant: a pure data structure. No allocation, no I/O, no decisions;
//! it only remembers. Design and history: docs/notes/mlos-objtab.md.

#![no_std]
#![forbid(unsafe_code)]

mod meta;
mod probe;
mod table;

pub use meta::{
    CostNs, Mutability, NextUse, ObjectMeta, Precision, ProviderId, Rung, SessionId, Tier,
};
pub use table::Table;

impl<const N: usize> Default for Table<N> {
    fn default() -> Self {
        Self::EMPTY
    }
}
