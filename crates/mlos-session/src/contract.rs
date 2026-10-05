//! What a session was promised, and what it was given.
//!
//! Invariant: `Delivered` only ever moves toward worse, so it answers for
//! the whole life of the session. Design: docs/notes/mlos-session.md.

use mlos_objtab::{Precision, SessionId};

use crate::Sessions;

/// What a session was promised. `resident_ceiling` is enforced by the
/// manager; `latency_ceiling` is read by the parameter-major scheduler,
/// which serves a session past it out of turn; `quality_floor` is the
/// coarsest precision the degradation ladder may take its objects to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Contract {
    /// The coarsest precision acceptable. `Precision::Ternary`, the
    /// coarsest there is, means anything goes.
    pub quality_floor: Precision,
    /// The most acquires served to other sessions this one will wait
    /// between two of its own before it is served out of turn, or zero.
    /// Acquires, not nanoseconds: the kernel's clock is the acquire count
    /// (`Manager::clock`), the same unit `next_use` is in.
    pub latency_ceiling: u32,
    /// The most bytes this session may hold resident, or zero.
    pub resident_ceiling: u64,
    /// Tokens of context it will hold as KV, which is what admission
    /// charges it for in bytes. Zero: undeclared, and nothing is charged.
    pub context: u32,
}

impl Contract {
    /// No promises: the contract every existing workload runs under.
    pub const NONE: Self = Self {
        quality_floor: Precision::Ternary,
        latency_ceiling: 0,
        resident_ceiling: 0,
        context: 0,
    };

    /// Whether the contract allows an object at `precision`: no coarser
    /// than the floor.
    #[must_use]
    pub const fn permits(&self, precision: Precision) -> bool {
        precision as u8 <= self.quality_floor as u8
    }
}

impl Default for Contract {
    fn default() -> Self {
        Self::NONE
    }
}

/// What a session was actually given, against what it was promised. Each
/// field only moves toward worse, so `ml_session_contract` answers for
/// the whole life of the session.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Delivered {
    /// The coarsest precision any of its objects was served at.
    pub coarsest: Precision,
    /// The longest a token took, in acquires from the end of the previous
    /// token (or the start) to its own end.
    pub worst_period: u32,
    /// The most bytes it held resident at once.
    pub peak_resident: u64,
    /// The deepest ladder rung it was put on.
    pub deepest: crate::Rung,
    /// Bytes the ladder read to degrade its objects: requantized,
    /// summarised. What `Rc` counts.
    pub recomputed: u64,
}

impl Default for Delivered {
    fn default() -> Self {
        Self {
            coarsest: Precision::Fp16,
            worst_period: 0,
            peak_resident: 0,
            deepest: crate::Rung::L0,
            recomputed: 0,
        }
    }
}

impl<const N: usize> Sessions<N> {
    /// Records how long `owner`'s last token took, keeping the worst.
    pub fn took(&mut self, owner: SessionId, period: u32) {
        if let Some(session) = self.get_mut(owner) {
            session.delivered.worst_period = session.delivered.worst_period.max(period);
        }
    }
}
