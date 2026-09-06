//! Somewhere to put bytes.

/// A byte sink the kernel can print to.
///
/// One method, and it stays one method. A console is the first thing a
/// kernel needs and the last thing it should spend design budget on:
/// formatting, line discipline and buffering all belong above this, in
/// code that is not architecture-specific.
///
/// Takes `&self` rather than `&mut self` because the console is shared
/// and may be written from an interrupt handler. Implementations
/// serialise internally.
pub trait Console {
    /// Writes every byte, blocking until the device has accepted them.
    ///
    /// Infallible by construction: there is no useful way to report that
    /// the console failed, because reporting it would need the console.
    fn write(&self, bytes: &[u8]);
}
