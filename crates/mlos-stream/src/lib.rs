//! A declared sequence of objects, and where the workload is in it.
//!
//! The mechanism by which a transformer hands an operating system its own
//! future. `docs/PRD.md` s.1 claims that most ML state is read in an order
//! known in advance; a stream is that order, written down, and it is what
//! finally writes `ObjectMeta::next_use` -- a field that has existed since
//! M2 step 001 with nothing to set it.
//!
//! ## Cyclic, because a decode loop is
//!
//! A stream is not an arbitrary list. Every token of a dense transformer
//! reads the same objects in the same order, so what gets declared is one
//! period and the cursor runs on past the end of it. Declaring a thousand
//! tokens' worth of accesses would be storing the same hundred-odd ids a
//! thousand times, and a kernel has nowhere to put that.
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

use mlos_abi::{Error, ObjectId, Result};
use mlos_objtab::NextUse;

/// How many objects one period of a stream may declare.
///
/// The synthetic model's sweep is 128 weight tiles, so this is room to
/// spare without being a growable collection the kernel would have to
/// allocate for.
pub const PERIOD: usize = 256;

/// A declared sequence, and how far through it the workload is.
pub struct Stream {
    declared: [ObjectId; PERIOD],
    length: usize,
    cursor: u32,
}

impl Stream {
    /// Nothing declared. Every object reads as `Never`, which is what the
    /// table said before streams existed.
    pub const EMPTY: Self = Self {
        declared: [ObjectId(0); PERIOD],
        length: 0,
        cursor: 0,
    };

    /// `ml_stream_declare`: this session will acquire these, in this
    /// order, repeatedly.
    ///
    /// Replaces whatever was declared before, and resets the cursor: a
    /// new declaration is a new workload, and carrying a position across
    /// would point into a sequence that no longer exists.
    pub fn declare(&mut self, objects: &[ObjectId]) -> Result<()> {
        let room = self
            .declared
            .get_mut(..objects.len())
            .ok_or(Error::NoBudget)?;
        room.copy_from_slice(objects);
        self.length = objects.len();
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

    /// When `id` is next wanted NEXT, as a position in this stream.
    ///
    /// Strictly after the cursor, because this is asked while the object
    /// is being acquired: the use happening now is not the one a policy
    /// needs to know about.
    ///
    /// `Never` for an object the stream does not mention -- which is an
    /// honest answer rather than a maximum: it means nothing has declared
    /// a future for it, not that it will not be wanted.
    ///
    /// A scan of one period, once per acquire. An acquire already costs a
    /// table lookup at best and a provider fetch at worst, so a hundred
    /// integer compares beside it is not the thing to optimise -- and it
    /// is bounded by the declaration rather than by the object table,
    /// which is the part that matters.
    #[must_use]
    pub fn next_after(&self, id: ObjectId) -> NextUse {
        let declared = &self.declared[..self.length];
        let Some(offset) = declared.iter().position(|held| *held == id) else {
            return NextUse::Never;
        };
        let period = self.length as u32;
        let ahead = match (offset as u32 + period - self.cursor % period) % period {
            // The object sitting exactly on the cursor is the one being
            // acquired right now, so its NEXT use is a whole period away
            // rather than here. Answering zero would say "wanted now"
            // forever -- the position never changes, the cursor runs past
            // it, and a policy computing `at - now` keeps saturating to
            // zero and pins the object it should have evicted first.
            // Found by watching `objs` after advancing the stream.
            0 => period,
            first => first,
        };
        NextUse::At(self.cursor + ahead)
    }
}
