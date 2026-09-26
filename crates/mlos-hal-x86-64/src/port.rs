//! Port I/O, MSRs and the TSC: the instructions every x86-64 driver
//! needs and none should re-derive.
//!
//! `unsafe fn` because a port or MSR write can do anything the hardware
//! behind it does -- reset the machine, remap memory. The caller names
//! the register and owns the consequence.

use core::arch::asm;

/// Reads the byte at I/O port `port`.
///
/// # Safety
///
/// `port` must be a device register that is safe to read.
#[must_use]
pub unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    // SAFETY: per the caller's contract; `in` has no memory effects.
    unsafe { asm!("in al, dx", in("dx") port, out("al") value, options(nomem, nostack)) };
    value
}

/// Writes `value` to I/O port `port`.
///
/// # Safety
///
/// `port` must be a device register, and the write what the caller means.
pub unsafe fn outb(port: u16, value: u8) {
    // SAFETY: per the caller's contract; `out` has no memory effects.
    unsafe { asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack)) };
}

/// Reads model-specific register `msr`.
///
/// # Safety
///
/// `msr` must exist on this CPU, or the read raises #GP.
#[must_use]
pub unsafe fn rdmsr(msr: u32) -> u64 {
    let (low, high): (u32, u32);
    // SAFETY: per the caller's contract.
    unsafe {
        asm!("rdmsr", in("ecx") msr, out("eax") low, out("edx") high, options(nomem, nostack))
    };
    u64::from(high) << 32 | u64::from(low)
}

/// The time-stamp counter. Safe: `rdtsc` at CPL 0 only reads.
#[must_use]
pub fn rdtsc() -> u64 {
    let (low, high): (u32, u32);
    // SAFETY: reads the TSC; no memory effects, no fault at CPL 0.
    unsafe { asm!("rdtsc", out("eax") low, out("edx") high, options(nomem, nostack)) };
    u64::from(high) << 32 | u64::from(low)
}
