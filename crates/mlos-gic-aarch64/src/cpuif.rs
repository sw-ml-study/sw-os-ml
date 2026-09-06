//! The CPU interface, which on GICv3 is system registers rather than MMIO.

use core::arch::asm;

/// Accept every priority. The redistributor sets per-interrupt priorities;
/// masking here as well would be a second place to look when an interrupt
/// does not arrive.
const ACCEPT_ALL: u64 = 0xff;

/// Enables this CPU's interface for Group 1 interrupts.
///
/// `ICC_SRE_EL1.SRE` comes first and needs its own `isb`: until it is set,
/// the other `ICC_*` registers are not architecturally accessible, and
/// writes to them are ignored rather than faulting -- which looks exactly
/// like a controller that was configured and does nothing.
///
/// # Safety
///
/// Call once per CPU, after its redistributor is awake.
pub unsafe fn enable() {
    // SAFETY: system register writes on the calling CPU only.
    unsafe {
        asm!(
            "mrs {tmp}, icc_sre_el1",
            "orr {tmp}, {tmp}, #1",
            "msr icc_sre_el1, {tmp}",
            "isb",
            "msr icc_pmr_el1, {pmr}",
            "msr icc_igrpen1_el1, {one}",
            "isb",
            tmp = out(reg) _,
            pmr = in(reg) ACCEPT_ALL,
            one = in(reg) 1u64,
            options(nomem, nostack),
        );
    }
}

/// Takes the highest-priority pending Group 1 interrupt.
///
/// Returns the raw `ICC_IAR1_EL1` value. 1023 is the architectural
/// "spurious" answer, meaning nothing was pending after all.
#[must_use]
pub fn acknowledge() -> u32 {
    let intid: u64;
    // SAFETY: reading the acknowledge register. It has the side effect of
    // moving the interrupt to active, which is exactly what is wanted.
    unsafe { asm!("mrs {}, icc_iar1_el1", out(reg) intid, options(nostack)) };
    intid as u32
}

/// Signals that the handler for an acknowledged interrupt has finished.
///
/// Must pair with every [`acknowledge`] that returned a real interrupt.
/// Skipping it leaves the interrupt active and the controller will not
/// deliver another at that priority -- a hang that looks like the timer
/// stopped.
pub fn end_of_interrupt(intid: u32) {
    // SAFETY: write of a previously acknowledged interrupt id.
    unsafe { asm!("msr icc_eoir1_el1, {}", in(reg) u64::from(intid), options(nostack)) };
}
