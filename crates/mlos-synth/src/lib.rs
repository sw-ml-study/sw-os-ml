//! What a synthetic model *is*: its shape, and the tiers it comes from.
//!
//! Definition only. The one live instance -- the manager holding it, the
//! arena it faults into, the device it reads from -- is `mlos-lab`. The
//! split is the difference between "a transformer has eight layers of
//! sixteen tiles" and "this machine currently has seven of them
//! resident", and keeping them apart means the shape can be described
//! without a running kernel to describe it on.

#![no_std]

pub mod disk;
pub mod model;
pub mod tiers;

pub use model::{ACTIVATION_BYTES, LAYERS, TILE_BYTES, TILES};

/// What to call this model in anything that has to name it.
///
/// A trace carries an `ObjectId` per access and no sizes; the sizes come
/// from the model the ids were taken against. Naming it in the trace is
/// what makes replaying against the wrong one a caught error rather than
/// a table of numbers about nothing.
pub const MODEL: &str = "synth-8x16";
