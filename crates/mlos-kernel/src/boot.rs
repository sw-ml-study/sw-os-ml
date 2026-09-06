//! Bring-up: find out what this machine is, say so, then map it.
//!
//! Its own module so the entry point stays a single decision -- run this,
//! and if it declines, stop.

use mlos_device::{Irq, IrqController, Timer};
use mlos_gic_aarch64::Gic;
use mlos_hal::BootInfo;

use mlos_hal_aarch64::{GenericTimer, TIMER_PPI};
use mlos_machine::Machine;
use mlos_pl011::Pl011;
use mlsh::{Facts, Shell};

use crate::{banner, handlers};

/// Probes the device tree, opens the console it names, reports the
/// machine, then turns on translation.
///
/// `None` if the device tree cannot be read or names no console. That is
/// deliberate rather than a fallback: a machine we cannot read is one we
/// cannot run on, and limping along on a guessed console address would
/// hide the failure instead of showing it.
///
/// # Safety
///
/// Called once, on the boot core, with the MMU off. `dtb` must be what the
/// arm64 boot protocol left in `x0`.
pub unsafe fn bring_up(dtb: *const u8) -> Option<()> {
    // SAFETY: forwarded to `describe`, whose contract this is.
    let machine = unsafe { Machine::probe(dtb) }?
        .reserving(mlos_hal_aarch64::extent().0, mlos_hal_aarch64::extent().1);
    let uart = machine.uart_base?;
    let mut console = Pl011::at(uart);

    let info = BootInfo::new(machine.regions.as_slice(), machine.cpu_count);
    banner::report(&mut console, dtb as usize, &info, uart);

    // SAFETY: boot core, MMU off, once. The map covers the kernel image
    // and its stack, so the code that must survive the switch is mapped.
    unsafe { mlos_mmu_aarch64::enable_identity_map(info.regions) };
    banner::mmu(&mut console);

    // SAFETY: boot core, once, with the console known.
    unsafe { arm_interrupts(&mut console, uart, &machine) }?;

    Shell::default().run(
        &mut console,
        &facts(&machine, uart),
        mlos_hal_aarch64::wait_for_interrupt,
    );
}

/// Installs the vector table, brings up the interrupt controller, and
/// starts the timer.
///
/// Order is load-bearing throughout, and each step is quiet when wrong:
/// vectors before anything can trap, the controller before an interrupt
/// has anywhere to go, the timer before it is unmasked, and the unmask
/// last. Unmasking early with a half-built controller behind it fires
/// immediately and repeatedly, which is far harder to read than a machine
/// that simply never ticks.
///
/// # Safety
///
/// Call once, on the boot core, with interrupts masked and the console
/// already known.
unsafe fn arm_interrupts(console: &mut Pl011, uart: usize, machine: &Machine) -> Option<()> {
    // SAFETY: boot core, once. Neither handler allocates or takes a lock.
    unsafe { mlos_trap_aarch64::install(banner::fault, handlers::on_irq) };

    // SAFETY: forwarded to `route`, whose contract this is.
    let (gic, uart_irq) = unsafe { route(console, machine) }?;

    // Twice a second: slow enough to read on a console, fast enough that
    // a boot capture of a few seconds shows time actually passing.
    let interval = GenericTimer.frequency().0 / 2;
    banner::interrupts(console, GenericTimer.frequency().0, TIMER_PPI, uart_irq);

    // SAFETY: boot core, interrupts still masked, nothing has run yet.
    unsafe { handlers::publish(uart, gic, interval, (TIMER_PPI, uart_irq)) };
    GenericTimer.arm(interval);

    // SAFETY: vectors installed, controller up, timer armed.
    unsafe { mlos_trap_aarch64::unmask() };
    Some(())
}

/// Brings up the interrupt controller and routes the two sources that
/// exist: the timer, private to this CPU, and the console, shared.
///
/// # Safety
///
/// Call once, on the boot core, with interrupts masked.
unsafe fn route(console: &Pl011, machine: &Machine) -> Option<(Gic, u32)> {
    let (dist, redist) = machine.gic?;
    // SAFETY: the windows came from the device tree's `arm,gic-v3` node.
    let gic = unsafe { Gic::new(dist as usize, redist as usize) };

    let uart_irq = machine.uart_irq?;
    gic.enable(Irq(TIMER_PPI));
    gic.enable(Irq(uart_irq));
    console.enable_receive_interrupt();
    Some((gic, uart_irq))
}

/// Everything the shell can report on, gathered once.
fn facts(machine: &Machine, uart: usize) -> Facts<'_> {
    Facts {
        info: BootInfo::new(machine.regions.as_slice(), machine.cpu_count),
        total: machine.regions.as_slice().iter().map(|r| r.len).sum(),
        image: mlos_hal_aarch64::extent(),
        uart,
        uart_irq: machine.uart_irq.unwrap_or_default(),
        timer_irq: TIMER_PPI,
        gic: machine.gic,
        ticks: &handlers::TICKS,
    }
}
