//! A snapshot of the counters, with the ratios `docs/PRD.md` s.5.2 asks
//! for computed from it rather than kept alongside.
//!
//! Invariant: a report is a copy; reading one changes nothing. Design:
//! docs/notes/mlos-metrics.md.

use mlos_abi::ObjectClass;

use crate::Counters;

/// The counters at one moment.
#[derive(Clone, Copy)]
pub struct Report {
    /// Misses, per class.
    pub faults: [u32; 8],
    /// Bytes fetched from providers, per class.
    pub fetched: [u64; 8],
    /// Bytes served from residency without a read, per class.
    pub hits: [u64; 8],
    /// Bytes resident right now, per class.
    pub held: [u64; 8],
    /// Bytes registered in total.
    pub registered: u64,
    /// Bytes resident in total: the sum of `held`.
    pub resident: u64,
}

impl Report {
    /// `Rm`: resident bytes per thousand registered, or `None` with
    /// nothing registered.
    #[must_use]
    pub const fn residency_per_mille(&self) -> Option<u32> {
        if self.registered == 0 {
            return None;
        }
        Some((self.resident.saturating_mul(1000) / self.registered) as u32)
    }

    /// Misses across every class.
    #[must_use]
    pub const fn total_faults(&self) -> u32 {
        let (mut total, mut at) = (0u32, 0);
        while at < self.faults.len() {
            total = total.saturating_add(self.faults[at]);
            at += 1;
        }
        total
    }

    /// The class that faulted most, with its count, if any did.
    #[must_use]
    pub fn worst(&self) -> Option<(ObjectClass, u32)> {
        ObjectClass::ALL
            .into_iter()
            .map(|class| (class, self.faults[class.index()]))
            .filter(|(_, count)| *count > 0)
            .max_by_key(|(_, count)| *count)
    }
}

impl Counters {
    /// The counts right now.
    #[must_use]
    pub const fn report(&self) -> Report {
        let (mut resident, mut at) = (0u64, 0);
        while at < CLASSES {
            resident = resident.saturating_add(self.held[at]);
            at += 1;
        }
        Report {
            faults: self.faults,
            fetched: self.fetched,
            hits: self.hits,
            held: self.held,
            registered: self.registered,
            resident,
        }
    }
}

const CLASSES: usize = ObjectClass::ALL.len();
