//! The TSC as a clock: how fast it runs, found out rather than assumed.
//!
//! Invariant: the rate is read from `CPUID.15H` when the CPU reports it,
//! otherwise counted against the 8254 PIT, and which it was is reported.
//! Design and history: docs/notes/mlos-apic-x86-64.md.

use core::sync::atomic::{AtomicU32, Ordering};

use mlos_hal_x86_64::port::{inb, outb, rdtsc};

/// How far [`now`] shifts the TSC right so its rate fits the `u32` that
/// `mlsh`'s clock takes. Zero below 4.29 GHz; set by [`tsc_rate`].
static SHIFT: AtomicU32 = AtomicU32::new(0);

/// How long a calibration counts for.
pub const CALIBRATION_MS: u32 = 50;
/// The 8254's input clock.
const PIT_HZ: u64 = 1_193_182;

/// Where a clock's rate came from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Source {
    /// Read from `CPUID.15H`.
    Cpuid,
    /// Counted against the 8254 PIT.
    Pit,
}

/// The clock `mlsh` reads: the TSC, shifted as [`tsc_rate`] decided.
#[must_use]
pub fn now() -> u64 {
    rdtsc() >> SHIFT.load(Ordering::Relaxed)
}

/// The rate of [`now`] in Hz, and how it was found. Call once, at boot.
#[must_use]
pub fn tsc_rate() -> (u32, Source) {
    let (hz, source) = measure();
    let shift = (64 - (hz >> 32).leading_zeros()).min(31);
    SHIFT.store(shift, Ordering::Relaxed);
    ((hz >> shift) as u32, source)
}

/// The TSC's rate in Hz, read or counted.
fn measure() -> (u64, Source) {
    if core::arch::x86_64::__cpuid(0).eax >= 0x15 {
        let leaf = core::arch::x86_64::__cpuid(0x15);
        if leaf.eax != 0 && leaf.ebx != 0 && leaf.ecx != 0 {
            return (
                u64::from(leaf.ecx) * u64::from(leaf.ebx) / u64::from(leaf.eax),
                Source::Cpuid,
            );
        }
    }
    let start = rdtsc();
    wait_ms(CALIBRATION_MS);
    (
        (rdtsc() - start) * u64::from(1000 / CALIBRATION_MS),
        Source::Pit,
    )
}

/// Spins for `ms` milliseconds, timed by PIT channel 0.
///
/// Channel 0 free-runs in mode 2 from 65536; its count is latched and
/// read until the elapsed PIT ticks cover `ms`. No interrupt is involved:
/// IRQ 0 is masked at the PIC and not routed at the IOAPIC.
pub fn wait_ms(ms: u32) {
    let target = PIT_HZ * u64::from(ms) / 1000;
    for (port, value) in [(0x43, 0x34), (0x40, 0), (0x40, 0)] {
        // SAFETY: 8254 mode/command (0x43) and channel 0 data (0x40), in
        // the sequence the chip defines: channel 0, lo/hi, mode 2, 65536.
        unsafe { outb(port, value) };
    }
    let (mut elapsed, mut last) = (0u64, 0x1_0000u64);
    while elapsed < target {
        // SAFETY: latch channel 0, then read its count low byte first.
        let now = unsafe {
            outb(0x43, 0x00);
            let low = inb(0x40);
            u64::from(u16::from_le_bytes([low, inb(0x40)]))
        };
        elapsed += last.wrapping_sub(now) & 0xffff;
        last = now;
    }
}
