//! Where the kernel image actually sits.
//!
//! The device tree describes what the *machine* has. It knows nothing
//! about what a loader put into it, so the fact that the bottom of RAM is
//! occupied by us has to come from somewhere else -- and the only thing
//! that knows is the linker script that placed us there.

unsafe extern "C" {
    /// First byte of the image. Defined by `linker/aarch64.ld`.
    static __kernel_start: u8;
    /// One past the last byte, including `.bss` and the boot stack.
    static __kernel_end: u8;
}

/// The image's `(base, length)` in physical memory.
///
/// Includes `.bss` and the boot stack, which occupy RAM but no file bytes.
/// Reporting only the file's worth would leave the stack looking
/// allocatable, which is a subtle way to hand out the ground you are
/// standing on.
#[must_use]
pub fn extent() -> (u64, u64) {
    // `&raw const` rather than a reference: these symbols have no valid
    // value, only an address, and forming a `&u8` to them would claim
    // otherwise.
    let start = &raw const __kernel_start as u64;
    let end = &raw const __kernel_end as u64;
    (start, end - start)
}
