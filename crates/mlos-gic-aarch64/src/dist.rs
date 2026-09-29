//! The distributor: system-wide interrupt routing.
//!
//! Invariant: affinity routing (`ARE`) is on before anything is routed;
//! without it the redistributors are not addressed. Design:
//! docs/notes/mlos-gic-aarch64.md.

use core::ptr;

/// `GICD_CTLR`.
const CTLR: usize = 0x0000;
/// `GICD_CTLR.EnableGrp1NS` -- deliver Group 1 non-secure interrupts.
const ENABLE_GRP1: u32 = 1 << 1;
/// `GICD_CTLR.ARE_NS` -- affinity routing. Required for the
/// redistributors to be addressed at all.
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

/// Enables Group 1 delivery with affinity routing, and waits for the
/// write to land.
///
/// # Safety
///
/// `base` must be a GICv3 distributor's MMIO window.
pub unsafe fn enable(base: usize) {
    let ctlr = base + CTLR;
    // SAFETY: caller guarantees the window. Volatile because the device,
    // not the compiler, decides what a read means.
    unsafe {
        // ARE first, then Group 1: delivery before routing would advertise
        // interrupts with nowhere to go.
        ptr::write_volatile(ctlr as *mut u32, ARE);
        ptr::write_volatile(ctlr as *mut u32, ARE | ENABLE_GRP1);
        // Nothing further is configured until the CTLR write has landed.
        while ptr::read_volatile(ctlr as *const u32) & RWP != 0 {
            core::hint::spin_loop();
        }
    }
}

/// Routes one shared interrupt to CPU 0 and unmasks it. `GICD_IROUTER`
/// exists from interrupt 32 up, and only with `ARE` set.
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
