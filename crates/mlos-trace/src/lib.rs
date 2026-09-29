//! An access trace: what a workload asked for, in order.
//!
//! Invariant: an access is a session and an `ObjectId` and nothing else;
//! what the system did about it belongs to the event stream. Design and
//! history: docs/notes/mlos-trace.md.

#![no_std]
#![forbid(unsafe_code)]

mod derive;
mod read;
mod write;

use mlos_abi::ObjectId;
use mlos_objtab::SessionId;

pub use derive::from_events;
pub use read::{Error, parse};
pub use write::render;

/// The format version, in the first line of every trace.
pub const VERSION: u32 = 1;

/// The word that starts a trace file, so a wrong file fails loudly.
pub const MAGIC: &str = "mlos-trace";

/// One thing a workload asked for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Access {
    /// Who asked. Sessions are what M4 makes real; today there is one.
    pub session: SessionId,
    /// What they asked for.
    pub object: ObjectId,
}

impl Access {
    /// Somewhere to parse into, before anything has been parsed. Not
    /// `Default`: object zero decodes to no class, so an all-zero id is
    /// rejected rather than read as a real access.
    pub const EMPTY: Self = Self {
        session: SessionId(0),
        object: ObjectId(0),
    };
}

/// What a trace is of, and where it came from. Borrowed from the text
/// it was parsed out of.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Header<'a> {
    /// The model the ids name. A replay against a different one is wrong.
    pub model: &'a str,
    /// Where the trace came from, for whoever has to reproduce it.
    pub source: &'a str,
}

/// A parsed trace: what it is of, and what it asked for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Trace<'a> {
    /// What it is a trace of.
    pub header: Header<'a>,
    /// Every access, in the order the workload made them.
    pub accesses: &'a [Access],
}
