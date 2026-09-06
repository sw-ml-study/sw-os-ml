//! What MLOS says about the machine it woke up on.
//!
//! Its own module because the entry point should read as a sequence of
//! decisions, not as a print statement with a probe attached.

use core::{fmt::Write, sync::atomic::Ordering};

use mlos_hal::BootInfo;
use mlos_pl011::Pl011;
use mlos_trap_aarch64::Trap;

use crate::handlers::CONSOLE;

/// Reports what the device tree said, so a boot that reaches here proves
/// the whole chain: Image header, `x0`, the tree walk, and the console
/// address discovered from it.
pub fn report(console: &mut impl Write, dtb: usize, info: &BootInfo<'_>, uart: usize) {
    let _ = writeln!(console, "\nMLOS aarch64");
    let _ = writeln!(console, "  dtb      {dtb:#018x}");
    let _ = writeln!(console, "  console  pl011 @ {uart:#x}");
    let _ = writeln!(console, "  cpus     {}", info.cpu_count);
    let _ = writeln!(console, "  usable   {} MiB", info.usable_bytes() >> 20);
    for region in info.regions {
        let (base, len, kind) = (region.base, region.len, region.kind);
        let _ = writeln!(console, "    {base:#012x} + {len:#x}  {kind:?}");
    }
}

/// Reports that translation is on, reading `SCTLR_EL1.M` back rather than
/// asserting it. Printed through a device block of the table just
/// installed: if the mapping were wrong, this line would not appear.
pub fn mmu(console: &mut impl Write) {
    let _ = writeln!(
        console,
        "  mmu      identity, 1 GiB blocks, SCTLR_EL1.M={}",
        u8::from(mlos_mmu_aarch64::is_enabled())
    );
}

/// Reports the interrupt setup, so the ticks that follow are legible.
pub fn interrupts(console: &mut impl Write, frequency: u32, ppi: u32, uart: u32) {
    let _ = writeln!(
        console,
        "  gicv3    up, timer ppi {ppi}, console spi {uart}"
    );
    let _ = writeln!(
        console,
        "  timer    {frequency} Hz counter, ticking at 2 Hz"
    );
}

/// Reports a fault, then stops.
///
/// Stops rather than returns: nothing that reaches the vectors today is
/// recoverable, and resuming into the instruction that faulted would fault
/// again, forever, with the console filling up.
pub fn fault(trap: &Trap) -> ! {
    let base = CONSOLE.load(Ordering::Relaxed);
    if base != 0 {
        mlos_trap_aarch64::describe(trap, &mut Pl011::at(base));
    }
    loop {
        core::hint::spin_loop();
    }
}
