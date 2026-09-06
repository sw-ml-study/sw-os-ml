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
