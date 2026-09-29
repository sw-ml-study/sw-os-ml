//! Knowing the future, because the whole trace is already here.
//!
//! Invariant: the chain is `mlos_stream::chain`, the same one the kernel
//! runs; only the buffer allocation is the simulator's. Design and
//! history: docs/notes/mlos-sim.md.

use mlos_abi::ObjectId;
use mlos_objtab::NextUse;
use mlos_stream::NEVER;
use mlos_trace::Access;

/// Where the next use of each access's object is.
pub struct Foresight {
    /// For access `i`, the index of the next access to the same object,
    /// or `NEVER`.
    next: Vec<u32>,
}

impl Foresight {
    /// Chains the whole trace, recording where each object turns up next.
    /// Buffers are sized from the trace, so `chain` cannot fail.
    #[must_use]
    pub fn read(accesses: &[Access]) -> Self {
        let objects: Vec<ObjectId> = accesses.iter().map(|access| access.object).collect();
        let mut next = vec![NEVER; objects.len()];
        let mut seen = vec![(ObjectId(0), 0u32); objects.len()];
        mlos_stream::chain(&objects, &mut next, &mut seen).expect("buffers sized from the trace");
        Self { next }
    }

    /// Where the object last wanted at `tick` turns up next, as a
    /// zero-based trace index. `tick` is one-based, as
    /// `ObjectMeta::used_tick` records it.
    #[must_use]
    pub fn after(&self, tick: u32) -> Option<u32> {
        match *self.next.get(tick.checked_sub(1)? as usize)? {
            NEVER => None,
            at => Some(at),
        }
    }
}

/// Where the object being acquired at `tick` is next wanted, as a
/// one-based tick a policy can compare against `Residency::now`. Looked
/// up once, at acquire time; the answer holds until the next acquire.
#[must_use]
pub fn wanted_at(seen: &Foresight, tick: u32) -> NextUse {
    match seen.after(tick) {
        // Indices are zero-based and ticks are one-based; off by one here
        // makes the next access the most evictable object.
        Some(at) => NextUse::At(at + 1),
        None => NextUse::Never,
    }
}
