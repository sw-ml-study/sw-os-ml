//! Lifting a runtime document off a captured console.
//!
//! MLOS has no filesystem, so the guest writes its layout to the console
//! and the host cuts it back out. Crude, and honest about being crude: a
//! serial line is the only channel out of the machine at M2, and inventing
//! a file system to avoid admitting that would be a much larger lie than a
//! pair of brace-delimited markers.
//!
//! The document is found by structure rather than by a sentinel -- a line
//! that is exactly `{` opens it and a line that is exactly `}` closes it,
//! and nothing else `mlsh` prints begins a line with a brace. Step 010
//! adds an event stream that shares this console, which is when a real
//! framing will be worth the trouble.

use std::io;

/// Where `mlos runtime` writes.
pub const OUT: &str = "build/runtime-layout.json";

/// The boot script that produces one: register, fill memory, then emit.
///
/// A sweep before the snapshot on purpose. A layout of an arena nothing
/// has been put in is a picture of an empty box, and the whole point of
/// the runtime document is what residency looks like under pressure --
/// the arena is a quarter the size of the model, so the sweep stops part
/// way and the boundary that leaves is the interesting line in the image.
pub const SCRIPT: &str = "model;sweep;layout";

/// The JSON document in `console`, with host line endings.
pub fn extract(console: &str) -> io::Result<String> {
    let lines: Vec<&str> = console.lines().map(str::trim_end).collect();
    let open = lines
        .iter()
        .position(|line| *line == "{")
        .ok_or_else(|| missing("never printed a layout", console))?;
    let close = lines[open..]
        .iter()
        .position(|line| *line == "}")
        .ok_or_else(|| missing("printed a layout that never ended", console))?;
    Ok(lines[open..=open + close].join("\n") + "\n")
}

/// Why no document came back, with the console attached.
///
/// The console text is the error, not decoration. A guest that did not
/// print a layout usually did not get as far as the shell, and the reason
/// is somewhere in what it did print -- so putting it in front of whoever
/// ran the command saves them running it again to look.
fn missing(what: &str, console: &str) -> io::Error {
    io::Error::other(format!("the guest {what}. Console said:\n{console}"))
}
