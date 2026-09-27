//! Transmit: polled, like the PL011's, and for the same reason -- a
//! kernel that cannot print until interrupts work cannot say why not.

use core::fmt;

use mlos_device::Console;

use crate::{Uart16550, port};

/// Line status register offset.
const LSR: u16 = 5;
/// `LSR` bit 5: the transmit holding register is empty.
const LSR_THRE: u8 = 1 << 5;

impl Uart16550 {
    /// Transmits one byte, spinning until the holding register is free.
    fn put(&self, byte: u8) {
        while port::read(self.base() + LSR) & LSR_THRE == 0 {
            core::hint::spin_loop();
        }
        port::write(self.base(), byte);
    }
}

impl Console for Uart16550 {
    fn write(&self, bytes: &[u8]) {
        for &byte in bytes {
            if byte == b'\n' {
                self.put(b'\r'); // terminals want CRLF; the kernel should not care
            }
            self.put(byte);
        }
    }
}

impl fmt::Write for Uart16550 {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        Console::write(self, s.as_bytes());
        Ok(())
    }
}
