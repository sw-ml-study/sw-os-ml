//! Claims on an object, and what they mean.

use mlos_abi::ObjectId;

/// How long a consumer needs an object, and how strongly.
///
/// Leases rather than reference counts, because a count says only *how
/// many* while a lease says *what kind*. A policy that cannot tell a
/// pinned object from a speculatively prefetched one has to treat the
/// prefetch as sacred, which defeats the point of prefetching.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lease {
    /// Resident until released; counts against the session's budget.
    Pin,
    /// Resident for one operation; may be revoked between operations.
    Borrow,
    /// Will be consumed in order, once. The holder promises not to look
    /// back, which is what lets the object be dropped as it passes.
    Streaming,
    /// Fetched on a guess. Free to drop, and a speculative lease that is
    /// never upgraded is exactly what a prefetch miss *is* -- which is
    /// how `Ph` gets counted.
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
    /// Where its bytes are, right now.
    ///
    /// "Right now" is the whole point: the address is valid while the
    /// lease is held and means nothing afterwards. A consumer that stores
    /// it past a release has assumed the thing this design exists to
    /// prevent -- that where an object lives is a property of the object.
    pub address: u64,
    /// How many bytes.
    pub size: u32,
}
