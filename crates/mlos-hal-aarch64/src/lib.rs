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
mod uart;

pub use uart::Pl011;

/// The PL011 on QEMU's `virt` machine.
///
/// **A bring-up crutch, and deliberately a short-lived one.** The address
/// is correct for `-M virt` and for nothing else; a real platform learns
/// it from the device tree. It exists because the alternative is debugging
/// entry code with no way to print, and step 005 deletes it.
#[must_use]
pub const fn early_console() -> Pl011 {
    Pl011::at(0x0900_0000)
}
