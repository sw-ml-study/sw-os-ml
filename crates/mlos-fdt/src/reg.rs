//! Decoding `reg` properties.
//!
//! Invariant: cell widths come from the parent node's `#address-cells`
//! and `#size-cells`, never from the value itself. Design and history:
//! docs/notes/mlos-fdt.md.

/// Reads `count` consecutive big-endian cells starting at cell `at`.
/// Refuses zero or more than two: a wider address would not fit a `u64`.
fn cells(value: &[u8], at: usize, count: u32) -> Option<u64> {
    if count == 0 || count > 2 {
        return None;
    }
    let start = at * 4;
    let bytes = value.get(start..start + (count as usize * 4))?;
    Some(bytes.iter().fold(0u64, |acc, &b| (acc << 8) | u64::from(b)))
}

/// Reads the `index`th `(address, size)` pair from a `reg` value. `None`
/// if the value is too short, rather than reading zeros.
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

/// A device tree string property with its NUL terminator stripped, if it
/// is valid UTF-8. The terminator is inside the declared length and
/// `trim_ascii_end` does not remove it, so it is stripped here.
#[must_use]
pub const fn string(value: &[u8]) -> Option<&str> {
    let mut end = value.len();
    while end > 0 && value[end - 1] == 0 {
        end -= 1;
    }
    match core::str::from_utf8(value.split_at(end).0) {
        Ok(text) => Some(text.trim_ascii_end()),
        Err(_) => None,
    }
}
