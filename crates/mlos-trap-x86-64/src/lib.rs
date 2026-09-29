//! x86-64 exceptions and interrupts: the IDT, the entry stubs, and the
//! report.
//!
//! Invariant: an exception (vectors 0-31) is fatal and its reporter never
//! returns; an interrupt (32-255) returns, and its handler acknowledges the
//! controller itself. Design and history: docs/notes/mlos-trap-x86-64.md.

#![no_std]
// Empty anywhere but a bare x86-64 target, like `mlos-hal-x86-64`.
#![cfg(all(target_arch = "x86_64", target_os = "none"))]

mod idt;
mod trap;

use core::sync::atomic::{AtomicPtr, Ordering};

pub use trap::{Trap, describe};

/// Where to send a trap report: a plain `fn`, because the handler runs at
/// an arbitrary moment and can depend on nothing it might have borrowed.
static REPORTER: AtomicPtr<()> = AtomicPtr::new(core::ptr::null_mut());

/// Where to send an interrupt, as a `fn(vector)`.
static ON_IRQ: AtomicPtr<()> = AtomicPtr::new(core::ptr::null_mut());

/// Installs the IDT: exceptions to `reporter`, interrupts to `on_irq`.
///
/// # Safety
///
/// Call once, on the boot CPU, with interrupts off. Both must be safe to
/// call from an interrupt: no allocation, no lock the interrupted code
/// could hold, no assumption about which stack they are on. `on_irq`
/// must acknowledge the interrupt itself.
pub unsafe fn install(reporter: fn(&Trap) -> !, on_irq: fn(u8)) {
    REPORTER.store(reporter as *mut (), Ordering::Release);
    ON_IRQ.store(on_irq as *mut (), Ordering::Release);
    // SAFETY: forwarded; boot CPU, once, interrupts off.
    unsafe { idt::load() };
}

/// Called by `entry.s` with the frame the CPU and the stub pushed.
///
/// # Safety
///
/// Only from `mlos_trap_common`, with `frame` pointing at that frame.
#[unsafe(no_mangle)]
unsafe extern "C" fn mlos_trap_dispatch(frame: *const [u64; 7]) -> ! {
    // SAFETY: the stub's contract: seven words, vector first.
    let trap = unsafe { Trap::capture(&*frame) };
    let reporter = REPORTER.load(Ordering::Acquire);
    if !reporter.is_null() {
        // SAFETY: only `install` stores here, and only a `fn(&Trap) -> !`.
        let reporter: fn(&Trap) -> ! = unsafe { core::mem::transmute(reporter) };
        reporter(&trap);
    }
    loop {
        // SAFETY: nowhere to report; park with interrupts off.
        unsafe { core::arch::asm!("cli; hlt", options(nomem, nostack)) };
    }
}

/// Called by `entry.s` for vectors 32-255, with the vector.
///
/// # Safety
///
/// Only from `mlos_irq_common`, which has saved what this may clobber.
#[unsafe(no_mangle)]
unsafe extern "C" fn mlos_irq_dispatch(vector: u64) {
    let on_irq = ON_IRQ.load(Ordering::Acquire);
    if !on_irq.is_null() {
        // SAFETY: only `install` stores here, and only a `fn(u8)`.
        let on_irq: fn(u8) = unsafe { core::mem::transmute(on_irq) };
        on_irq(vector as u8);
    }
}
