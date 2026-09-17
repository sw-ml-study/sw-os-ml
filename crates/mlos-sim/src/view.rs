//! The resident set as a policy sees it.
//!
//! Apart from `resident.rs` because it is a different audience. The
//! inherent methods there are the simulator's -- put this in, take that
//! out, how full are we. This is the narrow, read-only window a policy
//! gets, and it is the same window the kernel will have to open over its
//! own table at step 009.
//!
//! `at` hands back a copy rather than a reference on purpose: a policy
//! holding a borrow into the resident set could not be called while the
//! simulator mutates it, and the kernel could not offer one at all over an
//! open-addressed table with tombstones.

use mlos_abi::ObjectId;
use mlos_objtab::ObjectMeta;
use mlos_policy::Residency;

use crate::Resident;

impl Residency for Resident {
    fn len(&self) -> usize {
        self.held.len()
    }

    fn at(&self, index: usize) -> Option<(ObjectId, ObjectMeta)> {
        self.held.get(index).copied()
    }
}
