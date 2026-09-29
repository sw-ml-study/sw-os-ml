#![no_std]
#![forbid(unsafe_code)]

//! A fixed region with a coalescing free list.
//!
//! Invariant: a release the free list cannot record is refused, never
//! leaked, and the bytes stay the caller's. Design and history:
//! docs/notes/mlos-arena.md.

mod holes;

use mlos_abi::{Error, Result};

use holes::Hole;

/// How many separate free extents the arena can track. A release that
/// would need one more is refused by [`Arena::release`].
pub const HOLES: usize = 64;

/// What the arena looks like right now, read in one call so the fields
/// agree with each other.
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
    /// The largest single run of free bytes: what can actually be placed.
    pub largest: u64,
}

/// A region that resident objects are placed in. Borrowed for a
/// lifetime, so the kernel's static buffer and the simulator's `Vec` can
/// both back one.
pub struct Arena<'a> {
    bytes: &'a mut [u8],
    base: u64,
    free: [Hole; HOLES],
    holes: usize,
    used: usize,
}

impl<'a> Arena<'a> {
    /// What a placement is rounded up to, in bytes. Public because the
    /// layout emitter draws the arena in these units.
    pub const ALIGN: u32 = 16;

    /// An arena over `bytes`, entirely free. Safe: every address it hands
    /// out is inside memory it exclusively borrows.
    #[must_use]
    pub fn new(bytes: &'a mut [u8]) -> Self {
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
    /// `NoBudget` when no single run is large enough, which after
    /// evictions is stricter than "not enough free bytes".
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
    /// `NoBudget` when the free list is full, and then nothing changes:
    /// the caller still owns the bytes and the object is still resident.
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
