//! An access trace: what a workload asked for, in order.
//!
//! Not what the system did about it. `mlos-events` records that -- placed,
//! hit, refused -- and those are properties of the POLICY under test. A
//! trace is the question; an event stream is one policy's answer to it.
//! Keeping them apart is what lets the same workload be replayed against
//! four policies and scored on the same question.
//!
//! So a trace carries a session and an `ObjectId` per access and nothing
//! else. No tiers, no sizes, no costs, no residency. Every one of those is
//! a property of the system rather than of the workload, and baking one
//! into the trace would mean each policy was being asked something
//! slightly different.
//!
//! `no_std` and allocation-free, because step 008 replays the same trace
//! inside the kernel. Parsing fills a caller-provided slice; rendering
//! writes to a `fmt::Write`. Reading the file off a disk is the host's
//! business and is three lines wherever it is wanted.
//!
//! ## The model is not in the trace, and that is a trap
//!
//! An `ObjectId` names an object but does not say how big it is, and a
//! simulator cannot tell when a budget is full without knowing. Sizes come
//! from the model the trace was taken against -- `mlos-synth` today, a
//! `.spm` sidecar at M3 step 4. The header names that model so replaying a
//! trace against the wrong one is caught rather than silently producing
//! numbers about nothing.

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
    /// Somewhere to parse into, before anything has been parsed.
    ///
    /// Not `Default`: object zero does not decode to a class, on purpose,
    /// so that an all-zero id is rejected rather than read as object 0 of
    /// model 0. A constant named for what it is says "buffer fill" where
    /// `Default` would imply "a reasonable access".
    pub const EMPTY: Self = Self {
        session: SessionId(0),
        object: ObjectId(0),
    };
}

/// What a trace is of, and where it came from.
///
/// Borrowed from the text it was parsed out of, so a header costs nothing
/// and a trace can be parsed in a kernel with no allocator.
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
