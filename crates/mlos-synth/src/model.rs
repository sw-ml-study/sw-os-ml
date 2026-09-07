//! A transformer's worth of objects, without a transformer.
//!
//! Eight layers of sixteen tiles, plus a scale and an activation each.
//! No arithmetic happens: the point is the *access pattern* and the
//! residency pressure, which is what the object manager is being asked
//! about. `docs/plan.md` M1 milestone 33 makes the case -- a compelling
//! demonstration of an ML operating system needs no neural network in it.

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

/// Weights: immutable, backed by storage, and impossible to recompute.
///
/// That last part is the interesting field. There is no computation that
/// produces a trained weight, so eviction may demote these and must never
/// discard them -- which is a decision the table can only make because
/// the distinction is recorded.
#[must_use]
pub fn weights() -> ObjectMeta {
    ObjectMeta {
        size: TILE_BYTES,
        precision: Precision::Q4,
        tier: Tier::Cold,
        provider: ProviderId(2),
        handle: 0,
        resident_at: 0,
        next_use: NextUse::Never,
        reuse_count: 0,
        reload_cost: CostNs(4_000_000),
        recompute_cost: CostNs::IMPOSSIBLE,
        share_count: 0,
        mutability: Mutability::Immutable,
        owner: SessionId(0),
    }
}

/// Activations: transient, and cheaper to rebuild than to store.
///
/// The mirror image of weights, and the reason both fields exist. An
/// ordinary kernel must find somewhere to put a page it evicts; this one
/// can decide the object was never worth keeping.
#[must_use]
pub fn activations() -> ObjectMeta {
    ObjectMeta {
        size: ACTIVATION_BYTES,
        precision: Precision::Fp16,
        tier: Tier::Archive,
        provider: ProviderId(3),
        handle: 0,
        resident_at: 0,
        next_use: NextUse::Never,
        reuse_count: 0,
        reload_cost: CostNs::IMPOSSIBLE,
        recompute_cost: CostNs(70_000),
        share_count: 0,
        mutability: Mutability::Mutable,
        owner: SessionId(0),
    }
}
