//! Claims on an object, and what they mean.
//!
//! Invariant: a handle's address is valid only while its lease is held.
//! Design: docs/notes/mlos-objman.md.

use mlos_abi::ObjectId;

/// How long a consumer needs an object, and how strongly. A kind, not a
/// count, so a policy can tell a pin from a prefetch.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lease {
    /// Resident until released; counts against the session's budget.
    Pin,
    /// Resident for one operation; may be revoked between operations.
    Borrow,
    /// Will be consumed in order, once; the holder promises not to look
    /// back.
    Streaming,
    /// Fetched on a guess. Free to drop.
    Speculative,
}

impl Lease {
    /// Whether holding this lease should stop the object being evicted.
    #[must_use]
    pub const fn pins(self) -> bool {
        matches!(self, Self::Pin)
    }
}

/// A resident object a caller may read.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Handle {
    /// Which object.
    pub id: ObjectId,
    /// Where its bytes are, right now. Valid while the lease is held and
    /// meaningless afterwards.
    pub address: u64,
    /// How many bytes.
    pub size: u32,
}
