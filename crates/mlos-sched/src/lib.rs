//! Whose turn it is: a scheduler sees every session's declared stream
//! and names the session to serve next.
//!
//! Invariant: a scheduler reads positions and next objects only, holds no
//! state the sessions do not, and names a live session or none. The same
//! code runs in the simulator and the kernel. Design and history:
//! docs/notes/mlos-sched.md.

#![no_std]
#![forbid(unsafe_code)]

#[cfg(feature = "alloc")]
mod merge;
mod parameter;
mod process;

use mlos_abi::ObjectId;

#[cfg(feature = "alloc")]
pub use merge::{Stream, merge};
pub use parameter::ParameterMajor;
pub use process::ProcessMajor;

/// The sessions as a scheduler may see them: how far each has got, and
/// what each wants next.
pub trait Waiting {
    /// How many sessions there are, live or finished.
    fn sessions(&self) -> usize;

    /// Where `session` is: its token and its index within that token, or
    /// `None` once it has nothing left to ask for.
    fn position(&self, session: usize) -> Option<(u32, u32)>;

    /// The object `session` will ask for next, or `None` if finished.
    fn next_of(&self, session: usize) -> Option<ObjectId>;
}

/// A scheduling rule: which session is served next.
pub trait Schedule {
    /// The session to serve one access for, or `None` when every session
    /// is finished.
    fn pick(&mut self, waiting: &dyn Waiting) -> Option<usize>;
}
