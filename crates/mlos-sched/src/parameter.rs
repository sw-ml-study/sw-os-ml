//! Parameter-major: the model passes memory once, and every session
//! waiting on an object is served before the next one is read.
//!
//! Invariant: the session served is on the lowest token, and among those
//! the furthest behind within it; ties go to the object the most sessions
//! want next, then the lowest session. Design: docs/notes/mlos-sched.md.

use crate::{Schedule, Waiting};

/// Lockstep over sessions: nobody starts a token until everyone has
/// finished the last, and within a token the sessions that are behind
/// catch up before the ones ahead move on. Shared objects are therefore
/// read once and hit by every other session; private ones (KV) are each
/// session's own.
#[derive(Clone, Copy, Debug, Default)]
pub struct ParameterMajor;

impl Schedule for ParameterMajor {
    fn pick(&mut self, waiting: &dyn Waiting) -> Option<usize> {
        (0..waiting.sessions())
            .filter_map(|s| waiting.position(s).map(|at| (s, at)))
            .min_by_key(|&(s, (token, index))| (token, index, usize::MAX - shared(waiting, s), s))
            .map(|(s, _)| s)
    }
}

/// How many sessions want the same object `session` wants next.
fn shared(waiting: &dyn Waiting, session: usize) -> usize {
    let Some(object) = waiting.next_of(session) else {
        return 0;
    };
    (0..waiting.sessions())
        .filter(|&t| waiting.next_of(t) == Some(object))
        .count()
}
