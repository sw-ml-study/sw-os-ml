//! The resident tier as a provider: a `read` is a copy from a physical
//! address.
//!
//! Invariant: no read leaves the window the provider was constructed
//! over; `bounds::within` proves it before the copy. Design and history:
//! docs/notes/mlos-provider-dram.md.

#![no_std]

use core::ptr;

use mlos_abi::Result;
use mlos_objtab::{CostNs, ProviderId};
use mlos_provider::{Cost, Located, Provider};

mod bounds;

/// A window of physical memory that objects may live in. A read outside
/// it is an error, never a memory access.
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

    /// Zero: resident memory is free, near enough, next to any other tier.
    fn cost(&self, object: Located) -> Cost {
        let _ = object;
        Cost {
            latency: CostNs(0),
            bytes_per_ms: 0,
        }
    }
}
