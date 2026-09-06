//! The distributor: system-wide interrupt routing.

use core::ptr;

/// `GICD_CTLR`.
const CTLR: usize = 0x0000;
/// `GICD_CTLR.EnableGrp1NS` -- deliver Group 1 non-secure interrupts.
const ENABLE_GRP1: u32 = 1 << 1;
/// `GICD_CTLR.ARE_NS` -- affinity routing. GICv3's defining feature, and
/// not optional: with it clear the redistributors are not addressed at
/// all and every per-CPU interrupt is invisible.
const ARE: u32 = 1 << 4;
/// `GICD_CTLR.RWP` -- a register write is still propagating.
const RWP: u32 = 1 << 31;

/// Enables Group 1 delivery with affinity routing.
///
/// # Safety
///
/// `base` must be a GICv3 distributor's MMIO window.
pub unsafe fn enable(base: usize) {
    let ctlr = base + CTLR;
    // SAFETY: caller guarantees the window. Volatile because the device,
    // not the compiler, decides what a read means.
    unsafe {
        // ARE first, then Group 1: enabling delivery before routing exists
        // would advertise interrupts that have nowhere to go.
        ptr::write_volatile(ctlr as *mut u32, ARE);
        ptr::write_volatile(ctlr as *mut u32, ARE | ENABLE_GRP1);
        // Writes to CTLR are not instantaneous. Proceeding while RWP is
        // set means configuring a controller that has not finished being
        // configured.
        while ptr::read_volatile(ctlr as *const u32) & RWP != 0 {
            core::hint::spin_loop();
        }
    }
}
