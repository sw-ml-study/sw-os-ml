//! Is this workload fair -- can the comparison come out either way?
//!
//! The question this step exists to answer, and it must be answered
//! before any policy is compared here. A workload where one policy cannot
//! lose is not a test of that policy; it is a description of the
//! workload.
//!
//! **The risk in this file is tuning until the answer is the one wanted.**
//! `docs/plan.md` names it: do not adjust the weighting until it works.
//! What was actually adjusted, and why it is legitimate, is written down
//! in the crate docs -- a single-session full-prefix decode turned out to
//! be a pure cyclic scan, which no recency policy can win, and multi-
//! session serving is MORE realistic rather than more convenient. The
//! fairness is a consequence of the realism, not the reason for it.
//!
//! What must not be hidden is that the workload has a knob and the answer
//! moves with it. `session_count_is_the_mechanism` measures that
//! directly, and step 006 has to report it.

use mlos_abi::ObjectClass;
use mlos_policy::{Demand, FIFO, LRU};
use mlos_sim::{Model, compare, replay};
use mlos_trace::{Access, Trace};
use mlos_workload::Decode;

/// Reads for each policy at one budget.
fn reads(decode: &Decode, held: &[Access], kib: u64) -> (u64, u64, u64) {
    let trace = Trace {
        header: decode.header(),
        accesses: held,
    };
    let table = compare(&trace, decode, kib * 1024, &[&Demand, &FIFO, &LRU]);
    let of = |name: &str| table.iter().find(|(n, _)| *n == name).expect(name).1.reads;
    (of("demand"), of("fifo"), of("lru"))
}

/// How many of these accesses are KV rather than weights.
fn kv(held: &[Access]) -> usize {
    held.iter()
        .filter(|a| a.object.class() == Some(ObjectClass::KvBlock))
        .count()
}

#[test]
fn the_two_halves_are_comparable_in_size() {
    // If either half dominated, the other would stop influencing the
    // answer and this would be the easy case wearing a longer name.
    let decode = Decode::of(4, 40);
    let held = decode.trace();
    let (cache, weights) = (kv(&held), held.len() - kv(&held));
    println!("weights {weights} accesses, kv {cache} accesses");

    let ratio = weights as f64 / cache as f64;
    assert!(
        (0.5..2.0).contains(&ratio),
        "one half dominates: weights {weights}, kv {cache}"
    );
}

#[test]
fn lru_can_beat_fifo_here() {
    // THE FAIRNESS TEST. On the recorded dense sweep these two were
    // identical and both scored zero: the workload could not tell them
    // apart, so nothing measured on it could mean anything.
    let decode = Decode::of(4, 40);
    let held = decode.trace();
    let (_, fifo, lru) = reads(&decode, &held, 192);
    println!("at 192 KiB: fifo {fifo} reads, lru {lru} reads");
    assert!(
        lru < fifo,
        "LRU must be ABLE to win here or the workload is not fair: \
         fifo {fifo}, lru {lru}"
    );
}

#[test]
fn policy_only_matters_inside_a_band() {
    // The most useful thing measured this step, and step 006 must report
    // it rather than picking one budget. Outside the band every policy is
    // identical and the comparison is empty: below it the cyclic weight
    // sweep misses everything whatever you evict, above it the whole
    // working set fits and nothing is ever evicted at all.
    let decode = Decode::of(4, 40);
    let held = decode.trace();

    let (_, starved_fifo, starved_lru) = reads(&decode, &held, 64);
    assert_eq!(starved_fifo, held.len() as u64, "below the band all miss");
    assert_eq!(starved_lru, starved_fifo, "and no policy can differ");

    let (_, roomy_fifo, roomy_lru) = reads(&decode, &held, 512);
    assert_eq!(roomy_fifo, roomy_lru, "above the band nothing is evicted");
    assert!(roomy_fifo < starved_fifo / 10, "and almost everything hits");

    let (_, fifo, lru) = reads(&decode, &held, 192);
    assert!(lru != fifo, "inside the band the policies differ");
    assert!(
        fifo < starved_fifo && fifo > roomy_fifo,
        "192 KiB is inside it"
    );
}

#[test]
fn session_count_is_the_mechanism() {
    // Where LRU's advantage actually comes from, measured rather than
    // assumed. The first guess was wrong: re-reading a KV prefix every
    // round is itself a cyclic scan, so recency is no better there than
    // it is for weights.
    //
    // What makes recency informative is sessions FINISHING. A session
    // that has stopped holds KV nobody will read again; an active one
    // holds early blocks it reads every round. LRU distinguishes those.
    // FIFO cannot -- it evicts by age, so it throws away an active
    // session's early blocks while a finished session's later blocks
    // survive for being younger.
    let alone = Decode::of(1, 40);
    let (_, one_fifo, one_lru) = reads(&alone, &alone.trace(), 192);
    println!("1 session at 192 KiB: fifo {one_fifo}, lru {one_lru}");

    let many = Decode::of(4, 40);
    let (_, many_fifo, many_lru) = reads(&many, &many.trace(), 192);
    println!("4 sessions at 192 KiB: fifo {many_fifo}, lru {many_lru}");

    // One session is a pure cycle and LRU is structurally pessimal on it.
    assert!(
        one_lru > one_fifo,
        "a lone session is a cycle; LRU must lose"
    );
    // Add sessions that finish at different times and recency starts to
    // mean something.
    assert!(many_lru < many_fifo, "diversity is what LRU can exploit");
}

#[test]
fn demand_refuses_most_of_the_workload() {
    // Its read count is NOT comparable with the others and step 006 must
    // not report it as though it were. Demand minimises reads by refusing
    // to serve, which any policy can do and none should be praised for.
    let decode = Decode::of(4, 40);
    let held = decode.trace();
    let trace = Trace {
        header: decode.header(),
        accesses: &held,
    };
    let out = replay(&trace, &decode, 192 * 1024, &Demand);
    let (_, fifo, lru) = reads(&decode, &held, 192);
    println!(
        "demand: {} reads, {} refused of {} ({} per 1000 hit). fifo {fifo}, lru {lru}",
        out.reads,
        out.refused,
        held.len(),
        out.hit_per_mille()
    );

    assert_eq!(
        out.evicted, 0,
        "demand paging evicts nothing, by definition"
    );
    // It does a thirtieth of the work -- and drops a quarter of the
    // requests to get there. Reporting 384 against 11942 as though they
    // answered the same question would be the most misleading row in any
    // table, so step 006 must report what each policy REFUSED beside what
    // it read.
    assert!(out.reads * 10 < lru, "demand should do far fewer reads");
    assert!(
        out.refused * 10 > held.len() as u64,
        "demand refused only {} of {}",
        out.refused,
        held.len()
    );
    // The policies it is compared against refused nothing at all, which
    // is exactly what makes the read counts incomparable.
    let served = replay(&trace, &decode, 192 * 1024, &LRU);
    assert_eq!(served.refused, 0, "LRU served the whole workload");
}

#[test]
fn every_object_the_trace_names_is_one_the_model_has() {
    // A mismatched pair would show up as a refusal-shaped number that is
    // really a bug. The simulator counts it separately; this makes sure
    // it never has to.
    let decode = Decode::of(4, 8);
    for access in decode.trace() {
        assert!(
            decode.meta(access.object).is_some(),
            "{:#018x} is in the trace and not in the model",
            access.object.0
        );
    }
}
