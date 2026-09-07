//! Where a faulted-in object is put.
//!
//! A bump allocator, and deliberately nothing more. Eviction is M3: until
//! a policy decides what to throw away, the honest behaviour when memory
//! runs out is to say so. A general allocator here would let the manager
//! quietly succeed at a point where the interesting question -- what
//! should have been evicted -- is the one being dodged.

use mlos_abi::{Error, Result};

/// A region that resident objects are placed in.
///
/// Owns the bytes, rather than a base address and a length. An earlier
/// version held only numbers, which looked tidy and meant the fault path
/// could allocate space it had no way to *fill*: providers write into a
/// slice, and there was none to give them.
pub struct Arena {
    bytes: &'static mut [u8],
    base: u64,
    used: usize,
}

impl Arena {
    /// An arena over `bytes`.
    ///
    /// Safe, which is worth saying because it looks like it should not
    /// be. The address it hands out is the address of memory it holds a
    /// `&'static mut` to, so it cannot name anything it does not own.
    #[must_use]
    pub fn new(bytes: &'static mut [u8]) -> Self {
        let base = bytes.as_ptr() as u64;
        Self {
            bytes,
            base,
            used: 0,
        }
    }

    /// Reserves `size` bytes, returning where they are and room to fill.
    ///
    /// Both, in one call, because they are one decision. Separating them
    /// is what let the fault path allocate and then forget to fetch.
    ///
    /// `NoBudget` when the arena is full -- a residency decision nobody
    /// has made yet, not a failure. It is the error a session's admission
    /// contract exists to prevent.
    pub fn place(&mut self, size: u32) -> Result<(u64, &mut [u8])> {
        // Sixteen-byte aligned: enough for anything a device will DMA
        // into, and it keeps one object's tail out of the next one's
        // cache line.
        let want = (size as usize).next_multiple_of(16);
        let end = self.used.checked_add(want).ok_or(Error::NoBudget)?;
        let room = self.bytes.get_mut(self.used..end).ok_or(Error::NoBudget)?;
        let at = self.base + self.used as u64;
        self.used = end;
        Ok((at, &mut room[..size as usize]))
    }

    /// How many bytes are resident, and how many the arena holds.
    ///
    /// The numerator of `Rm` in `docs/PRD.md` s.5.2 -- resident bytes over
    /// model bytes -- which is the ratio this whole system optimises.
    #[must_use]
    pub const fn occupancy(&self) -> (u64, u64) {
        (self.used as u64, self.bytes.len() as u64)
    }
}
