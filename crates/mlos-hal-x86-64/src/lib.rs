//! x86-64 platform support for MLOS.
//!
//! The twin of `mlos-hal-aarch64`: it owns the entry point and the few
//! facts about the CPU that only the architecture can answer. The guest is
//! QEMU `microvm` booted by PVH (`docs/plan.md`, saga `mlos-x86-64`), so
//! the entry is 32-bit protected mode with paging off and the
//! `hvm_start_info` pointer in `%ebx` -- see [`boot`].
//!
//! `unsafe` lives here by design (AGENTS.md, "Hard constraints"). Every
//! block names the invariant it relies on.

#![no_std]
// Empty anywhere but a bare x86-64 target. `target_os = "none"` as well as
// the architecture, because a Linux x86-64 HOST is also `target_arch =
// "x86_64"`, and this crate's 32-bit entry has no business in a host build.
#![cfg(all(target_arch = "x86_64", target_os = "none"))]

mod boot;
mod phys;

pub use boot::{gigabyte_pages, long_mode, start_info_valid};
pub use phys::{bytes, c_str};

unsafe extern "C" {
    /// First byte of the image. Defined by `linker/x86_64.ld`.
    static __kernel_start: u8;
    /// One past the last byte, including `.bss` and the boot stack.
    static __kernel_end: u8;
}

/// The image's `(base, length)` in physical memory, `.bss` and the boot
/// stack included -- the same contract as `mlos_hal_aarch64::extent`.
#[must_use]
pub fn extent() -> (u64, u64) {
    // `&raw const`: these symbols have an address and no value.
    let start = &raw const __kernel_start as u64;
    let end = &raw const __kernel_end as u64;
    (start, end - start)
}

/// Waits for the next interrupt: `hlt`, the x86-64 `wfi`.
pub fn wait_for_interrupt() {
    // SAFETY: halts until an interrupt. No memory effects, no stack use.
    unsafe { core::arch::asm!("hlt", options(nomem, nostack)) };
}

/// Ends the VM through QEMU's `isa-debug-exit` device at port `0xf4`.
///
/// QEMU exits with status `(code << 1) | 1`, which is how a boot test gets
/// a real answer before there is a console to read. Without the device the
/// write goes nowhere and the CPU parks, which a test sees as a timeout.
pub fn qemu_exit(code: u8) -> ! {
    // SAFETY: port 0xf4 is the debug-exit device `mlos run` attaches; an
    // unclaimed port write has no effect on an x86 machine.
    unsafe {
        core::arch::asm!("out dx, al", in("dx") 0xf4u16, in("al") code, options(nomem, nostack))
    };
    loop {
        wait_for_interrupt();
    }
}
