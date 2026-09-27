//! What the CPU can tell us about why it stopped, and saying it.

use core::{arch::asm, fmt::Write};

/// The architectural names of the 32 exception vectors, short form.
pub const VECTOR_NAMES: [&str; 32] = [
    "#DE divide",
    "#DB debug",
    "NMI",
    "#BP breakpoint",
    "#OF overflow",
    "#BR bound",
    "#UD invalid opcode",
    "#NM no fpu",
    "#DF double fault",
    "coprocessor overrun",
    "#TS invalid tss",
    "#NP segment absent",
    "#SS stack",
    "#GP general protection",
    "#PF page fault",
    "reserved",
    "#MF x87",
    "#AC alignment",
    "#MC machine check",
    "#XM simd",
    "#VE virtualization",
    "#CP control protection",
    "reserved",
    "reserved",
    "reserved",
    "reserved",
    "reserved",
    "reserved",
    "#HV hypervisor injection",
    "#VC vmm communication",
    "#SX security",
    "reserved",
];

/// A snapshot of why an exception was taken: the x86-64 twin of the
/// aarch64 `Trap`'s ESR/ELR/FAR/SPSR.
#[derive(Clone, Copy)]
pub struct Trap {
    /// Which vector fired, indexing [`VECTOR_NAMES`].
    pub vector: u64,
    /// The error code, where the vector has one; zero otherwise.
    pub error: u64,
    /// `RIP`: the faulting instruction, or the one after a trap.
    pub rip: u64,
    /// `CR2`: the address a page fault was trying to reach. Stale for
    /// every other vector.
    pub cr2: u64,
    /// `RFLAGS` at the moment of the exception.
    pub rflags: u64,
}

impl Trap {
    /// Reads the frame the stub built and `CR2`.
    ///
    /// # Safety
    ///
    /// Only from the exception path, before anything can fault again and
    /// overwrite `CR2`.
    #[must_use]
    pub unsafe fn capture(frame: &[u64; 7]) -> Self {
        let cr2: u64;
        // SAFETY: reading CR2 at CPL 0 has no side effects.
        unsafe { asm!("mov {}, cr2", out(reg) cr2, options(nomem, nostack)) };
        let [vector, error, rip, _cs, rflags, _rsp, _ss] = *frame;
        Self {
            vector,
            error,
            rip,
            cr2,
            rflags,
        }
    }
}

/// Prints the report, in the same shape as `mlos_trap_aarch64::describe`.
pub fn describe(trap: &Trap, out: &mut impl Write) {
    let name = VECTOR_NAMES
        .get(trap.vector as usize)
        .copied()
        .unwrap_or("?");
    let _ = writeln!(out, "\n!! trap {} ({name})", trap.vector);
    let _ = writeln!(out, "   error  {:#018x}", trap.error);
    let _ = writeln!(out, "   rip    {:#018x}", trap.rip);
    let _ = writeln!(out, "   cr2    {:#018x}", trap.cr2);
    let _ = writeln!(out, "   rflags {:#018x}", trap.rflags);
}
