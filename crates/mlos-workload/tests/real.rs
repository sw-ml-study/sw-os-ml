//! The decode loop over a real model's shape, and what the policies do on it.
//!
//! The fast tests pin what the sidecar said and what the loop does with
//! it. The ignored one prints the comparison at several budgets, with
//! the wall time the simulator took, because the step that produced this
//! trace was asked whether anyone would wait for it:
//!
//! ```text
//! cargo test -p mlos-workload --test real -- --ignored --nocapture
//! ```

use std::time::Instant;

use mlos_abi::ObjectClass;
use mlos_policy::{Demand, FIFO, LRU, NEXT_USE, Policy};
use mlos_sim::{Model, compare};
use mlos_trace::Trace;
use mlos_workload::{Context, MINICPM, Real, Shape};

/// The checkpoint's own precision.
const F16: u32 = 2;

fn minicpm() -> Shape<'static> {
    Shape::parse(MINICPM, F16).expect("the committed sidecar parses")
}

#[test]
fn the_sidecar_describes_the_checkpoint_it_came_from() {
    let shape = minicpm();
    assert_eq!(shape.name, "minicpm5-1b.spm");
    assert_eq!(
        shape.streams.len(),
        219,
        "219 tensors in the safetensors headers"
    );
    assert_eq!(shape.layers, 24);
    assert_eq!(shape.streams.iter().filter(|s| s.rotating).count(), 169);
    // Seven matrices per layer plus the untied output head.
    assert_eq!(24 * 7 + 1, 169);
    let elements: u64 = shape
        .streams
        .iter()
        .map(|s| u64::from(s.rows) * u64::from(s.cols))
        .sum();
    assert_eq!(
        elements, 1_080_632_832,
        "parameters, as the extractor counted them"
    );
    // Two KV heads of 128 for keys and again for values, at two bytes.
    assert_eq!(shape.kv_block_bytes(), 1024);
}

#[test]
fn streams_outside_any_layer_are_filed_one_past_the_last() {
    let shape = minicpm();
    let head = shape
        .streams
        .iter()
        .find(|s| s.name == "lm_head.weight")
        .expect("the head");
    assert_eq!(head.layer, 24);
    assert!(head.rotating, "an untied head is swept per token");
    let embed = shape
        .streams
        .iter()
        .find(|s| s.name == "model.embed_tokens.weight")
        .expect("the embedding");
    assert_eq!(embed.layer, 24);
    assert!(!embed.rotating, "gathered by token id, so it stays");
    assert_ne!(
        head.tensor, embed.tensor,
        "distinct objects in the same pseudo-layer"
    );
    assert_eq!(head.bytes, 130_560 * 1536 * F16);
}

#[test]
fn a_token_sweeps_the_rotating_streams_in_declared_order_and_reads_its_cache() {
    let shape = minicpm();
    let real = Real::of(&shape, 1, 2, Context::DECODE_ONLY);
    let held = real.trace();
    let resident = shape.streams.iter().filter(|s| !s.rotating).count();
    // Round 0: the resident prefix, then the rotating sweep with one KV
    // block per layer (position 0) after each v_proj.
    let first_token = &held[resident..resident + 169 + 24];
    let mut sweep = shape.streams.iter().filter(|s| s.rotating);
    let mut kv = 0;
    for access in first_token {
        if access.object.class() == Some(ObjectClass::KvBlock) {
            kv += 1;
            continue;
        }
        let stream = sweep.next().expect("a rotating stream still to come");
        assert_eq!(
            access.object,
            Real::object(stream),
            "out of declared order at {}",
            stream.name
        );
    }
    assert_eq!(kv, 24, "one cached position per layer on the first token");
    assert!(
        sweep.next().is_none(),
        "the whole rotating region was swept"
    );
    // Round 1 reads two positions per layer: the cache grew.
    let second = held.len() - resident - (169 + 24);
    assert_eq!(second, 169 + 24 * 2);
}

#[test]
fn every_object_the_trace_names_is_one_the_model_has() {
    let shape = minicpm();
    let real = Real::of(&shape, 2, 3, Context::DECODE_ONLY);
    for access in real.trace() {
        assert!(
            real.meta(access.object).is_some(),
            "{:?} has no size",
            access.object
        );
    }
}

