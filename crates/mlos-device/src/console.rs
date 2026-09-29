//! Somewhere to put bytes. Design: docs/notes/mlos-device.md.

/// A byte sink the kernel can print to. `&self`, because the console is
/// shared and may be written from an interrupt handler; implementations
/// serialise internally.
pub trait Console {
    /// Writes every byte, blocking until the device has accepted them.
    /// Infallible: a console failure cannot be reported without a console.
    fn write(&self, bytes: &[u8]);
}
