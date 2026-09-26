//! What the simulator says the kernel should see.
#[test]
#[ignore = "prints the expected counts"]
fn expected() {
    use mlos_policy::{Demand, FIFO, LRU, NEXT_USE, Policy};
    use mlos_sim::{Outcome, compare};
    use mlos_trace::Trace;
    use mlos_workload::Decode;

    let (sessions, rounds) = (2u16, 16u16);
    let decode = Decode::of(sessions, rounds);
    let held = decode.trace();
    println!("trace: {} accesses", held.len());
    let trace = Trace {
        header: decode.header(),
        accesses: &held,
    };
    for kib in [16u64, 24, 32, 48] {
        let policies = [&Demand as &dyn Policy, &FIFO, &LRU, &NEXT_USE];
        for (name, out) in compare(&trace, &decode, kib * 1024, &policies) {
            let Outcome {
                reads,
                hits,
                bytes,
                evicted,
                refused,
                ..
            } = out;
            println!(
                "{kib:>3} KiB {name:>9}: {reads} reads, {hits} hits, {bytes} bytes, {evicted} evicted, {refused} refused"
            );
        }
    }
}
