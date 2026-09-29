//! The simulated resident set: a `Vec` the policy scans, over a real
//! arena the bytes are placed in.
//!
//! Invariant: the policy sees only what the kernel's table could show
//! it, and no policy answer depends on enumeration order. Design and
//! history: docs/notes/mlos-sim.md.

use std::collections::HashMap;

use mlos_abi::{ObjectId, Result};
use mlos_arena::Arena;
use mlos_objtab::{NextUse, ObjectMeta};

/// What is in memory, and where each piece of it is.
pub struct Resident<'a> {
    /// Visible to `view.rs`, which opens the policy's window onto it.
    pub(crate) held: Vec<(ObjectId, ObjectMeta)>,
    /// Where each resident object sits in `held`.
    index: HashMap<ObjectId, usize>,
    /// Where the bytes go; `run.rs` asks it whether a placement fits.
    pub(crate) arena: Arena<'a>,
    /// Where the replay has got to.
    pub(crate) cursor: u32,
}

impl<'a> Resident<'a> {
    /// An empty set over `arena`.
    #[must_use]
    pub fn new(arena: Arena<'a>) -> Self {
        Self {
            held: Vec::new(),
            index: HashMap::new(),
            arena,
            cursor: 0,
        }
    }

    /// Records an acquire, and says whether it was a hit.
    pub fn touch(&mut self, id: ObjectId, now: u32, next: NextUse) -> bool {
        self.cursor = now;
        let Some((_, meta)) = self.index.get(&id).and_then(|at| self.held.get_mut(*at)) else {
            return false;
        };
        meta.used_tick = now;
        meta.reuse_count = meta.reuse_count.saturating_add(1);
        // Written once per acquire and never revisited between them.
        meta.next_use = next;
        true
    }

    /// Places an object, or says the arena would not take it. `NoBudget`
    /// means no single free run is large enough, as in the kernel.
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
        self.index.insert(id, self.held.len());
        self.held.push((id, meta));
        Ok(())
    }

    /// Takes an object out, giving its bytes back to the arena. A release
    /// the arena refuses leaves the object where it was; nothing changes.
    pub fn remove(&mut self, id: ObjectId) -> Result<u64> {
        let at = *self.index.get(&id).ok_or(mlos_abi::Error::BadObject)?;
        let (_, meta) = self.held[at];
        self.arena.release(meta.resident_at, meta.size)?;
        self.held.swap_remove(at);
        self.index.remove(&id);
        if let Some((moved, _)) = self.held.get(at) {
            self.index.insert(*moved, at);
        }
        Ok(u64::from(meta.size))
    }
}
