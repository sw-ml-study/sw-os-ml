//! Lifting a runtime document and event stream off a captured console.
//!
//! Invariant: the layout is the lines from a bare `{` to a bare `}`, and
//! nothing else `mlsh` prints begins a line with a brace; events carry
//! the `@ev` marker. Design: docs/notes/mlos-image-map.md.

use std::{fs, io, path::PathBuf};

use mlos_events::MARKER;
use mlos_trace::{Access, Header};

/// Where `mlos runtime` writes the snapshot.
pub const OUT: &str = "build/runtime-layout.json";

/// Where it writes the event stream.
pub const EVENTS: &str = "build/runtime-events.jsonl";

/// Where it writes the access trace derived from that stream.
pub const TRACE: &str = "build/runtime.trace";

/// The boot script that produces one: register, sweep twice so the
/// stream has hits as well as placements and refusals, then emit.
pub const SCRIPT: &str = "model;sweep;sweep;layout;trace";

/// Sessions and rounds for the replay both sides run. The boot test reads
/// this to build the simulator's side, so it must match what the disk
/// writer renders.
pub const REPLAY: (u16, u16) = (2, 16);

/// The access trace an event stream records: what the workload asked
/// for, not what the manager did about it.
pub fn trace(events: &str) -> io::Result<String> {
    let mut into = vec![Access::EMPTY; events.lines().count()];
    let filled = mlos_trace::from_events(events, &mut into)
        .map_err(|why| io::Error::other(format!("cannot read the event stream: {why:?}")))?;

    let mut text = String::new();
    let header = Header {
        model: mlos_synth::MODEL,
        source: EVENTS,
    };
    mlos_trace::render(&mut text, header, &into[..filled]).map_err(io::Error::other)?;
    Ok(text)
}

/// Writes `text` to `path`, making its directory if need be.
pub fn save(path: &str, text: &str) -> io::Result<PathBuf> {
    let out = PathBuf::from(path);
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&out, text)?;
    Ok(out)
}

/// The event lines in `console`, with the marker stripped: plain JSON
/// Lines.
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
    // The whole console goes in the error: the reason is usually in it.
    let missing = |what: &str| io::Error::other(format!("the guest {what}. Said:\n{console}"));
    let lines: Vec<&str> = console.lines().map(str::trim_end).collect();
    let open = lines
        .iter()
        .position(|line| *line == "{")
        .ok_or_else(|| missing("never printed a layout"))?;
    let close = lines[open..]
        .iter()
        .position(|line| *line == "}")
        .ok_or_else(|| missing("printed a layout that never ended"))?;
    Ok(lines[open..=open + close].join("\n") + "\n")
}
