//! The ARM generic timer.

use core::arch::asm;

use mlos_device::{Hertz, Ticks, Timer};

/// `CNTP_CTL_EL0.ENABLE`.
const ENABLE: u64 = 1 << 0;

/// The EL1 physical timer, whose interrupt is private interrupt 30.
///
/// A fixed number, from the Arm architecture rather than from the device
/// tree: the tree's `interrupts` property describes the same thing, but
/// PPI 30 is architectural for the non-secure EL1 physical timer and
/// parsing three cells of interrupt specifier to rediscover it would be
/// ceremony, not portability.
pub struct GenericTimer;

/// The interrupt this timer raises.
pub const TIMER_PPI: u32 = 30;

impl GenericTimer {
    /// Arms the timer `ticks` from now and enables it.
    ///
    /// `CNTP_TVAL_EL0` is a countdown, so writing it is both "when" and
    /// "start counting". It is also how the interrupt is cleared: the
    /// timer asserts its output for as long as the count is negative, so a
    /// handler that does not rearm gets called again immediately, forever.
    pub fn arm(&self, ticks: u32) {
        // SAFETY: writes to this CPU's own timer registers.
        unsafe {
            asm!(
                "msr cntp_tval_el0, {ticks}",
                "msr cntp_ctl_el0, {enable}",
                "isb",
                ticks = in(reg) u64::from(ticks),
                enable = in(reg) ENABLE,
                options(nomem, nostack),
            );
        }
    }
}

impl Timer for GenericTimer {
    fn now(&self) -> Ticks {
        let count: u64;
        // SAFETY: reading the physical counter. No side effects.
        unsafe { asm!("mrs {}, cntpct_el0", out(reg) count, options(nomem, nostack)) };
        Ticks(count)
    }

    fn frequency(&self) -> Hertz {
        let frequency: u64;
        // SAFETY: reading the counter frequency. Firmware sets it; on
        // QEMU `virt` it is 62.5 MHz, but reading beats assuming -- a
        // hard-coded rate is a kernel that keeps time on one board only.
        unsafe { asm!("mrs {}, cntfrq_el0", out(reg) frequency, options(nomem, nostack)) };
        Hertz(frequency as u32)
    }

    fn set_deadline(&self, at: Ticks) {
        let remaining = at.0.saturating_sub(self.now().0);
        self.arm(u32::try_from(remaining).unwrap_or(u32::MAX));
    }
}
