//! One event per line, self-delimited.
//!
//! JSON Lines behind a `@ev ` marker. The marker is what lets a consumer
//! pull the stream out of a console it shares with the shell's prose
//! without parsing the prose -- `grep '^@ev '` is the whole extractor.
//! It also keeps the stream distinct from a layout document, which is the
//! only other JSON on that console and begins a line with a brace.
//!
//! Self-delimited per line, deliberately: a capture that was cut short
//! mid-run still yields every complete line before the cut. A single JSON
//! array would yield nothing at all without its closing bracket, and a
//! truncated capture is the normal case -- `mlos run --capture` kills the
//! guest after a fixed number of seconds.
//!
//! Every key is present on every line, including the ones that do not
//! apply. A consumer reading columns does not have to handle a missing
//! field, and the cost is a few bytes on a stream that is already bounded
//! by how much a console can carry.

use core::fmt::Write;

use mlos_spaces::{Where, tier};

use crate::Ring;

/// The marker that starts every event line.
pub const MARKER: &str = "@ev";

/// The shell's `trace` verb: print the stream, or switch recording.
///
/// `trace` prints what has been recorded; `trace on` and `trace off`
/// control whether the fault path records at all. The switch exists so
/// the cost of recording can be MEASURED -- sweep with it off, sweep with
/// it on, compare -- rather than asserted to be small.
pub fn verb(out: &mut impl Write, ring: &mut Ring, args: &str) {
    match args.split_whitespace().next() {
        Some("on") => ring.enabled = true,
        Some("off") => ring.enabled = false,
        _ => write(out, ring),
    }
}

/// Writes every event in `ring`, then says how many were lost.
pub fn write(out: &mut impl Write, ring: &Ring) {
    for event in ring.events() {
        let region = mlos_spaces::object(Where::Dram, event.object).unwrap_or_default();
        let _ = write!(
            out,
            "{MARKER} {{\"seq\":{},\"event\":\"{}\",\"region\":{region},\"object\":\"{}\",\
             \"session\":{},\"offset\":{},\"bytes\":{},\"cost\":{},\"tier\":\"{}\",\
             \"why\":\"{}\"}}\r\n",
            event.seq,
            event.kind.name(),
            event.object.0,
            event.session.0,
            event.offset,
            event.bytes,
            event.cost,
            tier(event.tier),
            why(event.why),
        );
    }
    lost(out, ring.dropped());
}

/// How many events were overwritten before anything read them.
///
/// An event like any other so a consumer reading line by line does not
/// need a second shape for it, and never omitted when zero: "no dropped
/// line" and "a dropped line saying zero" are the same fact only if you
/// already trust the emitter.
fn lost(out: &mut impl Write, dropped: u32) {
    let _ = write!(
        out,
        "{MARKER} {{\"seq\":0,\"event\":\"dropped\",\"region\":0,\"object\":\"0\",\
         \"session\":0,\"offset\":0,\"bytes\":{dropped},\"cost\":0,\"tier\":\"\",\
         \"why\":\"\"}}\r\n"
    );
}

/// Why an acquire was refused, as a word.
///
/// Exhaustive with no wildcard arm, so a new error code is a compile error
/// here rather than an event that says nothing about what went wrong.
const fn why(error: Option<mlos_abi::Error>) -> &'static str {
    let Some(error) = error else {
        return "";
    };
    match error {
        mlos_abi::Error::BadObject => "bad-object",
        mlos_abi::Error::BadClass => "bad-class",
        mlos_abi::Error::NotResident => "not-resident",
        mlos_abi::Error::NoBudget => "no-budget",
        mlos_abi::Error::Refused => "refused",
        mlos_abi::Error::NoProvider => "no-provider",
        mlos_abi::Error::Revoked => "revoked",
        mlos_abi::Error::Denied => "denied",
    }
}
