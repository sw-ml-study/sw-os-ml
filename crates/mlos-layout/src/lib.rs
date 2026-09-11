//! The `sw-ml-study.system-layout` document.
//!
//! A cross-repo contract, not an MLOS format. It is pinned by
//! `../sw-mlpl/docs/storage-layout-viz.md`, emitted first by SWTOS, parsed
//! by sw-mlpl's array language and drawn by demo-extensions' native3d.
//! MLOS is the second producer.
//!
//! Nothing here knows what an `ML_OBJECT` is. That is the point of the
//! boundary the contract draws: the operating system knows storage
//! semantics, the visualizer knows geometry, and the only thing crossing
//! between them is a table. Region kinds, owners and ids are opaque
//! strings and numbers at this level -- `mlos-image-map` supplies MLOS's
//! vocabulary, and SWTOS supplies a different one through the same shape.
//!
//! Columnar (struct-of-arrays) rather than an array of objects, because
//! that is what sw-mlpl's `parse_json` ingests today: homogeneous numeric
//! arrays become numeric arrays, all-string arrays become string lists,
//! and the layout math then runs elementwise over whole columns.

#![forbid(unsafe_code)]

mod check;
mod fill;
mod render;

pub use fill::fill;

/// The constant every consumer identifies the format by.
pub const SCHEMA: &str = "sw-ml-study.system-layout";

/// The contract version.
pub const VERSION: u32 = 1;

/// A named address space: a flash chip, a RAM bank, an arena.
pub struct Space {
    /// The key `Region::space` joins to.
    pub key: String,
    /// What to call it on screen.
    pub name: String,
    /// The allocation unit, in bytes. One rendered cell.
    pub block: u64,
    /// How many bytes the space holds.
    pub capacity: u64,
}

/// One classified extent within a space.
///
/// The `tier`, `object` and `state` columns are MLOS's extensions. The
/// contract permits extra columns, and this is what an ML object store
/// knows that a flash image does not -- which is the argument for sharing
/// a format rather than forking one. Regions they do not apply to carry
/// the empty string.
pub struct Region {
    /// Stable across snapshots; what picking and cross-highlighting use.
    pub id: u32,
    /// Which [`Space::key`] this sits in.
    pub space: String,
    /// Its class, for the Purpose colour mode.
    pub kind: String,
    /// What to call it on screen.
    pub name: String,
    /// Whose it is, for the Owner colour mode.
    pub owner: String,
    /// Byte offset within the space.
    pub start: u64,
    /// Length in bytes.
    pub length: u64,
    /// Residency tier, for ML objects.
    pub tier: String,
    /// The decimal `ObjectId`, for ML objects. A string because a `u64`
    /// does not survive a JSON number in every consumer.
    pub object: String,
    /// Residency state, for the State colour mode.
    pub state: String,
}

/// A relationship between two regions, by [`Region::id`].
pub struct Edge {
    /// What kind of relationship: `describes`, `loads-to`, `backs`.
    pub kind: String,
    /// The `region_id` it runs from.
    pub from: u32,
    /// The `region_id` it runs to.
    pub to: u32,
}

/// A whole layout, ready to render.
#[derive(Default)]
pub struct Doc {
    /// Every space, in display order.
    pub spaces: Vec<Space>,
    /// Every region. Index-aligned columns are produced from this.
    pub regions: Vec<Region>,
    /// Relationships, which may be empty.
    pub edges: Vec<Edge>,
}

impl Doc {
    /// The document as JSON, stamped with the producer's `revision`.
    #[must_use]
    pub fn render(&self, producer: &str, revision: &str) -> String {
        render::document(self, producer, revision)
    }

    /// Whether this satisfies the contract, and what is wrong if not.
    ///
    /// Called by the emitter before it writes, so a malformed document is
    /// caught here rather than in somebody else's repository.
    pub fn check(&self) -> Result<(), String> {
        check::check(self)
    }
}
