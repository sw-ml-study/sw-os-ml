//! The ARM generic timer.
//!
//! Invariant: a handler that does not rearm is re-entered the instant it
//! returns. Design: docs/notes/mlos-hal-aarch64.md.

use core::arch::asm;

use mlos_device::{Hertz, Ticks, Timer};

/// `CNTP_CTL_EL0.ENABLE`.
const ENABLE: u64 = 1 << 0;

/// The EL1 physical timer. Its interrupt is PPI 30, fixed by the
/// architecture.
pub struct GenericTimer;

/// The interrupt this timer raises.
pub const TIMER_PPI: u32 = 30;

impl GenericTimer {
    /// Arms the timer `ticks` from now and enables it. Writing the
    /// countdown is also what clears the interrupt.
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
