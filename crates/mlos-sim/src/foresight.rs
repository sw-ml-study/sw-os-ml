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

/// Where the object being acquired at `tick` is next wanted.
///
/// One lookup, at acquire time, and never revisited -- the object is not
/// wanted again before the position this returns, so the answer stays
/// true until the next acquire rewrites it. An earlier version wrote
/// DISTANCES instead and had to walk every resident object on every
/// access to keep them current; a kernel could not afford that, and the
/// step that said so is the reason `NextUse` carries a position.
///
/// `after` answers in trace INDICES and the caller counts one-based
/// ticks. Mixing the two is what made the first version wrong, and wrong
/// in the worst available way: an object whose next use was the very next
/// access fell through to `Never` and became the MOST evictable thing in
/// the table. Belady was being told to throw away exactly what it was
/// about to need, and it lost to LRU by three times.
#[must_use]
pub fn wanted_at(seen: &Foresight, tick: u32) -> NextUse {
    match seen.after(tick) {
        // Indices are zero-based and ticks are one-based, so the position
        // a policy compares against `Residency::now` is the index plus
        // one. Getting this wrong is invisible in every test but the
        // measurement.
        Some(at) => NextUse::At(at + 1),
        None => NextUse::Never,
    }
}
