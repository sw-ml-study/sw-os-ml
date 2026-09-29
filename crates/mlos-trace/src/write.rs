//! Rendering a trace: a header, then one text line per access.
//!
//! Invariant: the header carries the count, so a reader can size its
//! buffer before parsing. Design: docs/notes/mlos-trace.md.

use core::fmt::Write;

use crate::{Access, Header, MAGIC, VERSION};

/// Writes a whole trace: header, then one line per access.
pub fn render(out: &mut impl Write, header: Header<'_>, accesses: &[Access]) -> core::fmt::Result {
    writeln!(out, "{MAGIC} {VERSION}")?;
    writeln!(out, "model {}", header.model)?;
    writeln!(out, "source {}", header.source)?;
    writeln!(out, "accesses {}", accesses.len())?;
    for access in accesses {
        writeln!(out, "{} {:016x}", access.session.0, access.object.0)?;
    }
    Ok(())
}
