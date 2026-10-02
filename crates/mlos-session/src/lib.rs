//! Sessions as kernel objects: a record, a contract, a budget, and what
//! each one owns.
//!
//! Invariant: a session's `resident` bytes never exceed its contract's
//! `resident_ceiling` when one is set, and a destroyed session leaves no
//! object of its own in the table. Design and history:
//! docs/notes/mlos-session.md.

#![no_std]
#![forbid(unsafe_code)]

mod budget;
mod table;

pub use mlos_objtab::SessionId;

/// How many sessions a manager can hold at once. Sixteen is more than any
/// workload here declares; a seventeenth is refused, which is admission
/// control's first, crude form.
pub const MAX_SESSIONS: usize = 16;

/// What a session was promised. `resident_ceiling` is enforced by the
/// manager; `latency_ceiling` is read by the parameter-major scheduler,
/// which serves a session past it out of turn; the quality floor waits
/// for M5's degradation ladder. Zero means "none".
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Contract {
    /// The lowest precision the session will accept, as a `Precision`
    /// discriminant, or zero for any.
    pub quality_floor: u8,
    /// The most acquires served to other sessions this one will wait
    /// between two of its own before it is served out of turn, or zero.
    /// Acquires, not nanoseconds: the kernel's clock is the acquire count
    /// (`Manager::clock`), the same unit `next_use` is in.
    pub latency_ceiling: u32,
    /// The most bytes this session may hold resident, or zero.
    pub resident_ceiling: u64,
}

impl Contract {
    /// No promises: the contract every existing workload runs under.
    pub const NONE: Self = Self {
        quality_floor: 0,
        latency_ceiling: 0,
        resident_ceiling: 0,
    };
}

/// One session: who it is, what it was promised, what it holds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Session {
    /// Its id, which is what `ObjectMeta::owner` and a session-scoped
    /// `ObjectId` carry.
    pub id: SessionId,
    /// What it was promised.
    pub contract: Contract,
    /// Bytes it owns that are resident right now.
    pub resident: u64,
}

/// The sessions a manager knows, in fixed slots.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Sessions<const N: usize> {
    slots: [Option<Session>; N],
}

impl<const N: usize> Sessions<N> {
    /// No sessions.
    pub const EMPTY: Self = Self { slots: [None; N] };

    /// The session in slot `index`, if one lives there. Slot order is
    /// stable while nothing is created or destroyed.
    #[must_use]
    pub fn at(&self, index: usize) -> Option<&Session> {
        self.slots.get(index)?.as_ref()
    }

    /// How many sessions are live.
    #[must_use]
    pub fn live(&self) -> u32 {
        self.slots.iter().filter(|s| s.is_some()).count() as u32
    }

    /// Adopts every id in `ids` that is not already known, under
    /// `contract`, and says how many were new. What a replay does with a
    /// trace's session column before it starts.
    pub fn adopt_each(
        &mut self,
        ids: impl IntoIterator<Item = SessionId>,
        contract: Contract,
    ) -> u64 {
        ids.into_iter()
            .filter(|id| self.adopt(*id, contract).is_ok())
            .count() as u64
    }
}
