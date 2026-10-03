//! `session`: create, list and end sessions by hand.
//!
//! Invariant: ending a session evicts what it owned before the record
//! goes, and says how much went. Design: docs/notes/mlsh.md.

use core::fmt::Write;

use mlos_objman::Contract;
use mlos_objtab::{Precision, SessionId};

/// `session` lists; `session new [KIB] [WAIT]` creates one with an
/// optional resident ceiling in KiB and latency ceiling in acquires;
/// `session end ID` destroys one.
pub fn session(out: &mut impl Write, args: &str) {
    let mut words = args.split_whitespace();
    let verb = words.next();
    let mut rest: [Option<&str>; 3] = [None; 3];
    rest.iter_mut()
        .zip(words)
        .for_each(|(slot, word)| *slot = Some(word));
    let number = |at: usize| rest[at].and_then(|w| w.parse::<u64>().ok());
    match verb {
        None => list(out),
        Some("new") => new(out, (number(0), number(1), rest[2])),
        Some("end") => end(out, number(0)),
        Some(_) => drop(writeln!(
            out,
            "  usage: session | session new [KIB] [WAIT] [FLOOR] | session end ID"
        )),
    }
}

/// Creates a session: `new [KIB] [WAIT] [FLOOR]`, a resident ceiling in
/// KiB, a latency ceiling in acquires, and the coarsest precision it will
/// accept (fp16, bf16, q8, q4, q3, ternary).
fn new(out: &mut impl Write, (kib, wait, floor): (Option<u64>, Option<u64>, Option<&str>)) {
    let floor = match floor {
        Some("fp16") => Precision::Fp16,
        Some("bf16") => Precision::Bf16,
        Some("q8") => Precision::Q8,
        Some("q4") => Precision::Q4,
        Some("q3") => Precision::Q3,
        _ => Precision::Ternary,
    };
    let contract = Contract {
        resident_ceiling: kib.map_or(0, |kib| kib << 10),
        latency_ceiling: wait.map_or(0, |w| u32::try_from(w).unwrap_or(u32::MAX)),
        quality_floor: floor,
    };
    let _ = match mlos_lab::with(|held| held.sessions.create(contract)) {
        Some(Ok(id)) => writeln!(out, "  session {} created", id.0),
        Some(Err(why)) => writeln!(out, "  refused: {why:?}"),
        None => Ok(()),
    };
}

/// Every live session: what it was promised, and what it got.
fn list(out: &mut impl Write) {
    let shown = mlos_lab::with(|held| {
        let mut shown = 0;
        for s in held.sessions.each() {
            let (c, d) = (s.contract, s.delivered);
            let _ = writeln!(
                out,
                "  session {:<3} promised floor {:?}, wait {}, resident {} KiB; delivered {:?}, worst period {}, peak {} B, holds {} B",
                s.id.0,
                c.quality_floor,
                c.latency_ceiling,
                c.resident_ceiling >> 10,
                d.coarsest,
                d.worst_period,
                d.peak_resident,
                s.resident
            );
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
