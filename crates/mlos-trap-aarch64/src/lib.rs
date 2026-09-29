//! aarch64 exception vectors: faults are reported and stop, IRQs return.
//!
//! Invariant: a handler runs on an arbitrary stack at an arbitrary moment
//! and may not allocate, lock, or borrow. Design and history:
//! docs/notes/mlos-trap-aarch64.md.

#![no_std]
// Empty on any other architecture, so the workspace gate can sweep every
// crate without an exclude list.
#![cfg(target_arch = "aarch64")]

mod trap;
mod vectors;

use core::sync::atomic::{AtomicUsize, Ordering};

pub use trap::{Trap, VECTOR_NAMES, describe};

/// Where to send an interrupt, as a raw function pointer.
static HANDLER: AtomicUsize = AtomicUsize::new(0);

/// Where to send a trap report, as a raw function pointer.
static REPORTER: AtomicUsize = AtomicUsize::new(0);

/// Installs the vector table and the functions to dispatch to.
///
/// # Safety
///
/// Call once, on the boot core. `reporter` and `handler` must be safe to
/// call from an exception context: no allocation, no locks they could
/// already hold, and no assumption about which stack they are on.
pub unsafe fn install(reporter: fn(&Trap) -> !, handler: fn()) {
    REPORTER.store(reporter as *const () as usize, Ordering::Relaxed);
    HANDLER.store(handler as *const () as usize, Ordering::Relaxed);
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

/// Unmasks IRQs. Call last, once the vectors and the controller are up.
///
/// # Safety
///
/// The vector table must be installed and the interrupt controller
/// brought up, or the first interrupt goes somewhere undefined.
pub unsafe fn unmask() {
    // SAFETY: clears the I bit on this CPU only.
    unsafe { core::arch::asm!("msr daifclr, #2", options(nomem, nostack)) };
}

/// Where every IRQ entry lands, with the interrupted context already on
/// the stack. Returns, unlike [`report`].
extern "C" fn irq() {
    let handler = HANDLER.load(Ordering::Relaxed);
    if handler != 0 {
        // SAFETY: only ever stored by `install`, from a `fn()`.
        let handler: fn() = unsafe { core::mem::transmute(handler) };
        handler();
    }
}

/// Where every fault entry lands. Reads the syndrome before anything can
/// overwrite it, reports, and parks if nobody registered.
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
