//! Interrupts: arming the controllers, and what each vector does.
//!
//! The x86-64 counterpart of the aarch64 kernel's `handlers.rs` and its
//! `arm_interrupts`. Two sources, as there: the timer, private to this
//! CPU, at the 2 Hz the shell counts; and the console's receive line.

use core::fmt::Write;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use mlos_apic_x86_64::{IOAPIC_BASE, Lapic, clock};
use mlos_uart16550::{COM1, Uart16550};

/// The timer's vector, and COM1's: clear of the exceptions (0-31) and of
/// where the silenced PIC was parked (0xe0+).
pub const TIMER_VECTOR: u8 = 0x30;
const COM1_VECTOR: u8 = 0x34;
/// COM1's ISA line: IRQ 4, by PC convention.
pub const COM1_IRQ: u8 = 4;
/// Twice a second, as on aarch64: slow enough to read, fast enough that a
/// few seconds of capture shows time passing.
const TICK_HZ: u32 = 2;

/// Timer ticks since the timer started.
pub static TICKS: AtomicU32 = AtomicU32::new(0);
/// Bytes have been queued since the shell last looked (for `wait_unless`).
pub static PENDING: AtomicBool = AtomicBool::new(false);
/// The LAPIC's base, for the EOI every handler ends with. Zero until armed.
static LAPIC: AtomicU64 = AtomicU64::new(0);

/// Silences the PIC, enables the LAPIC, routes COM1 through the IOAPIC,
/// measures the clocks, starts the timer and unmasks -- reporting each.
/// Returns the clock's rate for `mlsh`, or `None` if there is no LAPIC,
/// in which case the kernel keeps polling.
///
/// Order is load-bearing, as on aarch64: the IDT is already in, the
/// controllers come up before anything is routed to them, the rates are
/// measured before the timer needs one, and `sti` is last.
pub fn arm(out: &mut Uart16550) -> Option<u32> {
    mlos_apic_x86_64::disable_pic();
    let lapic = Lapic::enable()?;
    LAPIC.store(lapic.base, Ordering::Release);
    mlos_apic_x86_64::route(COM1_IRQ, COM1_VECTOR, lapic.id());
    Uart16550::at(COM1).enable_receive_interrupt();
    let (hz, source) = clock::tsc_rate();
    let rate = lapic.timer_rate();
    lapic.start_timer(TIMER_VECTOR, TICK_HZ, rate);
    report(out, lapic, (hz, source), rate);
    // SAFETY: IDT loaded, controllers up, timer armed, handlers ready.
    unsafe { core::arch::asm!("sti", options(nomem, nostack)) };
    Some(hz)
}

/// What `arm` found, in the banner's layout.
fn report(out: &mut Uart16550, lapic: Lapic, (hz, source): (u32, clock::Source), rate: u32) {
    let (base, id) = (lapic.base, lapic.id());
    let how = if source == clock::Source::Cpuid {
        "read from cpuid 15h"
    } else {
        "calibrated against the PIT"
    };
    let _ = writeln!(
        out,
        "interrupts  lapic id {id} @ {base:#x}, ioapic @ {IOAPIC_BASE:#x}"
    );
    let _ = writeln!(
        out,
        "timer       lapic, vector {TIMER_VECTOR:#x}, {TICK_HZ} Hz ({rate} counts/s)"
    );
    let _ = writeln!(out, "com1        irq {COM1_IRQ} -> vector {COM1_VECTOR:#x}");
    let _ = writeln!(out, "clock       tsc, {} MHz, {how}", hz / 1_000_000);
}

/// Called by the trap crate's entry for every vector from 32 up.
pub fn on_irq(vector: u8) {
    match vector {
        TIMER_VECTOR => _ = TICKS.fetch_add(1, Ordering::Relaxed),
        COM1_VECTOR => receive(),
        mlos_apic_x86_64::SPURIOUS => return, // no EOI for a spurious interrupt
        _ => {}
    }
    Lapic {
        base: LAPIC.load(Ordering::Acquire),
    }
    .eoi();
}

/// Drains COM1 into `mlsh`'s queue. `Ctrl-D` marks the end instead, so
/// the guest exits after the shell has run what came before it.
pub fn receive() {
    while let Some(byte) = Uart16550::at(COM1).read() {
        if byte == crate::END {
            crate::ENDING.store(true, Ordering::Relaxed);
        } else {
            let _ = mlos_queue::push(byte);
        }
        PENDING.store(true, Ordering::Release);
    }
}
