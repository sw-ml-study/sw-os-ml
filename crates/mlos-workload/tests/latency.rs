//! What lockstep costs a session in waiting, and what the escape costs
//! everyone in reads.
//!
//! ```text
//! cargo test -p mlos-workload --test latency -- --ignored --nocapture
//! ```

use std::collections::BTreeMap;

use mlos_objtab::SessionId;
use mlos_policy::NEXT_USE;
use mlos_sched::{Lane, ParameterMajor, ProcessMajor, Schedule, merge};
use mlos_sim::replay;
use mlos_trace::{Access, Trace};
use mlos_workload::{Context, MINICPM, Real, Shape};

/// Per session: the mean and worst period of its tokens, in acquires
/// from the first access of one token to the first access of the next
/// (or the end of the trace for the last). What a user of that session
/// waits for each token.
fn periods(trace: &[Access], lanes: &[Lane]) -> BTreeMap<u16, (u32, u32)> {
    let mut out = BTreeMap::new();
    for lane in lanes {
        let s = lane.session.0;
        let (mut starts, mut last) = (Vec::new(), 0usize);
        let (mut seen, mut token) = (0usize, 0usize);
        for (at, _) in trace.iter().enumerate().filter(|(_, a)| a.session.0 == s) {
            if seen == 0 {
                starts.push(at);
            }
            seen += 1;
            last = at;
            if seen == lane.tokens.get(token).map_or(usize::MAX, Vec::len) {
                seen = 0;
                token += 1;
            }
        }
        starts.push(last + 1); // the last token ends with its last access
        let spans: Vec<u32> = starts.windows(2).map(|w| (w[1] - w[0]) as u32).collect();
        let mean = spans.iter().sum::<u32>() / spans.len().max(1) as u32;
        out.insert(s, (mean, spans.iter().copied().max().unwrap_or(0)));
    }
    out
}

fn lanes(real: &Real<'_>, ceiling_for_one: u32) -> Vec<Lane> {
    (0..real.loop_.sessions)
        .map(|s| Lane {
            session: SessionId(s + 1),
            ceiling: if s == 0 { ceiling_for_one } else { 0 },
            tokens: real.tokens(s),
        })
        .collect()
}

#[test]
#[ignore = "prints the latency table; run with --ignored --nocapture"]
fn the_cost_of_lockstep_and_the_price_of_leaving_it() {
    let shape = Shape::parse(MINICPM, 2).expect("sidecar");
    let real = Real::of(&shape, 4, 40, Context::DECODE_ONLY);
    let budget = 1536u64 << 20;
    println!("\n### MiniCPM5-1B, 4 sessions x 40 rounds, 1536 MiB, next-use\n");
    println!(
        "| schedule | session 1 ceiling | reads | session 1 period: mean / worst | session 4 period: mean / worst |"
    );
    println!("| --- | --- | --- | --- | --- |");
    let mut rows: Vec<(&str, u32, Box<dyn Schedule>)> =
        vec![("process-major", 0, Box::new(ProcessMajor::default()))];
    for ceiling in [0u32, 2048, 1024, 512, 256, 128, 32] {
        rows.push(("parameter-major", ceiling, Box::new(ParameterMajor)));
    }
    for (name, ceiling, mut schedule) in rows {
        let held = lanes(&real, ceiling);
        let merged = merge(schedule.as_mut(), &held);
        let trace = Trace {
            header: real.header(),
            accesses: &merged,
        };
        let reads = replay(&trace, &real, budget, &NEXT_USE).reads;
        let w = periods(&merged, &held);
        let (s1, s4) = (w[&1], w[&4]);
        let ceiling = if ceiling == 0 {
            "none".to_string()
        } else {
            ceiling.to_string()
        };
        println!(
            "| {name} | {ceiling} | {reads} | {} / {} | {} / {} |",
            s1.0, s1.1, s4.0, s4.1
        );
    }
}

#[test]
fn a_ceiling_bounds_the_period_of_the_session_that_has_one() {
    let shape = Shape::parse(MINICPM, 2).expect("sidecar");
    let real = Real::of(&shape, 4, 8, Context::DECODE_ONLY);
    let own = real.tokens(0)[0].len() as u32;
    let free = merge(&mut ParameterMajor, &lanes(&real, 0));
    let capped = merge(&mut ParameterMajor, &lanes(&real, 128));
    let (_, free_worst) = periods(&free, &lanes(&real, 0))[&1];
    let (_, capped_worst) = periods(&capped, &lanes(&real, 128))[&1];
    assert!(
        free_worst > 3 * own,
        "lockstep makes session 1 wait for three others: {free_worst}"
    );
    // A ceiling bounds the wait before a token and the wait within it
    // separately, so a period is at most the token plus twice the ceiling.
    assert!(
        capped_worst <= own + 2 * 128,
        "the ceiling holds: {capped_worst} vs {own} + 256"
    );
    assert_eq!(free.len(), capped.len(), "every access is still served");
}
