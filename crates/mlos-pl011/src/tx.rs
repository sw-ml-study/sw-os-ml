//! Transmit: polled, because printing is not hot and a kernel that
//! cannot print until interrupts work cannot report why they do not.

use core::{fmt, ptr};

use mlos_device::Console;

use crate::Pl011;

/// Data register: writing a byte transmits it.
const DR: usize = 0x00;
/// Flag register.
const FR: usize = 0x18;
/// `FR` bit 5: transmit FIFO full.
const FR_TXFF: u32 = 1 << 5;

impl Pl011 {
    /// Transmits one byte, spinning until the FIFO has room.
    fn put(&self, byte: u8) {
        // SAFETY: `base` names a PL011's MMIO window, per `at`'s contract.
        // Volatile because the device, not the compiler, decides what a
        // read means -- an optimised-away reload of FR would spin forever
        // on a full FIFO.
        unsafe {
            while ptr::read_volatile((self.base() + FR) as *const u32) & FR_TXFF != 0 {}
            ptr::write_volatile((self.base() + DR) as *mut u32, u32::from(byte));
        }
    }
}

impl Console for Pl011 {
    fn write(&self, bytes: &[u8]) {
        for &byte in bytes {
            if byte == b'\n' {
                self.put(b'\r'); // terminals want CRLF; the kernel should not care
            }
            self.put(byte);
        }
    }
}

impl fmt::Write for Pl011 {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write(s.as_bytes());
        Ok(())
    }
}
