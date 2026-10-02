//! The one manager, its arena, the replay buffers, and where they live.
//!
//! Invariant: every static here is touched only from the shell loop on
//! the boot core, and this module holds all of the crate's `unsafe`.
//! Design and history: docs/notes/mlos-lab.md.

use core::cell::UnsafeCell;

use mlos_abi::ObjectId;
use mlos_objman::Manager;
use mlos_objtab::SessionId;
use mlos_synth::disk::Disk;
use mlos_trace::Access;

use crate::{ARENA_BYTES, CAPACITY};

/// The one manager, its arena, and the providers behind it.
struct Statics {
    arena: UnsafeCell<[u8; ARENA_BYTES]>,
    manager: UnsafeCell<Option<Manager<'static, CAPACITY>>>,
    disk: UnsafeCell<Option<Disk>>,
}

// SAFETY: touched only from the shell loop, on the boot core, with
// interrupts enabled but no handler reaching any of it -- the console
// interrupt queues a byte and returns.
unsafe impl Sync for Statics {}

/// Room for the replay trace, the accesses parsed out of it, and the
/// declaration made from those. The storage a `Stream` borrows.
struct Replay {
    text: UnsafeCell<[u8; crate::replay::TEXT]>,
    accesses: UnsafeCell<[Access; crate::replay::ACCESSES]>,
    declared: UnsafeCell<[ObjectId; crate::replay::ACCESSES]>,
    next: UnsafeCell<[u32; crate::replay::ACCESSES]>,
    seen: UnsafeCell<[(ObjectId, u32); crate::replay::DISTINCT]>,
    grouped: UnsafeCell<[ObjectId; crate::replay::ACCESSES]>,
    shape: UnsafeCell<[(u32, u32); crate::replay::ACCESSES]>,
    lanes: UnsafeCell<[crate::lanes::Lane; mlos_session::MAX_SESSIONS]>,
    sessions: UnsafeCell<[SessionId; crate::replay::ACCESSES]>,
}

// SAFETY: touched only from the shell loop, on the boot core; see
// `Statics`.
unsafe impl Sync for Replay {}

/// Storage for a replay.
static REPLAY: Replay = Replay {
    text: UnsafeCell::new([0; crate::replay::TEXT]),
    accesses: UnsafeCell::new([Access::EMPTY; crate::replay::ACCESSES]),
    declared: UnsafeCell::new([ObjectId(0); crate::replay::ACCESSES]),
    next: UnsafeCell::new([0; crate::replay::ACCESSES]),
    seen: UnsafeCell::new([(ObjectId(0), 0); crate::replay::DISTINCT]),
    grouped: UnsafeCell::new([ObjectId(0); crate::replay::ACCESSES]),
    shape: UnsafeCell::new([(0, 0); crate::replay::ACCESSES]),
    lanes: UnsafeCell::new([(SessionId(0), 0, 0, 0); mlos_session::MAX_SESSIONS]),
    sessions: UnsafeCell::new([SessionId(0); crate::replay::ACCESSES]),
};

/// Every buffer a replay needs.
pub struct Room {
    /// The trace text as it comes off the device.
    pub text: &'static mut [u8],
    /// The accesses parsed out of it.
    pub accesses: &'static mut [Access],
    /// Those accesses' objects, in order: what gets declared.
    pub declared: &'static mut [ObjectId],
    /// Where each declared access is next wanted.
    pub next: &'static mut [u32],
    /// Scratch for building that chain; one slot per distinct object.
    pub seen: &'static mut [(ObjectId, u32)],
    /// The trace's objects grouped by session, for the scheduler.
    pub grouped: &'static mut [ObjectId],
    /// Each grouped access's token and index.
    pub shape: &'static mut [(u32, u32)],
    /// One lane per session.
    pub lanes: &'static mut [crate::lanes::Lane],
    /// The session of each access in the merged order.
    pub sessions: &'static mut [SessionId],
}

/// All of it at once, so no buffer can be borrowed while another is not.
pub fn replay_room() -> Room {
    // SAFETY: single-threaded access from the shell loop; see `Statics`.
    unsafe {
        Room {
            text: &mut *REPLAY.text.get(),
            accesses: &mut *REPLAY.accesses.get(),
            declared: &mut *REPLAY.declared.get(),
            next: &mut *REPLAY.next.get(),
            seen: &mut *REPLAY.seen.get(),
            grouped: &mut *REPLAY.grouped.get(),
            shape: &mut *REPLAY.shape.get(),
            lanes: &mut *REPLAY.lanes.get(),
            sessions: &mut *REPLAY.sessions.get(),
        }
    }
}

/// Storage.
static STATE: Statics = Statics {
    arena: UnsafeCell::new([0; ARENA_BYTES]),
    manager: UnsafeCell::new(None),
    disk: UnsafeCell::new(None),
};

/// The manager, whether or not it has been built.
pub fn manager() -> &'static mut Option<Manager<'static, CAPACITY>> {
    // SAFETY: single-threaded access from the shell loop; see `Statics`.
    unsafe { &mut *STATE.manager.get() }
}

/// The arena's bytes.
pub fn arena() -> &'static mut [u8] {
    // SAFETY: as above, and handed to exactly one manager at a time --
    // `register` replaces the manager that held it in the same breath.
    unsafe { &mut *STATE.arena.get() }
}

/// Finds the disk once, keeping whatever `probe` answers.
pub fn remember_disk(probe: impl FnOnce() -> Option<Disk>) -> Option<&'static Disk> {
    // SAFETY: single-threaded access from the shell loop; see `Statics`.
    let held = unsafe { &mut *STATE.disk.get() };
    if held.is_none() {
        *held = probe();
    }
    held.as_ref()
}
