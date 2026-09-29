//! The `sw-ml-study.system-layout` document: a columnar table of spaces,
//! regions and edges.
//!
//! Contract: pinned by `../sw-mlpl/docs/storage-layout-viz.md`, shared
//! with SWTOS; region kinds, owners and ids are opaque here. Design:
//! docs/notes/mlos-layout.md.

#![forbid(unsafe_code)]

mod fill;
mod read;
mod render;
mod valid;

pub use fill::fill;
pub use read::Columns;
pub use valid::validate;

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

/// One classified extent within a space. `tier`, `object`, `state`,
/// `reuse`, `cost` and `next_use` are MLOS's extension columns; regions
/// they do not apply to carry the empty string or zero.
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
    /// The decimal `ObjectId`, for ML objects. A string, not a JSON
    /// number.
    pub object: String,

    /// Residency state, for the State colour mode.
    pub state: String,
    /// How many times this object has been wanted.
    pub reuse: u64,
    /// What getting it back would cost, in nanoseconds.
    pub cost: u64,
    /// When it will next be wanted: `never`, `distance N`, `probability P`.
    pub next_use: String,
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
}
