//! The simulated resident set.
//!
//! A `Vec` with linear scans, deliberately, because that is what the
//! kernel's open-addressed table can offer a policy and the point of the
//! simulator is to run the code the kernel will run. A `HashMap` here
//! would let a policy be written that the kernel cannot host, and the
//! discovery would come at step 009 after every number had been produced.
//!
//! Insertion order is preserved and never rearranged, which is what makes
//! [`Residency::at`] stable -- a policy asked twice about an unchanged set
//! must name the same victim, or a replay stops being deterministic and
//! every comparison becomes an argument.

use mlos_abi::ObjectId;
use mlos_objtab::{NextUse, ObjectMeta};

/// What is in memory, and how much of it there is.
#[derive(Default)]
pub struct Resident {
    /// Visible to `view.rs`, which opens the policy's window onto it.
    pub(crate) held: Vec<(ObjectId, ObjectMeta)>,
    bytes: u64,
    /// Where the replay has got to, which is this simulator's equivalent
    /// of a declared stream's cursor.
    pub(crate) cursor: u32,
}

impl Resident {
    /// Records an acquire, and says whether it was a hit.
    ///
    /// Finding and touching in one call because they are one decision:
    /// separating them leaves a caller holding an index into a collection
    /// it is about to mutate, which is the shape of a bug rather than of
    /// an API.
    pub fn touch(&mut self, id: ObjectId, now: u32, next: NextUse) -> bool {
        self.cursor = now;
        let Some((_, meta)) = self.held.iter_mut().find(|(held, _)| *held == id) else {
            return false;
        };
        meta.used_tick = now;
        meta.reuse_count = meta.reuse_count.saturating_add(1);
        // Written ONCE, here, and not touched again until this object is
        // acquired again -- which is the whole point of storing a
        // position rather than a distance. The earlier version walked
        // every resident object on every access to keep distances
        // current; a kernel could not afford that and now neither side
        // does it.
        meta.next_use = next;
        true
    }

    /// Puts an object in, and says nothing about whether it fits.
    ///
    /// The caller decides that, because only the caller knows the budget
    /// and what it was willing to evict to get there.
    pub fn insert(&mut self, id: ObjectId, mut meta: ObjectMeta, now: u32, next: NextUse) {
        self.cursor = now;
        meta.placed_tick = now;
        meta.used_tick = now;
        meta.next_use = next;
        meta.reuse_count = meta.reuse_count.saturating_add(1);
        self.bytes += u64::from(meta.size);
        self.held.push((id, meta));
    }

    /// Takes an object out, returning what it freed.
    pub fn remove(&mut self, id: ObjectId) -> u64 {
        let Some(at) = self.held.iter().position(|(held, _)| *held == id) else {
            return 0;
        };
        let (_, meta) = self.held.remove(at);
        self.bytes -= u64::from(meta.size);
        u64::from(meta.size)
    }

    /// How many bytes are resident.
    #[must_use]
    pub const fn bytes(&self) -> u64 {
        self.bytes
    }
}
