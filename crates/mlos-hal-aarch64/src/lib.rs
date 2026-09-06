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
mod devicetree;
mod uart;

pub use devicetree::{MAX_REGIONS, Machine};
pub use uart::Pl011;
