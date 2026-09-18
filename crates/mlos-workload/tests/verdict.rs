//! The M3 comparison, printed as the document it becomes.
//!
//! Run it and redirect the output:
//!
//! ```text
//! cargo test -p mlos-workload --test verdict -- --ignored --nocapture
//! ```
//!
//! `#[ignore]`d because it measures rather than checks. What CHECKS the
//! result is `optimal.rs`, which holds the two invariants this table
//! would otherwise be able to violate quietly: LRU must beat FIFO where
//! recency predicts reuse, and known-next-use must never lose to a
//! baseline, because it has strictly more information and a loss is a bug
//! in the policy rather than a finding about the thesis.

use mlos_abi::{Fields, ObjectClass, ObjectId};
use mlos_objtab::ObjectMeta;
use mlos_policy::{Demand, FIFO, LRU, NEXT_USE, Policy};
use mlos_sim::{Model, Outcome, compare};
use mlos_trace::{Access, Trace};
use mlos_workload::Decode;

/// Budgets spanning the whole range, not only where the answer is good.
const BUDGETS: [u64; 7] = [96, 128, 160, 192, 224, 256, 384];

/// Every policy, in reporting order.
fn policies() -> [&'static dyn Policy; 4] {
    [&Demand, &FIFO, &LRU, &NEXT_USE]
}

/// One row of results.
fn run(decode: &Decode, held: &[Access], kib: u64) -> Vec<(&'static str, Outcome)> {
    let trace = Trace {
        header: decode.header(),
        accesses: held,
    };
    compare(&trace, decode, kib * 1024, &policies())
}

#[test]
#[ignore = "prints the report; run with --ignored --nocapture"]
fn the_table() {
    println!("## Provider reads, by budget and session count\n");
    println!("Lower is better. Demand paging is shown as reads+refusals:");
    println!("its read count is bought by declining to serve, and is not");
    println!("comparable with policies that served every access.\n");

    for sessions in [1u16, 2, 4, 8] {
        let decode = Decode::of(sessions, 40);
        let held = decode.trace();
        println!(
            "### {sessions} session{}, 40 rounds -- {} accesses\n",
            if sessions == 1 { "" } else { "s" },
            held.len()
        );
        println!("| budget | demand | FIFO | LRU | next-use | vs best baseline |");
        println!("| --- | --- | --- | --- | --- | --- |");
        for kib in BUDGETS {
            let table = run(&decode, &held, kib);
            let of = |n: &str| table.iter().find(|(m, _)| *m == n).expect(n).1;
            let (fifo, lru, next) = (of("fifo").reads, of("lru").reads, of("next-use").reads);
            let best = fifo.min(lru).max(1);
            let gain = 100 - (next as i64 * 100 / best as i64);
            println!(
                "| {kib} KiB | {}+{} | {fifo} | {lru} | **{next}** | {gain:+}% |",
                of("demand").reads,
                of("demand").refused
            );
        }
        println!();
    }
}

#[test]
#[ignore = "prints the report; run with --ignored --nocapture"]
fn the_costs() {
    println!("## Bytes moved and recovery cost, four sessions\n");
    println!("| budget | policy | reads | KiB moved | evicted | refused | cost (ms) |");
    println!("| --- | --- | --- | --- | --- | --- | --- |");
    let decode = Decode::of(4, 40);
    let held = decode.trace();
    for kib in [128u64, 192, 256] {
        for (name, out) in run(&decode, &held, kib) {
            println!(
                "| {kib} KiB | {name} | {} | {} | {} | {} | {} |",
                out.reads,
                out.bytes / 1024,
                out.evicted,
                out.refused,
                out.cost / 1_000_000
            );
        }
    }
    println!();
}

/// Whole-tensor objects: one per layer, sixteen tiles' worth.
///
/// For PRD Q1, which asks whether an object should be a weight TILE or a
/// whole tensor. Coarser objects mean a table an order of magnitude
/// smaller and eviction that can only throw away sixteen tiles at once.
struct Coarse(Decode);

/// The tensor object a tile belongs to.
fn tensor_of(id: ObjectId) -> ObjectId {
    let fields = id.fields();
    match id.class() {
        Some(ObjectClass::WeightTile) => ObjectId::new(
            ObjectClass::WeightTile,
            Fields {
                model: fields.model,
                layer: fields.layer,
                tensor: 0,
                tile: 0,
            },
        ),
        _ => id,
    }
}

impl Model for Coarse {
    fn meta(&self, id: ObjectId) -> Option<ObjectMeta> {
        let mut meta = self.0.meta(id)?;
        if id.class() == Some(ObjectClass::WeightTile) {
            meta.size *= 16; // one layer's tiles, as one object
        }
        meta.reload_cost = fetching(meta.size);
        Some(meta)
    }
}

/// Whole-tile objects, priced the same way, so the two are comparable.
struct Fine(Decode);

impl Model for Fine {
    fn meta(&self, id: ObjectId) -> Option<ObjectMeta> {
        let mut meta = self.0.meta(id)?;
        meta.reload_cost = fetching(meta.size);
        Some(meta)
    }
}

/// What fetching `size` bytes costs, as `mlos-synth`'s backing tier
/// charges it: three milliseconds to the first byte, then a gigabyte a
/// second.
///
/// Recomputed for BOTH granularities rather than taken from the model,
/// because `ObjectMeta::reload_cost` is a precomputed scalar sized for a
/// one-kibibyte tile. Leaving it alone would have let a sixteen-kibibyte
/// object arrive for the price of a one-kibibyte one, which is not a
/// finding about granularity -- it is a unit error that happens to
/// flatter the answer.
fn fetching(size: u32) -> mlos_objtab::CostNs {
    mlos_objtab::CostNs(3_000_000 + size)
}

#[test]
#[ignore = "prints the report; run with --ignored --nocapture"]
fn the_granularity_question() {
    println!("## PRD Q1: tile-granular against tensor-granular\n");
    println!("Same workload, same budget, same policy. Reads are not");
    println!("comparable across granularities -- a coarse object serves");
    println!("sixteen tiles' worth of accesses -- so the metric is BYTES");
    println!("MOVED, which is what a provider is actually charged for.\n");
    println!("| budget | granularity | objects in table | KiB moved | cost (ms) |");
    println!("| --- | --- | --- | --- | --- | ");

    let decode = Decode::of(4, 40);
    let fine = decode.trace();
    let coarse: Vec<Access> = fine
        .iter()
        .map(|access| Access {
            session: access.session,
            object: tensor_of(access.object),
        })
        .collect();

    for kib in [128u64, 192, 256] {
        let distinct = |held: &[Access]| {
            let mut seen: Vec<u64> = held.iter().map(|a| a.object.0).collect();
            seen.sort_unstable();
            seen.dedup();
            seen.len()
        };
        let at = |held: &[Access], model: &dyn Model| {
            let trace = Trace {
                header: decode.header(),
                accesses: held,
            };
            compare(&trace, model, kib * 1024, &policies())
                .iter()
                .find(|(name, _)| *name == "next-use")
                .expect("next-use")
                .1
        };
        let sharp = at(&fine, &Fine(decode));
        let blunt = at(&coarse, &Coarse(decode));

        println!(
            "| {kib} KiB | tile (1 KiB) | {} | {} | {} |",
            distinct(&fine),
            sharp.bytes / 1024,
            sharp.cost / 1_000_000
        );
        println!(
            "| {kib} KiB | tensor (16 KiB) | {} | {} | {} |",
            distinct(&coarse),
            blunt.bytes / 1024,
            blunt.cost / 1_000_000
        );
    }
    println!();
}
