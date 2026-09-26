//! x86 port I/O: the only `unsafe` in the crate.
//!
//! Safe to call because a UART's registers are the only ports this crate
//! is ever handed, and reading or writing one affects nothing but the
//! UART. That is the invariant every block below relies on.

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
