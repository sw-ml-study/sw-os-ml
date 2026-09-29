//! KV blocks: the half of a decode loop that accumulates.
//!
//! Invariant: the class is session-scoped, so `Fields::model` carries a
//! session id, never a model id; otherwise one session's KV could alias
//! another's. Design and history: docs/notes/mlos-synth.md.

use mlos_abi::{Fields, ObjectClass, ObjectId};
use mlos_objtab::{
    CostNs, Mutability, NextUse, ObjectMeta, Precision, ProviderId, SessionId, Tier,
};

/// Bytes per block, per layer: one position's keys and values.
pub const BYTES: u32 = 256;

/// The id of one session's KV block for one layer and position.
#[must_use]
pub fn block(session: u16, layer: u16, position: u16) -> ObjectId {
    ObjectId::new(
        ObjectClass::KvBlock,
        Fields {
            model: session,
            layer,
            tensor: position,
            tile: 0,
        },
    )
}

/// What the table would know about a KV block: mutable, owned by
/// `session`, warm, and recomputable at a price above reloading.
#[must_use]
pub fn meta(session: u16) -> ObjectMeta {
    ObjectMeta {
        size: BYTES,
        precision: Precision::Fp16,
        tier: Tier::Warm,
        home: Tier::Warm,
        provider: ProviderId(3),
        handle: 0,
        resident_at: 0,
        next_use: NextUse::Never,
        reuse_count: 0,
        placed_tick: 0,
        used_tick: 0,
        // Spill and read back.
        reload_cost: CostNs(400_000),
        // Re-run attention over the prefix. Must stay dearer than the spill.
        recompute_cost: CostNs(2_000_000),
        share_count: 0,
        mutability: Mutability::Mutable,
        owner: SessionId(session),
    }
}
