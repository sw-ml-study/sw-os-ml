//! aarch64 exception vectors.
//!
//! Before this crate, `VBAR_EL1` was zero and every exception branched
//! into unmapped nothing -- which is exactly what step 001 saw when a
//! stack push faulted and the machine spun at `0x200` with no way to say
//! why. Diagnosing that took a disassembler and a register dump. The point
//! of a vector table is that the machine tells you instead.
//!
//! Reporting only: no handler here resumes. Interrupts, which must save
//! and restore state to return, are the next step; a fault that reaches
//! these vectors today is a bug, and stopping at it is the correct
//! response.

#![no_std]

mod trap;
mod vectors;

use core::sync::atomic::{AtomicUsize, Ordering};

pub use trap::{Trap, VECTOR_NAMES};

/// Where to send a trap report, as a raw function pointer.
///
/// An atomic rather than a `static mut`, and a plain `fn` rather than a
/// closure, because a handler runs on an arbitrary stack at an arbitrary
/// moment and must not depend on anything it might have borrowed.
static REPORTER: AtomicUsize = AtomicUsize::new(0);

/// Installs the vector table and the function to report through.
///
/// # Safety
///
/// Call once, on the boot core. `reporter` must be safe to call from an
/// exception context: no allocation, no locks it could already hold, and
/// no assumption about which stack it is on.
pub unsafe fn install(reporter: fn(&Trap) -> !) {
    REPORTER.store(reporter as *const () as usize, Ordering::Relaxed);
    let table = vectors::vector_table as *const () as usize;
    // SAFETY: `vector_table` is 2048-aligned by the linker script, which
    // is what VBAR_EL1 requires. The `isb` makes the write take effect
    // before an exception could be taken against the old value.
    unsafe {
        core::arch::asm!(
            "msr vbar_el1, {table}",
            "isb",
            table = in(reg) table,
            options(nomem, nostack),
        );
    }
}

/// Where every vector entry lands.
///
/// Reads the syndrome first, before anything else can overwrite it, then
/// hands it to whoever registered. If nobody did, park: there is no way to
/// report and no state worth returning to.
extern "C" fn report(vector: usize) -> ! {
    // SAFETY: entered directly from a vector, so this is the first code to
    // run since the exception and the syndrome registers still describe it.
    let trap = unsafe { Trap::capture(vector) };

    let reporter = REPORTER.load(Ordering::Relaxed);
    if reporter != 0 {
        // SAFETY: only ever stored by `install`, from a `fn(&Trap) -> !`.
        let reporter: fn(&Trap) -> ! = unsafe { core::mem::transmute(reporter) };
        reporter(&trap);
    }
    loop {
        core::hint::spin_loop();
    }
}
