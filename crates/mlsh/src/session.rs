//! `session`: create, list and end sessions by hand.
//!
//! Invariant: a session is created only through admission, and a refusal
//! is printed with its reason; ending a session evicts what it owned
//! before the record goes, and says how much went; a squeeze walks the
//! ladder and says how deep it went. Design: docs/notes/mlsh.md.

use core::fmt::Write;

use mlos_objman::Contract;
use mlos_objtab::{Precision, SessionId};

/// The words after `session new`: resident ceiling in KiB, latency
/// ceiling in acquires, quality floor, context in tokens.
type Words<'a> = [Option<&'a str>; 4];

/// `session` lists; `session new [KIB] [WAIT] [FLOOR] [CTX]` asks
/// admission for one; `session end ID` destroys one; `session squeeze
/// KIB` shrinks the budget and walks the ladder.
pub fn session(out: &mut impl Write, args: &str) {
    let mut words = args.split_whitespace();
    let verb = words.next();
    let mut rest: Words<'_> = [None; 4];
    rest.iter_mut()
        .zip(words)
        .for_each(|(slot, word)| *slot = Some(word));
    match verb {
        None => list(out),
        Some("new") => new(out, &rest),
        Some("end") => end(out, rest[0].and_then(|w| w.parse().ok())),
        Some("squeeze") => squeeze(out, rest[0].and_then(|w| w.parse().ok())),
        Some(_) => drop(writeln!(
            out,
            "  usage: session | session new [KIB] [WAIT] [FLOOR] [CTX] | session end ID | session squeeze KIB"
        )),
    }
}

/// Asks admission for a session: a resident ceiling in KiB, a latency
/// ceiling in acquires, the coarsest precision it will accept (fp16,
/// bf16, q8, q4, q3, ternary) and the tokens of context it will hold,
/// which it arrives with already cached.
fn new(out: &mut impl Write, words: &Words<'_>) {
    let number = |at: usize| words[at].and_then(|w| w.parse::<u64>().ok());
    let clamp = |n: u64| u32::try_from(n).unwrap_or(u32::MAX);
    let contract = Contract {
        resident_ceiling: number(0).map_or(0, |kib| kib << 10),
        latency_ceiling: number(1).map_or(0, clamp),
        quality_floor: match words[2] {
            Some("fp16") => Precision::Fp16,
            Some("bf16") => Precision::Bf16,
            Some("q8") => Precision::Q8,
            Some("q4") => Precision::Q4,
            Some("q3") => Precision::Q3,
            _ => Precision::Ternary,
        },
        context: number(3).map_or(0, clamp),
    };
    let template = (mlos_lab::LAYERS, mlos_synth::kv::meta(0));
    let arrived = |held: &mut _| mlos_ladder::arrive(held, &mlos_lab::LADDER, contract, template);
    let _ = match mlos_lab::with(arrived) {
        Some(Ok((id, _))) => writeln!(out, "  session {} created", id.0),
        Some(Err(why)) => writeln!(out, "  refused: {why}"),
        None => Ok(()),
    };
}

/// Every live session, then how many like them the budget admits: none
/// once a squeeze has left no room for another at full context.
fn list(out: &mut impl Write) {
    let counted = mlos_lab::with(|held| {
        held.sessions.each().for_each(|s| row(out, s));
        (held.sessions.live(), held.capacity.admits(&held.sessions))
    });
    let _ = match counted {
        Some((0, _)) => writeln!(out, "  no sessions (try `session new`)"),
        Some((_, n)) => writeln!(out, "  budget admits {n} session(s) like these"),
        None => Ok(()),
    };
}

/// One session: what it was promised, and what it got.
fn row(out: &mut impl Write, s: &mlos_objman::Session) {
    let (c, d) = (s.contract, s.delivered);
    let _ = writeln!(
        out,
        "  session {:<3} on {} promised floor {:?}, wait {}, resident {} KiB, context {} tok; delivered {:?}, worst period {}, peak {} B, read {} B, holds {} B",
        s.id.0,
        s.rung,
        c.quality_floor,
        c.latency_ceiling,
        c.resident_ceiling >> 10,
        c.context,
        d.coarsest,
        d.worst_period,
        d.peak_resident,
        d.recomputed,
        s.resident
    );
}

/// Ends one session and says what it took with it.
fn end(out: &mut impl Write, id: Option<u16>) {
    let Some(id) = id else {
        let _ = writeln!(out, "  usage: session end ID");
        return;
    };
    let _ = match mlos_lab::with(|held| held.destroy_session(SessionId(id))) {
        Some(Ok(evicted)) => writeln!(out, "  session {id} ended, {evicted} object(s) evicted"),
        Some(Err(why)) => writeln!(out, "  not ended: {why:?}"),
        None => Ok(()),
    };
}

/// Shrinks the budget to `kib` and walks the ladder until the sessions
/// fit, then says how deep it went and what it cost.
fn squeeze(out: &mut impl Write, kib: Option<u64>) {
    let Some(kib) = kib else {
        let _ = writeln!(out, "  usage: session squeeze KIB");
        return;
    };
    let walked = |held: &mut _| mlos_ladder::squeeze(held, &mlos_lab::LADDER, kib << 10);
    if let Some(done) = mlos_lab::with(walked) {
        let _ = writeln!(
            out,
            "  squeezed to {kib} KiB: reached {}, read {} B, {} terminated, {}",
            done.reached,
            done.recomputed,
            done.terminated,
            if done.fits { "fits" } else { "still over" }
        );
    }
}
