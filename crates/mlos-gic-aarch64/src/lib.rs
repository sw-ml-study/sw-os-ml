//! A GICv3 interrupt controller.
//!
//! Only v3. A GICv2 is a different device with a different programming
//! model, and QEMU's `virt` will hand you either one depending on the
//! accelerator -- TCG defaults to v2, HVF to v3 -- so `scripts/boot.sh`
//! pins `gic-version=3` rather than letting the host decide what the guest
//! is driving.
//!
//! Three pieces, in an order that matters: the distributor routes
//! system-wide, the redistributor holds this CPU's private interrupts, and
//! the CPU interface is system registers. Each will accept writes before
//! the one above it is ready, and ignore them.

#![no_std]

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
    /// Dispatches by interrupt number, because the two kinds live in
    /// different devices: interrupts below 32 are private to a CPU and
    /// configured in its redistributor, everything above is shared and
    /// configured in the distributor. Getting this the wrong way round
    /// writes to a register that exists and does nothing.
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
