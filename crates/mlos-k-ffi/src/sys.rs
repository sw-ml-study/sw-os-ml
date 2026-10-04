//! K's fixed-arity syscall boundary.

use core::cell::UnsafeCell;

type Write = unsafe extern "C" fn(*const u8, usize);
type Read = unsafe extern "C" fn(*mut u8, usize) -> usize;

struct Hooks(UnsafeCell<Option<(Write, Read)>>);

// SAFETY: hooks are written once before K runs and then read-only.
unsafe impl Sync for Hooks {}

static HOOKS: Hooks = Hooks(UnsafeCell::new(None));

pub unsafe fn install(write: Write, read: Read) {
    // SAFETY: the caller guarantees this is the one pre-run write.
    unsafe { *HOOKS.0.get() = Some((write, read)) };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn k_sys(
    nr: u64,
    _fd: u64,
    buf: u64,
    len: u64,
    _d: u64,
    _e: u64,
    _f: u64,
) -> u64 {
    // SAFETY: installed before K starts and never mutated afterwards.
    let Some((write, read)) = (unsafe { (*HOOKS.0.get()).as_ref() }) else {
        return u64::MAX;
    };
    match nr {
        0 => {
            // SAFETY: K supplied a writable buffer of `len` bytes.
            unsafe { read(buf as *mut u8, len as usize) as u64 }
        }
        1 => {
            // SAFETY: K supplied a readable buffer of `len` bytes.
            unsafe { write(buf as *const u8, len as usize) };
            len
        }
        60 => 0,
        _ => u64::MAX - 37,
    }
}
