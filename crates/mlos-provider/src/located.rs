//! An object, and where its provider will find it.
//!
//! Invariant: `Located` is built from a table entry, never assembled by
//! hand. Design: docs/notes/mlos-provider.md.

use mlos_abi::ObjectId;
use mlos_objtab::{CostNs, ObjectMeta};

/// Everything a provider needs to fetch one object: the id for providers
/// that rebuild, the handle for providers that read.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Located {
    /// Which object.
    pub id: ObjectId,
    /// Where, as its provider understands "where".
    pub handle: u64,
    /// How many bytes it is.
    pub size: u32,
}

impl Located {
    /// Reads a table entry as a fetch request.
    #[must_use]
    pub const fn new(id: ObjectId, meta: &ObjectMeta) -> Self {
        Self {
            id,
            handle: meta.handle,
            size: meta.size,
        }
    }
}

/// What getting an object back would cost: a latency paid once and a
/// throughput that scales with size.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Cost {
    /// Before the first byte arrives.
    pub latency: CostNs,
    /// How fast the rest follows. Zero means "no faster than instantly",
    /// which is what resident memory costs.
    pub bytes_per_ms: u32,
}

impl Cost {
    /// What this costs for an object of `size` bytes: the one number
    /// eviction compares. Latency alone when `bytes_per_ms` is zero.
    #[must_use]
    pub const fn for_bytes(&self, size: u32) -> CostNs {
        if self.bytes_per_ms == 0 {
            return self.latency;
        }
        let transfer = (size as u64 * 1_000_000) / self.bytes_per_ms as u64;
        CostNs(self.latency.0.saturating_add(transfer as u32))
    }
}
