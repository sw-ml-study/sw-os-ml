//! The verbs that make the object manager do something.
//!
//! Separate from `report`, which only looks. The distinction matters more
//! here than it usually would: `sweep` and `get` change residency, so
//! running one changes what the other reports, and knowing which is which
//! is the difference between exploring a system and disturbing it.

use core::fmt::Write;

/// Registers the synthetic model, optionally with a different budget.
///
/// `model` for the default, `model 8` for eight kibibytes -- which is the
/// knob worth having, because the whole subject is what happens when
/// memory is smaller than the model.
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

/// Sweeps the model, faulting every tile in.
///
/// Timed, because a sweep is the only workload MLOS has and the elapsed
/// figure is how every later claim about overhead gets checked. The
/// generic timer runs at tens of megahertz, so a sweep is thousands of
/// counts rather than the zero the 2 Hz tick would report.
///
/// Fixed point to the nanosecond, not rounded microseconds. A sweep that
/// faults is milliseconds and a sweep that only hits is a few
/// microseconds, and a unit that reads the first one well throws the
/// second one away -- which is exactly the sweep that can measure what
/// anything on the fault path costs.
///
/// Stopping early is not a failure. The arena is smaller than the model,
/// nothing evicts yet, and running out is the honest outcome -- it is the
/// problem M3 exists to solve, and how far the sweep got is the number
/// that will be compared.
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

/// Declares the model's sweep, or moves it on.
///
/// `stream` declares; `stream N` advances by N. The verb exists so the
/// one field no page-based system can hold -- when an object is next
/// wanted -- can be watched being written, in `objs` and in the layout
/// document, by something other than a test.
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
