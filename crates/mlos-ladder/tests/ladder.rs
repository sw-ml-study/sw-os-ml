//! The ladder's arithmetic, its walk, and what a squeeze does to a
//! manager's table, arena and accounts.

use mlos_admit::Capacity;
use mlos_ladder::{Fate, Move, Rung, Shape, arrive, next, squeeze};
use mlos_objman::{Arena, Contract, Manager};
use mlos_objtab::{Precision, SessionId};
use mlos_session::{MAX_SESSIONS, Sessions};

const SHAPE: Shape = Shape {
    per_block: 1,
    hot: 2,
};
const KV: u64 = 256;
const RUNGS: [Rung; 6] = [Rung::L0, Rung::L1, Rung::L2, Rung::L3, Rung::L4, Rung::L5];

fn contract(floor: Precision, context: u32) -> Contract {
    Contract {
        quality_floor: floor,
        context,
        ..Contract::NONE
    }
}

#[test]
fn l0_needs_what_admission_charged() {
    for shape in [
        SHAPE,
        Shape {
            per_block: 16,
            hot: 8,
        },
    ] {
        for context in [0u32, 1, 5, 17, 2048, 2051] {
            assert_eq!(shape.need(Rung::L0, context, KV), u64::from(context) * KV);
        }
    }
}

#[test]
fn every_rung_holds_no_more_than_the_one_above() {
    let shape = Shape {
        per_block: 16,
        hot: 8,
    };
    for context in [1u32, 100, 300, 2048, 2088] {
        let needs: Vec<u64> = RUNGS.iter().map(|r| shape.need(*r, context, KV)).collect();
        assert!(
            needs.windows(2).all(|w| w[1] <= w[0]),
            "{context}: {needs:?}"
        );
    }
}

#[test]
fn the_rungs_on_ten_one_token_blocks() {
    // Ten blocks, two hot, eight cold; the span is the oldest four.
    let held = |rung| SHAPE.need(rung, 10, KV);
    assert_eq!(held(Rung::L1), 8 * 128 + 2 * 256);
    assert_eq!(held(Rung::L2), 8 * 64 + 2 * 256);
    // One summary for the span, four candidates.
    assert_eq!(held(Rung::L3), 64 + 4 * 64 + 2 * 256);
    // Half the candidates kept.
    assert_eq!(held(Rung::L4), 64 + 2 * 64 + 2 * 256);
    assert_eq!(held(Rung::L5), 2 * 256);
    assert_eq!(SHAPE.fate(Rung::L3, 0, 10), Fate::Summary);
    assert_eq!(SHAPE.fate(Rung::L4, 9, 10), Fate::Keep(Precision::Fp16));
}

#[test]
fn a_step_reads_what_it_rewrites_and_the_span_it_summarises() {
    let cost = |from, to| SHAPE.cost(from, to, 10, KV);
    assert_eq!(cost(Rung::L0, Rung::L1), 8 * 256);
    assert_eq!(cost(Rung::L1, Rung::L2), 8 * 128);
    // The span, four Q4 blocks, is read into one.
    assert_eq!(cost(Rung::L2, Rung::L3), 4 * 64);
    assert_eq!(cost(Rung::L3, Rung::L4), 0);
    assert_eq!(cost(Rung::L4, Rung::L5), 0);
}

#[test]
fn the_walk_stops_at_each_floor_then_terminates_the_newest() {
    let capacity = Capacity {
        bytes: 1,
        kv_per_token: KV,
        ..Capacity::NONE
    };
    let mut live = Sessions::<MAX_SESSIONS>::EMPTY;
    for floor in [Precision::Fp16, Precision::Q8, Precision::Ternary] {
        live.create(contract(floor, 10)).expect("room");
    }
    let mut seen = Vec::new();
    while let Move::Deepen(rung) = next(&capacity, &SHAPE, &live) {
        seen.push(rung);
        for at in 0..MAX_SESSIONS {
            let id = match live.at(at) {
                Some(s) if mlos_ladder::movable(s, rung) => s.id,
                _ => continue,
            };
            live.get_mut(id).expect("live").rung = rung;
        }
    }
    assert_eq!(seen, RUNGS[1..]);
    let rungs: Vec<Rung> = live.each().map(|s| s.rung).collect();
    assert_eq!(rungs, [Rung::L0, Rung::L1, Rung::L5]);
    assert_eq!(
        next(&capacity, &SHAPE, &live),
        Move::Terminate(SessionId(3))
    );
}

#[test]
fn a_squeeze_shrinks_resident_blocks_in_place_and_drops_the_rest() {
    let mut bytes = vec![0u8; 64 << 10];
    let mut held: Manager<'_, 256> = Manager::new(Arena::new(&mut bytes));
    held.attach(&mlos_synth::tiers::RECOMPUTE).expect("attach");
    held.capacity = Capacity {
        kv_per_token: KV,
        ..Capacity::NONE
    };
    let template = mlos_synth::kv::meta(0);
    let wanted = contract(Precision::Ternary, 10);
    let (id, placed) = arrive(&mut held, &SHAPE, wanted, (1, template)).expect("admit");
    assert_eq!(placed, 10);
    let mut expected = Vec::new();
    for budget in [10 * KV, 2400, 1100, 900, 750, 600] {
        let done = squeeze(&mut held, &SHAPE, budget);
        let s = *held.sessions.get(id).expect("live");
        let need = SHAPE.need(s.rung, 10, KV);
        assert!(done.fits && need <= budget, "{budget}: {done:?}");
        assert_eq!(s.resident, need, "{budget}: on {}", s.rung);
        assert_eq!(held.arena.occupancy().used, need, "{budget}");
        expected.push(s.rung);
        if s.rung == Rung::L5 {
            break;
        }
    }
    assert_eq!(
        expected,
        [Rung::L0, Rung::L1, Rung::L2, Rung::L3, Rung::L4, Rung::L5]
    );
    let s = held.sessions.get(id).expect("live");
    assert_eq!(s.delivered.deepest, Rung::L5);
    assert_eq!(s.delivered.coarsest, Precision::Q4);
    assert_eq!(s.delivered.recomputed, 8 * 256 + 8 * 128 + 4 * 64);
}

#[test]
fn a_budget_below_the_last_rung_terminates_and_reports_l7() {
    let mut bytes = vec![0u8; 64 << 10];
    let mut held: Manager<'_, 256> = Manager::new(Arena::new(&mut bytes));
    held.capacity.kv_per_token = KV;
    for _ in 0..3 {
        held.create_session(contract(Precision::Ternary, 10))
            .expect("admit");
    }
    let done = squeeze(&mut held, &SHAPE, 2 * 2 * 256);
    assert_eq!(
        (done.reached, done.terminated, done.fits),
        (Rung::L7, 1, true)
    );
    let ids: Vec<u16> = held.sessions.each().map(|s| s.id.0).collect();
    assert_eq!(ids, [1, 2]);
}
