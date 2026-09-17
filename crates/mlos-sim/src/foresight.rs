//! Knowing the future, because the whole trace is already here.
//!
//! This is what makes a simulator a simulator. Belady's rule is normally
//! unimplementable because it needs to know when an object will next be
//! wanted; a replay harness holds the entire access sequence, so it can
//! simply look.
//!
//! **That is not cheating, and it is not the experiment either.** The
//! claim `docs/PRD.md` makes is that a transformer HANDS the operating
//! system this knowledge -- a declared stream says which objects come
//! next, in order -- so the kernel can have it without a trace. Step 007's
//! `ml_stream_declare` is where that arrives. Until then the simulator
//! supplies it, and the numbers say what a policy WOULD do given
//! knowledge the kernel is about to be given.
//!
//! No object's next use is looked up by identity. `ObjectMeta` already
//! records `used_tick` -- when it was last wanted -- and an object that
//! has not been wanted since is still sitting at that point in the trace.
//! So the next occurrence after any resident object's last use is a
//! single indexed read, and the map from object to position that would
//! otherwise be needed does not have to exist.

use mlos_objtab::NextUse;
use mlos_trace::Access;

use crate::Resident;

/// Where the next use of each access's object is.
pub struct Foresight {
    /// For access `i`, the index of the next access to the same object,
    /// or `NEVER`.
    next: Vec<u32>,
}

/// No further use in this trace.
const NEVER: u32 = u32::MAX;

impl Foresight {
    /// Reads the whole trace backwards, recording where each object
    /// turns up next.
    ///
    /// Backwards because that is the direction the answer falls out in:
    /// walking from the end, the last position seen for an object IS its
    /// next use from any earlier point.
    #[must_use]
    pub fn read(accesses: &[Access]) -> Self {
        let mut next = vec![NEVER; accesses.len()];
        let mut latest: Vec<(u64, u32)> = Vec::new();
        for (at, access) in accesses.iter().enumerate().rev() {
            let id = access.object.0;
            match latest.iter_mut().find(|(held, _)| *held == id) {
                Some((_, position)) => {
                    next[at] = *position;
                    *position = at as u32;
                }
                None => latest.push((id, at as u32)),
            }
        }
        Self { next }
    }

    /// Where the object last wanted at `tick` turns up next.
    ///
    /// `tick` is one-based, as `ObjectMeta::used_tick` records it.
    #[must_use]
    pub fn after(&self, tick: u32) -> Option<u32> {
        match *self.next.get(tick.checked_sub(1)? as usize)? {
            NEVER => None,
            at => Some(at),
        }
    }
}

/// Writes each resident object's distance-to-next-use into the table.
///
/// A distance from NOW, recomputed every access, because that is what
/// `ml_stream_advance` will do in the kernel: advancing a stream is what
/// makes every resident object's future one step nearer. Storing an
/// absolute position instead would be cheaper and would not survive
/// contact with a kernel that has no trace to index into.
pub fn foresee(resident: &mut Resident, now: u32, seen: &Foresight) {
    // `after` answers in trace INDICES and `now` is a one-based tick.
    // Mixing the two is what made the first version of this wrong, and
    // wrong in the worst available way: an object whose next use was the
    // very next access failed the comparison, fell through to `Never`,
    // and became the MOST evictable thing in the table. Belady was being
    // told to throw away exactly what it was about to need, and it lost
    // to LRU by three times -- which is how the bug was found, because a
    // policy with strictly more information cannot honestly do that.
    let here = now.saturating_sub(1);
    for (_, meta) in &mut resident.held {
        meta.next_use = match seen.after(meta.used_tick) {
            // Zero distance is the access being served right now: the
            // least evictable thing there is, not the most.
            Some(at) if at >= here => NextUse::Distance(at - here),
            _ => NextUse::Never,
        };
    }
}
