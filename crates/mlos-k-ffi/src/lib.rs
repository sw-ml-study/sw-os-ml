//! The narrow bridge between MLOS and k's freestanding C core.
//!
//! Invariant: only integer and pointer values cross the ABI; callbacks are
//! installed once before k starts and remain valid for the kernel lifetime.

#![no_std]

mod fp;
mod sys;

unsafe extern "C" {
    fn k_main(argc: i32, argv: *const *const u8) -> i32;
}

/// Installs the kernel callbacks used by k's `k_sys` ABI.
///
/// # Safety
///
/// Must be called once before [`run`], with callbacks that remain valid.
pub unsafe fn install(
    write: unsafe extern "C" fn(*const u8, usize),
    read: unsafe extern "C" fn(*mut u8, usize) -> usize,
) {
    // SAFETY: forwarded to the one-time syscall hook installation.
    unsafe { sys::install(write, read) };
}

/// Enters the K REPL and returns when K invokes its exit operation.
pub fn run() {
    static NAME: &[u8] = b"k\0";
    let argv = [NAME.as_ptr(), core::ptr::null()];
    // SAFETY: the C entry point and argv are linked and valid for this call.
    unsafe { k_main(1, argv.as_ptr()) };
}

/// Enables the AArch64 FP/SIMD registers for K's portable vector helpers.
#[cfg(target_arch = "aarch64")]
pub fn enable_fp() {
    fp::enable();
}

/// Does nothing on host builds, where the kernel-only register is absent.
#[cfg(not(target_arch = "aarch64"))]
pub fn enable_fp() {}

/// Disables the AArch64 FP/SIMD registers after K exits.
#[cfg(target_arch = "aarch64")]
pub fn disable_fp() {
    fp::disable();
}

/// Does nothing on host builds, where the kernel-only register is absent.
#[cfg(not(target_arch = "aarch64"))]
pub fn disable_fp() {}
