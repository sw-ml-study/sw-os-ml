//! The baselines, and the harness's own test.
//!
//! On a workload with real reuse, LRU must beat FIFO. If it does not, the
//! simulator is wrong and every number after this is worthless -- so this
//! file is as much a test of `mlos-sim` as of the policies, and if it
//! fails the harness is what to fix, not the expectation.
//!
//! The patterns here are hand-made, and that is fine because they are
//! TESTS rather than the measurement. The measurement runs on a recorded
//! or model-derived trace; these exist to pin behaviour that is known in
//! advance from first principles.

use mlos_abi::{Fields, ObjectClass, ObjectId};
use mlos_objtab::{
    CostNs, Mutability, NextUse, ObjectMeta, Precision, ProviderId, SessionId, Tier,
};
use mlos_policy::{Demand, FIFO, LRU, Policy};
use mlos_sim::{Model, compare, replay};
use mlos_trace::{Access, Header, Trace};

/// Every object is this big, so a budget is countable in objects.
const SIZE: u32 = 1024;

/// A model where every id exists and every object is the same size.
struct Uniform;

impl Model for Uniform {
    fn meta(&self, _id: ObjectId) -> Option<ObjectMeta> {
        Some(ObjectMeta {
            size: SIZE,
            precision: Precision::Q4,
            tier: Tier::Cold,
            home: Tier::Cold,
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

/// An id for object `n`.
fn object(n: u16) -> ObjectId {
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

/// The accesses for a sequence of object numbers.
fn accesses(order: &[u16]) -> Vec<Access> {
    order
        .iter()
        .map(|n| Access {
            session: SessionId(1),
            object: object(*n),
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
fn lru_beats_fifo_when_one_object_is_wanted_repeatedly() {
    // THE HARNESS'S OWN TEST. Object 1 is wanted on every other access;
    // everything else is wanted once. FIFO throws 1 away because it
    // arrived first and FIFO knows nothing else; LRU keeps it because it
    // was just used. If this comes out the other way round, mlos-sim is
    // wrong.
    let held = accesses(&[1, 2, 1, 3, 1, 4, 1, 5, 1, 6, 1, 7]);
    let budget = 3 * u64::from(SIZE);
    let table = compare(&as_trace(&held), &Uniform, budget, &[&FIFO, &LRU]);

    let reads = |name: &str| table.iter().find(|(n, _)| *n == name).expect(name).1.reads;
    assert!(
        reads("lru") < reads("fifo"),
        "LRU must beat FIFO where recency predicts reuse -- \
         lru {} reads, fifo {} reads. If this fails, fix the harness.",
        reads("lru"),
        reads("fifo")
    );
}

#[test]
fn fifo_beats_lru_on_a_cyclic_sweep() {
    // The mirror image, and the reason a dense sweep proves nothing. In a
    // cycle longer than the budget, the object LRU just touched is the one
    // it will want LAST -- so LRU evicts exactly what it is about to need
    // and misses everything. FIFO, knowing less, does no worse.
    let cycle: Vec<u16> = (0..6).cycle().take(30).collect();
    let held = accesses(&cycle);
    let budget = 4 * u64::from(SIZE);
    let table = compare(&as_trace(&held), &Uniform, budget, &[&FIFO, &LRU]);

    let reads = |name: &str| table.iter().find(|(n, _)| *n == name).expect(name).1.reads;
    assert!(
        reads("lru") >= reads("fifo"),
        "on a cycle LRU cannot beat FIFO: lru {}, fifo {}",
        reads("lru"),
        reads("fifo")
    );
    // And both are terrible: every access misses.
    assert_eq!(reads("lru"), 30, "LRU misses everything on a cycle");
}

#[test]
fn demand_refuses_rather_than_thrashing() {
    // It never evicts, so it never does needless work -- it simply stops
    // serving once full. Whether that is better depends on what a caller
    // does with a refusal, which is what admission control is for.
    let cycle: Vec<u16> = (0..6).cycle().take(30).collect();
    let held = accesses(&cycle);
    let budget = 4 * u64::from(SIZE);
    let out = replay(&as_trace(&held), &Uniform, budget, &Demand);

    assert_eq!(
        out.evicted, 0,
        "demand paging evicts nothing, by definition"
    );
    assert_eq!(out.reads, 4, "it fills once and stops");
    assert!(out.refused > 0);
    // The whole point of having it in the table: it moves the fewest
    // bytes of any policy and serves the fewest accesses. Cheap and
    // useless is a corner of the space worth seeing.
    let table = compare(&as_trace(&held), &Uniform, budget, &[&Demand, &FIFO, &LRU]);
    let bytes = |name: &str| table.iter().find(|(n, _)| *n == name).expect(name).1.bytes;
    assert!(bytes("demand") < bytes("fifo"));
}

#[test]
fn fifo_and_lru_agree_when_nothing_is_reused() {
    // They read different clocks, so they differ only when an object is
    // used after it was placed. With no reuse the two clocks are the same
    // clock, and the policies are the same policy.
    let held = accesses(&(0..20).collect::<Vec<u16>>());
    let budget = 4 * u64::from(SIZE);
    let table = compare(&as_trace(&held), &Uniform, budget, &[&FIFO, &LRU]);
    assert_eq!(table[0].1, table[1].1);
}

#[test]
fn every_baseline_is_deterministic() {
    let held = accesses(&[3, 1, 4, 1, 5, 9, 2, 6, 5, 3, 5, 8, 9, 7, 9, 3]);
    let budget = 3 * u64::from(SIZE);
    for policy in [&Demand as &dyn Policy, &FIFO, &LRU] {
        let once = replay(&as_trace(&held), &Uniform, budget, policy);
        let again = replay(&as_trace(&held), &Uniform, budget, policy);
        assert_eq!(once, again, "{} is not deterministic", policy.name());
    }
}

#[test]
fn a_budget_of_one_object_still_works() {
    // The degenerate case a scan-based policy is most likely to get wrong.
    let held = accesses(&[1, 2, 1, 2]);
    for policy in [&FIFO as &dyn Policy, &LRU] {
        let out = replay(&as_trace(&held), &Uniform, u64::from(SIZE), policy);
        assert_eq!(out.reads, 4, "{} should miss every time", policy.name());
        assert_eq!(out.evicted, 3);
    }
}
