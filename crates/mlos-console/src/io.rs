//! Reading and writing, whichever console it is.

use core::fmt;

use mlos_device::Console;

use crate::Terminal;

impl Console for Terminal {
    fn write(&self, bytes: &[u8]) {
        match self {
            Self::Pl011(uart) => uart.write(bytes),
            Self::Virtio(console) => console.write(bytes),
        }
    }
}

impl fmt::Write for Terminal {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        match self {
            Self::Pl011(uart) => uart.write_str(text),
            Self::Virtio(console) => console.write_str(text),
        }
    }
}
