//! Decoding `reg` properties.
//!
//! A `reg` value is a flat run of big-endian cells whose widths come from
//! the *parent* node's `#address-cells` and `#size-cells`. That indirection
//! is why this is a function taking both, rather than something a caller
//! can read off the bytes.

/// Reads `count` consecutive big-endian cells starting at cell `at`.
///
/// Refuses more than two cells: a device tree may in principle use wider
/// addresses, but nothing that would fit in a `u64` does, and silently
/// truncating an address is worse than admitting we cannot read it.
fn cells(value: &[u8], at: usize, count: u32) -> Option<u64> {
    if count == 0 || count > 2 {
        return None;
    }
    let start = at * 4;
    let bytes = value.get(start..start + (count as usize * 4))?;
    Some(bytes.iter().fold(0u64, |acc, &b| (acc << 8) | u64::from(b)))
}

/// Reads the `index`th `(address, size)` pair from a `reg` value.
///
/// `None` if the value is too short, which is how a truncated or
/// mis-specified property is rejected rather than read as zeros.
#[must_use]
pub fn reg_pair(
    value: &[u8],
    address_cells: u32,
    size_cells: u32,
    index: usize,
) -> Option<(u64, u64)> {
    let stride = (address_cells + size_cells) as usize;
    let base = index * stride;
    let address = cells(value, base, address_cells)?;
    let size = cells(value, base + address_cells as usize, size_cells)?;
    Some((address, size))
}

/// A device tree string property, if it is valid UTF-8.
///
/// Property strings are NUL-terminated, and the terminator is inside the
/// value's declared length -- so a caller that compares the raw bytes to a
/// string literal is comparing against a trailing zero and always losing.
#[must_use]
pub const fn string(value: &[u8]) -> Option<&str> {
    match core::str::from_utf8(value) {
        Ok(text) => Some(text.trim_ascii_end()),
        Err(_) => None,
    }
}
