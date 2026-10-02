//! What the object manager counts: faults, hits and residency, per class.
//!
//! Invariant: every counter is per object class and saturates, never
//! wraps; `held` is signed because eviction gives bytes back. Design and
//! history: docs/notes/mlos-metrics.md.

#![no_std]
#![forbid(unsafe_code)]

mod headline;
mod report;

use mlos_abi::ObjectClass;

pub use headline::Headline;
pub use report::Report;

const CLASSES: usize = ObjectClass::ALL.len();

/// The running counts.
#[derive(Clone, Copy)]
pub struct Counters {
    faults: [u32; CLASSES],
    fetched: [u64; CLASSES],
    hits: [u64; CLASSES],
    held: [u64; CLASSES],
    registered: u64,
}

impl Counters {
    /// Nothing counted yet.
    pub const EMPTY: Self = Self {
        faults: [0; CLASSES],
        fetched: [0; CLASSES],
        hits: [0; CLASSES],
        held: [0; CLASSES],
        registered: 0,
    };

    /// One miss on `class`, fetching `bytes` from a provider.
    pub const fn fault(&mut self, class: ObjectClass, bytes: u32) {
        let at = class.index();
        self.faults[at] = self.faults[at].saturating_add(1);
        self.fetched[at] = self.fetched[at].saturating_add(bytes as u64);
    }

    /// One hit on `class`: `bytes` applied without a read.
    pub const fn hit(&mut self, class: ObjectClass, bytes: u32) {
        let at = class.index();
        self.hits[at] = self.hits[at].saturating_add(bytes as u64);
    }

    /// `bytes` of `class` registered with the manager.
    pub const fn registered(&mut self, bytes: u32) {
        self.registered = self.registered.saturating_add(bytes as u64);
    }

    /// `delta` bytes of `class` became resident (placed) or stopped being
    /// (evicted).
    pub const fn held(&mut self, class: ObjectClass, delta: i64) {
        let at = class.index();
        self.held[at] = self.held[at].saturating_add_signed(delta);
    }
}
