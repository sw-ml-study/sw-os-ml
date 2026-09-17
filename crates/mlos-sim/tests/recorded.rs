//! The baselines against the trace MLOS actually recorded.
//!
//! `examples/viz/runtime.trace` is a real run: a kernel reading a real
//! virtio-blk device, two sweeps of the synthetic model, derived from the
//! event stream rather than written by hand.
//!
//! **It is also the degenerate case, and that is the point of running it
//! here.** A dense sweep re-reads nothing within a pass, so LRU evicts
//! precisely what it is about to need. Seeing it be pessimal for the
//! right reason is worth having on the record BEFORE step 004 makes the
//! workload fair -- otherwise the first fair result has nothing to be
//! compared against.

use std::fs;

use mlos_abi::{ObjectClass, ObjectId};
use mlos_objtab::ObjectMeta;
use mlos_policy::{Demand, FIFO, LRU};
use mlos_sim::{Model, compare};
use mlos_synth::model;
use mlos_trace::{Access, parse};

/// The synthetic model's metadata, by class.
struct Synth;

impl Model for Synth {
    fn meta(&self, id: ObjectId) -> Option<ObjectMeta> {
        match id.class()? {
            ObjectClass::WeightTile => Some(model::weights()),
            ObjectClass::Activation => Some(model::activations()),
            _ => None,
        }
    }
}

/// The committed trace.
fn recorded() -> String {
    fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/viz/runtime.trace"
    ))
    .expect("the committed trace")
}

#[test]
fn the_recorded_sweep_replays_against_every_baseline() {
    let text = recorded();
    let mut room = vec![Access::EMPTY; 4096];
    let trace = parse(&text, &mut room).expect("a valid trace");
    assert_eq!(trace.header.model, "synth-8x16");

    // The budget the guest actually ran with: a quarter of the model's
    // weights, which is what makes a sweep run out.
    let budget = 32 * 1024;
    let table = compare(&trace, &Synth, budget, &[&Demand, &FIFO, &LRU]);

    for (name, out) in &table {
        println!(
            "{name:8} reads {:4}  bytes {:7}  evicted {:4}  refused {:4}  hits/1000 {}",
            out.reads,
            out.bytes,
            out.evicted,
            out.refused,
            out.hit_per_mille()
        );
        assert_eq!(out.mismatched, 0, "{name}: the trace and model disagree");
    }

    let of = |name: &str| table.iter().find(|(n, _)| *n == name).expect(name).1;
    // DOING NOTHING WINS. Demand refuses two accesses and keeps the 32
    // tiles it already had, which the second sweep then hits; FIFO and
    // LRU evict what they are about to need and miss every single access.
    // Refusing beating replacing is PRD F4's argument for admission
    // control arriving uninvited, in a measurement nobody designed to
    // show it.
    assert!(
        of("demand").reads < of("fifo").reads,
        "on a cyclic sweep, evicting nothing beats evicting badly"
    );
    assert_eq!(of("demand").evicted, 0);
    assert_eq!(
        of("fifo").hit_per_mille(),
        0,
        "a cycle defeats FIFO entirely"
    );
    assert_eq!(of("lru").hit_per_mille(), 0, "and LRU equally");
    // Indistinguishable here: the first pass never re-reads, so the two
    // clocks are the same clock. That equality IS the measurement saying
    // this workload is too easy to learn anything from.
    assert_eq!(
        of("fifo").reads,
        of("lru").reads,
        "a dense sweep cannot tell FIFO and LRU apart"
    );
}
