//! What the kernel does when an interrupt arrives.
//!
//! Invariant: everything here runs at an arbitrary moment on an arbitrary
//! stack, so nothing here allocates, takes a lock, or borrows anything.
//! Design and history: docs/notes/mlos-kernel.md.

use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicU32, Ordering},
};

use mlos_console::Terminal;
use mlos_device::IrqController;
use mlos_gic_aarch64::Gic;
use mlos_hal_aarch64::GenericTimer;

/// A value published once at boot, on the boot core with interrupts
/// masked, and only read afterwards.
pub(crate) struct Published<T>(pub(crate) UnsafeCell<Option<T>>);

// SAFETY: written once before interrupts are unmasked, read-only after.
unsafe impl<T> Sync for Published<T> {}

/// The interrupt controller, to acknowledge through.
static GIC: Published<Gic> = Published(UnsafeCell::new(None));
/// The console, whichever kind this machine has.
pub(crate) static CONSOLE: Published<Terminal> = Published(UnsafeCell::new(None));
/// Timer ticks between rearming.
static INTERVAL: AtomicU32 = AtomicU32::new(0);
/// Ticks seen. Read by the shell, which borrows it rather than asking.
pub(crate) static TICKS: AtomicU32 = AtomicU32::new(0);
/// Which interrupt the timer raises.
static TIMER_IRQ: AtomicU32 = AtomicU32::new(0);
/// Which interrupt the console raises.
static UART_IRQ: AtomicU32 = AtomicU32::new(0);

/// Publishes what the handlers need, before any of them can run.
///
/// # Safety
///
/// Call once, on the boot core, with interrupts still masked.
pub unsafe fn publish(console: Terminal, gic: Gic, interval: u32, irqs: (u32, u32)) {
    INTERVAL.store(interval, Ordering::Relaxed);
    TIMER_IRQ.store(irqs.0, Ordering::Relaxed);
    UART_IRQ.store(irqs.1, Ordering::Relaxed);
    // SAFETY: the only writes, before anything can read them.
    unsafe {
        *GIC.0.get() = Some(gic);
        *CONSOLE.0.get() = Some(console);
    }
}

/// Services one interrupt: acknowledge, dispatch by source, complete.
/// Every claim must reach `complete` or delivery at this priority stops;
/// the timer must be rearmed or the handler is re-entered forever.
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
        TICKS.fetch_add(1, Ordering::Relaxed);
    } else if irq.0 == UART_IRQ.load(Ordering::Relaxed) {
        queue_input();
    }
    gic.complete(irq);
}

/// Moves everything waiting in the console into the shell's queue. Queue
/// only: dispatching here would hold the interrupt active.
fn queue_input() {
    // SAFETY: published before interrupts were unmasked, never rewritten.
    let Some(console) = (unsafe { (*CONSOLE.0.get()).as_ref() }) else {
        return;
    };
    console.clear_interrupt();
    while let Some(byte) = console.read() {
        mlsh::push(byte);
    }
}
