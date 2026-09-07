//! An object, and where its provider will find it.

use mlos_abi::ObjectId;
use mlos_objtab::{CostNs, ObjectMeta};

/// Everything a provider needs to fetch one object.
///
/// Both the id and the handle, because providers need different halves of
/// it. A block store needs the handle and does not care what the bytes
/// mean. A recompute provider needs the id -- it has to know *which*
/// activation to rebuild, and that is exactly what class, layer and
/// tensor say.
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

/// What getting an object back would cost.
///
/// Two numbers rather than one because they behave differently: latency
/// is paid once however small the object, throughput scales with its
/// size. An eviction policy comparing a 2 MB activation against a 40 MB
/// expert gets the wrong answer from either number alone.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Cost {
    /// Before the first byte arrives.
    pub latency: CostNs,
    /// How fast the rest follows. Zero means "no faster than instantly",
    /// which is what resident memory costs.
    pub bytes_per_ms: u32,
}

impl Cost {
    /// What this costs for an object of `size` bytes.
    ///
    /// The number eviction actually compares, and the reason `cost` is on
    /// the provider trait at all: a policy that cannot ask what recovery
    /// would cost is guessing, which is precisely what LRU does.
    #[must_use]
    pub const fn for_bytes(&self, size: u32) -> CostNs {
        if self.bytes_per_ms == 0 {
            return self.latency;
        }
        let transfer = (size as u64 * 1_000_000) / self.bytes_per_ms as u64;
        CostNs(self.latency.0.saturating_add(transfer as u32))
    }
}
