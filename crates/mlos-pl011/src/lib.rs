//! The Arm PL011 UART, split by direction: polled transmit,
//! interrupt-driven receive.
//!
//! Invariant: nothing here reprograms the UART; the loader's baud rate
//! and enables are relied on as found. Design: docs/notes/mlos-pl011.md.

#![no_std]

mod rx;
mod tx;

/// A PL011 UART at a known base address, used as the loader left it.
#[derive(Clone, Copy)]
pub struct Pl011 {
    base: usize,
}

impl Pl011 {
    /// A UART at `base`. Nothing verifies one is there: the caller is
    /// asserting it, and every access in this crate relies on that.
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
