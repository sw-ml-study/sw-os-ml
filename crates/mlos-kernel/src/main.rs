//! The MLOS microkernel: the entry point and the last resort.
//!
//! Invariant: `mlos_main` is entered once, by `_start`, on the boot core,
//! with a stack and a zeroed `.bss`. Design and history:
//! docs/notes/mlos-kernel.md.

#![no_std]
#![no_main]

#[cfg(target_arch = "aarch64")]
mod banner;
#[cfg(target_arch = "aarch64")]
mod boot;
#[cfg(target_arch = "aarch64")]
mod handlers;

use core::panic::PanicInfo;

/// Kernel entry, reached from the architecture's `_start`. `dtb` is what
/// the arm64 boot protocol left in `x0`; a machine whose tree cannot be
/// read parks rather than guessing a console.
///
/// # Safety
///
/// Called exactly once, by `_start`, on the boot core, with a valid stack
/// and `.bss` already zeroed. Never called from Rust.
#[cfg(target_arch = "aarch64")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mlos_main(dtb: *const u8) -> ! {
    // SAFETY: forwarded to `bring_up`, whose contract this is.
    let _ = unsafe { boot::bring_up(dtb) };
    halt()
}

// x86-64: the entry is `mlos-hal-x86-64`'s `_start` and `mlos_main` is in
// `mlos-kernel-x86-64`. Linked for its `#[no_mangle]` symbol.
#[cfg(target_arch = "x86_64")]
use mlos_kernel_x86_64 as _;

/// Last resort. There is no scheduler to yield to, so the machine stops.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    halt()
}

/// Parks the CPU.
fn halt() -> ! {
    loop {
        core::hint::spin_loop();
    }
}
