//! The host driver's view of per-session lanes: cursors, clock, and when
//! each lane's token became due.
//!
//! Invariant: `advance` is the only thing that moves a cursor, and it
//! reports a token's period the moment the token ends. Design:
//! docs/notes/mlos-sched.md.

extern crate alloc;

use alloc::vec::Vec;

use mlos_abi::ObjectId;

use crate::merge::Lane;
use crate::{At, Waiting};

/// The lanes with a cursor each, and when each one's current token
/// became due.
pub struct Lanes<'a> {
    lanes: &'a [Lane],
    cursors: Vec<(u32, u32)>,
    due: Vec<u32>,
    clock: u32,
}

impl Waiting for Lanes<'_> {
    fn sessions(&self) -> usize {
        self.lanes.len()
    }

    fn at(&self, session: usize) -> Option<At> {
        let (token, index) = *self.cursors.get(session)?;
        let lane = self.lanes.get(session)?;
        (usize::try_from(token).ok()? < lane.tokens.len()).then_some(At {
            token,
            index,
            waited: self.clock - self.due[session] - index,
            ceiling: lane.ceiling,
        })
    }

    fn next_of(&self, session: usize) -> Option<ObjectId> {
        let at = self.at(session)?;
        let tokens = &self.lanes[session].tokens;
        tokens
            .get(at.token as usize)?
            .get(at.index as usize)
            .copied()
    }
}

impl<'a> Lanes<'a> {
    /// Every lane at its start.
    #[must_use]
    pub fn over(lanes: &'a [Lane]) -> Self {
        Self {
            lanes,
            cursors: alloc::vec![(0, 0); lanes.len()],
            due: alloc::vec![0; lanes.len()],
            clock: 0,
        }
    }

    /// One access served to `session`: the clock ticks and its cursor
    /// moves. If that ended a token, the token's period -- acquires since
    /// the previous one ended -- and the next is due from now.
    pub fn advance(&mut self, session: usize) -> Option<u32> {
        self.clock += 1;
        let (token, index) = self.cursors[session];
        let done = index + 1 >= self.lanes[session].tokens[token as usize].len() as u32;
        self.cursors[session] = if done {
            (token + 1, 0)
        } else {
            (token, index + 1)
        };
        if !done {
            return None;
        }
        let period = self.clock - self.due[session];
        self.due[session] = self.clock;
        Some(period)
    }
}
