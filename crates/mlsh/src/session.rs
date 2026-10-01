//! `session`: create, list and end sessions by hand.
//!
//! Invariant: ending a session evicts what it owned before the record
//! goes, and says how much went. Design: docs/notes/mlsh.md.

use core::fmt::Write;

use mlos_objman::Contract;
use mlos_objtab::SessionId;
use mlos_session::MAX_SESSIONS;

/// `session` lists; `session new [KIB]` creates one with an optional
/// resident ceiling; `session end ID` destroys one.
pub fn session(out: &mut impl Write, args: &str) {
    let mut words = args.split_whitespace();
    let verb = words.next();
    let number = words.next().and_then(|w| w.parse::<u64>().ok());
    match verb {
        None => list(out),
        Some("new") => {
            let contract = Contract {
                resident_ceiling: number.map_or(0, |kib| kib << 10),
                ..Contract::NONE
            };
            let _ = match mlos_lab::with(|held| held.sessions.create(contract)) {
                Some(Ok(id)) => writeln!(out, "  session {} created", id.0),
                Some(Err(why)) => writeln!(out, "  refused: {why:?}"),
                None => Ok(()),
            };
        }
        Some("end") => end(out, number),
        Some(_) => drop(writeln!(
            out,
            "  usage: session | session new [KIB] | session end ID"
        )),
    }
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
