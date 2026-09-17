//! Building a trace from a recorded event stream.
//!
//! The first trace must not be a literal. `docs/architecture.md` s.12
//! names hand-written traces as a risk, and the reason is that a trace
//! written by the person hoping for a result is a trace that can encode
//! the result without anyone meaning to. A recording cannot.
//!
//! An event stream is exactly a recording of acquires. Every `placed`,
//! every `hit` and every `refused` is one acquire that really happened,
//! on a real kernel reading a real device, in the order it happened.
//!
//! `refused` counts, and that is the interesting case. The workload asked;
//! a policy declined. What the workload asked for is a property of the
//! workload, and under a different policy that same acquire might have
//! succeeded -- which is the whole reason a trace must not record what the
//! system did.
//!
//! `dropped` does not count: it is the ring's own bookkeeping. A stream
//! that dropped anything is a stream with a hole in it, and a trace built
//! from one would be missing accesses with no way to tell. Refuse it.

use mlos_abi::ObjectId;
use mlos_objtab::SessionId;

use crate::{Access, read::Error};

/// The event kinds that are acquires.
const ACQUIRES: [&str; 3] = ["\"placed\"", "\"hit\"", "\"refused\""];

/// Fills `into` with the accesses in an event stream, returning how many.
///
/// Errors rather than truncating if the stream lost events: a trace with
/// an invisible hole in it would produce a plausible wrong number, which
/// is worse than producing none.
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
        // A stream that lost events is missing acquires, and nothing
        // downstream could tell. Better to refuse than to measure a
        // workload that is quietly not the one that ran.
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

/// One numeric field of a JSON line, quoted or not.
///
/// `key` carries its own quotes and colon -- `"\"bytes\":"` -- because
/// building that string here would need an allocator, and this crate does
/// not have one.
fn field(line: &str, key: &str) -> Result<u64, Error> {
    let rest = line.split(key).nth(1).ok_or(Error::BadAccess)?;
    let value = rest.trim_start_matches([':', ' ', '"']);
    let value = value
        .split(['"', ',', '}'])
        .next()
        .ok_or(Error::BadAccess)?;
    value.trim().parse().map_err(|_| Error::BadAccess)
}
