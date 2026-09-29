//! The resident set as a policy sees it: the same read-only window the
//! kernel opens over its own table.
//!
//! Invariant: `at` returns a copy, never a borrow into the set. Design:
//! docs/notes/mlos-sim.md.

use mlos_abi::ObjectId;
use mlos_objtab::ObjectMeta;
use mlos_policy::Residency;

use crate::Resident;

impl Residency for Resident<'_> {
    fn len(&self) -> usize {
        self.held.len()
    }

    fn at(&self, index: usize) -> Option<(ObjectId, ObjectMeta)> {
        self.held.get(index).copied()
    }

    fn now(&self) -> u32 {
        self.cursor
    }
}
