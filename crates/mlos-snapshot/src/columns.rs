//! Writing columns to a console, one pass each, with no buffer.
//!
//! Invariant: the separator goes before every item but the first, so the
//! JSON is valid without knowing the row count. Write errors are dropped.
//! Design: docs/notes/mlos-snapshot.md.

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

/// A column of strings, quoted but not escaped, written by `of`. Every
/// value must be a fixed word or a decimal number; `tests/document.rs`
/// pins that.
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
