//! The redistributor: one per CPU, and where PPIs are configured.
//!
//! Private peripheral interrupts -- the timer among them -- are not
//! configured in the distributor. Each CPU has its own copy, in the
//! redistributor's second 64 KiB frame. Looking for them in the
//! distributor is a common way to write a GICv3 driver that silently
//! never delivers anything.

use core::ptr;

/// `GICR_WAKER`, in the RD frame.
const WAKER: usize = 0x0014;
/// `GICR_WAKER.ProcessorSleep`.
const PROCESSOR_SLEEP: u32 = 1 << 1;
/// `GICR_WAKER.ChildrenAsleep`.
const CHILDREN_ASLEEP: u32 = 1 << 2;

/// The SGI frame sits one 64 KiB frame past the RD frame.
const SGI_FRAME: usize = 0x1_0000;
/// `GICR_IGROUPR0`, in the SGI frame.
const IGROUPR0: usize = SGI_FRAME + 0x0080;
/// `GICR_ISENABLER0`, in the SGI frame.
const ISENABLER0: usize = SGI_FRAME + 0x0100;
/// `GICR_IPRIORITYR`, in the SGI frame: one byte per interrupt.
const IPRIORITYR: usize = SGI_FRAME + 0x0400;

/// Brings this CPU's redistributor out of sleep.
///
/// # Safety
///
/// `base` must be this CPU's GICv3 redistributor window.
pub unsafe fn wake(base: usize) {
    let waker = (base + WAKER) as *mut u32;
    // SAFETY: caller guarantees the window.
    unsafe {
        let value = ptr::read_volatile(waker) & !PROCESSOR_SLEEP;
        ptr::write_volatile(waker, value);
        // The redistributor acknowledges by clearing ChildrenAsleep.
        // Configuring it before it has woken is configuring nothing.
        while ptr::read_volatile(waker.cast_const()) & CHILDREN_ASLEEP != 0 {
            core::hint::spin_loop();
        }
    }
}

/// Routes one PPI to this CPU as a Group 1 interrupt and unmasks it.
///
/// `priority` is numerically lower for more urgent, and must be below
/// `ICC_PMR_EL1` or the CPU interface will never present it.
///
/// # Safety
///
/// `base` must be this CPU's redistributor window, and `intid` a private
/// interrupt (16..32) -- shared interrupts live in the distributor.
pub unsafe fn enable_ppi(base: usize, intid: u32, priority: u8) {
    // SAFETY: caller guarantees the window and the interrupt's range.
    unsafe {
        let group = (base + IGROUPR0) as *mut u32;
        ptr::write_volatile(group, ptr::read_volatile(group.cast_const()) | 1 << intid);
        ptr::write_volatile((base + IPRIORITYR + intid as usize) as *mut u8, priority);
        ptr::write_volatile((base + ISENABLER0) as *mut u32, 1 << intid);
    }
}
