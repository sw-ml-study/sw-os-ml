//! The 16550 UART, reached through x86 port I/O: the x86-64 guest's
//! console.
//!
//! Invariant: `unsafe` is confined to [`port`], and `init` leaves the FIFO
//! control register alone. Design and history: docs/notes/mlos-uart16550.md.

#![no_std]
// Empty anywhere but a bare x86-64 target: `in`/`out` exist only on x86,
// and a Linux x86-64 host must not pick this up either.
#![cfg(all(target_arch = "x86_64", target_os = "none"))]

mod port;
mod rx;
mod tx;

/// COM1's I/O port base.
pub const COM1: u16 = 0x3f8;

/// Line control: 8 data bits, no parity, one stop bit.
const LCR_8N1: u8 = 0x03;
/// Line control: divisor latch access, which turns ports 0 and 1 into
/// the baud divisor.
const LCR_DLAB: u8 = 0x80;
/// Modem control: DTR and RTS, with `OUT2` (the interrupt gate) clear.
const MCR_DTR_RTS: u8 = 0x03;

/// A 16550 UART at an I/O port base.
#[derive(Clone, Copy)]
pub struct Uart16550 {
    base: u16,
}

impl Uart16550 {
    /// A UART at `base`. The caller asserts one lives there; the only way
    /// to check would be to touch it.
    #[must_use]
    pub const fn at(base: u16) -> Self {
        Self { base }
    }

    /// The I/O port base.
    #[must_use]
    pub const fn base(&self) -> u16 {
        self.base
    }

    /// Sets 115200 8N1, interrupts off.
    ///
    /// Invariant: the FIFO control register is left alone. Enabling or
    /// resetting the FIFO discards bytes that have already arrived, and
    /// input sent before the kernel looked must not be lost.
    pub fn init(&self) {
        port::write(self.base + 1, 0x00); // IER: no interrupts
        port::write(self.base + 3, LCR_DLAB);
        port::write(self.base, 0x01); // divisor 1: 115200 baud
        port::write(self.base + 1, 0x00);
        port::write(self.base + 3, LCR_8N1);
        port::write(self.base + 4, MCR_DTR_RTS);
    }
}
