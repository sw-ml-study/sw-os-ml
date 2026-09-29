//! One residency transition, and how to describe one.
//!
//! Invariant: an event is built once, after the outcome is known, never
//! recorded and amended. Design: docs/notes/mlos-events.md.

use mlos_abi::{Error, ObjectId};
use mlos_objtab::{CostNs, ObjectMeta, SessionId, Tier};

use crate::Kind;

/// One residency transition.
#[derive(Clone, Copy)]
pub struct Event {
    /// Monotonic, assigned on record. The ordering a replay depends on.
    pub seq: u32,
    /// What happened.
    pub kind: Kind,
    /// Which object.
    pub object: ObjectId,
    /// Who asked for it.
    pub session: SessionId,
    /// Where in the arena it went, for [`Kind::Placed`].
    pub offset: u64,
    /// How many bytes moved. Zero when none did.
    pub bytes: u32,
    /// What it cost, in nanoseconds, as the provider charges it.
    pub cost: u32,
    /// The tier it came from.
    pub tier: Tier,
    /// Why not, for [`Kind::Refused`].
    pub why: Option<Error>,
}

impl Event {
    /// An object that arrived in the arena at `offset`.
    #[must_use]
    pub const fn placed(
        object: ObjectId,
        by: SessionId,
        meta: &ObjectMeta,
        cost: CostNs,
        offset: u64,
    ) -> Self {
        Self {
            seq: 0,
            kind: Kind::Placed,
            object,
            session: by,
            offset,
            bytes: meta.size,
            cost: cost.0,
            tier: meta.tier,
            why: None,
        }
    }

    /// An object that could not be had, and why. `bytes` is zero because
    /// nothing moved; `cost` is what the fetch would have charged.
    #[must_use]
    pub const fn refused(
        object: ObjectId,
        by: SessionId,
        meta: &ObjectMeta,
        cost: CostNs,
        why: Error,
    ) -> Self {
        Self {
            seq: 0,
            kind: Kind::Refused,
            object,
            session: by,
            offset: 0,
            bytes: 0,
            cost: cost.0,
            tier: meta.tier,
            why: Some(why),
        }
    }

    /// An object thrown away to make room. `bytes` is what came back.
    #[must_use]
    pub const fn evicted(object: ObjectId, by: SessionId, meta: &ObjectMeta) -> Self {
        Self {
            seq: 0,
            kind: Kind::Evicted,
            object,
            session: by,
            offset: 0,
            bytes: meta.size,
            cost: 0,
            tier: meta.home,
            why: None,
        }
    }

    /// An object that was already resident when it was wanted. No cost:
    /// nothing was fetched.
    #[must_use]
    pub const fn hit(object: ObjectId, by: SessionId, bytes: u32) -> Self {
        Self {
            seq: 0,
            kind: Kind::Hit,
            object,
            session: by,
            offset: 0,
            bytes,
            cost: 0,
            tier: Tier::Warm,
            why: None,
        }
    }
}
