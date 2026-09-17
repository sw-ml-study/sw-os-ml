//! Where the policies actually differ, printed as a table.
//!
//! Not an assertion -- a tool. `#[ignore]`d because it measures rather
//! than checks, and run on demand:
//!
//! ```text
//! cargo test -p mlos-workload --test sweep -- --ignored --nocapture
//! ```
//!
//! It exists because "which policy is better" turned out to be the wrong
//! question. Outside a band of budgets every policy is identical: below
//! it the cyclic weight sweep misses everything whatever is evicted,
//! above it the working set fits and nothing is evicted at all. Only the
//! band between measures anything, and step 006's verdict has to report
//! the shape of it rather than one row from the middle.

#[test]
#[ignore = "prints a table; run with --ignored --nocapture"]
fn budget_sweep() {
    use mlos_policy::{Demand, FIFO, LRU, NEXT_USE, Policy};
    use mlos_sim::compare;
    use mlos_trace::Trace;
    use mlos_workload::Decode;

    for (sessions, rounds) in [(1u16, 40u16), (2, 40), (4, 40), (8, 40)] {
        let decode = Decode::of(sessions, rounds);
        let held = decode.trace();
        let kv = held
            .iter()
            .filter(|a| a.object.class() == Some(mlos_abi::ObjectClass::KvBlock))
            .count();
        println!(
            "\n== {sessions} sessions, {rounds} rounds: {} accesses ({} weights, {kv} kv)",
            held.len(),
            held.len() - kv
        );
        let trace = Trace {
            header: decode.header(),
            accesses: &held,
        };
        println!(
            "{:>6} {:>11} {:>8} {:>8} {:>9}  vs best baseline",
            "KiB", "demand", "fifo", "lru", "next-use"
        );
        for kib in [32u64, 64, 96, 128, 160, 192, 256, 384, 512] {
            let policies = [&Demand as &dyn Policy, &FIFO, &LRU, &NEXT_USE];
            let table = compare(&trace, &decode, kib * 1024, &policies);
            let of = |n: &str| table.iter().find(|(m, _)| *m == n).expect(n).1;
            let (fifo, lru, next) = (of("fifo").reads, of("lru").reads, of("next-use").reads);
            // Demand is printed as reads+refusals, because its read count
            // is bought by not serving the workload and is not comparable
            // with policies that served all of it.
            let demand = format!("{}+{}", of("demand").reads, of("demand").refused);
            let best = fifo.min(lru).max(1);
            let gain = 100 - (next as i64 * 100 / best as i64);
            println!("{kib:>6} {demand:>11} {fifo:>8} {lru:>8} {next:>9}  {gain:>+4}%");
        }
    }
}
