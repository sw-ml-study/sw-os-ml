//! The G4 report's tables, printed as markdown so the report cannot drift
//! from the measurement that produced it.
//!
//! ```text
//! cargo test -p mlos-workload --test g4 -- --ignored --nocapture --test-threads=1
//! ```
//!
//! Every row is `compare` at one budget: reads and mebibytes moved for
//! each policy, demand as reads+refusals because its reads are bought by
//! not serving the workload. Three workloads: the synthetic verdict
//! trace, and the real shape with and without context.

use std::time::Instant;

use mlos_policy::{Demand, FIFO, LRU, NEXT_USE, Policy};
use mlos_sim::{Model, Outcome, compare};
use mlos_trace::{Access, Trace};
use mlos_workload::{Context, Decode, MINICPM, Real, Shape};

fn row(name: &str, model: &dyn Model, trace: &Trace<'_>, budget: u64, unit: u64) {
    let started = Instant::now();
    let policies = [&Demand as &dyn Policy, &FIFO, &LRU, &NEXT_USE];
    let table = compare(trace, model, budget, &policies);
    let of = |n: &str| table.iter().find(|(m, _)| *m == n).expect(n).1;
    let cell = |o: Outcome| format!("{} / {:.1}", o.reads, o.bytes as f64 / 1_048_576.0);
    let (fifo, lru, next) = (of("fifo"), of("lru"), of("next-use"));
    let best = fifo.reads.min(lru.reads).max(1);
    let gain = 100 - (next.reads as i64 * 100 / best as i64);
    println!(
        "| {name} | {}+{} | {} | {} | **{}** | {gain:+}% | {:.0} s |",
        of("demand").reads,
        of("demand").refused,
        cell(fifo),
        cell(lru),
        cell(next),
        started.elapsed().as_secs_f64()
    );
    let _ = unit;
}

fn header(title: &str, accesses: &[Access]) {
    let kv = accesses
        .iter()
        .filter(|a| a.object.class() == Some(mlos_abi::ObjectClass::KvBlock))
        .count();
    println!(
        "\n### {title}\n\n{} accesses ({} weights, {kv} KV).\n",
        accesses.len(),
        accesses.len() - kv
    );
    println!(
        "| budget | demand reads+refused | FIFO reads / MiB | LRU reads / MiB | next-use reads / MiB | vs best baseline | time |"
    );
    println!("| --- | --- | --- | --- | --- | --- | --- |");
}

#[test]
#[ignore = "prints the G4 report tables; run with --ignored --nocapture --test-threads=1"]
fn synthetic() {
    let decode = Decode::of(4, 40);
    let held = decode.trace();
    let trace = Trace {
        header: decode.header(),
        accesses: &held,
    };
    header("Synthetic 8x16 model, 4 sessions x 40 rounds", &held);
    for kib in [96u64, 128, 160, 192, 224, 256, 384] {
        row(&format!("{kib} KiB"), &decode, &trace, kib << 10, 1024);
    }
}

#[test]
#[ignore = "prints the G4 report tables; run with --ignored --nocapture --test-threads=1"]
fn real_decode_only() {
    let shape = Shape::parse(MINICPM, 2).expect("sidecar");
    let real = Real::of(&shape, 4, 40, Context::DECODE_ONLY);
    let held = real.trace();
    let trace = Trace {
        header: real.header(),
        accesses: &held,
    };
    header(
        "MiniCPM5-1B at F16, 4 sessions x 40 rounds, no prompt",
        &held,
    );
    for mib in [512u64, 1024, 1536, 1664, 1792, 2048] {
        row(&format!("{mib} MiB"), &real, &trace, mib << 20, 1 << 20);
    }
}

#[test]
#[ignore = "prints the G4 report tables; run with --ignored --nocapture --test-threads=1"]
fn real_with_context() {
    let shape = Shape::parse(MINICPM, 2).expect("sidecar");
    let served = Context {
        prefix: 2048,
        tokens_per_block: 16,
    };
    let real = Real::of(&shape, 8, 40, served);
    let held = real.trace();
    let trace = Trace {
        header: real.header(),
        accesses: &held,
    };
    header(
        "MiniCPM5-1B at F16, 8 sessions x 40 rounds, 2,048-token prompts, 16-token KV blocks",
        &held,
    );
    for mib in [1536u64, 1664, 1792, 1920, 2048, 2176, 2560] {
        row(&format!("{mib} MiB"), &real, &trace, mib << 20, 1 << 20);
    }
}
