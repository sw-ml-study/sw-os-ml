//! The verbs that make the object manager do something.
//!
//! Invariant: nothing in `report` changes residency; everything here may.
//! Design: docs/notes/mlsh.md.

use core::fmt::Write;

/// Registers the synthetic model, optionally with a different budget:
/// `model` for the default, `model 8` for eight kibibytes.
pub fn model(out: &mut impl Write, args: &str) {
    let budget = args
        .split_whitespace()
        .next()
        .and_then(|kib| kib.parse::<usize>().ok())
        .map_or(mlos_lab::ARENA_BYTES, |kib| kib * 1024);

    match mlos_lab::register(budget) {
        Ok((objects, bytes)) => registered(out, objects, bytes, budget),
        Err(error) => {
            let _ = writeln!(out, "  could not register: {error:?}");
        }
    }
}

/// What was registered, and where its bytes will come from.
fn registered(out: &mut impl Write, objects: u32, bytes: u64, budget: usize) {
    let _ = writeln!(
        out,
        "  registered {objects} objects, {} KiB across 3 tiers",
        bytes >> 10
    );
    let _ = writeln!(out, "  budget     {} KiB of arena", budget >> 10);
    let source = if mlos_lab::on_disk() {
        "virtio-blk"
    } else {
        "a pattern (no disk attached)"
    };
    let _ = writeln!(out, "  weights    from {source}");
}

/// Sweeps the model, faulting every tile in, and reports the elapsed
/// time to the nanosecond. Stopping early is not a failure; how far it
/// got is the number.
pub fn sweep(out: &mut impl Write, clock: (fn() -> u64, u32)) {
    let (now, hz) = clock;
    let started = now();
    let swept = mlos_lab::sweep(1);
    let ticks = now().saturating_sub(started);
    let ns = ticks.saturating_mul(1_000_000_000) / u64::from(hz).max(1);
    let _ = writeln!(
        out,
        "  acquired {} of {} tiles in {}.{:03} us",
        swept.acquired,
        swept.total,
        ns / 1000,
        ns % 1000
    );
    match swept.stopped {
        Some(error) => {
            let _ = writeln!(out, "  stopped: {error:?} -- no eviction policy yet");
        }
        None => {
            let _ = writeln!(out, "  swept the whole model");
        }
    }
}

/// Replays the recorded workload under one policy, and says what it cost.
/// The counts must match `mlos-sim`'s for the same trace and budget
/// exactly: no tolerance, no rounding.
pub fn replay(out: &mut impl Write, args: &str) {
    let name = args.split_whitespace().next().unwrap_or("demand");
    if !mlos_lab::choose(name) {
        let _ = writeln!(
            out,
            "  no such policy: {name} (demand, fifo, lru, next-use)"
        );
        return;
    }
    match mlos_lab::replay() {
        Ok(done) => {
            let _ = writeln!(
                out,
                "  {name}: {} reads, {} hits, {} bytes, {} evicted, {} refused",
                done.reads, done.hits, done.bytes, done.evicted, done.refused
            );
        }
        Err(why) => drop(writeln!(out, "  could not replay: {why:?}")),
    }
}

/// Declares the model's sweep (`stream`), or advances it by N
/// (`stream N`).
pub fn stream(out: &mut impl Write, args: &str) {
    match args.split_whitespace().next().and_then(|n| n.parse().ok()) {
        Some(steps) => match mlos_lab::advance(steps) {
            Some(at) => drop(writeln!(out, "  advanced to position {at}")),
            None => drop(writeln!(out, "  no model registered (try `model`)")),
        },
        None => match mlos_lab::declare() {
            Some(count) => drop(writeln!(out, "  declared {count} objects, cursor at 0")),
            None => drop(writeln!(out, "  could not declare a stream")),
        },
    }
}
