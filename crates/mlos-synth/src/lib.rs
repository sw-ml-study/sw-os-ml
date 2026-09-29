//! What a synthetic model *is*: its shape, and the tiers it comes from.
//!
//! Definition only; the live instance is `mlos-lab`. Design and history:
//! docs/notes/mlos-synth.md.

#![no_std]

pub mod disk;
pub mod kv;
pub mod model;
pub mod tiers;

pub use model::{ACTIVATION_BYTES, LAYERS, TILE_BYTES, TILES};

/// What to call this model in anything that has to name it, so a trace
/// replayed against the wrong model is a caught error.
pub const MODEL: &str = "synth-8x16";
