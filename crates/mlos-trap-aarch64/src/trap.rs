//! What the CPU can tell us about why it stopped.

use core::{arch::asm, fmt::Write};

/// The sixteen entries of an aarch64 vector table, in architectural order.
///
/// Which one fired is diagnostic on its own. A fault from `CurrentSpx` is
/// the kernel's own bug; the same fault from `Lower64` is a userspace one,
/// and long before there is userspace, seeing `Lower64` at all would mean
/// something is very wrong.
pub const VECTOR_NAMES: [&str; 16] = [
    "CurrentSp0/sync",
    "CurrentSp0/irq",
    "CurrentSp0/fiq",
    "CurrentSp0/serror",
    "CurrentSpx/sync",
    "CurrentSpx/irq",
    "CurrentSpx/fiq",
    "CurrentSpx/serror",
    "Lower64/sync",
    "Lower64/irq",
    "Lower64/fiq",
    "Lower64/serror",
    "Lower32/sync",
    "Lower32/irq",
    "Lower32/fiq",
    "Lower32/serror",
];

/// A snapshot of why an exception was taken.
#[derive(Clone, Copy)]
pub struct Trap {
    /// Which vector fired, indexing [`VECTOR_NAMES`].
    pub vector: usize,
    /// `ESR_EL1`: the syndrome. Its top six bits are the exception class.
    pub esr: u64,
    /// `ELR_EL1`: the instruction that faulted, or the one to return to.
    pub elr: u64,
    /// `FAR_EL1`: the address that faulted. Only meaningful for aborts.
    pub far: u64,
    /// `SPSR_EL1`: the processor state at the moment of the exception.
    pub spsr: u64,
}

impl Trap {
    /// Reads the syndrome registers.
    ///
    /// # Safety
    ///
    /// Call only from an exception handler, before anything else can
    /// overwrite `ESR_EL1`, `ELR_EL1` or `FAR_EL1` -- taking a second
    /// exception first would report that one instead.
    #[must_use]
    pub unsafe fn capture(vector: usize) -> Self {
        let (esr, elr, far, spsr): (u64, u64, u64, u64);
        // SAFETY: reads of exception syndrome registers. No side effects,
        // and always accessible at EL1.
        unsafe {
            asm!(
                "mrs {esr},  esr_el1",
                "mrs {elr},  elr_el1",
                "mrs {far},  far_el1",
                "mrs {spsr}, spsr_el1",
                esr = out(reg) esr, elr = out(reg) elr,
                far = out(reg) far, spsr = out(reg) spsr,
                options(nomem, nostack),
            );
        }
        Self {
            vector,
            esr,
            elr,
            far,
            spsr,
        }
    }

    /// The exception class, `ESR_EL1[31:26]`.
    ///
    /// `0x25` is a data abort from the current EL -- overwhelmingly the
    /// one a kernel meets first, and what a wrong page table produces.
    #[must_use]
    pub const fn exception_class(self) -> u8 {
        ((self.esr >> 26) & 0x3f) as u8
    }
}

/// Writes what the CPU said about a fault.
///
/// Lives with `Trap` rather than in the kernel because it is entirely
/// about this type: which registers matter, and what their bits mean.
pub fn describe(trap: &Trap, out: &mut impl Write) {
    let name = VECTOR_NAMES.get(trap.vector).copied().unwrap_or("?");
    let _ = writeln!(out, "\n!! trap {} ({name})", trap.vector);
    let _ = writeln!(
        out,
        "   esr  {:#018x}  ec {:#04x}",
        trap.esr,
        trap.exception_class()
    );
    let _ = writeln!(out, "   elr  {:#018x}", trap.elr);
    let _ = writeln!(out, "   far  {:#018x}", trap.far);
    let _ = writeln!(out, "   spsr {:#018x}", trap.spsr);
}
