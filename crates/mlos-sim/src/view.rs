//! The resident set as a policy sees it: the same read-only window the
//! kernel opens over its own table.
//!
//! Invariant: `at` returns a copy, never a borrow into the set. Design:
//! docs/notes/mlos-sim.md.

use mlos_abi::ObjectId;
use mlos_objtab::ObjectMeta;
use mlos_policy::Residency;

use crate::Resident;

impl Resident<'_> {
    /// Bytes of `class` resident right now.
    #[must_use]
    pub fn held_of(&self, class: mlos_abi::ObjectClass) -> u64 {
        self.held
            .iter()
            .filter(|(id, _)| id.class() == Some(class))
            .map(|(_, meta)| u64::from(meta.size))
            .sum()
    }
}

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
