//! aarch64 platform support for MLOS.
//!
//! The first code in this repo that must be right on real silicon. It owns
//! the entry point and, for now, an early console at a hardcoded address;
//! step 005 replaces that address with one discovered from the device tree.
//!
//! `unsafe` lives here by design (AGENTS.md, "Hard constraints"). Every
//! block names the invariant it relies on.

#![no_std]

mod boot;
mod timer;

pub use timer::{GenericTimer, TIMER_PPI};

// The image extent lives here rather than in its own module: it is one
// function, and `mlos-hal-aarch64` has four modules, which is the
// `sw-checklist` gate. See AGENTS.md -- design to the gate, not the line.
//
// The device tree describes what the MACHINE has. It knows nothing about
// what a loader put into it, so the fact that the bottom of RAM is
// occupied by us has to come from the linker script that placed us there.

unsafe extern "C" {
    /// First byte of the image. Defined by `linker/aarch64.ld`.
    static __kernel_start: u8;
    /// One past the last byte, including `.bss` and the boot stack.
    static __kernel_end: u8;
}

/// The image's `(base, length)` in physical memory.
///
/// Includes `.bss` and the boot stack, which occupy RAM but no file bytes.
/// Reporting only the file's worth would leave the stack looking
/// allocatable, which is a subtle way to hand out the ground you are
/// standing on.
#[must_use]
pub fn extent() -> (u64, u64) {
    // `&raw const` rather than a reference: these symbols have no valid
    // value, only an address, and forming a `&u8` to them would claim
    // otherwise.
    let start = &raw const __kernel_start as u64;
    let end = &raw const __kernel_end as u64;
    (start, end - start)
}

/// Waits for the next interrupt.
///
/// The architecture's answer to "nothing to do": `wfi` here, `hlt` on
/// x86-64. It lives with the architecture rather than with the loop that
/// calls it, so the loop does not have to know which one it is on.
pub fn wait_for_interrupt() {
    // SAFETY: waits for an interrupt. No memory effects, no stack use.
    unsafe { core::arch::asm!("wfi", options(nomem, nostack)) };
}
