//! What MLOS says about the machine it woke up on.
//!
//! Its own module because the entry point should read as a sequence of
//! decisions, not as a print statement with a probe attached.

use core::fmt::Write;

use mlos_hal::BootInfo;

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

/// Announces the self-test that follows, so the trap output that comes
/// next reads as deliberate rather than as a crash.
pub fn selftest(console: &mut impl Write) {
    let _ = writeln!(
        console,
        "  vectors  installed; faulting on purpose to prove it"
    );
}
