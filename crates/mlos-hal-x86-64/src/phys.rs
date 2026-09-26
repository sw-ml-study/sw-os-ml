//! Physical memory as byte slices, for the structures the loader left.
//!
//! The PVH start info, its memory map and the command line are at
//! physical addresses the loader chose. The boot map identity-maps the
//! first 1 GiB, so an address below that IS a pointer; above it, nothing
//! is mapped and these return empty rather than fault.
//!
//! This is the only place a loader-supplied address becomes a reference.
//! Parsing what is there is `mlos-pvh`'s job, in safe code.

/// The end of the identity map `entry.s` builds.
const MAPPED: u64 = 1 << 30;

/// Longest command line read. QEMU's own limit is lower; this is only a
/// guard against a missing terminator walking off through memory.
const MAX_CMDLINE: usize = 4096;

/// `len` bytes at physical `addr`, or empty if any of them is unmapped.
///
/// # Safety
///
/// The range must be memory the loader wrote and nothing else mutates
/// for `'static` -- the PVH structures, which the kernel never writes.
#[must_use]
pub unsafe fn bytes(addr: u64, len: usize) -> &'static [u8] {
    if addr == 0 || addr.saturating_add(len as u64) > MAPPED {
        return &[];
    }
    // SAFETY: identity-mapped (checked above), loader-written and never
    // mutated (caller's contract), and `u8` has no alignment requirement.
    unsafe { core::slice::from_raw_parts(addr as *const u8, len) }
}

/// The NUL-terminated string at physical `addr`, or `""` if it is unmapped,
/// unterminated within [`MAX_CMDLINE`] bytes, or not UTF-8.
///
/// # Safety
///
/// As for [`bytes`].
#[must_use]
pub unsafe fn c_str(addr: u64) -> &'static str {
    let room = usize::try_from(MAPPED.saturating_sub(addr))
        .unwrap_or(0)
        .min(MAX_CMDLINE);
    // SAFETY: forwarded; `room` keeps the window inside the identity map.
    let window = unsafe { bytes(addr, room) };
    let text = window
        .split(|&b| b == 0)
        .next()
        .filter(|t| t.len() < window.len());
    text.and_then(|t| core::str::from_utf8(t).ok())
        .unwrap_or("")
}
