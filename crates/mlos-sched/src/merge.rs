//! Driving a schedule over per-session streams into one trace.
//!
//! Invariant: every access of every session appears exactly once, in
//! the order the schedule chose; the scheduler never sees more than
//! positions, waits and next objects. Design: docs/notes/mlos-sched.md.

extern crate alloc;

use alloc::vec::Vec;

use mlos_abi::ObjectId;
use mlos_objtab::SessionId;
use mlos_trace::Access;

use crate::{At, Schedule, Waiting};

/// One session's lane: who it is, how long it will wait, and its tokens,
/// each the objects one step asks for in order.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Lane {
    /// The session.
    pub session: SessionId,
    /// `Contract::latency_ceiling`, in acquires; zero for none.
    pub ceiling: u32,
    /// Its stream, token by token.
    pub tokens: Vec<Vec<ObjectId>>,
}

/// The lanes with a cursor each, and when each one's current token
/// became due.
struct Lanes<'a> {
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

/// The trace `schedule` produces from `lanes`.
pub fn merge(schedule: &mut dyn Schedule, lanes: &[Lane]) -> Vec<Access> {
    let mut held = Lanes {
        lanes,
        cursors: alloc::vec![(0, 0); lanes.len()],
        due: alloc::vec![0; lanes.len()],
        clock: 0,
    };
    let mut out = Vec::new();
    while let Some(s) = schedule.pick(&held)
        && let Some(object) = held.next_of(s)
    {
        let session = lanes[s].session;
        out.push(Access { session, object });
        held.clock += 1;
        let (token, index) = held.cursors[s];
        let done = index + 1 >= lanes[s].tokens[token as usize].len() as u32;
        held.cursors[s] = if done {
            (token + 1, 0)
        } else {
            (token, index + 1)
        };
        held.due[s] = if done { held.clock } else { held.due[s] }; // next token due now
    }
    out
}
