//! A transformer's worth of objects, without a transformer: eight layers
//! of sixteen tiles, plus an activation each.
//!
//! Design: docs/notes/mlos-synth.md.

use mlos_abi::{Fields, ObjectClass, ObjectId};
use mlos_objtab::{CostNs, Mutability, NextUse, ObjectMeta, Precision, SessionId, Tier};

use mlos_objtab::ProviderId;

/// Layers in the synthetic model.
pub const LAYERS: u16 = 8;
/// Weight tiles per layer.
pub const TILES: u16 = 16;
/// Bytes per tile.
pub const TILE_BYTES: u32 = 1024;
/// Bytes per activation.
pub const ACTIVATION_BYTES: u32 = 2048;

/// Which model this is.
const MODEL: u16 = 1;

/// The id of one weight tile.
#[must_use]
pub fn tile(layer: u16, tensor: u16) -> ObjectId {
    ObjectId::new(
        ObjectClass::WeightTile,
        Fields {
            model: MODEL,
            layer,
            tensor,
            tile: 0,
        },
    )
}

/// The id of one layer's activation.
#[must_use]
pub fn activation(layer: u16) -> ObjectId {
    ObjectId::new(
        ObjectClass::Activation,
        Fields {
            model: MODEL,
            layer,
            tensor: 0,
            tile: 0,
        },
    )
}

/// Weights: immutable, backed by storage, and impossible to recompute, so
/// eviction may demote them and must never discard them.
#[must_use]
pub fn weights() -> ObjectMeta {
    ObjectMeta {
        size: TILE_BYTES,
        precision: Precision::Q4,
        tier: Tier::Cold,
        home: Tier::Cold,
        provider: ProviderId(2),
        handle: 0,
        resident_at: 0,
        next_use: NextUse::Never,
        reuse_count: 0,
        placed_tick: 0,
        used_tick: 0,
        reload_cost: CostNs(4_000_000),
        recompute_cost: CostNs::IMPOSSIBLE,
        share_count: 0,
        mutability: Mutability::Immutable,
        owner: SessionId(0),
    }
}

/// Activations: transient, mutable, and cheaper to recompute than to
/// reload.
#[must_use]
pub fn activations() -> ObjectMeta {
    ObjectMeta {
        size: ACTIVATION_BYTES,
        precision: Precision::Fp16,
        tier: Tier::Archive,
        home: Tier::Archive,
        provider: ProviderId(3),
        handle: 0,
        resident_at: 0,
        next_use: NextUse::Never,
        reuse_count: 0,
        placed_tick: 0,
        used_tick: 0,
        reload_cost: CostNs::IMPOSSIBLE,
        recompute_cost: CostNs(70_000),
        share_count: 0,
        mutability: Mutability::Mutable,
        owner: SessionId(0),
    }
}
