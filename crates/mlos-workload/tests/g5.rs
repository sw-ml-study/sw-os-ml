//! Gate G5's tables: process-major against parameter-major, by session
//! count and budget, under LRU and next-use.
//!
//! ```text
//! cargo test -p mlos-workload --test g5 -- --ignored --nocapture --test-threads=1
//! ```

use std::time::Instant;

use mlos_abi::ObjectClass;
use mlos_objtab::SessionId;
use mlos_policy::{LRU, NEXT_USE, Policy};
use mlos_sched::{Lane, ParameterMajor, ProcessMajor, merge};
use mlos_sim::{Model, Outcome, replay};
use mlos_trace::{Access, Header, Trace};
use mlos_workload::{Context, Decode, MINICPM, Real, Shape};

fn run(
    model: &dyn Model,
    header: Header<'_>,
    accesses: &[Access],
    budget: u64,
    policy: &dyn Policy,
) -> Outcome {
    let trace = Trace { header, accesses };
    replay(&trace, model, budget, policy)
}

/// One row: reads under each schedule and policy, and the gain.
fn row(name: &str, model: &dyn Model, header: Header<'_>, streams: &[Lane], budget: u64) {
    let started = Instant::now();
    let process = merge(&mut ProcessMajor::default(), streams);
    let parameter = merge(&mut ParameterMajor, streams);
    assert_eq!(process.len(), parameter.len());
    let kv = process
        .iter()
        .filter(|a| a.object.class() == Some(ObjectClass::KvBlock))
        .count();
    let mut cells = Vec::new();
    for policy in [&LRU as &dyn Policy, &NEXT_USE] {
        let pm = run(model, header, &process, budget, policy).reads;
        let qm = run(model, header, &parameter, budget, policy).reads;
        let gain = 100 - (qm as i64 * 100 / pm.max(1) as i64);
        cells.push(format!("{pm} | {qm} | {gain:+}%"));
    }
    println!(
        "| {name} | {} | {} | {} | {:.0} s |",
        kv * 100 / process.len().max(1),
        cells[0],
        cells[1],
        started.elapsed().as_secs_f64()
    );
}

fn header_line(title: &str) {
    println!("\n### {title}\n");
    println!(
        "| sessions, budget | KV % of accesses | LRU: process / parameter / gain | next-use: process / parameter / gain | time |"
    );
    println!("| --- | --- | --- | --- | --- |");
}

#[test]
#[ignore = "prints the G5 tables; run with --ignored --nocapture --test-threads=1"]
fn synthetic() {
    header_line("Synthetic 8x16 model, 40 rounds");
    for sessions in [1u16, 2, 4, 8] {
        let decode = Decode::of(sessions, 40);
        let streams: Vec<_> = (0..sessions)
            .map(|s| Lane {
                session: SessionId(s + 1),
                ceiling: 0,
                tokens: decode.tokens(s),
            })
            .collect();
        for kib in [96u64, 128, 160, 192, 256] {
            row(
                &format!("{sessions}, {kib} KiB"),
                &decode,
                decode.header(),
                &streams,
                kib << 10,
            );
        }
    }
}

#[test]
#[ignore = "prints the G5 tables; run with --ignored --nocapture --test-threads=1"]
fn real_decode_only() {
    header_line("MiniCPM5-1B at F16, 40 rounds, no prompt");
    let shape = Shape::parse(MINICPM, 2).expect("sidecar");
    for sessions in [1u16, 2, 4, 8] {
        let real = Real::of(&shape, sessions, 40, Context::DECODE_ONLY);
        let streams: Vec<_> = (0..sessions)
            .map(|s| Lane {
                session: SessionId(s + 1),
                ceiling: 0,
                tokens: real.tokens(s),
            })
            .collect();
        for mib in [1024u64, 1536, 1664] {
            row(
                &format!("{sessions}, {mib} MiB"),
                &real,
                real.header(),
                &streams,
                mib << 20,
            );
        }
    }
}

#[test]
#[ignore = "prints the G5 tables; run with --ignored --nocapture --test-threads=1"]
fn real_with_context() {
    header_line("MiniCPM5-1B at F16, 40 rounds, 2,048-token prompts, 16-token KV blocks");
    let shape = Shape::parse(MINICPM, 2).expect("sidecar");
    let served = Context {
        prefix: 2048,
        tokens_per_block: 16,
    };
    for sessions in [4u16, 8] {
        let real = Real::of(&shape, sessions, 40, served);
        let streams: Vec<_> = (0..sessions)
            .map(|s| Lane {
                session: SessionId(s + 1),
                ceiling: 0,
                tokens: real.tokens(s),
            })
            .collect();
        row(
            &format!("{sessions}, 1792 MiB"),
            &real,
            real.header(),
            &streams,
            1792 << 20,
        );
    }
}
