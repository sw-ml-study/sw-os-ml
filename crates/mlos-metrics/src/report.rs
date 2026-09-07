//! Reading the counters out.

use mlos_abi::ObjectClass;

use crate::Counters;

/// A snapshot, in a shape something can print.
#[derive(Clone, Copy)]
pub struct Report {
    /// Faults per class, indexed by [`ObjectClass::index`].
    pub faults: [u32; 8],
    /// Bytes fetched per class.
    pub fetched: [u64; 8],
    /// Bytes of registered objects.
    pub registered: u64,
    /// Bytes currently resident.
    pub resident: u64,
}

impl Report {
    /// `Rm`: resident bytes over model bytes, in parts per thousand.
    ///
    /// Per mille rather than a float, because this is read in a kernel
    /// with no floating point and printed by a shell with no formatter.
    /// The ratio matters more than the precision: 3/1000 resident is the
    /// interesting fact, not whether it is 0.31% or 0.34%.
    ///
    /// `None` when nothing is registered -- a ratio with no denominator
    /// is not zero, it is unanswerable, and reporting zero would read as
    /// "nothing is resident" rather than "nothing exists".
    #[must_use]
    pub const fn residency_per_mille(&self) -> Option<u32> {
        if self.registered == 0 {
            return None;
        }
        Some((self.resident.saturating_mul(1000) / self.registered) as u32)
    }

    /// Total faults across every class.
    #[must_use]
    pub const fn total_faults(&self) -> u32 {
        let (mut total, mut at) = (0u32, 0);
        while at < self.faults.len() {
            total = total.saturating_add(self.faults[at]);
            at += 1;
        }
        total
    }

    /// The class that faulted most, and how often.
    ///
    /// The single most useful line of a report: it names what the system
    /// is actually struggling with, which is the question a total never
    /// answers.
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
    /// A snapshot of everything counted so far.
    #[must_use]
    pub const fn report(&self) -> Report {
        Report {
            faults: self.faults,
            fetched: self.fetched,
            registered: self.registered,
            resident: self.resident,
        }
    }
}
