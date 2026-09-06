//! Reporting a fault out of the console.
//!
//! The console is reached through a static base address rather than a
//! borrowed handle: a trap reporter is a plain `fn`, called at an
//! arbitrary moment on an arbitrary stack, so it cannot capture anything.

use core::{
    fmt::Write,
    sync::atomic::{AtomicUsize, Ordering},
};

use mlos_hal_aarch64::Pl011;
use mlos_trap_aarch64::{Trap, VECTOR_NAMES};

/// The console base, published for the reporter to find. Zero until boot
/// has discovered one, in which case a fault cannot be reported at all.
static CONSOLE: AtomicUsize = AtomicUsize::new(0);

/// Tells the reporter where the console is.
pub fn publish_console(base: usize) {
    CONSOLE.store(base, Ordering::Relaxed);
}

/// Prints what the CPU said about the fault, then stops.
///
/// Stops rather than returns: nothing that reaches these vectors today is
/// recoverable, and resuming into the instruction that faulted would just
/// fault again, forever, with the console filling up.
pub fn report(trap: &Trap) -> ! {
    let base = CONSOLE.load(Ordering::Relaxed);
    if base != 0 {
        let mut console = Pl011::at(base);
        let name = VECTOR_NAMES.get(trap.vector).copied().unwrap_or("?");
        let _ = writeln!(console, "\n!! trap {} ({})", trap.vector, name);
        let _ = writeln!(
            console,
            "   esr  {:#018x}  ec {:#04x}",
            trap.esr,
            trap.exception_class()
        );
        let _ = writeln!(console, "   elr  {:#018x}", trap.elr);
        let _ = writeln!(console, "   far  {:#018x}", trap.far);
        let _ = writeln!(console, "   spsr {:#018x}", trap.spsr);
    }
    loop {
        core::hint::spin_loop();
    }
}
