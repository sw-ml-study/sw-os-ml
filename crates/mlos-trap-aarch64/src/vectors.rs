//! The vector table.
//!
//! Sixteen entries, each 128 bytes, the whole table aligned to 2048 --
//! `VBAR_EL1` has no bits for anything finer. The linker script places
//! `.text.vectors` on that boundary.
//!
//! Every entry does the same two things: load its own index and branch.
//! That is all 128 bytes needs to hold, and keeping the assembly to the
//! irreducible minimum is deliberate -- code reached only when something
//! has already gone wrong is code that gets tested least.

use crate::report;

/// One vector entry: aligned to its 128-byte slot, tagged with its index.
macro_rules! vector {
    ($index:literal) => {
        concat!(".balign 0x80\n", "mov x0, #", $index, "\n", "b {report}\n")
    };
}

/// The table `VBAR_EL1` points at.
///
/// Naked and in its own section: the compiler must emit nothing before
/// the first entry, because `VBAR_EL1` addresses the table itself and
/// anything at offset 0 that is not entry 0 is taken as entry 0.
#[unsafe(naked)]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.vectors")]
pub extern "C" fn vector_table() -> ! {
    core::arch::naked_asm!(
        vector!(0), vector!(1), vector!(2), vector!(3),      // Current EL, SP0
        vector!(4), vector!(5), vector!(6), vector!(7),      // Current EL, SPx
        vector!(8), vector!(9), vector!(10), vector!(11),    // Lower EL, AArch64
        vector!(12), vector!(13), vector!(14), vector!(15),  // Lower EL, AArch32
        report = sym report,
    )
}
