//! Reporting a fault out of the console.
//!
//! The console is reached through a static base address rather than a
//! borrowed handle: a trap reporter is a plain `fn`, called at an
//! arbitrary moment on an arbitrary stack, so it cannot capture anything.

use core::{
    fmt::Write,
    sync::atomic::{AtomicU32, AtomicUsize, Ordering},
};

use core::cell::UnsafeCell;

use mlos_device::IrqController;
use mlos_gic_aarch64::Gic;
use mlos_hal_aarch64::{GenericTimer, Pl011};
use mlos_trap_aarch64::{Trap, VECTOR_NAMES};

/// A value published once at boot and read from interrupt context.
///
/// Not a lock: there is nothing to contend with. It is written on the boot
/// core with interrupts still masked, and only read afterwards.
struct Published<T>(UnsafeCell<Option<T>>);

// SAFETY: written once before interrupts are unmasked, read-only after.
unsafe impl<T> Sync for Published<T> {}

/// The interrupt controller, for the handler to acknowledge through.
static GIC: Published<Gic> = Published(UnsafeCell::new(None));

/// How many timer ticks to count between rearming.
static INTERVAL: AtomicU32 = AtomicU32::new(0);

/// Ticks seen so far, so the console shows time actually passing.
static TICKS: AtomicU32 = AtomicU32::new(0);

/// The console base, published for the reporter to find. Zero until boot
/// has discovered one, in which case a fault cannot be reported at all.
static CONSOLE: AtomicUsize = AtomicUsize::new(0);

/// Publishes what the handlers will need, before any of them can run.
///
/// # Safety
///
/// Call once, on the boot core, with interrupts still masked.
pub unsafe fn publish(base: usize, gic: Gic, interval: u32) {
    CONSOLE.store(base, Ordering::Relaxed);
    INTERVAL.store(interval, Ordering::Relaxed);
    // SAFETY: the only write, before anything can read it.
    unsafe { *GIC.0.get() = Some(gic) };
}

/// Services one interrupt: acknowledge, rearm, report, complete.
///
/// Rearming is not optional and not cosmetic. The generic timer asserts
/// its output for as long as its countdown is negative, so a handler that
/// acknowledges without rearming is called again the instant it returns,
/// forever -- a livelock that looks exactly like a hang.
pub fn on_irq() {
    // SAFETY: published before interrupts were unmasked, never written
    // again, so this is a shared read of an initialised value.
    let Some(gic) = (unsafe { (*GIC.0.get()).as_ref() }) else {
        return;
    };
    let Some(irq) = gic.claim() else {
        return; // spurious: nothing was pending after all
    };

    GenericTimer.arm(INTERVAL.load(Ordering::Relaxed));
    let tick = TICKS.fetch_add(1, Ordering::Relaxed) + 1;
    let base = CONSOLE.load(Ordering::Relaxed);
    if base != 0 {
        let _ = writeln!(Pl011::at(base), "  tick     {tick} (irq {})", irq.0);
    }

    // Last: until this, the controller will not deliver another interrupt
    // at this priority, and the timer would appear to have stopped.
    gic.complete(irq);
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
