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

use std::{fs, io, path::PathBuf};

use mlos_trace::MARKER;

/// Where `mlos runtime` writes the snapshot.
pub const OUT: &str = "build/runtime-layout.json";

/// Where it writes the event stream.
pub const EVENTS: &str = "build/runtime-events.jsonl";

/// The boot script that produces one: register, fill memory, then emit.
///
/// A sweep before the snapshot on purpose. A layout of an arena nothing
/// has been put in is a picture of an empty box, and the whole point of
/// the runtime document is what residency looks like under pressure --
/// the arena is a quarter the size of the model, so the sweep stops part
/// way and the boundary that leaves is the interesting line in the image.
///
/// TWICE, because one sweep produces no hits. The second walks the same
/// tiles, finds the first 32 already resident, and stops at the same
/// place -- so the stream carries all three kinds of event rather than
/// two, and a consumer can tell a re-use from a fetch without having to
/// be told that the missing kind exists.
pub const SCRIPT: &str = "model;sweep;sweep;layout;trace";

/// Writes `text` to `path`, making its directory if need be.
///
/// Here rather than in the caller because both emitted files and both
/// callers want it, and a second copy is a second place for the
/// directory-creation to be forgotten.
pub fn save(path: &str, text: &str) -> io::Result<PathBuf> {
    let out = PathBuf::from(path);
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&out, text)?;
    Ok(out)
}

/// The event lines in `console`, marked and one per line.
///
/// Extracting the stream is a prefix match and nothing else, which is the
/// whole reason the marker exists: a consumer sharing this console with
/// the shell's prose should not have to parse the prose. The marker is
/// stripped, leaving plain JSON Lines.
#[must_use]
pub fn events(console: &str) -> String {
    let marked = console
        .lines()
        .filter_map(|line| line.trim_end().strip_prefix(MARKER))
        .map(str::trim_start);
    marked.collect::<Vec<_>>().join("\n") + "\n"
}

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
