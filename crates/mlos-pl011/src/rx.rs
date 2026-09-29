//! Receive: interrupt-driven.
//!
//! Invariant: a receive interrupt is both cleared and drained. Clearing
//! alone re-raises it; draining alone leaves the line asserted. Design:
//! docs/notes/mlos-pl011.md.

use core::ptr;

use mlos_device::Console;

use crate::Pl011;

/// Data register. Reading takes a byte; the upper bits carry framing and
/// parity errors, discarded here.
const DR: usize = 0x00;
/// Flag register.
const FR: usize = 0x18;
/// `FR` bit 4: receive FIFO empty.
const FR_RXFE: u32 = 1 << 4;
/// Interrupt mask set/clear.
const IMSC: usize = 0x38;
/// Interrupt clear.
const ICR: usize = 0x44;
/// `IMSC` bit 4: receive interrupt.
const IMSC_RX: u32 = 1 << 4;
/// `IMSC` bit 6: receive timeout, raised when the FIFO holds something
/// but has stopped filling. Without it a lone keystroke waits for the
/// FIFO trigger level.
const IMSC_RT: u32 = 1 << 6;

impl Pl011 {
    /// Takes one byte, or `None` if the receive FIFO is empty.
    #[must_use]
    pub fn read(&self) -> Option<u8> {
        // SAFETY: `base` names a PL011's MMIO window, per `at`'s contract.
        unsafe {
            if ptr::read_volatile((self.base() + FR) as *const u32) & FR_RXFE != 0 {
                return None;
            }
            Some(ptr::read_volatile((self.base() + DR) as *const u32) as u8)
        }
    }

    /// Enables the receive and receive-timeout interrupts, clearing any
    /// pending interrupt first: unmasking on top of a latched one fires
    /// before there is a handler to drain it.
    pub fn enable_receive_interrupt(&self) {
        // SAFETY: as above.
        unsafe {
            ptr::write_volatile((self.base() + ICR) as *mut u32, u32::MAX);
            ptr::write_volatile((self.base() + IMSC) as *mut u32, IMSC_RX | IMSC_RT);
        }
    }

    /// Acknowledges whatever the device is reporting. The FIFO must be
    /// drained as well; clearing alone re-raises immediately.
    pub fn clear_interrupt(&self) {
        // SAFETY: as above.
        unsafe { ptr::write_volatile((self.base() + ICR) as *mut u32, u32::MAX) };
    }

    /// Clears the interrupt and drains the receive FIFO, echoing what
    /// arrived. Loops because a receive timeout can deliver several bytes,
    /// and stopping after one leaves the rest latched.
    pub fn drain_echo(&self) {
        self.clear_interrupt();
        while let Some(byte) = self.read() {
            match byte {
                b'\r' | b'\n' => self.write(b"\r\n"),
                0x7f | 0x08 => self.write(b"\x08 \x08"), // backspace, visibly
                byte => self.write(&[byte]),
            }
        }
    }
}
