//! `session`: create, list and end sessions by hand.
//!
//! Invariant: ending a session evicts what it owned before the record
//! goes, and says how much went. Design: docs/notes/mlsh.md.

use core::fmt::Write;

use mlos_objman::Contract;
use mlos_objtab::SessionId;
use mlos_session::MAX_SESSIONS;

/// `session` lists; `session new [KIB] [WAIT]` creates one with an
/// optional resident ceiling in KiB and latency ceiling in acquires;
/// `session end ID` destroys one.
pub fn session(out: &mut impl Write, args: &str) {
    let mut words = args.split_whitespace();
    let verb = words.next();
    let mut numbers = words.filter_map(|w| w.parse::<u64>().ok());
    let (first, second) = (numbers.next(), numbers.next());
    match verb {
        None => list(out),
        Some("new") => new(out, first, second),
        Some("end") => end(out, first),
        Some(_) => drop(writeln!(
            out,
            "  usage: session | session new [KIB] [WAIT] | session end ID"
        )),
    }
}

/// Creates a session with an optional resident ceiling (KiB) and latency
/// ceiling (acquires).
fn new(out: &mut impl Write, kib: Option<u64>, wait: Option<u64>) {
    let contract = Contract {
        resident_ceiling: kib.map_or(0, |kib| kib << 10),
        latency_ceiling: wait.map_or(0, |wait| u32::try_from(wait).unwrap_or(u32::MAX)),
        ..Contract::NONE
    };
    let _ = match mlos_lab::with(|held| held.sessions.create(contract)) {
        Some(Ok(id)) => writeln!(out, "  session {} created", id.0),
        Some(Err(why)) => writeln!(out, "  refused: {why:?}"),
        None => Ok(()),
    };
}

/// Every live session: id, what it holds, and its ceiling.
fn list(out: &mut impl Write) {
    let shown = mlos_lab::with(|held| {
        let mut shown = 0;
        for slot in 0..MAX_SESSIONS {
            let Some(s) = held.sessions.at(slot) else {
                continue;
            };
            let _ = write!(
                out,
                "  session {:<3} resident {:>7} B  ceiling ",
                s.id.0, s.resident
            );
            let _ = match s.contract.resident_ceiling {
                0 => writeln!(out, "none"),
                bytes => writeln!(out, "{} KiB", bytes >> 10),
            };
            shown += 1;
        }
        shown
    });
    if shown == Some(0) {
        let _ = writeln!(out, "  no sessions (try `session new`)");
    }
}

/// Ends one session and says what it took with it.
fn end(out: &mut impl Write, id: Option<u64>) {
    let Some(id) = id.and_then(|n| u16::try_from(n).ok()) else {
        let _ = writeln!(out, "  usage: session end ID");
        return;
    };
    let _ = match mlos_lab::with(|held| held.destroy_session(SessionId(id))) {
        Some(Ok(evicted)) => writeln!(out, "  session {id} ended, {evicted} object(s) evicted"),
        Some(Err(why)) => writeln!(out, "  not ended: {why:?}"),
        None => Ok(()),
    };
}
