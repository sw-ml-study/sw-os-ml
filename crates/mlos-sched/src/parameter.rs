//! Parameter-major: the model passes memory once, and every session
//! waiting on an object is served before the next one is read.
//!
//! Invariant: a session past its latency ceiling is served first; else
//! the session served is on the lowest token and the furthest behind
//! within it, ties to the object the most sessions want next, then the
//! lowest session. Design: docs/notes/mlos-sched.md.

use crate::{Schedule, Waiting};

/// Lockstep over sessions: nobody starts a token until everyone has
/// finished the last, and within a token the sessions that are behind
/// catch up before the ones ahead move on. Shared objects are therefore
/// read once and hit by every other session; private ones (KV) are each
/// session's own. A session that has waited past its ceiling leaves the
/// lockstep and is served now, at the price of reading alone.
#[derive(Clone, Copy, Debug, Default)]
pub struct ParameterMajor;

impl Schedule for ParameterMajor {
    fn pick(&mut self, waiting: &dyn Waiting) -> Option<usize> {
        if let Some(late) = overdue(waiting) {
            return Some(late);
        }
        (0..waiting.sessions())
            .filter_map(|s| waiting.at(s).map(|at| (s, at)))
            .min_by_key(|&(s, at)| (at.token, at.index, usize::MAX - shared(waiting, s), s))
            .map(|(s, _)| s)
    }
}

/// The lowest live session that has waited at least its ceiling, if one
/// has a ceiling at all. The escape hatch.
fn overdue(waiting: &dyn Waiting) -> Option<usize> {
    (0..waiting.sessions()).find(|&s| {
        waiting
            .at(s)
            .is_some_and(|at| at.ceiling != 0 && at.waited >= at.ceiling)
    })
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
