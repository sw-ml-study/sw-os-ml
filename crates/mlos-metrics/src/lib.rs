//! What MLOS counts: faults and bytes per object class, and residency.
//!
//! Invariant: every counter saturates; none wraps. Design and history:
//! docs/notes/mlos-metrics.md.

#![no_std]
#![forbid(unsafe_code)]

mod report;

use mlos_abi::ObjectClass;

pub use report::Report;

/// How many classes there are to count.
const CLASSES: usize = ObjectClass::ALL.len();

/// Running totals, saturating throughout.
#[derive(Clone, Copy)]
pub struct Counters {
    faults: [u32; CLASSES],
    fetched: [u64; CLASSES],
    registered: u64,
    resident: u64,
}

impl Counters {
    /// Nothing counted yet.
    pub const EMPTY: Self = Self {
        faults: [0; CLASSES],
        fetched: [0; CLASSES],
        registered: 0,
        resident: 0,
    };

    /// Records a fault on `class` that moved `bytes` from a provider:
    /// one more fault, `bytes` more fetched.
    pub const fn fault(&mut self, class: ObjectClass, bytes: u32) {
        let at = class.index();
        self.faults[at] = self.faults[at].saturating_add(1);
        self.fetched[at] = self.fetched[at].saturating_add(bytes as u64);
    }

    /// Records that an object of `bytes` was registered, whether or not
    /// it is resident. The denominator of `Rm`.
    pub const fn registered(&mut self, bytes: u32) {
        self.registered = self.registered.saturating_add(bytes as u64);
    }

    /// Records a change in resident bytes, in either direction.
    pub const fn resident(&mut self, delta: i64) {
        self.resident = self.resident.saturating_add_signed(delta);
    }
}
