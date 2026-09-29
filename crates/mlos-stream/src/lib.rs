//! A declared sequence of objects, and where the workload is in it.
//!
//! Invariant: a declaration is finite, and past its end the answer is
//! `Never`; positions are ticks, one-based, and advancing is one
//! increment. Design and history: docs/notes/mlos-stream.md.

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
    /// Nothing declared. Every object reads as `Never`.
    pub const EMPTY: Self = Self {
        declared: &[],
        next: &[],
        cursor: 0,
    };

    /// `ml_stream_declare`: this session will acquire these, in this
    /// order. `next` must be the chain [`chain`] built over the same
    /// slice, at least as long; `NoBudget` otherwise. Replaces any earlier
    /// declaration and resets the cursor.
    pub fn declare(&mut self, objects: &'a [ObjectId], next: &'a [u32]) -> Result<()> {
        if next.len() < objects.len() {
            return Err(Error::NoBudget);
        }
        self.declared = objects;
        self.next = next;
        self.cursor = 0;
        Ok(())
    }

    /// `ml_stream_advance`: the workload has moved on by `steps`. One
    /// addition, whatever the table holds.
    pub const fn advance(&mut self, steps: u32) {
        self.cursor = self.cursor.saturating_add(steps);
    }

    /// Where the stream has got to.
    #[must_use]
    pub const fn cursor(&self) -> u32 {
        self.cursor
    }

    /// When `id` is next wanted, as a tick a policy can compare against
    /// `Residency::now`. Ticks are one-based, so position `p` is tick
    /// `p + 1`. Strictly after the cursor, since this is asked during the
    /// acquire at the cursor. `Never` if the rest of the declaration does
    /// not mention `id`.
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
