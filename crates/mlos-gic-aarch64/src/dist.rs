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

/// `GICD_IGROUPR`, one bit per interrupt.
const IGROUPR: usize = 0x0080;
/// `GICD_ISENABLER`, one bit per interrupt.
const ISENABLER: usize = 0x0100;
/// `GICD_IPRIORITYR`, one byte per interrupt.
const IPRIORITYR: usize = 0x0400;
/// `GICD_IROUTER`, one 64-bit affinity per interrupt, from interrupt 32.
const IROUTER: usize = 0x6000;

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

/// Routes one shared interrupt to CPU 0 and unmasks it.
///
/// Shared interrupts are the distributor's, unlike the private ones each
/// redistributor owns. `GICD_IROUTER` only exists from interrupt 32 up,
/// and only means anything with affinity routing enabled -- which is why
/// [`enable`] sets `ARE` before anything is routed.
///
/// # Safety
///
/// `base` must be a GICv3 distributor's window and `intid` at least 32.
pub unsafe fn enable_spi(base: usize, intid: u32, priority: u8) {
    let (word, bit) = ((intid / 32) as usize * 4, intid % 32);
    // SAFETY: caller guarantees the window and the interrupt's range.
    unsafe {
        let group = (base + IGROUPR + word) as *mut u32;
        ptr::write_volatile(group, ptr::read_volatile(group.cast_const()) | 1 << bit);
        ptr::write_volatile((base + IPRIORITYR + intid as usize) as *mut u8, priority);
        // Affinity 0.0.0.0: the boot core, the only one running.
        ptr::write_volatile((base + IROUTER + intid as usize * 8) as *mut u64, 0);
        ptr::write_volatile((base + ISENABLER + word) as *mut u32, 1 << bit);
    }
}
