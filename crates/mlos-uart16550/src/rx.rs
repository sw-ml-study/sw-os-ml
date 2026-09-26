//! Receive: polled for now. The interrupt-driven path is step
//! `x86-interrupts`, which needs the IOAPIC this step does not have.

use crate::{Uart16550, port};

/// Line status register offset.
const LSR: u16 = 5;
/// `LSR` bit 0: a received byte is waiting.
const LSR_DR: u8 = 1 << 0;

impl Uart16550 {
    /// Takes one byte, or `None` if nothing has arrived.
    #[must_use]
    pub fn read(&self) -> Option<u8> {
        if port::read(self.base() + LSR) & LSR_DR == 0 {
            return None;
        }
        Some(port::read(self.base()))
    }
}
