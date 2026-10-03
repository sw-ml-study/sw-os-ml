//! Whose turn it is: a scheduler sees every session's declared stream
//! and names the session to serve next.
//!
//! Invariant: a scheduler reads positions, waits and next objects only,
//! holds no state the sessions do not, and names a live session or none.
//! The same code runs in the simulator and the kernel. Design and
//! history: docs/notes/mlos-sched.md.

#![no_std]
#![forbid(unsafe_code)]

#[cfg(feature = "alloc")]
mod lanes;
#[cfg(feature = "alloc")]
mod merge;
mod parameter;
mod process;

use mlos_abi::ObjectId;

#[cfg(feature = "alloc")]
pub use merge::{Lane, merge, merge_timed};
pub use parameter::ParameterMajor;
pub use process::ProcessMajor;

/// Where a live session is, and how long it has been waiting.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct At {
    /// Which token of its stream it is on.
    pub token: u32,
    /// Which access within that token it will ask for next.
    pub index: u32,
    /// Acquires served to other sessions since this one's current token
    /// became due -- when its previous token ended, or the start. The
    /// latency a session feels is how long its token takes, not the gap
    /// between two of its accesses, so that is what is bounded.
    pub waited: u32,
    /// The most it agreed to wait, in acquires, or zero for no limit:
    /// `Contract::latency_ceiling`.
    pub ceiling: u32,
}

/// The sessions as a scheduler may see them.
pub trait Waiting {
    /// How many sessions there are, live or finished.
    fn sessions(&self) -> usize;

    /// Where `session` is, or `None` once it has nothing left to ask for.
    fn at(&self, session: usize) -> Option<At>;

    /// The object `session` will ask for next, or `None` if finished.
    fn next_of(&self, session: usize) -> Option<ObjectId>;
}

/// A scheduling rule: which session is served next.
pub trait Schedule {
    /// The session to serve one access for, or `None` when every session
    /// is finished.
    fn pick(&mut self, waiting: &dyn Waiting) -> Option<usize>;
}
