//! The one manager, its arena, and where they live.
//!
//! Statics because the kernel has no allocator and the manager must
//! outlive every command that touches it. Their own module because that
//! is where the `unsafe` is: everything else in this crate is ordinary
//! code, and keeping the two apart is what makes "where is the unsafe"
//! answerable by looking at a file name.

use core::cell::UnsafeCell;

use mlos_abi::ObjectId;
use mlos_objman::Manager;
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
/// declaration made from those.
///
/// Statics because the kernel has no allocator and these are far too
/// large for a stack that already nearly lost to a 40 KiB `Manager`.
/// This is the storage a `Stream` borrows: the declaration is as long as
/// the workload, so it cannot live inside the manager and it cannot be
/// grown.
struct Replay {
    text: UnsafeCell<[u8; crate::replay::TEXT]>,
    accesses: UnsafeCell<[Access; crate::replay::ACCESSES]>,
    declared: UnsafeCell<[ObjectId; crate::replay::ACCESSES]>,
    next: UnsafeCell<[u32; crate::replay::ACCESSES]>,
    seen: UnsafeCell<[(ObjectId, u32); crate::replay::DISTINCT]>,
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
    /// Scratch for building that chain -- one slot per distinct object.
    pub seen: &'static mut [(ObjectId, u32)],
}

/// All of it at once.
///
/// One accessor rather than five because a replay needs all of them and
/// handing them out separately would let one be borrowed while the others
/// were not -- which is the shape of exactly the aliasing this module
/// exists to keep in one place.
pub fn replay_room() -> Room {
    // SAFETY: single-threaded access from the shell loop; see `Statics`.
    unsafe {
        Room {
            text: &mut *REPLAY.text.get(),
            accesses: &mut *REPLAY.accesses.get(),
            declared: &mut *REPLAY.declared.get(),
            next: &mut *REPLAY.next.get(),
            seen: &mut *REPLAY.seen.get(),
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
