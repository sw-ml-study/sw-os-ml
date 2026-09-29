//! Building a trace from a recorded event stream.
//!
//! Invariant: every `placed`, `hit` and `refused` is one acquire; a
//! stream whose `dropped` count is non-zero has a hole in it and is
//! refused. Design: docs/notes/mlos-trace.md.

use mlos_abi::ObjectId;
use mlos_objtab::SessionId;

use crate::{Access, read::Error};

/// The event kinds that are acquires.
const ACQUIRES: [&str; 3] = ["\"placed\"", "\"hit\"", "\"refused\""];

/// Fills `into` with the accesses in an event stream, returning how many.
/// `TooFew` rather than a truncated trace if the stream lost events.
pub fn from_events(events: &str, into: &mut [Access]) -> Result<usize, Error> {
    let mut filled = 0;
    for line in events.lines() {
        if dropped(line)? {
            continue;
        }
        if !ACQUIRES.iter().any(|kind| line.contains(kind)) {
            continue;
        }
        *into.get_mut(filled).ok_or(Error::TooMany)? = access(line)?;
        filled += 1;
    }
    Ok(filled)
}

/// Whether this line is the ring's dropped count, and whether it is zero.
fn dropped(line: &str) -> Result<bool, Error> {
    if !line.contains("\"dropped\"") {
        return Ok(false);
    }
    match field(line, "\"bytes\":")? {
        0 => Ok(true),
        _ => Err(Error::TooFew),
    }
}

/// One access from an event line.
fn access(line: &str) -> Result<Access, Error> {
    Ok(Access {
        session: SessionId(
            u16::try_from(field(line, "\"session\":")?).map_err(|_| Error::BadAccess)?,
        ),
        object: ObjectId(field(line, "\"object\":")?),
    })
}

/// One numeric field of a JSON line, quoted or not. `key` carries its
/// own quotes and colon, `"\"bytes\":"`; there is no allocator to add them.
fn field(line: &str, key: &str) -> Result<u64, Error> {
    let rest = line.split(key).nth(1).ok_or(Error::BadAccess)?;
    let value = rest.trim_start_matches([':', ' ', '"']);
    let value = value
        .split(['"', ',', '}'])
        .next()
        .ok_or(Error::BadAccess)?;
    value.trim().parse().map_err(|_| Error::BadAccess)
}
