//! What each rung of the ladder buys in resident bytes and costs in `Rc`
//! and `Ks`, on the real shape, in the band.
//!
//! ```text
//! cargo test -p mlos-workload --test ladder -- --ignored --nocapture
//! ```
//!
//! Each session is put on a rung before it decodes: its 2,048-token
//! prompt is what the rung degrades, the tokens it then generates are
//! hot. The trace reads what the rung leaves -- the summary for the
//! span, half the candidates, the hot window alone -- at the size and
//! precision the rung leaves it.

use std::time::Instant;

use mlos_abi::{ObjectClass, ObjectId};
use mlos_ladder::{Fate, Rung};
use mlos_objtab::{ObjectMeta, SessionId};
use mlos_policy::NEXT_USE;
use mlos_sched::{Lane, ParameterMajor, merge};
use mlos_sim::{Model, replay};
use mlos_trace::Trace;
use mlos_workload::{Context, MINICPM, Real, Shape};

const RUNGS: [Rung; 6] = [Rung::L0, Rung::L1, Rung::L2, Rung::L3, Rung::L4, Rung::L5];
const PROMPT: Context = Context {
    prefix: 2048,
    tokens_per_block: 16,
};
/// Sixteen-token blocks, the newest eight (128 tokens) hot.
const LADDER: mlos_ladder::Shape = mlos_ladder::Shape {
    per_block: 16,
    hot: 8,
};

/// The real shape with every session's prompt on `rung`.
struct Degraded<'a> {
    real: &'a Real<'a>,
    rung: Rung,
}

impl Degraded<'_> {
    /// What the rung makes of `id`, if it is a prompt block.
    fn fate(&self, id: ObjectId) -> Option<Fate> {
        let blocks = LADDER.blocks(u32::from(PROMPT.prefix));
        let at = u32::from(id.fields().tensor);
        (id.class() == Some(ObjectClass::KvBlock) && at < blocks)
            .then(|| LADDER.fate(self.rung, at, blocks))
    }
}

impl Model for Degraded<'_> {
    fn meta(&self, id: ObjectId) -> Option<ObjectMeta> {
        let mut meta = self.real.meta(id)?;
        if let Some(fate) = self.fate(id) {
            meta.size = u32::try_from(fate.bytes(u64::from(meta.size))).ok()?;
            meta.precision = fate.precision()?;
        }
        Some(meta)
    }
}

/// Bytes read to take one session's prompt from L0 down to `rung`, one
/// rung at a time, as the walk does.
fn recomputed(rung: Rung, kv: u64) -> u64 {
    let context = u32::from(PROMPT.prefix);
    RUNGS
        .windows(2)
        .take_while(|w| w[1] <= rung)
        .map(|w| LADDER.cost(w[0], w[1], context, kv))
        .sum()
}

/// One row: the rung's bytes, `Rc`, and a replay under next-use.
fn row(real: &Real<'_>, rung: Rung, budget: u64) {
    let started = Instant::now();
    let model = Degraded { real, rung };
    let lanes: Vec<Lane> = (0..real.loop_.sessions)
        .map(|s| Lane {
            session: SessionId(s + 1),
            ceiling: 0,
            tokens: real
                .tokens(s)
                .into_iter()
                .map(|t| {
                    t.into_iter()
                        .filter(|id| model.fate(*id) != Some(Fate::Dropped))
                        .collect()
                })
                .collect(),
        })
        .collect();
    let tokens: u64 = (0..real.loop_.sessions)
        .map(|s| u64::from(real.loop_.length(s)))
        .sum();
    let accesses = merge(&mut ParameterMajor, &lanes);
    let trace = Trace {
        header: real.header(),
        accesses: &accesses,
    };
    let out = replay(&trace, &model, budget, &NEXT_USE);
    let kv = u64::from(real.shape.kv_block_bytes()) * u64::from(real.shape.layers);
    let held = LADDER.need(rung, u32::from(PROMPT.prefix), kv);
    let rc = recomputed(rung, kv) * u64::from(real.loop_.sessions) / tokens.max(1);
    println!(
        "| {rung} | {:.1} MiB | {} | {} | {:.0} MiB | {} | {rc} B | {:.1} MiB | {:.1} s |",
        held as f64 / f64::from(1 << 20),
        accesses.len(),
        out.reads,
        out.bytes as f64 / f64::from(1 << 20),
        out.refused,
        out.headline(budget).ks as f64 / f64::from(1 << 20),
        started.elapsed().as_secs_f64()
    );
}

#[test]
#[ignore = "prints the ladder table; run with --ignored --nocapture"]
fn what_each_rung_buys_on_the_real_shape() {
    let shape = Shape::parse(MINICPM, 2).expect("sidecar");
    let real = Real::of(&shape, 4, 40, PROMPT);
    let budget = 1792u64 << 20;
    println!(
        "\n### MiniCPM5-1B, 4 sessions, 2,048-token prompts, 40 rounds, 1792 MiB, next-use, parameter-major\n"
    );
    println!(
        "| rung | prompt KV per session | accesses | reads | bytes read | refused | Rc per token | Ks at end | time |"
    );
    println!("| --- | --- | --- | --- | --- | --- | --- | --- | --- |");
    for rung in RUNGS {
        row(&real, rung, budget);
    }
}

#[test]
#[ignore = "prints the walk table; run with --ignored --nocapture"]
fn where_the_walk_stops_as_the_room_shrinks() {
    let shape = Shape::parse(MINICPM, 2).expect("sidecar");
    let kv = u64::from(shape.kv_block_bytes()) * u64::from(shape.layers);
    let context = u32::from(PROMPT.prefix);
    println!("\n### Four sessions of 2,048 tokens, floors anything-goes: where the walk stops\n");
    println!("| KV room | rung | held | freed against L0 |");
    println!("| --- | --- | --- | --- |");
    for mib in [256u64, 192, 128, 96, 64, 48, 32, 24, 16] {
        let room = mib << 20;
        let fits = RUNGS
            .into_iter()
            .find(|r| 4 * LADDER.need(*r, context, kv) <= room);
        let held = fits.map_or(0, |r| 4 * LADDER.need(r, context, kv));
        let full = 4 * LADDER.need(Rung::L0, context, kv);
        let name = fits.map_or("L7 (terminate)".to_string(), |r| r.to_string());
        println!(
            "| {mib} MiB | {name} | {:.1} MiB | {:.1} MiB |",
            held as f64 / f64::from(1 << 20),
            full.saturating_sub(held) as f64 / f64::from(1 << 20)
        );
    }
}
