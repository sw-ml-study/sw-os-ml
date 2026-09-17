//! The harness's own correctness, before any number it produces matters.
//!
//! Every figure in M3 comes out of `replay`, so a bug here is a bug in
//! every result and would be invisible in all of them. Two properties
//! carry the weight: the same inputs give the same counts, and the budget
//! cannot differ between policies being compared.

use mlos_abi::{Fields, ObjectClass, ObjectId};
use mlos_objtab::{
    CostNs, Mutability, NextUse, ObjectMeta, Precision, ProviderId, SessionId, Tier,
};
use mlos_policy::{Policy, Residency};
use mlos_sim::{Model, Outcome, compare, replay};
use mlos_trace::{Access, Header, Trace};

/// Every object is this big, so budgets are countable in objects.
const SIZE: u32 = 1024;

/// A model where every id exists and every object is the same size.
struct Uniform;

impl Model for Uniform {
    fn meta(&self, _id: ObjectId) -> Option<ObjectMeta> {
        Some(ObjectMeta {
            size: SIZE,
            precision: Precision::Q4,
            tier: Tier::Cold,
            provider: ProviderId(2),
            handle: 0,
            resident_at: 0,
            next_use: NextUse::Never,
            reuse_count: 0,
            placed_tick: 0,
            used_tick: 0,
            reload_cost: CostNs(1000),
            recompute_cost: CostNs::IMPOSSIBLE,
            share_count: 0,
            mutability: Mutability::Immutable,
            owner: SessionId(0),
        })
    }
}

/// A model that knows nothing, for the mismatched-pair case.
struct Empty;

impl Model for Empty {
    fn meta(&self, _id: ObjectId) -> Option<ObjectMeta> {
        None
    }
}

/// Evicts whatever the resident set offers first.
struct First;

impl Policy for First {
    fn name(&self) -> &'static str {
        "first"
    }
    fn victim(&self, resident: &dyn Residency, _wanting: &ObjectMeta) -> Option<ObjectId> {
        resident.at(0).map(|(id, _)| id)
    }
}

/// Never evicts anything, like demand paging.
struct Never;

impl Policy for Never {
    fn name(&self) -> &'static str {
        "never"
    }
    fn victim(&self, _resident: &dyn Residency, _wanting: &ObjectMeta) -> Option<ObjectId> {
        None
    }
}

/// An id for tile `n`.
fn tile(n: u16) -> ObjectId {
    ObjectId::new(
        ObjectClass::WeightTile,
        Fields {
            model: 1,
            layer: n / 16,
            tensor: n % 16,
            tile: 0,
        },
    )
}

/// A trace of the given tiles, in order.
fn trace(tiles: &[u16]) -> Vec<Access> {
    tiles
        .iter()
        .map(|n| Access {
            session: SessionId(1),
            object: tile(*n),
        })
        .collect()
}

/// Wraps accesses as a trace.
fn as_trace(accesses: &[Access]) -> Trace<'_> {
    Trace {
        header: Header {
            model: "uniform",
            source: "a test",
        },
        accesses,
    }
}

#[test]
fn a_hit_costs_nothing_and_a_miss_costs_a_read() {
    let accesses = trace(&[1, 1, 1]);
    let out = replay(&as_trace(&accesses), &Uniform, 4 * u64::from(SIZE), &Never);
    assert_eq!(
        out,
        Outcome {
            hits: 2,
            reads: 1,
            bytes: u64::from(SIZE),
            cost: 1000,
            ..Outcome::default()
        }
    );
}

#[test]
fn a_budget_that_fits_everything_never_evicts() {
    let accesses = trace(&[1, 2, 3, 4]);
    let out = replay(&as_trace(&accesses), &Uniform, 4 * u64::from(SIZE), &First);
    assert_eq!(out.reads, 4);
    assert_eq!(out.evicted, 0);
    assert_eq!(out.refused, 0);
}

#[test]
fn a_policy_that_never_evicts_refuses_once_it_is_full() {
    // Demand paging's shape, and the floor the comparison is against: it
    // does not thrash, it simply stops serving.
    let accesses = trace(&[1, 2, 3, 4, 5]);
    let out = replay(&as_trace(&accesses), &Uniform, 2 * u64::from(SIZE), &Never);
    assert_eq!(out.reads, 2, "only two ever fit");
    assert_eq!(out.refused, 3);
    assert_eq!(out.evicted, 0);
}

#[test]
fn eviction_makes_room_and_is_counted() {
    let accesses = trace(&[1, 2, 3, 4, 5]);
    let out = replay(&as_trace(&accesses), &Uniform, 2 * u64::from(SIZE), &First);
    assert_eq!(out.reads, 5, "every access was eventually served");
    assert_eq!(out.evicted, 3, "three had to go to make room");
    assert_eq!(out.refused, 0);
}

#[test]
fn the_budget_is_never_exceeded() {
    // The one invariant a residency simulator must not break. If it can
    // hold more than the budget, every comparison is measuring a system
    // with more memory than it was given.
    for budget in 1..6u64 {
        let accesses = trace(&[1, 2, 3, 4, 5, 1, 2, 6, 7]);
        let out = replay(
            &as_trace(&accesses),
            &Uniform,
            budget * u64::from(SIZE),
            &First,
        );
        let held = out.reads + out.hits - out.evicted;
        assert!(
            held <= budget + out.hits,
            "budget {budget} exceeded: {out:?}"
        );
    }
}

#[test]
fn replaying_twice_gives_the_same_counts() {
    // Nondeterminism here would make every later number arguable, and
    // there would be no way to tell from the numbers themselves.
    let accesses = trace(&[3, 1, 4, 1, 5, 9, 2, 6, 5, 3, 5]);
    let once = replay(&as_trace(&accesses), &Uniform, 3 * u64::from(SIZE), &First);
    let again = replay(&as_trace(&accesses), &Uniform, 3 * u64::from(SIZE), &First);
    assert_eq!(once, again);
}

#[test]
fn compare_gives_every_policy_the_same_budget() {
    // The enforcement is the signature: one budget argument for all of
    // them, so a comparison where they differed cannot be expressed.
    let accesses = trace(&[1, 2, 3, 4, 5, 1, 2]);
    let table = compare(
        &as_trace(&accesses),
        &Uniform,
        2 * u64::from(SIZE),
        &[&First, &Never],
    );
    assert_eq!(table.len(), 2);
    assert_eq!(table[0].0, "first");
    assert_eq!(table[1].0, "never");
    // Different policies, same workload: they must differ somewhere or
    // the harness is not distinguishing them at all.
    assert_ne!(table[0].1, table[1].1);
}

#[test]
fn a_trace_and_model_that_do_not_match_say_so() {
    // Not a refusal. A refusal is a residency decision; this is the two
    // files not being about the same thing, and hiding it behind a
    // plausible number is how a whole comparison ends up meaningless.
    let accesses = trace(&[1, 2, 3]);
    let out = replay(&as_trace(&accesses), &Empty, 8 * u64::from(SIZE), &First);
    assert_eq!(out.mismatched, 3);
    assert_eq!(out.reads, 0);
    assert_eq!(out.refused, 0);
}

#[test]
fn an_empty_trace_costs_nothing() {
    let out = replay(&as_trace(&[]), &Uniform, 1024, &First);
    assert_eq!(out, Outcome::default());
    assert_eq!(out.hit_per_mille(), 0);
}
