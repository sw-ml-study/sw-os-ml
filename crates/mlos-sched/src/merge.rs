//! Driving a schedule over per-session streams into one trace.
//!
//! Invariant: every access of every session appears exactly once, in
//! the order the schedule chose; the scheduler never sees more than
//! positions and next objects. Design: docs/notes/mlos-sched.md.

extern crate alloc;

use alloc::vec::Vec;

use mlos_abi::ObjectId;
use mlos_objtab::SessionId;
use mlos_trace::Access;

use crate::{Schedule, Waiting};

/// Per-session streams, each a sequence of tokens, with a cursor per
/// session.
struct Streams<'a> {
    sessions: &'a [Stream],
    cursors: Vec<(u32, u32)>,
}

impl Waiting for Streams<'_> {
    fn sessions(&self) -> usize {
        self.sessions.len()
    }

    fn position(&self, session: usize) -> Option<(u32, u32)> {
        let (token, index) = *self.cursors.get(session)?;
        let tokens = &self.sessions.get(session)?.1;
        (usize::try_from(token).ok()? < tokens.len()).then_some((token, index))
    }

    fn next_of(&self, session: usize) -> Option<ObjectId> {
        let (token, index) = self.position(session)?;
        let tokens = &self.sessions[session].1;
        tokens.get(token as usize)?.get(index as usize).copied()
    }
}

/// One session's stream: its id and its tokens, each the objects one
/// step asks for in order.
pub type Stream = (SessionId, Vec<Vec<ObjectId>>);

/// The trace `schedule` produces from `sessions`.
pub fn merge(schedule: &mut dyn Schedule, sessions: &[Stream]) -> Vec<Access> {
    let mut streams = Streams {
        sessions,
        cursors: alloc::vec![(0, 0); sessions.len()],
    };
    let mut out = Vec::new();
    while let Some(s) = schedule.pick(&streams)
        && let Some(object) = streams.next_of(s)
    {
        out.push(Access {
            session: sessions[s].0,
            object,
        });
        let (token, index) = streams.cursors[s];
        let length = sessions[s].1[token as usize].len() as u32;
        streams.cursors[s] = if index + 1 < length {
            (token, index + 1)
        } else {
            (token + 1, 0)
        };
    }
    out
}
