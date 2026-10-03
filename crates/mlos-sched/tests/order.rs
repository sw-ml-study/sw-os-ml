//! The two schedules, on streams small enough to write the answer down.

use mlos_abi::{Fields, ObjectClass, ObjectId};
use mlos_objtab::SessionId;
use mlos_sched::{Lane, ParameterMajor, ProcessMajor, Schedule, merge, merge_timed};

fn w(n: u16) -> ObjectId {
    ObjectId::new(
        ObjectClass::WeightTile,
        Fields {
            model: 1,
            layer: 0,
            tensor: n,
            tile: 0,
        },
    )
}

fn k(session: u16, n: u16) -> ObjectId {
    ObjectId::new(
        ObjectClass::KvBlock,
        Fields {
            model: session,
            layer: 0,
            tensor: n,
            tile: 0,
        },
    )
}

fn lane(session: u16, ceiling: u32, tokens: Vec<Vec<ObjectId>>) -> Lane {
    Lane {
        session: SessionId(session),
        ceiling,
        tokens,
    }
}

fn order(schedule: &mut dyn Schedule, lanes: &[Lane]) -> Vec<(u16, ObjectId)> {
    merge(schedule, lanes)
        .into_iter()
        .map(|a| (a.session.0, a.object))
        .collect()
}

#[test]
fn process_major_serves_whole_tokens_round_robin_and_skips_the_finished() {
    let a = lane(1, 0, vec![vec![w(1), w(2)], vec![w(1), w(2)]]);
    let b = lane(2, 0, vec![vec![w(1), w(2), k(2, 0)]]);
    let got = order(&mut ProcessMajor::default(), &[a, b]);
    let want = vec![
        (1, w(1)),
        (1, w(2)),
        (2, w(1)),
        (2, w(2)),
        (2, k(2, 0)),
        (1, w(1)),
        (1, w(2)),
    ];
    assert_eq!(got, want);
}

#[test]
fn parameter_major_reads_each_weight_once_per_token_for_everyone() {
    // Two sessions on one token; B has a longer cache phase than A.
    let a = lane(1, 0, vec![vec![w(1), w(2), k(1, 0), w(3)]]);
    let b = lane(2, 0, vec![vec![w(1), w(2), k(2, 0), k(2, 1), w(3)]]);
    let got = order(&mut ParameterMajor, &[a, b]);
    let want = vec![
        (1, w(1)),
        (2, w(1)),
        (1, w(2)),
        (2, w(2)),
        (1, k(1, 0)),
        (2, k(2, 0)),
        (1, w(3)),
        (2, k(2, 1)),
        (2, w(3)),
    ];
    assert_eq!(got, want);
}

#[test]
fn parameter_major_holds_everyone_at_the_token_boundary() {
    // A's tokens are short; it must not run a token ahead of B.
    let a = lane(1, 0, vec![vec![w(1)], vec![w(1)]]);
    let b = lane(2, 0, vec![vec![w(1), k(2, 0), k(2, 1)], vec![w(1)]]);
    let got = order(&mut ParameterMajor, &[a, b]);
    let want = vec![
        (1, w(1)),
        (2, w(1)),
        (2, k(2, 0)),
        (2, k(2, 1)),
        (1, w(1)),
        (2, w(1)),
    ];
    assert_eq!(got, want);
}

#[test]
fn a_session_past_its_ceiling_leaves_the_lockstep() {
    // A will wait at most two of B's acquires per token. A's first token
    // ends at clock 1; B then takes two acquires, A is overdue and starts
    // its next token alone, before B has finished the first.
    let a = lane(1, 2, vec![vec![w(1)], vec![w(1)]]);
    let b = lane(
        2,
        0,
        vec![vec![w(1), k(2, 0), k(2, 1), k(2, 2)], vec![w(1)]],
    );
    let got = order(&mut ParameterMajor, &[a, b]);
    let want = vec![
        (1, w(1)),
        (2, w(1)),
        (2, k(2, 0)),
        (1, w(1)),
        (2, k(2, 1)),
        (2, k(2, 2)),
        (2, w(1)),
    ];
    assert_eq!(got, want);
}

#[test]
fn both_schedules_serve_every_access_exactly_once() {
    let a = lane(
        1,
        0,
        vec![vec![w(1), w(2)], vec![w(1), k(1, 0)], vec![w(1)]],
    );
    let b = lane(2, 0, vec![vec![w(1), k(2, 0), k(2, 1)]]);
    let total = 5 + 3;
    let mut process = ProcessMajor::default();
    let mut parameter = ParameterMajor;
    for (name, schedule) in [
        ("process", &mut process as &mut dyn Schedule),
        ("parameter", &mut parameter),
    ] {
        let got = order(schedule, &[a.clone(), b.clone()]);
        assert_eq!(got.len(), total, "{name}");
        assert_eq!(got.iter().filter(|(s, _)| *s == 1).count(), 5, "{name}");
    }
}

#[test]
fn the_worst_period_is_measured_end_to_end_per_token() {
    // B's second token waits for A's whole first token before it ends.
    let a = lane(1, 0, vec![vec![w(1), w(2), w(3)]]);
    let b = lane(2, 0, vec![vec![w(1)], vec![w(2)]]);
    let (_, worst) = merge_timed(&mut ProcessMajor::default(), &[a, b]);
    // A: one token, ends at clock 3. B: token 1 ends at clock 4; token 2
    // ends at clock 5, one acquire after the first ended.
    assert_eq!(worst, vec![3, 4]);
}
