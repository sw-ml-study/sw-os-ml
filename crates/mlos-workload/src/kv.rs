//! KV blocks: the half of a decode loop that accumulates.
//!
//! Here rather than in `mlos-synth` because `mlos-synth` describes what a
//! model IS -- a fixed inventory of weights, the same for every run --
//! and KV is not that. It is produced by running the model, it grows with
//! the conversation, it belongs to one session, and two sessions asking
//! the same question hold different KV. It is a property of the WORKLOAD.
//!
//! When M4 makes sessions real the kernel will register these and this
//! will move. Until then nothing in the kernel has ever seen a KV block,
//! and putting the definition where the only user is keeps that honest.
//!
//! The class is session-scoped: [`ObjectClass::is_session_scoped`] says
//! so, and it means `Fields::model` carries a session id rather than a
//! model id. Getting that wrong would let one session's KV alias
//! another's, which is why the ABI made it a function rather than a
//! convention.

use mlos_abi::{Fields, ObjectClass, ObjectId};
use mlos_objtab::{
    CostNs, Mutability, NextUse, ObjectMeta, Precision, ProviderId, SessionId, Tier,
};

/// Bytes per block, per layer.
///
/// Small beside a weight tile on purpose. One block is one position's
/// keys and values for one layer; a tile is a slab of a weight matrix.
/// The interesting pressure comes from how MANY blocks accumulate, not
/// from any one of them being large.
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

/// What the table would know about a KV block.
///
/// The opposite of a weight tile in every respect a policy cares about.
/// A tile is immutable, shared by every session, and cannot be recomputed
/// at any price. A block is mutable, owned by one session, and CAN be
/// recomputed -- by re-running attention over the prefix, which is
/// expensive and gets more expensive the further back it sits.
///
/// `Warm` rather than `Cold`: a block does not come from storage the way
/// a weight does. It comes from having done the work once.
#[must_use]
pub fn meta(session: u16) -> ObjectMeta {
    ObjectMeta {
        size: BYTES,
        precision: Precision::Fp16,
        tier: Tier::Warm,
        provider: ProviderId(3),
        handle: 0,
        resident_at: 0,
        next_use: NextUse::Never,
        reuse_count: 0,
        placed_tick: 0,
        used_tick: 0,
        // Spilling to storage and reading it back, which is what a real
        // serving engine does under pressure.
        reload_cost: CostNs(400_000),
        // Re-running attention over the prefix. Dearer than the spill,
        // which is why a cost-aware policy would spill rather than drop.
        recompute_cost: CostNs(2_000_000),
        share_count: 0,
        mutability: Mutability::Mutable,
        owner: SessionId(session),
    }
}
