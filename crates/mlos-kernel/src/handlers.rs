//! What the kernel does when the CPU hands it control.
//!
//! Two paths that differ in kind. A fault is a bug: it reports and stops.
//! An interrupt is expected: it services and returns. They share only the
//! console, which is why they share a module.
//!
//! Everything here runs at an arbitrary moment on an arbitrary stack, so
//! nothing here allocates, takes a lock, or borrows anything.

use core::{
    cell::UnsafeCell,
    fmt::Write,
    sync::atomic::{AtomicU32, AtomicUsize, Ordering},
};

use mlos_device::IrqController;
use mlos_gic_aarch64::Gic;
use mlos_hal_aarch64::GenericTimer;
use mlos_pl011::Pl011;
use mlos_trap_aarch64::Trap;

/// A value published once at boot and read from interrupt context.
///
/// Not a lock: there is nothing to contend with. It is written on the boot
/// core with interrupts still masked, and only read afterwards.
struct Published<T>(UnsafeCell<Option<T>>);

// SAFETY: written once before interrupts are unmasked, read-only after.
unsafe impl<T> Sync for Published<T> {}

/// The interrupt controller, to acknowledge through.
static GIC: Published<Gic> = Published(UnsafeCell::new(None));
/// The console's base address; zero until boot has found one.
static CONSOLE: AtomicUsize = AtomicUsize::new(0);
/// Timer ticks between rearming.
static INTERVAL: AtomicU32 = AtomicU32::new(0);
/// Ticks seen, so the console shows time actually passing.
static TICKS: AtomicU32 = AtomicU32::new(0);
/// Which interrupt the timer raises.
static TIMER_IRQ: AtomicU32 = AtomicU32::new(0);
/// Which interrupt the console raises.
static UART_IRQ: AtomicU32 = AtomicU32::new(0);

/// Publishes what the handlers need, before any of them can run.
///
/// # Safety
///
/// Call once, on the boot core, with interrupts still masked.
pub unsafe fn publish(base: usize, gic: Gic, interval: u32, irqs: (u32, u32)) {
    CONSOLE.store(base, Ordering::Relaxed);
    INTERVAL.store(interval, Ordering::Relaxed);
    TIMER_IRQ.store(irqs.0, Ordering::Relaxed);
    UART_IRQ.store(irqs.1, Ordering::Relaxed);
    // SAFETY: the only write, before anything can read it.
    unsafe { *GIC.0.get() = Some(gic) };
}

/// Services one interrupt: acknowledge, dispatch by source, complete.
///
/// The claim/complete pair brackets everything. Until `complete`, the
/// controller delivers nothing more at this priority, so a path that
/// returns early without it stops the system dead.
///
/// Rearming the timer is equally load-bearing in the other direction: it
/// asserts its output for as long as its countdown is negative, so a
/// handler that acknowledges without rearming is re-entered the instant it
/// returns, forever -- a livelock that reads as a hang.
pub fn on_irq() {
    // SAFETY: published before interrupts were unmasked, never written
    // again, so this is a shared read of an initialised value.
    let Some(gic) = (unsafe { (*GIC.0.get()).as_ref() }) else {
        return;
    };
    let Some(irq) = gic.claim() else {
        return; // spurious: nothing was pending after all
    };

    if irq.0 == TIMER_IRQ.load(Ordering::Relaxed) {
        GenericTimer.arm(INTERVAL.load(Ordering::Relaxed));
        let tick = TICKS.fetch_add(1, Ordering::Relaxed) + 1;
        if tick <= 2 {
            let _ = writeln!(console(), "  tick     {tick}");
        }
    } else if irq.0 == UART_IRQ.load(Ordering::Relaxed) {
        console().drain_echo();
    }
    gic.complete(irq);
}

/// Reports a fault, then stops.
///
/// Stops rather than returns: nothing that reaches the vectors today is
/// recoverable, and resuming into the instruction that faulted would fault
/// again, forever, with the console filling up.
pub fn report(trap: &Trap) -> ! {
    if CONSOLE.load(Ordering::Relaxed) != 0 {
        mlos_trap_aarch64::describe(trap, &mut console());
    }
    loop {
        core::hint::spin_loop();
    }
}

/// The published console.
fn console() -> Pl011 {
    Pl011::at(CONSOLE.load(Ordering::Relaxed))
}
