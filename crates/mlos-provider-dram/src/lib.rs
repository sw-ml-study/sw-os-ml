//! The resident tier.
//!
//! Barely a provider, and that is the point. Its `handle` is a physical
//! address and its `read` is a copy, so it is the one provider that
//! cannot fail for reasons of its own. It exists because everything above
//! it should not have to know that resident memory is a special case:
//! a fault on a `Warm` object and a fault on a `Cold` one take the same
//! path, and only the cost differs.
//!
//! `unsafe` lives here because reading from a physical address is exactly
//! what this crate is for (AGENTS.md, "Hard constraints").

#![no_std]

use core::ptr;

use mlos_abi::{Error, Result};
use mlos_objtab::{CostNs, ProviderId};
use mlos_provider::{Cost, Located, Provider};

mod bounds;

/// A window of physical memory that objects may live in.
///
/// Bounded on purpose. A provider that will read any address it is handed
/// turns a corrupt table entry into an arbitrary memory read; one that
/// knows its own extent turns the same entry into an error.
pub struct Dram {
    id: ProviderId,
    base: u64,
    length: u64,
}

impl Dram {
    /// A provider over `[base, base + length)`.
    ///
    /// # Safety
    ///
    /// The range must be mapped, readable, and reserved for object
    /// storage for as long as this provider exists.
    #[must_use]
    pub const unsafe fn new(id: ProviderId, base: u64, length: u64) -> Self {
        Self { id, base, length }
    }
}

impl Provider for Dram {
    fn id(&self) -> ProviderId {
        self.id
    }

    fn read(&self, object: Located, offset: u32, into: &mut [u8]) -> Result<u32> {
        let want = into.len().min(object.size.saturating_sub(offset) as usize);
        let from = bounds::within(self.base, self.length, object, offset, want)?;
        // SAFETY: `within` proved the whole range lies inside the window
        // this provider was constructed over, which its contract requires
        // to be mapped and readable. The copy is byte-wise, so alignment
        // does not enter into it.
        unsafe { ptr::copy_nonoverlapping(from as *const u8, into.as_mut_ptr(), want) };
        Ok(want as u32)
    }

    /// Free, near enough.
    ///
    /// Not literally -- a resident read still costs a cache miss -- but
    /// the number exists so eviction can compare tiers, and against three
    /// milliseconds of NVMe the difference is noise. Reporting a real
    /// figure here would be false precision.
    fn cost(&self, object: Located) -> Cost {
        let _ = object;
        Cost {
            latency: CostNs(0),
            bytes_per_ms: 0,
        }
    }
}
