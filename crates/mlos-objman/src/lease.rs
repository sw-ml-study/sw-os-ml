//! Claims on an object, and what they mean.
//!
//! Invariant: a handle's address is valid only while its lease is held,
//! and `share_count` is the number of leases that hold the object right
//! now -- raised by acquire, lowered by release, zeroed by eviction.
//! Design: docs/notes/mlos-objman.md.

use mlos_abi::{Error, ObjectId, Result};
use mlos_objtab::SessionId;

use crate::Manager;

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

    /// Whether this lease is a hold that `share_count` counts. A guess is
    /// not a hold.
    #[must_use]
    pub const fn counts(self) -> bool {
        !matches!(self, Self::Speculative)
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

impl<const N: usize> Manager<'_, N> {
    /// `ml_release`: the holder is done with `id`, which it held under
    /// `lease`. Lowers `share_count` if the lease counted. `BadObject` if
    /// the object is unknown; releasing an evicted object is not an
    /// error, since eviction already dropped every lease.
    pub fn release(&mut self, id: ObjectId, lease: Lease) -> Result<()> {
        let held = self.table.get_mut(id).ok_or(Error::BadObject)?;
        if lease.counts() && held.resident_at != 0 {
            held.share_count = held.share_count.saturating_sub(1);
        }
        Ok(())
    }

    /// Acquires under a `Streaming` lease and releases it at once: one
    /// access by a consumer that will not look back, which is what a
    /// replayed trace and a sweep are. `share_count` is unchanged by it.
    pub fn consume(&mut self, id: ObjectId, by: SessionId) -> Result<Handle> {
        let handle = self.acquire(id, Lease::Streaming, by)?;
        self.release(id, Lease::Streaming)?;
        Ok(handle)
    }
}
