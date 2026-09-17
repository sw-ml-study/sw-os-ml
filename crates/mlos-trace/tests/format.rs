//! Record and replay are exact inverses, and the parser is strict.
//!
//! The first property is the one the milestone rests on: every policy is
//! scored by replaying one trace, and a trace that came back from disk
//! differently from how it went in would score four policies on four
//! slightly different workloads without anyone noticing.

use mlos_abi::{Fields, ObjectClass, ObjectId};
use mlos_objtab::SessionId;
use mlos_trace::{Access, Error, Header, parse, render};

/// A short trace with more than one session in it.
fn accesses() -> Vec<Access> {
    (0..8u16)
        .map(|at| Access {
            session: SessionId(1 + at % 3),
            object: ObjectId::new(
                ObjectClass::WeightTile,
                Fields {
                    model: 1,
                    layer: at / 4,
                    tensor: at % 4,
                    tile: 0,
                },
            ),
        })
        .collect()
}

/// The header those accesses would carry.
fn header() -> Header<'static> {
    Header {
        model: "synth-8x16",
        source: "a test",
    }
}

#[test]
fn a_trace_survives_the_round_trip_exactly() {
    let written = accesses();
    let mut text = String::new();
    render(&mut text, header(), &written).expect("a String accepts writes");

    let mut read = vec![
        Access {
            session: SessionId(0),
            object: ObjectId(0),
        };
        written.len()
    ];
    let trace = parse(&text, &mut read).expect("what we just wrote parses");

    assert_eq!(trace.accesses, written.as_slice());
    assert_eq!(trace.header, header());
}

#[test]
fn the_model_comes_back_with_it() {
    // A trace replayed against the wrong model gives sizes for objects
    // that are not those objects, and every budget decision after that is
    // about nothing. The name is in the file so the mismatch is catchable.
    let mut text = String::new();
    render(&mut text, header(), &accesses()).expect("writes");
    assert!(text.contains("model synth-8x16"), "{text}");
}

#[test]
fn an_empty_trace_is_a_trace() {
    let mut text = String::new();
    render(&mut text, header(), &[]).expect("writes");
    let trace = parse(&text, &mut []).expect("no accesses is a valid trace");
    assert!(trace.accesses.is_empty());
    assert_eq!(trace.header.model, "synth-8x16");
}

#[test]
fn a_file_that_is_not_a_trace_is_refused() {
    assert_eq!(parse("", &mut []), Err(Error::NotATrace));
    assert_eq!(parse("{\"json\": true}\n", &mut []), Err(Error::NotATrace));
    // A future version is refused rather than read as this one.
    assert_eq!(parse("mlos-trace 99\n", &mut []), Err(Error::NotATrace));
}

#[test]
fn a_truncated_trace_is_refused_rather_than_shortened() {
    // The failure this prevents: a trace cut short parses as a shorter
    // workload, every policy is scored on it, and the numbers are about a
    // run that never happened.
    let mut text = String::new();
    render(&mut text, header(), &accesses()).expect("writes");
    let cut: String = text.lines().take(6).collect::<Vec<_>>().join("\n");
    let mut room = vec![
        Access {
            session: SessionId(0),
            object: ObjectId(0)
        };
        8
    ];
    assert_eq!(parse(&cut, &mut room), Err(Error::TooFew));
}

#[test]
fn a_trace_bigger_than_the_buffer_is_refused() {
    let mut text = String::new();
    render(&mut text, header(), &accesses()).expect("writes");
    let mut room = vec![
        Access {
            session: SessionId(0),
            object: ObjectId(0)
        };
        2
    ];
    assert_eq!(parse(&text, &mut room), Err(Error::TooMany));
}

#[test]
fn a_malformed_access_line_is_refused() {
    let text = "mlos-trace 1\nmodel m\nsource s\naccesses 1\nnot an access\n";
    let mut room = vec![Access {
        session: SessionId(0),
        object: ObjectId(0),
    }];
    assert_eq!(parse(text, &mut room), Err(Error::BadAccess));
}

#[test]
fn comments_are_allowed_and_ignored() {
    // So a trace can say where it came from in prose without a consumer
    // having to understand the prose.
    let text =
        "# taken on a Tuesday\nmlos-trace 1\nmodel m\nsource s\naccesses 1\n1 0100000000000000\n";
    let mut room = vec![Access {
        session: SessionId(0),
        object: ObjectId(0),
    }];
    let trace = parse(text, &mut room).expect("comments are skipped");
    assert_eq!(trace.accesses.len(), 1);
    assert_eq!(trace.accesses[0].session, SessionId(1));
}
