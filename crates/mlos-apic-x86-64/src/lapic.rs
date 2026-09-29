//! The local APIC: this CPU's interrupt controller, and its timer.
//!
//! Invariant: registers are 32-bit, accessed volatile, inside the uncached
//! device window. Design: docs/notes/mlos-apic-x86-64.md.

use mlos_hal_x86_64::port::rdmsr;

/// `IA32_APIC_BASE`: where the LAPIC's registers are, and whether it is on.
const APIC_BASE_MSR: u32 = 0x1b;
/// Spurious-interrupt vector register, and its APIC-enable bit.
const SVR: u64 = 0xf0;
const SVR_ENABLE: u32 = 1 << 8;
/// Where a spurious interrupt goes. It needs no EOI.
pub const SPURIOUS: u8 = 0xff;

/// The local APIC, at the base the CPU reports.
#[derive(Clone, Copy)]
pub struct Lapic {
    /// Physical (= virtual, identity-mapped) base of the registers.
    pub base: u64,
}

impl Lapic {
    /// Finds and enables the LAPIC. `None` if the base `IA32_APIC_BASE`
    /// reports lies outside the device window.
    #[must_use]
    pub fn enable() -> Option<Self> {
        // SAFETY: IA32_APIC_BASE exists on every x86-64 CPU.
        let base = unsafe { rdmsr(APIC_BASE_MSR) } & !0xfff;
        if !(0xc000_0000..0x1_0000_0000).contains(&base) {
            return None;
        }
        let lapic = Self { base };
        lapic.write(SVR, SVR_ENABLE | u32::from(SPURIOUS));
        Some(lapic)
    }

    /// Reads the 32-bit register at `offset`.
    #[must_use]
    pub fn read(&self, offset: u64) -> u32 {
        // SAFETY: a LAPIC register inside its 4 KiB page, in the uncached
        // device window; 32-bit aligned, as the LAPIC requires.
        unsafe { ((self.base + offset) as *const u32).read_volatile() }
    }

    /// Writes the 32-bit register at `offset`.
    pub fn write(&self, offset: u64, value: u32) {
        // SAFETY: as for `read`.
        unsafe { ((self.base + offset) as *mut u32).write_volatile(value) };
    }
}
