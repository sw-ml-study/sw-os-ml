//! Receiving, where the console can. Design: docs/notes/mlos-console.md.

use crate::Terminal;

impl Terminal {
    /// Takes one byte, if one is waiting. Always `None` from a virtio
    /// console, which cannot receive yet.
    #[must_use]
    pub fn read(&self) -> Option<u8> {
        match self {
            Self::Pl011(uart) => uart.read(),
            Self::Virtio(_) => None,
        }
    }

    /// Acknowledges a receive interrupt, where there is one to acknowledge.
    pub fn clear_interrupt(&self) {
        if let Self::Pl011(uart) = self {
            uart.clear_interrupt();
        }
    }

    /// Which kind of console this is, for the boot banner.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Pl011(_) => "pl011",
            Self::Virtio(_) => "virtio",
        }
    }

    /// Enables the receive interrupt, where the console has one.
    pub fn enable_receive_interrupt(&self) {
        if let Self::Pl011(uart) = self {
            uart.enable_receive_interrupt();
        }
    }
}
