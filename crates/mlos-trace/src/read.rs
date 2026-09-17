//! Parsing a trace back.
//!
//! Into a caller-provided slice, never into a `Vec`: the kernel replays
//! the same traces at step 008 and has no allocator. The header carries
//! the count so a caller can size that slice before it starts.
//!
//! Strict about everything it can be strict about. A trace is an input to
//! a measurement, and a parser that quietly skipped a line it did not
//! understand would produce a shorter trace and a plausible wrong number
//! rather than an error.

use mlos_abi::ObjectId;
use mlos_objtab::SessionId;

use crate::{Access, Header, MAGIC, Trace, VERSION};

/// Why a trace would not parse.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Error {
    /// Not a trace file, or a version this build does not know.
    NotATrace,
    /// A header line is missing or out of order.
    BadHeader,
    /// A line is not `<session> <objectid-hex>`.
    BadAccess,
    /// The caller's slice is smaller than the header's count.
    TooMany,
    /// The header promised more accesses than the file holds.
    TooFew,
}

/// Parses `text`, filling `into` and borrowing the header from `text`.
pub fn parse<'a>(text: &'a str, into: &'a mut [Access]) -> Result<Trace<'a>, Error> {
    let mut lines = text.lines().filter(|line| !line.starts_with('#'));
    let (model, source, count) = head(&mut lines)?;
    if count > into.len() {
        return Err(Error::TooMany);
    }
    let mut filled = 0;
    for line in lines.take(count) {
        *into.get_mut(filled).ok_or(Error::TooMany)? = access(line)?;
        filled += 1;
    }
    if filled != count {
        return Err(Error::TooFew);
    }
    let header = Header { model, source };
    Ok(Trace {
        header,
        accesses: &into[..filled],
    })
}

/// The four header lines, in order.
fn head<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<(&'a str, &'a str, usize), Error> {
    let magic = lines.next().ok_or(Error::NotATrace)?;
    let version = magic.strip_prefix(MAGIC).ok_or(Error::NotATrace)?.trim();
    if version.parse() != Ok(VERSION) {
        return Err(Error::NotATrace);
    }
    let model = field(lines.next(), "model")?;
    let source = field(lines.next(), "source")?;
    let count = field(lines.next(), "accesses")?
        .parse()
        .map_err(|_| Error::BadHeader)?;
    Ok((model, source, count))
}

/// One `<key> <value>` header line.
fn field<'a>(line: Option<&'a str>, key: &str) -> Result<&'a str, Error> {
    line.and_then(|line| line.strip_prefix(key))
        .map(str::trim)
        .ok_or(Error::BadHeader)
}

/// One `<session> <objectid-hex>` line.
fn access(line: &str) -> Result<Access, Error> {
    let (session, object) = line.trim().split_once(' ').ok_or(Error::BadAccess)?;
    Ok(Access {
        session: SessionId(session.parse().map_err(|_| Error::BadAccess)?),
        object: ObjectId(u64::from_str_radix(object.trim(), 16).map_err(|_| Error::BadAccess)?),
    })
}
