//! One event per line: JSON Lines behind a `@ev ` marker.
//!
//! Contract: every key is present on every line, including the ones that
//! do not apply, and the stream ends with a `dropped` line even when it
//! says zero. Design: docs/notes/mlos-events.md.

use core::fmt::Write;

use mlos_spaces::{Where, tier};

use crate::Ring;

/// The marker that starts every event line.
pub const MARKER: &str = "@ev";

/// The shell's `trace` verb: `trace` prints what has been recorded,
/// `trace on` and `trace off` switch recording.
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

/// How many events were overwritten before anything read them, shaped
/// like any other event and written even when zero.
fn lost(out: &mut impl Write, dropped: u32) {
    let _ = write!(
        out,
        "{MARKER} {{\"seq\":0,\"event\":\"dropped\",\"region\":0,\"object\":\"0\",\
         \"session\":0,\"offset\":0,\"bytes\":{dropped},\"cost\":0,\"tier\":\"\",\
         \"why\":\"\"}}\r\n"
    );
}

/// Why an acquire was refused, as a word. No wildcard arm: a new error
/// code must be named here.
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
