//! Where a faulted-in object is put.
//!
//! A bump allocator, and deliberately nothing more. Eviction is M3: until
//! a policy decides what to throw away, the honest behaviour when memory
//! runs out is to say so. A general allocator here would let the manager
//! quietly succeed at a point where the interesting question -- what
//! should have been evicted -- is the one being dodged.

use mlos_abi::{Error, Result};

/// A region that resident objects are placed in.
pub struct Arena {
    base: u64,
    length: u64,
    used: u64,
}

impl Arena {
    /// An arena over `[base, base + length)`.
    ///
    /// Safe, which is worth saying because it looks like it should not
    /// be. The arena never dereferences anything: it does arithmetic and
    /// hands back numbers. Whoever *writes* to an address needs it to be
    /// real, and that is the provider -- which is `unsafe` and bounds
    /// checks against its own window. Putting the safety boundary at the
    /// one place that touches memory, rather than at every place that
    /// computes an address, is what keeps `unsafe` confined.
    #[must_use]
    pub const fn new(base: u64, length: u64) -> Self {
        Self {
            base,
            length,
            used: 0,
        }
    }

    /// Reserves `size` bytes, returning where they start.
    ///
    /// `NoBudget` when the arena is full -- which is a residency decision
    /// nobody has made yet, not a failure. It is the error a session's
    /// admission contract is meant to prevent.
    pub fn place(&mut self, size: u32) -> Result<u64> {
        // Sixteen-byte aligned: enough for anything a device will DMA
        // into, and it keeps one object's tail out of the next one's
        // cache line.
        let want = u64::from(size).next_multiple_of(16);
        let end = self.used.checked_add(want).ok_or(Error::NoBudget)?;
        if end > self.length {
            return Err(Error::NoBudget);
        }
        let at = self.base + self.used;
        self.used = end;
        Ok(at)
    }

    /// How many bytes are resident, and how many the arena holds.
    ///
    /// The numerator of `Rm` in `docs/PRD.md` s.5.2 -- resident bytes over
    /// model bytes -- which is the ratio this whole system optimises.
    #[must_use]
    pub const fn occupancy(&self) -> (u64, u64) {
        (self.used, self.length)
    }
}
