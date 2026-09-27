//! Receive: [`Uart16550::read`] takes what has arrived, and
//! [`Uart16550::enable_receive_interrupt`] makes each arrival raise IRQ 4
//! -- routed by the IOAPIC, so the kernel can sleep until a key is typed.

use crate::{Uart16550, port};

/// Line status register offset.
const LSR: u16 = 5;
/// `LSR` bit 0: a received byte is waiting.
const LSR_DR: u8 = 1 << 0;
/// Interrupt enable register offset, and its received-data-available bit.
const IER: u16 = 1;
const IER_RDA: u8 = 1 << 0;
/// Modem control offset; `OUT2` is what gates the UART's interrupt onto
/// the ISA line on a PC, and DTR|RTS stay as `init` set them.
const MCR: u16 = 4;
const MCR_OUT2_DTR_RTS: u8 = 0x0b;

impl Uart16550 {
    /// Takes one byte, or `None` if nothing has arrived.
    #[must_use]
    pub fn read(&self) -> Option<u8> {
        if port::read(self.base() + LSR) & LSR_DR == 0 {
            return None;
        }
        Some(port::read(self.base()))
    }

    /// Raises IRQ 4 whenever a byte arrives.
    ///
    /// A byte already waiting raises it as soon as this is enabled, so
    /// input typed before the kernel was listening is not stranded.
    pub fn enable_receive_interrupt(&self) {
        port::write(self.base() + MCR, MCR_OUT2_DTR_RTS);
        port::write(self.base() + IER, IER_RDA);
    }
}
