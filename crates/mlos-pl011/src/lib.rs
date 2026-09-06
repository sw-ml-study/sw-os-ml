//! The Arm PL011 UART.
//!
//! Its own crate rather than a module of `mlos-hal-aarch64`, because a
//! PrimeCell UART is not an aarch64 thing -- it turns up wherever Arm IP
//! does, and a RISC-V board with one would want exactly this code.
//!
//! Split by direction: transmit is polled, receive is interrupt-driven,
//! and the two have almost nothing in common beyond a base address.

#![no_std]

mod rx;
mod tx;

/// A PL011 UART at a known base address.
///
/// No initialisation: the loader has already configured the baud rate and
/// enabled the transmitter, and reprogramming it during bring-up is a good
/// way to lose the console at exactly the moment it becomes useful.
#[derive(Clone, Copy)]
pub struct Pl011 {
    base: usize,
}

impl Pl011 {
    /// A UART at `base`.
    ///
    /// The caller is asserting that a PL011 lives there. Nothing verifies
    /// it, because the only way to ask is to touch it.
    #[must_use]
    pub const fn at(base: usize) -> Self {
        Self { base }
    }

    /// The MMIO window this UART lives in.
    #[must_use]
    pub const fn base(&self) -> usize {
        self.base
    }
}
