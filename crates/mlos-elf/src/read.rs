//! Little-endian field reads that refuse to run off the end.
//!
//! Bounds-checked rather than trusting the file, because the file is an
//! input: a truncated ELF should produce a message naming the offset, not
//! a panic in a tool the user did not know parsed ELF.

use std::io;

/// `N` bytes at `at`, or an error naming where the file ran out.
fn field<const N: usize>(bytes: &[u8], at: usize) -> io::Result<[u8; N]> {
    at.checked_add(N)
        .and_then(|end| bytes.get(at..end))
        .and_then(|slice| slice.try_into().ok())
        .ok_or_else(|| io::Error::other(format!("ELF truncated at {at:#x}")))
}

/// A 16-bit field.
pub fn u16_at(bytes: &[u8], at: usize) -> io::Result<u16> {
    field(bytes, at).map(u16::from_le_bytes)
}

/// A 32-bit field.
pub fn u32_at(bytes: &[u8], at: usize) -> io::Result<u32> {
    field(bytes, at).map(u32::from_le_bytes)
}

/// A 64-bit field.
pub fn u64_at(bytes: &[u8], at: usize) -> io::Result<u64> {
    field(bytes, at).map(u64::from_le_bytes)
}
