//! What MLOS counts.
//!
//! A conventional kernel reports throughput and residency: how fast, and
//! how much memory. Those are the right numbers when RAM is cheap and the
//! workload is unpredictable. Neither holds here, so MLOS counts
//! different things (`docs/PRD.md` s.5.2) and the objective is **useful
//! work per resident byte**, not work per second.
//!
//! Per class, always. "40,000 faults" is a number; "38,000 of them cold
//! KV blocks" is a diagnosis, and it is the difference between knowing
//! the system is thrashing and knowing what to do about it.

#![no_std]
#![forbid(unsafe_code)]

mod report;

use mlos_abi::ObjectClass;

pub use report::Report;

/// How many classes there are to count.
const CLASSES: usize = ObjectClass::ALL.len();

/// Running totals.
///
/// Saturating throughout. A counter that wraps turns a long run into a
/// smaller number than a short one, which is worse than a counter that
/// admits it stopped: these are read to compare runs, and a wrap would
/// make a comparison quietly wrong rather than visibly stuck.
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

    /// Records a fault on `class` that moved `bytes` from a provider.
    ///
    /// Both numbers, because they answer different questions. The count
    /// says how often the system was wrong about what it would need; the
    /// bytes say what being wrong cost -- and a thousand faults on scales
    /// is cheap where ten on experts is not.
    pub const fn fault(&mut self, class: ObjectClass, bytes: u32) {
        let at = class.index();
        self.faults[at] = self.faults[at].saturating_add(1);
        self.fetched[at] = self.fetched[at].saturating_add(bytes as u64);
    }

    /// Records that an object was registered, whether or not it is
    /// resident.
    ///
    /// The denominator of `Rm`: what the model *is*, against what of it
    /// is in memory.
    pub const fn registered(&mut self, bytes: u32) {
        self.registered = self.registered.saturating_add(bytes as u64);
    }

    /// Records a change in resident bytes.
    ///
    /// Signed, because this has to work in both directions the moment
    /// eviction exists, and a counter that only goes up would make the
    /// first eviction look like a leak.
    pub const fn resident(&mut self, delta: i64) {
        self.resident = self.resident.saturating_add_signed(delta);
    }
}