#[test]
fn a_prompt_arrives_already_cached_and_blocks_page_it() {
    let shape = minicpm();
    let paged = Context {
        prefix: 100,
        tokens_per_block: 16,
    };
    let real = Real::of(&shape, 1, 1, paged);
    let held = real.trace();
    let kv = held
        .iter()
        .filter(|a| a.object.class() == Some(ObjectClass::KvBlock))
        .count();
    // 101 tokens cached after the first decode step, in blocks of 16:
    // seven blocks per layer, the last one part-filled.
    assert_eq!(kv, 24 * 7);
    let block = held
        .iter()
        .find(|a| a.object.class() == Some(ObjectClass::KvBlock))
        .expect("a block");
    assert_eq!(real.meta(block.object).map(|m| m.size), Some(16 * 1024));
}

/// One regime of the table: its shape, and the policies on it.
fn regime(shape: &Shape<'_>, sessions: u16, rounds: u16, context: Context, mibs: &[u64]) {
    let real = Real::of(shape, sessions, rounds, context);
    let held = real.trace();
    let kv = held
        .iter()
        .filter(|a| a.object.class() == Some(ObjectClass::KvBlock))
        .count();
    let kv_bytes = u64::from(shape.kv_block_bytes()) * u64::from(context.tokens_per_block);
    let cached: u64 = (0..sessions)
        .map(|s| {
            let tokens = u32::from(context.prefix) + u32::from(real.loop_.length(s));
            u64::from(tokens.div_ceil(u32::from(context.tokens_per_block))) * 24 * kv_bytes
        })
        .sum();
    println!(
        "\n== {sessions} sessions x {rounds} rounds, prefix {} in {}-token blocks: {} accesses ({} weights, {kv} kv); cache peaks near {} MiB",
        context.prefix,
        context.tokens_per_block,
        held.len(),
        held.len() - kv,
        cached >> 20
    );
    let trace = Trace {
        header: real.header(),
        accesses: &held,
    };
    println!(
        "{:>6} {:>13} {:>7} {:>7} {:>8}  vs best  {:>7} {:>6}",
        "MiB", "demand", "fifo", "lru", "next-use", "refused", "secs"
    );
    for &mib in mibs {
        let started = Instant::now();
        let policies = [&Demand as &dyn Policy, &FIFO, &LRU, &NEXT_USE];
        let table = compare(&trace, &real, mib << 20, &policies);
        let of = |n: &str| table.iter().find(|(m, _)| *m == n).expect(n).1;
        let (fifo, lru, next) = (of("fifo").reads, of("lru").reads, of("next-use").reads);
        let demand = format!("{}+{}", of("demand").reads, of("demand").refused);
        let best = fifo.min(lru).max(1);
        let gain = 100 - (next as i64 * 100 / best as i64);
        let refused = of("fifo").refused + of("lru").refused + of("next-use").refused;
        println!(
            "{mib:>6} {demand:>13} {fifo:>7} {lru:>7} {next:>8}  {gain:>+4}%   {refused:>7} {:>6.1}",
            started.elapsed().as_secs_f64()
        );
    }
}

#[test]
#[ignore = "prints a table and its timing; run with --ignored --nocapture"]
fn the_real_shape_table() {
    let shape = minicpm();
    let weights: u64 = shape.streams.iter().map(|s| u64::from(s.bytes)).sum();
    let swept: u64 = shape
        .streams
        .iter()
        .filter(|s| s.rotating)
        .map(|s| u64::from(s.bytes))
        .sum();
    println!(
        "\n# {} at F16: {} MiB of weights, {} MiB of them swept per token in {} streams; {} KiB of KV per token",
        shape.name,
        weights >> 20,
        swept >> 20,
        shape.streams.iter().filter(|s| s.rotating).count(),
        shape.kv_block_bytes() * 24 / 1024
    );
    let budgets = [512u64, 1024, 1536, 1792, 2048, 2560];
    regime(&shape, 4, 40, Context::DECODE_ONLY, &budgets);
    let served = Context {
        prefix: 2048,
        tokens_per_block: 16,
    };
    regime(&shape, 8, 40, served, &budgets);
}
