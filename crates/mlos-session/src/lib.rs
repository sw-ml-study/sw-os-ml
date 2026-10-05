//! Sessions as kernel objects: a record, a contract, a budget, what each
//! one owns, and what each one was actually given.
//!
//! Invariant: a session's `resident` bytes never exceed its contract's
//! `resident_ceiling` when one is set, a destroyed session leaves no
//! object of its own in the table, and `delivered` only ever gets worse.
//! Design and history: docs/notes/mlos-session.md.

#![no_std]
#![forbid(unsafe_code)]

mod budget;
mod contract;
mod table;

pub use contract::{Contract, Delivered};
pub use mlos_objtab::{Precision, Rung, SessionId};

/// How many sessions a manager can hold at once. Sixteen is more than any
/// workload here declares; a seventeenth is refused, which is admission
/// control's first, crude form.
pub const MAX_SESSIONS: usize = 16;

/// One session: who it is, what it was promised, what it holds, what it
/// got.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Session {
    /// Its id, which is what `ObjectMeta::owner` and a session-scoped
    /// `ObjectId` carry.
    pub id: SessionId,
    /// What it was promised.
    pub contract: Contract,
    /// Bytes it owns that are resident right now.
    pub resident: u64,
    /// What it has been given so far.
    pub delivered: Delivered,
    /// The ladder rung it is on now. Only the ladder moves it.
    pub rung: Rung,
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

    /// The live sessions, in slot order: the order they were created or
    /// adopted in while no slot was freed.
    pub fn each(&self) -> impl Iterator<Item = &Session> {
        self.slots.iter().filter_map(Option::as_ref)
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
