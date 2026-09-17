//! Rendering a trace.
//!
//! Text, one access per line, for the same reason the event stream is
//! text: a trace nobody can read is a trace nobody will question, and the
//! whole value of M3's numbers is that somebody else can dispute them.
//! It is also diffable, which matters when a trace is regenerated and the
//! question is what changed.
//!
//! Compact anyway. A line is a decimal session and a 16-digit hex
//! `ObjectId` -- about twenty bytes, against forty for the same thing as
//! JSON. At roughly two hundred accesses per token of a real model, the
//! difference is megabytes over a long trace and it costs nothing to
//! read.

use core::fmt::Write;

use crate::{Access, Header, MAGIC, VERSION};

/// Writes a whole trace: header, then one line per access.
///
/// The count is in the header so a reader can size a buffer before
/// parsing, which is what makes parsing possible at all without an
/// allocator.
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
