//! Writing columns to a console, one pass each.
//!
//! The host emitter builds a `Vec` per column and joins it. There is no
//! allocator here, so a column is written as it is walked: open the
//! bracket, write a separator before every item but the first, close it.
//! Separator-before rather than comma-after is what keeps the JSON valid
//! without knowing in advance how many rows there are.
//!
//! Errors are dropped rather than propagated. The sink is a console: if it
//! has stopped accepting bytes there is nowhere to report that to, and a
//! half-written document is already in front of whoever is reading it.

use core::fmt::{Display, Write};

/// Writes one column, taking each item from `of`.
pub fn column<T>(
    out: &mut impl Write,
    name: &str,
    rows: impl Iterator<Item = T>,
    of: impl Fn(&mut dyn Write, &T),
) {
    let _ = write!(out, "  \"{name}\": [");
    for (index, row) in rows.enumerate() {
        if index > 0 {
            let _ = out.write_str(", ");
        }
        of(out, &row);
    }
    let _ = out.write_str("],\r\n");
}

/// A column of numbers.
pub fn nums<T>(
    out: &mut impl Write,
    name: &str,
    rows: impl Iterator<Item = T>,
    of: impl Fn(&T) -> u64,
) {
    column(out, name, rows, |out, row| {
        let _ = write!(out, "{}", of(row));
    });
}

/// A column of strings, quoted, written by `of`.
///
/// Nothing MLOS puts in one of these needs escaping -- they are built out
/// of decimal numbers and fixed words -- and a `no_std` escaper with
/// nowhere to build the escaped string would have to go character by
/// character for no benefit. `tests/document.rs` pins the vocabulary, so a
/// value that would need escaping is a test failure rather than malformed
/// JSON in somebody else's parser.
pub fn text<T>(
    out: &mut impl Write,
    name: &str,
    rows: impl Iterator<Item = T>,
    of: impl Fn(&mut dyn Write, &T),
) {
    column(out, name, rows, |out, row| {
        let _ = out.write_str("\"");
        of(out, row);
        let _ = out.write_str("\"");
    });
}

/// A column of strings that already know how to print themselves.
pub fn strs<T, D: Display>(
    out: &mut impl Write,
    name: &str,
    rows: impl Iterator<Item = T>,
    of: impl Fn(&T) -> D,
) {
    text(out, name, rows, |out, row| {
        let _ = write!(out, "{}", of(row));
    });
}
