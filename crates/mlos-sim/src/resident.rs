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
//!
//! ## The budget is an arena, not a number
//!
//! It was a number -- resident bytes against a ceiling -- until step 009
//! put the kernel beside the simulator on one trace and they disagreed by
//! thirteen reads under next-use, and by none under FIFO or LRU. The
//! baselines evict in roughly the order things were placed, so their holes
//! coalesce and a byte count is a fair model of them. Next-use evicts
//! whatever is furthest away, wherever it sits, and a 1 KiB tile does not
//! fit in four 256-byte holes. The kernel paid for that; the simulator
//! did not know it existed.
//!
//! So the budget is now [`mlos_arena::Arena`], the same crate the kernel
//! places into, over a buffer the size of the budget. What fragmentation
//! costs a policy is part of what the policy costs, and a simulator that
//! left it out would be measuring a kernel nobody has.

use mlos_abi::{ObjectId, Result};
use mlos_arena::Arena;
use mlos_objtab::{NextUse, ObjectMeta};

/// What is in memory, and where each piece of it is.
pub struct Resident<'a> {
    /// Visible to `view.rs`, which opens the policy's window onto it.
    pub(crate) held: Vec<(ObjectId, ObjectMeta)>,
    /// Where the bytes go. `run.rs` asks it whether a placement would
    /// fit, which is the question `Manager::make_room` asks the kernel's.
    pub(crate) arena: Arena<'a>,
    /// Where the replay has got to, which is this simulator's equivalent
    /// of a declared stream's cursor.
    pub(crate) cursor: u32,
}

impl<'a> Resident<'a> {
    /// An empty set over `arena`.
    #[must_use]
    pub fn new(arena: Arena<'a>) -> Self {
        Self {
            held: Vec::new(),
            arena,
            cursor: 0,
        }
    }

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

    /// Places an object, or says the arena would not take it.
    ///
    /// `NoBudget` here after the caller has made room means what it means
    /// in the kernel: the free bytes exist and no single run of them is
    /// large enough. Counted as a refusal, because that is what the kernel
    /// reports for it.
    pub fn insert(
        &mut self,
        id: ObjectId,
        mut meta: ObjectMeta,
        now: u32,
        next: NextUse,
    ) -> Result<()> {
        self.cursor = now;
        let (address, _) = self.arena.place(meta.size)?;
        meta.resident_at = address;
        meta.placed_tick = now;
        meta.used_tick = now;
        meta.next_use = next;
        meta.reuse_count = meta.reuse_count.saturating_add(1);
        self.held.push((id, meta));
        Ok(())
    }

    /// Takes an object out, giving its bytes back to the arena.
    ///
    /// A release the arena refuses -- its free list is full -- leaves the
    /// object where it was, exactly as `Manager::evict` does. Nothing is
    /// changed, so the accounting cannot say bytes are free that are not.
    pub fn remove(&mut self, id: ObjectId) -> Result<u64> {
        let at = self
            .held
            .iter()
            .position(|(held, _)| *held == id)
            .ok_or(mlos_abi::Error::BadObject)?;
        let (_, meta) = self.held[at];
        self.arena.release(meta.resident_at, meta.size)?;
        self.held.remove(at);
        Ok(u64::from(meta.size))
    }
}
