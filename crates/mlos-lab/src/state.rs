//! The one manager, its arena, and where they live.
//!
//! Statics because the kernel has no allocator and the manager must
//! outlive every command that touches it. Their own module because that
//! is where the `unsafe` is: everything else in this crate is ordinary
//! code, and keeping the two apart is what makes "where is the unsafe"
//! answerable by looking at a file name.

use core::cell::UnsafeCell;

use mlos_objman::Manager;
use mlos_synth::disk::Disk;

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
