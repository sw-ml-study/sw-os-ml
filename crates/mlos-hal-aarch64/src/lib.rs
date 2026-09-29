//! aarch64 platform support: the entry point, the image extent, the timer.
//!
//! Invariant: every `unsafe` block here names what it relies on, and
//! nothing above this crate touches a system register. Design and
//! history: docs/notes/mlos-hal-aarch64.md.

#![no_std]
// Empty on any other architecture, so the workspace gate can sweep every
// crate without an exclude list.
#![cfg(target_arch = "aarch64")]

mod boot;
mod timer;

pub use timer::{GenericTimer, TIMER_PPI};

unsafe extern "C" {
    /// First byte of the image. Defined by `linker/aarch64.ld`.
    static __kernel_start: u8;
    /// One past the last byte, including `.bss` and the boot stack.
    static __kernel_end: u8;
}

/// The image's `(base, length)` in physical memory, including `.bss` and
/// the boot stack: RAM that is occupied but holds no file bytes.
#[must_use]
pub fn extent() -> (u64, u64) {
    // `&raw const`, not a reference: these symbols have an address but no
    // valid value.
    let start = &raw const __kernel_start as u64;
    let end = &raw const __kernel_end as u64;
    (start, end - start)
}

/// Waits for the next interrupt (`wfi`).
pub fn wait_for_interrupt() {
    // SAFETY: waits for an interrupt. No memory effects, no stack use.
    unsafe { core::arch::asm!("wfi", options(nomem, nostack)) };
}
