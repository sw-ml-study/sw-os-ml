//! x86-64 exceptions: the IDT, the entry stubs, and the report.
//!
//! The counterpart of `mlos-trap-aarch64`, with the same contract: a
//! fault is fatal, so the entry saves nothing it would need to return,
//! captures what the CPU says about why it stopped, and hands that to a
//! reporter that never returns. Interrupts, which DO return, are step
//! `x86-interrupts` and will get their own entry.
//!
//! `unsafe` lives here by design (AGENTS.md): the IDT, `lidt`, `cr2`.

#![no_std]
// Empty anywhere but a bare x86-64 target, like `mlos-hal-x86-64`.
#![cfg(all(target_arch = "x86_64", target_os = "none"))]

mod idt;
mod trap;

use core::sync::atomic::{AtomicPtr, Ordering};

pub use trap::{Trap, describe};

/// Where to send a trap report. An atomic, and a plain `fn`, for the same
/// reason as on aarch64: the handler runs at an arbitrary moment and must
/// not depend on anything it might have borrowed.
static REPORTER: AtomicPtr<()> = AtomicPtr::new(core::ptr::null_mut());

/// Installs the IDT, sending every exception to `reporter`.
///
/// # Safety
///
/// Call once, on the boot CPU, with interrupts off. `reporter` must be
/// safe to call from an exception: no allocation, no lock it could
/// already hold, no assumption about which stack it is on.
pub unsafe fn install(reporter: fn(&Trap) -> !) {
    REPORTER.store(reporter as *mut (), Ordering::Release);
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
