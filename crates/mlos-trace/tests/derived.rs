//! A trace built from a recording, not from a literal.
//!
//! `docs/architecture.md` s.12 names hand-written traces as a risk, and
//! the reason is specific: a trace written by whoever is hoping for a
//! result can encode the result without anyone meaning to. An event
//! stream cannot -- it is what a real kernel really did, in the order it
//! did it.

use mlos_abi::ObjectId;
use mlos_objtab::SessionId;
use mlos_trace::{Access, Error, from_events};

/// Somewhere to parse into.
fn room(n: usize) -> Vec<Access> {
    vec![
        Access {
            session: SessionId(0),
            object: ObjectId(0)
        };
        n
    ]
}

/// One event line, as `mlos-events` writes it once the marker is gone.
fn event(seq: u32, kind: &str, object: u64, session: u16) -> String {
    format!(
        "{{\"seq\":{seq},\"event\":\"{kind}\",\"region\":1,\"object\":\"{object}\",\
         \"session\":{session},\"offset\":0,\"bytes\":1024,\"cost\":3001024,\
         \"tier\":\"cold\",\"why\":\"\"}}"
    )
}

/// The ring's own bookkeeping line.
fn dropped(count: u32) -> String {
    format!(
        "{{\"seq\":0,\"event\":\"dropped\",\"region\":0,\"object\":\"0\",\"session\":0,\
         \"offset\":0,\"bytes\":{count},\"cost\":0,\"tier\":\"\",\"why\":\"\"}}"
    )
}

#[test]
fn every_kind_of_acquire_becomes_an_access() {
    // All three are things the workload ASKED for. What the policy did
    // about each is not a property of the workload, which is exactly why
    // a refusal has to be in the trace: under another policy it might
    // have succeeded, and the replay has to be able to find that out.
    let stream = [
        event(1, "placed", 100, 1),
        event(2, "hit", 100, 1),
        event(3, "refused", 200, 2),
        dropped(0),
    ]
    .join("\n");

    let mut into = room(8);
    let filled = from_events(&stream, &mut into).expect("a clean stream");
    assert_eq!(filled, 3);
    assert_eq!(into[0].object, ObjectId(100));
    assert_eq!(into[2].object, ObjectId(200));
}

#[test]
fn the_session_is_read_rather_than_assumed() {
    // There is one session today, so assuming 1 would be right -- and
    // would stop being right at M4, silently, in a file being measured
    // from. The event stream carries it for that reason.
    let stream = [event(1, "placed", 100, 7), dropped(0)].join("\n");
    let mut into = room(4);
    from_events(&stream, &mut into).expect("a clean stream");
    assert_eq!(into[0].session, SessionId(7));
}

#[test]
fn order_is_preserved() {
    let stream: Vec<String> = (1..=5)
        .map(|at| event(at, "placed", u64::from(at) * 10, 1))
        .chain([dropped(0)])
        .collect();
    let mut into = room(8);
    let filled = from_events(&stream.join("\n"), &mut into).expect("a clean stream");
    let ids: Vec<u64> = into[..filled].iter().map(|a| a.object.0).collect();
    assert_eq!(ids, [10, 20, 30, 40, 50]);
}

#[test]
fn a_stream_that_lost_events_is_refused() {
    // The failure this prevents is the quiet one: a ring that overflowed
    // is missing acquires, a trace built from it is a different workload,
    // and nothing downstream could tell. Refusing is the only honest
    // answer -- the lost accesses cannot be recovered.
    let stream = [event(1, "placed", 100, 1), dropped(12)].join("\n");
    let mut into = room(8);
    assert_eq!(from_events(&stream, &mut into), Err(Error::TooFew));
}

#[test]
fn a_stream_bigger_than_the_buffer_is_refused() {
    let stream: Vec<String> = (1..=10).map(|at| event(at, "hit", 1, 1)).collect();
    let mut into = room(4);
    assert_eq!(
        from_events(&stream.join("\n"), &mut into),
        Err(Error::TooMany)
    );
}

#[test]
fn the_committed_sample_yields_the_run_that_produced_it() {
    // The real thing: the event stream MLOS actually emitted, turned into
    // the trace of the sweep that produced it.
    let stream = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/viz/runtime-events.jsonl"
    ))
    .expect("the committed sample");

    let mut into = room(512);
    let filled = from_events(&stream, &mut into).expect("the sample lost nothing");
    // Two sweeps of a model that fits 32 tiles: 32 placed, 32 hits, and a
    // refusal at the end of each. The figures are the run's, not a choice.
    assert_eq!(
        filled, 66,
        "the sample has 67 lines, one of them the dropped count"
    );
    assert!(into[..filled].iter().all(|a| a.session == SessionId(1)));
    // A sweep walks tiles in order, so the first access is layer 0 tile 0.
    assert_eq!(into[0].object.fields().layer, 0);
    assert_eq!(into[0].object.fields().tensor, 0);
}
