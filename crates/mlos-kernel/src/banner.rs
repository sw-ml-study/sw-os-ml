//! What MLOS says about the machine it woke up on.
//!
//! Invariant: `fault` never returns. Design and history:
//! docs/notes/mlos-kernel.md.

use core::fmt::Write;

use mlos_hal::BootInfo;
use mlos_trap_aarch64::Trap;

use crate::handlers::CONSOLE;

/// Reports what the device tree said.
pub fn report(console: &mut impl Write, dtb: usize, info: &BootInfo<'_>, kind: &str) {
    let _ = writeln!(console, "\nMLOS aarch64");
    let _ = writeln!(console, "  dtb      {dtb:#018x}");
    let _ = writeln!(console, "  console  {kind}");
    let _ = writeln!(console, "  cpus     {}", info.cpu_count);
    let _ = writeln!(console, "  usable   {} MiB", info.usable_bytes() >> 20);
    for region in info.regions {
        let (base, len, kind) = (region.base, region.len, region.kind);
        let _ = writeln!(console, "    {base:#012x} + {len:#x}  {kind:?}");
    }
}

/// Reports that translation is on, reading `SCTLR_EL1.M` back rather than
/// asserting it.
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

/// Reports a fault, then stops: nothing that reaches the vectors today is
/// recoverable.
pub fn fault(trap: &Trap) -> ! {
    // SAFETY: published at boot and never rewritten. A fault before the
    // console exists has nowhere to report and simply stops.
    if let Some(console) = unsafe { *CONSOLE.0.get() } {
        mlos_trap_aarch64::describe(trap, &mut { console });
    }
    loop {
        core::hint::spin_loop();
    }
}
