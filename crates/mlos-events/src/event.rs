//! One residency transition, and how to describe one.
//!
//! Constructors rather than a struct literal at each call site, because
//! the fault path is not where the shape of an event should be decided.
//! It is also what lets the fault path record ONCE, after the outcome is
//! known, instead of recording a hope and amending it -- an amendment is a
//! second thing to remember on a path that already has two exits.

use mlos_abi::{Error, ObjectId};
use mlos_objtab::{CostNs, ObjectMeta, SessionId, Tier};

/// What happened.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// Bytes arrived in the arena. Residency went up.
    Placed,
    /// A resident object was wanted again. Nothing moved.
    Hit,
    /// There was no room, and nothing was thrown away to make some.
    ///
    /// Not a failure. Until M3 there is no policy to choose a victim, so
    /// refusing is the honest outcome -- and it is the most interesting
    /// event in the stream, because it is the moment the system ran out of
    /// the resource it exists to manage.
    Refused,
    /// A resident object was thrown away. Reserved for M3.
    Evicted,
}

impl Kind {
    /// The word a consumer matches on.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Placed => "placed",
            Self::Hit => "hit",
            Self::Refused => "refused",
            Self::Evicted => "evicted",
        }
    }
}

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
    ///
    /// Carried so an access trace derived from a stream can say who made
    /// each acquire rather than assume. There is one session today; the
    /// assumption would be right and would stop being right at M4,
    /// silently, in a file somebody was measuring from.
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

    /// An object that could not be had, and why.
    ///
    /// `bytes` is zero: nothing moved. The cost is kept, because what a
    /// refusal WOULD have cost is the number an admission policy is
    /// deciding against.
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

    /// An object that was already resident when it was wanted.
    ///
    /// The cheap case, and the one a residency policy exists to produce
    /// more of. No cost, because nothing was fetched -- which is the
    /// point of recording it at all.
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
