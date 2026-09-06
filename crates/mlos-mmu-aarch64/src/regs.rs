//! Programming the translation registers, and the order it must happen in.

use core::arch::asm;

/// `MAIR_EL1`: attribute 0 is Device-nGnRE (`0x04`), attribute 1 is Normal
/// write-back read/write-allocate (`0xff`). `descriptor::ATTR_*` index
/// into this, so the two must agree.
const MAIR: u64 = (0xff << 8) | 0x04;

/// `SCTLR_EL1.M` -- enable translation.
const SCTLR_M: u64 = 1 << 0;
/// `SCTLR_EL1.C` -- enable the data cache.
const SCTLR_C: u64 = 1 << 2;
/// `SCTLR_EL1.I` -- enable the instruction cache.
const SCTLR_I: u64 = 1 << 12;

/// Builds `TCR_EL1` for a 39-bit identity map with a 4 KiB granule.
///
/// `T0SZ` of 25 gives a 39-bit address space, whose initial lookup is at
/// level 1 -- which is exactly why the tables are a single level of 1 GiB
/// blocks and there is no level-0 table.
///
/// `IPS` is read from `ID_AA64MMFR0_EL1.PARange` rather than assumed.
/// Programming an intermediate size the implementation does not support
/// is architecturally unpredictable, and "unpredictable" on the
/// instruction after the MMU comes on is not a failure anyone can debug.
fn tcr() -> u64 {
    let mmfr0: u64;
    // SAFETY: reading an ID register. No side effects, always accessible
    // at EL1.
    unsafe { asm!("mrs {}, id_aa64mmfr0_el1", out(reg) mmfr0, options(nomem, nostack)) };
    let parange = mmfr0 & 0b1111;

    // TG0 (bits 15:14) is left zero: 0b00 *is* the 4 KiB granule, so the
    // field is absent from this expression rather than forgotten.
    25                    // T0SZ:  39-bit VA, initial lookup at level 1
        | (0b01 << 8)     // IRGN0: inner write-back, read/write-allocate
        | (0b01 << 10)    // ORGN0: outer, likewise
        | (0b11 << 12)    // SH0:   inner shareable
        | (1 << 23)       // EPD1:  no TTBR1 walks; nothing is mapped high
        | (parange << 32) // IPS:   as the implementation reports it
}

/// Installs the tables, without yet translating through them.
///
/// The barriers are the substance. `dsb ishst` publishes the table writes
/// before anything can walk them; the `isb` makes the new control
/// registers visible to instruction fetch; the TLB invalidate removes
/// anything cached from before we existed. Dropping any of them yields a
/// machine that boots on one host and hangs on another.
///
/// # Safety
///
/// `ttbr0` must point at a valid, complete level-1 table.
pub unsafe fn install(ttbr0: u64) {
    // SAFETY: caller guarantees the table. Sequence and barriers are per
    // the Arm ARM's requirements for programming the EL1&0 regime.
    unsafe {
        asm!(
            "dsb ishst",
            "msr mair_el1, {mair}",
            "msr tcr_el1,  {tcr}",
            "msr ttbr0_el1, {ttbr0}",
            "isb",
            "tlbi vmalle1",
            "dsb ish",
            "isb",
            mair = in(reg) MAIR,
            tcr = in(reg) tcr(),
            ttbr0 = in(reg) ttbr0,
            options(nostack),
        );
    }
}

/// Turns translation on.
///
/// # Safety
///
/// The tables installed by [`install`] must identity map at least the
/// currently executing code, its stack and the console. The instruction
/// after the final `isb` executes under the new regime; if it is not
/// mapped, the machine stops there with no way to report it.
pub unsafe fn turn_on() {
    // SAFETY: caller guarantees the mapping covers what is still in use.
    unsafe {
        asm!(
            "mrs {tmp}, sctlr_el1",
            "orr {tmp}, {tmp}, {bits}",
            "msr sctlr_el1, {tmp}",
            "isb",
            bits = in(reg) SCTLR_M | SCTLR_C | SCTLR_I,
            tmp = out(reg) _,
            options(nostack),
        );
    }
}

/// Whether translation is enabled, read back from `SCTLR_EL1.M` -- so a
/// claim that the MMU is on can be checked rather than inferred from the
/// fact that we are still running.
#[must_use]
pub fn is_enabled() -> bool {
    let sctlr: u64;
    // SAFETY: reading a system control register. No side effects.
    unsafe { asm!("mrs {}, sctlr_el1", out(reg) sctlr, options(nomem, nostack)) };
    sctlr & SCTLR_M != 0
}
