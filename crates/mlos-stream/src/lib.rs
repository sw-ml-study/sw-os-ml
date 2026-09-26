//! A declared sequence of objects, and where the workload is in it.
//!
//! The mechanism by which a transformer hands an operating system its own
//! future. `docs/PRD.md` s.1 claims that most ML state is read in an order
//! known in advance; a stream is that order, written down, and it is what
//! finally writes `ObjectMeta::next_use` -- a field that has existed since
//! M2 step 001 with nothing to set it.
//!
//! ## Finite, because a decode loop is not actually cyclic
//!
//! An earlier version declared ONE PERIOD and ran the cursor past the end
//! of it, on the reasoning that every token reads the same objects in the
//! same order. That is true of the weights and false of everything else:
//! a KV cache ACCUMULATES, so token 17 reads sixteen blocks that token 1
//! could not have named, and a sequence with a growing tail has no
//! period. Under the cyclic model the kernel answered `Never` for every
//! KV block -- making the most-reused objects in the workload look like
//! the most evictable -- while the simulator, reading the whole trace,
//! knew better. The two disagreed by 27 reads out of 3758, which is the
//! sort of gap that looks like rounding and is not.
//!
//! So a declaration is a finite sequence, and running off the end of it
//! answers `Never`: nothing has declared a future beyond what was
//! declared. A workload that really does loop declares the loop it will
//! run.
//!
//! ## Borrowed, because the declaration is as long as the workload
//!
//! One period of a sweep fitted in a fixed array. A decode loop's whole
//! access sequence does not, and a kernel with no allocator cannot grow
//! one. The declarer owns the storage -- statics in the kernel, `Vec`s in
//! the simulator -- and a stream borrows it.
//!
//! ## Advancing is one increment, and the type is why
//!
//! `NextUse::At` holds a POSITION rather than a distance, which is what
//! makes [`advance`](Stream::advance) a single addition. A distance is
//! measured from somewhere: advancing a stream by one step would make
//! every resident object's distance wrong, and the kernel would have to
//! walk the object table to correct them -- a per-token cost over the very
//! structure it would be walking. A position does not move when the
//! cursor does. The subtraction happens once, in the policy, for the
//! handful of objects it actually compares.
//!
//! ## What it cannot yet express
//!
//! `NextUse::Probability` -- an MoE router's distribution -- is never
//! produced here, because nothing routes yet. The distinction is
//! preserved rather than collapsed: a declared stream says exactly WHEN,
//! and a router will say only HOW LIKELY, and a policy may act on the
//! first with certainty and weight by the second. Collapsing them would
//! licence evicting something the system merely guessed about as though
//! it knew.

#![no_std]
#![forbid(unsafe_code)]

mod chain;

pub use chain::{NEVER, chain};

use mlos_abi::{Error, ObjectId, Result};
use mlos_objtab::NextUse;

/// A declared sequence, and how far through it the workload is.
pub struct Stream<'a> {
    declared: &'a [ObjectId],
    /// For each position, where the same object is next declared.
    next: &'a [u32],
    cursor: u32,
}

impl<'a> Stream<'a> {
    /// Nothing declared. Every object reads as `Never`, which is what the
    /// table said before streams existed.
    pub const EMPTY: Self = Self {
        declared: &[],
        next: &[],
        cursor: 0,
    };

    /// `ml_stream_declare`: this session will acquire these, in this
    /// order.
    ///
    /// `next` is the chain [`chain`] builds over the same slice. Passed in
    /// rather than computed here because the kernel has nowhere to put it
    /// that a stream could own, and passing it is what forces both sides
    /// to build it with the same code.
    ///
    /// Replaces whatever was declared before, and resets the cursor: a
    /// new declaration is a new workload, and carrying a position across
    /// would point into a sequence that no longer exists.
    pub fn declare(&mut self, objects: &'a [ObjectId], next: &'a [u32]) -> Result<()> {
        if next.len() < objects.len() {
            return Err(Error::NoBudget);
        }
        self.declared = objects;
        self.next = next;
        self.cursor = 0;
        Ok(())
    }

    /// `ml_stream_advance`: the workload has moved on by `steps`.
    ///
    /// One addition, whatever the table holds. That is the property the
    /// whole design of `NextUse::At` exists to preserve.
    pub const fn advance(&mut self, steps: u32) {
        self.cursor = self.cursor.saturating_add(steps);
    }

    /// Where the stream has got to.
    #[must_use]
    pub const fn cursor(&self) -> u32 {
        self.cursor
    }

    /// When `id` is next wanted, as a tick a policy can compare against
    /// `Residency::now`.
    ///
    /// Ticks are one-based -- the first acquire happens at tick 1 --
    /// because that is how `ObjectMeta::used_tick` has counted since M2,
    /// and position `p` in the declaration is therefore tick `p + 1`.
    /// Mixing the two bases is not a cosmetic error: an object wanted by
    /// the very next access reads as `Never` and becomes the most
    /// evictable thing in the table.
    ///
    /// Strictly after the cursor, because this is asked while the object
    /// is being acquired: the use happening now is not the one a policy
    /// needs to know about. When the workload is running what it declared
    /// -- the case every measurement covers -- the object IS the one on
    /// the cursor, and the answer is one indexed read. Otherwise it is a
    /// scan of what remains, which is the honest general answer.
    ///
    /// `Never` for an object the rest of the declaration does not mention
    /// -- which is an honest answer rather than a maximum: it means
    /// nothing has declared a future for it, not that it will not be
    /// wanted.
    #[must_use]
    pub fn next_after(&self, id: ObjectId) -> NextUse {
        let at = self.cursor as usize;
        if self.declared.get(at) == Some(&id) {
            return match self.next[at] {
                NEVER => NextUse::Never,
                next => NextUse::At(next + 1),
            };
        }
        let rest = self.declared.get(at.saturating_add(1)..).unwrap_or(&[]);
        match rest.iter().position(|held| *held == id) {
            Some(offset) => NextUse::At((at + offset + 2) as u32),
            None => NextUse::Never,
        }
    }
}
