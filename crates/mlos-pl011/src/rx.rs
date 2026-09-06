//! Receive: interrupt-driven, because polling for a keystroke means
//! either burning a core or missing it.

use core::ptr;

use mlos_device::Console;

use crate::Pl011;

/// Data register. Reading takes a byte; the upper bits carry framing and
/// parity errors, discarded here -- a research kernel on an emulated UART
/// has no better answer than "ignore it".
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
/// `IMSC` bit 6: receive timeout -- fires when the FIFO holds something
/// but has stopped filling. Without it a lone keystroke waits for enough
/// friends to reach the FIFO trigger level, which for a person typing is
/// forever.
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

    /// Enables the receive and receive-timeout interrupts.
    ///
    /// Clears any pending interrupt first: the FIFO may already hold
    /// something from before we were listening, and unmasking on top of a
    /// latched interrupt fires immediately, before there is a handler to
    /// drain it.
    pub fn enable_receive_interrupt(&self) {
        // SAFETY: as above.
        unsafe {
            ptr::write_volatile((self.base() + ICR) as *mut u32, u32::MAX);
            ptr::write_volatile((self.base() + IMSC) as *mut u32, IMSC_RX | IMSC_RT);
        }
    }

    /// Acknowledges whatever the device is reporting.
    ///
    /// Separate from draining the FIFO, and both are required: clearing
    /// without draining re-raises immediately, draining without clearing
    /// leaves the controller believing the line is still asserted.
    pub fn clear_interrupt(&self) {
        // SAFETY: as above.
        unsafe { ptr::write_volatile((self.base() + ICR) as *mut u32, u32::MAX) };
    }

    /// Drains the receive FIFO, echoing what arrived.
    ///
    /// A bring-up convenience, not a line discipline: it has no notion of
    /// a line, a buffer or a cursor. Step 012's reader replaces it. The
    /// loop matters even so -- a receive timeout can deliver several bytes
    /// at once, and stopping after one leaves the rest latched.
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
