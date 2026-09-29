//! x86 port I/O: the only `unsafe` in the crate.
//!
//! Invariant: the only ports this crate is ever handed are a UART's
//! registers, and touching one affects nothing but the UART. Design:
//! docs/notes/mlos-uart16550.md.

use core::arch::asm;

/// Reads the byte at `port`.
pub fn read(port: u16) -> u8 {
    let value: u8;
    // SAFETY: `port` is a 16550 register (module invariant); `in` has no
    // memory effects.
    unsafe { asm!("in al, dx", in("dx") port, out("al") value, options(nomem, nostack)) };
    value
}

/// Writes `value` to `port`.
pub fn write(port: u16, value: u8) {
    // SAFETY: `port` is a 16550 register (module invariant); `out` has no
    // memory effects.
    unsafe { asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack)) };
}
