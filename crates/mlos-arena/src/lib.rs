#![no_std]
#![forbid(unsafe_code)]

//! A fixed region with a coalescing free list.
//!
//! Its own crate because it knows nothing about objects: it hands out
//! runs of bytes and takes them back, and `mlos-objman` is what decides
//! which object goes in one. `mlos-objman` was at four modules when
//! eviction arrived, and AGENTS.md says to make the sibling crate rather
//! than push a fifth concern into a crate that is full.
//!
//! Where a faulted-in object is put, and taken out of.
//!
//! It was a bump allocator until M3 step 008, deliberately: until a policy
//! could decide what to throw away, the honest behaviour when memory ran
//! out was to say so, and a general allocator would have let the manager
//! quietly succeed at the point where the interesting question -- what
//! should have been evicted -- was the one being dodged. Step 006
//! answered that question, so this is where acting on it becomes
//! possible.
//!
//! ## A sorted free list, coalesced on release
//!
//! Free extents are kept sorted by address and merged with their
//! neighbours the moment one is returned. Merging on release rather than
//! on demand is what keeps the list short: a thousand evictions of
//! adjacent tiles leave one hole, not a thousand.
//!
//! Fixed capacity, because there is no allocator. A release that would
//! need a `HOLES + 1`th extent is REFUSED rather than silently leaking
//! the memory -- see [`Arena::release`]. That is the failure mode worth
//! being loud about: memory that has been evicted and cannot be reused is
//! worse than memory that was never freed, because the accounting says it
//! is available.
//!
//! ## First fit, not best fit
//!
//! First fit is what a kernel can afford and it is not obviously worse.
//! Best fit leaves a trail of slivers too small for anything; first fit
//! leaves larger fragments nearer the end. `tests/fragmentation.rs`
//! measures what actually happens rather than trusting either story.

mod holes;

use mlos_abi::{Error, Result};

use holes::Hole;

/// How many separate free extents the arena can track.
///
/// Generous for a workload of uniform tiles, which coalesce into a
/// handful of runs. A workload of ragged sizes could exhaust it, and
/// [`Arena::release`] says so rather than losing the bytes.
pub const HOLES: usize = 64;

/// What the arena looks like right now.
///
/// One call rather than four accessors, because every caller that wants
/// one of these wants at least two, and a `largest` that disagreed with
/// the `used` it was read beside would be worse than either alone.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Occupancy {
    /// Where the arena's bytes begin, so a `resident_at` can be turned
    /// back into an offset within it.
    pub base: u64,
    /// Bytes currently holding objects. The numerator of `Rm` in
    /// `docs/PRD.md` s.5.2.
    pub used: u64,
    /// Bytes the arena holds in total.
    pub capacity: u64,
    /// The largest single run of free bytes.
    ///
    /// The fragmentation number. When this is far below `capacity -
    /// used`, the arena has free memory it cannot give to anything, and a
    /// policy evicting perfectly into it has not helped.
    pub largest: u64,
}

/// A region that resident objects are placed in.
pub struct Arena {
    bytes: &'static mut [u8],
    base: u64,
    free: [Hole; HOLES],
    holes: usize,
    used: usize,
}

impl Arena {
    /// What a placement is rounded up to, in bytes.
    ///
    /// Enough for anything a device will DMA into, and it keeps one
    /// object's tail out of the next one's cache line. Public because a
    /// layout emitter draws the arena in these units, and two statements
    /// of the same granularity would be one too many.
    pub const ALIGN: u32 = 16;

    /// An arena over `bytes`, entirely free.
    ///
    /// Safe, which is worth saying because it looks like it should not
    /// be. The address it hands out is the address of memory it holds a
    /// `&'static mut` to, so it cannot name anything it does not own.
    #[must_use]
    pub fn new(bytes: &'static mut [u8]) -> Self {
        let (base, len) = (bytes.as_ptr() as u64, bytes.len());
        let mut free = [Hole::default(); HOLES];
        free[0] = Hole { at: 0, len };
        Self {
            bytes,
            base,
            free,
            holes: 1,
            used: 0,
        }
    }

    /// Reserves `size` bytes, returning where they are and room to fill.
    ///
    /// Both, in one call, because they are one decision. Separating them
    /// is what let the fault path allocate and then forget to fetch.
    ///
    /// `NoBudget` when no single run is large enough -- which after
    /// evictions means "not enough CONTIGUOUS room", a different thing
    /// from "not enough room" and the reason `Occupancy::largest` exists.
    pub fn place(&mut self, size: u32) -> Result<(u64, &mut [u8])> {
        let want = (size as usize).next_multiple_of(Self::ALIGN as usize);
        let index = self.free[..self.holes]
            .iter()
            .position(|hole| hole.len >= want)
            .ok_or(Error::NoBudget)?;

        let at = self.free[index].at;
        self.free[index].at += want;
        self.free[index].len -= want;
        if self.free[index].len == 0 {
            self.free.copy_within(index + 1..self.holes, index);
            self.holes -= 1;
        }
        self.used += want;
        let room = self.bytes.get_mut(at..at + want).ok_or(Error::NoBudget)?;
        Ok((self.base + at as u64, &mut room[..size as usize]))
    }

    /// Gives `size` bytes at `address` back, merging with any neighbours.
    ///
    /// `NoBudget` when the free list is full, and the bytes are NOT taken
    /// -- the caller still owns them and the object is still resident.
    /// Losing them would leave the accounting claiming memory that
    /// nothing can ever hand out, which is a worse failure than refusing
    /// to evict.
    pub fn release(&mut self, address: u64, size: u32) -> Result<()> {
        let at = usize::try_from(address.checked_sub(self.base).ok_or(Error::BadObject)?)
            .map_err(|_| Error::BadObject)?;
        let len = (size as usize).next_multiple_of(Self::ALIGN as usize);
        let index = self.free[..self.holes]
            .iter()
            .position(|hole| hole.at > at)
            .unwrap_or(self.holes);

        if self.holes == HOLES {
            return Err(Error::NoBudget);
        }
        self.free.copy_within(index..self.holes, index + 1);
        self.free[index] = Hole { at, len };
        self.holes += 1;
        self.used -= len;
        holes::merge(&mut self.free, &mut self.holes, index);
        Ok(())
    }

    /// Where it is, how full it is, and how fragmented.
    #[must_use]
    pub fn occupancy(&self) -> Occupancy {
        let largest = self.free[..self.holes].iter().map(|hole| hole.len).max();
        Occupancy {
            base: self.base,
            used: self.used as u64,
            capacity: self.bytes.len() as u64,
            largest: largest.unwrap_or(0) as u64,
        }
    }
}
