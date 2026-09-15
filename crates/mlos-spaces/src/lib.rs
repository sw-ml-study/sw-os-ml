//! What MLOS calls its spaces, its regions and its ids.
//!
//! `no_std`, and that is the whole reason it exists apart from
//! `mlos-layout`. Two emitters produce the same contract: a host-side one
//! reading build artifacts, and one inside the kernel reading the live
//! object table. They cannot share a renderer -- the kernel has no
//! allocator and streams to a `Write` -- but they MUST share the id scheme
//! and the vocabulary, because a consumer joins the two files on
//! `region_id` and colours both from one palette. Two copies of that would
//! agree right up until the moment they mattered.
//!
//! The contract itself is `../sw-mlpl/docs/storage-layout-viz.md`.

#![no_std]
#![forbid(unsafe_code)]

mod ids;
mod vocab;

pub use ids::object;
pub use vocab::{NextUseText, kind, state, tier};

/// The constant every consumer identifies the format by.
pub const SCHEMA: &str = "sw-ml-study.system-layout";

/// The contract version.
pub const VERSION: u32 = 1;

/// The name this producer goes by in `provenance`.
pub const PRODUCER: &str = "mlos";

/// Which space a region lives in.
///
/// `disk` and `dram` appear in both the static and the runtime document;
/// `sysram` only in the static one, because a running kernel has no
/// symbols and cannot say where its own `.text` ended.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Where {
    /// The block device holding the model's weights.
    Disk = 1,
    /// The object arena: where a faulted-in object is placed.
    Dram = 2,
    /// Guest RAM as the machine presents it.
    Sysram = 3,
}

impl Where {
    /// The `spaces` key, and what `region_space` joins on.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Disk => "disk",
            Self::Dram => "dram",
            Self::Sysram => "sysram",
        }
    }

    /// Ids for this space's structural regions, in issue order.
    ///
    /// Class zero is not a valid `ObjectClass`, so the class-zero range of
    /// each space is free for regions that are not objects -- padding,
    /// free space, kernel sections. Both emitters take them from here in
    /// the same order, which is what keeps the arena's free region the
    /// same region in both documents.
    pub fn ids(self) -> impl Iterator<Item = u32> {
        (self as u32) << 28..
    }
}
