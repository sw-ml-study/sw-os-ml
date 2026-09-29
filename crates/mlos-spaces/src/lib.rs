//! What MLOS calls its spaces, its regions and its ids, for both layout
//! emitters.
//!
//! Contract: `../sw-mlpl/docs/storage-layout-viz.md`. The static and the
//! runtime document must agree on every id and every word here. Design:
//! docs/notes/mlos-spaces.md.

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

/// Which space a region lives in. `Sysram` appears only in the static
/// document.
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

    /// Ids for this space's structural regions (padding, free, kernel
    /// sections), in issue order: the class-zero range, which no object
    /// can take.
    pub fn ids(self) -> impl Iterator<Item = u32> {
        (self as u32) << 28..
    }
}
