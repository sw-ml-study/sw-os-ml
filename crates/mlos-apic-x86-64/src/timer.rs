//! The LAPIC timer, and the acknowledgement every interrupt ends with.
//!
//! Invariant: `timer_rate` counts with the timer masked, and `start_timer`
//! is given what it counted. Design: docs/notes/mlos-apic-x86-64.md.

use crate::{Lapic, clock};

/// This CPU's LAPIC id register; the id is in the top byte.
const ID: u64 = 0x20;
/// End-of-interrupt register.
const EOI: u64 = 0xb0;
/// Timer: LVT entry, initial count, current count, divide configuration.
const LVT_TIMER: u64 = 0x320;
const INITIAL: u64 = 0x380;
const CURRENT: u64 = 0x390;
const DIVIDE: u64 = 0x3e0;
/// Divide by 16, and the LVT's periodic-mode bit.
const DIVIDE_16: u32 = 0x3;
const PERIODIC: u32 = 1 << 17;
/// LVT mask bit: the timer counts but raises nothing.
const MASKED: u32 = 1 << 16;

impl Lapic {
    /// This CPU's LAPIC id, which is what the IOAPIC routes to.
    #[must_use]
    pub fn id(&self) -> u8 {
        (self.read(ID) >> 24) as u8
    }

    /// Acknowledges the interrupt being serviced. Every vector but the
    /// spurious one needs this, or the LAPIC delivers nothing lower again.
    pub fn eoi(&self) {
        self.write(EOI, 0);
    }

    /// How many timer counts (after the divide-by-16) pass in a second,
    /// measured against [`clock::wait_ms`] with the timer masked.
    #[must_use]
    pub fn timer_rate(&self) -> u32 {
        self.write(DIVIDE, DIVIDE_16);
        self.write(LVT_TIMER, MASKED);
        self.write(INITIAL, u32::MAX);
        clock::wait_ms(clock::CALIBRATION_MS);
        let counted = u32::MAX - self.read(CURRENT);
        self.write(INITIAL, 0);
        counted.saturating_mul(1000 / clock::CALIBRATION_MS)
    }

    /// Starts the timer: `vector` at `hz` per second, periodic, given the
    /// measured `rate` in counts per second.
    pub fn start_timer(&self, vector: u8, hz: u32, rate: u32) {
        self.write(DIVIDE, DIVIDE_16);
        self.write(LVT_TIMER, PERIODIC | u32::from(vector));
        self.write(INITIAL, (rate / hz.max(1)).max(1));
    }
}
