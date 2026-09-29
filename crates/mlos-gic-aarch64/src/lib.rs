//! A GICv3 interrupt controller, this CPU's view of it.
//!
//! Invariant: distributor, then redistributor, then CPU interface; each
//! accepts writes before the one above it is ready, and ignores them.
//! Design and history: docs/notes/mlos-gic-aarch64.md.

#![no_std]
// Empty on any other architecture, so the workspace gate can sweep every
// crate without an exclude list.
#![cfg(target_arch = "aarch64")]

mod cpuif;
mod dist;
mod redist;

use mlos_device::{Irq, IrqController};

/// Interrupts below this are private to a CPU; above, they are shared.
const PRIVATE_INTERRUPTS: u32 = 32;

/// The architectural "nothing was pending" answer from `ICC_IAR1_EL1`.
const SPURIOUS: u32 = 1023;

/// This CPU's view of a GICv3.
pub struct Gic {
    distributor: usize,
    redistributor: usize,
}

impl Gic {
    /// Brings up the distributor, this CPU's redistributor, and its CPU
    /// interface.
    ///
    /// # Safety
    ///
    /// `distributor` and `redistributor` must be the GICv3 windows the
    /// device tree reported, and this must run once per CPU with
    /// interrupts still masked.
    pub unsafe fn new(distributor: usize, redistributor: usize) -> Self {
        // SAFETY: caller guarantees the windows. Order is load-bearing:
        // affinity routing before the redistributor is addressable at all,
        // the redistributor awake before its registers mean anything, and
        // the CPU interface last because it is what starts delivery.
        unsafe {
            dist::enable(distributor);
            redist::wake(redistributor);
            cpuif::enable();
        }
        Self {
            distributor,
            redistributor,
        }
    }
}

impl IrqController for Gic {
    /// Dispatches by number: below 32 is this CPU's redistributor, above
    /// is the distributor. The other way round writes a register that
    /// exists and does nothing.
    fn enable(&self, irq: Irq) {
        // SAFETY: both windows came from `new`, whose contract covers
        // them. Priority 0 is the most urgent, below the accept-all mask.
        unsafe {
            if irq.0 < PRIVATE_INTERRUPTS {
                redist::enable_ppi(self.redistributor, irq.0, 0);
            } else {
                dist::enable_spi(self.distributor, irq.0, 0);
            }
        }
    }

    fn claim(&self) -> Option<Irq> {
        match cpuif::acknowledge() {
            SPURIOUS => None,
            intid => Some(Irq(intid)),
        }
    }

    fn complete(&self, irq: Irq) {
        cpuif::end_of_interrupt(irq.0);
    }
}
