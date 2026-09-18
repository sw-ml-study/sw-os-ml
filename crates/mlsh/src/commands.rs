//! The verbs.
//!
//! Deliberately few, and deliberately about the state MLOS actually has.
//! `mlsh` is not a Unix shell: there is no filesystem to navigate and no
//! processes to list. It is an inspector, and it grows a verb when the
//! kernel grows something worth inspecting -- the object table, at M2.

use core::{fmt::Write, sync::atomic::Ordering};

use crate::Facts;

/// Runs one line.
pub fn dispatch(line: &str, out: &mut impl Write, facts: &Facts<'_>) {
    let (verb, args) = line.trim().split_once(' ').unwrap_or((line.trim(), ""));
    if NEEDS_MODEL.contains(&verb) && mlos_lab::with(|_| ()).is_none() {
        let _ = writeln!(out, "  no model registered (try `model`)");
        return;
    }
    match verb {
        "" => {}
        "help" | "?" => _ = out.write_str(HELP),
        "mem" => mem(out, facts),
        "dev" => dev(out, facts),
        "sweep" => crate::objects::sweep(out, facts.clock),
        "arena" => crate::report::arena(out),
        "faults" => crate::report::faults(out),
        "layout" => _ = mlos_snapshot::write_for(out, facts.bootargs),
        "stream" => crate::objects::stream(out, args),
        "trace" => _ = mlos_lab::with(|held| mlos_events::verb(out, &mut held.events, args)),
        "model" => crate::objects::model(out, args),
        "get" => crate::acquire::get(out, args),
        "objs" => crate::report::objs(out, args),
        "ticks" => ticks(out, facts),
        other => _ = writeln!(out, "no such command: {other}   (try `help`)"),
    }
}

/// Timer ticks since boot.
fn ticks(out: &mut impl Write, facts: &Facts<'_>) {
    let ticks = facts.ticks.load(Ordering::Relaxed);
    let _ = writeln!(out, "{ticks} timer ticks since boot");
}

/// Verbs that need a model to already exist.
///
/// A constant, so the check is one line in `dispatch` and the apology
/// lives in one place -- three copies of it is three places to change.
const NEEDS_MODEL: [&str; 8] = [
    "sweep", "get", "objs", "arena", "faults", "layout", "trace", "stream",
];

/// What `help` prints.
const HELP: &str = concat!(
    "model [KiB]   register the model; optionally set the arena budget\r\n",
    "sweep         acquire every tile in order, faulting them in\r\n",
    "get L T       acquire one tile, and say if it had to fault\r\n",
    "objs [all]    what the table knows: tier, residency, use count\r\n",
    "arena         how full memory is, and what would still fit\r\n",
    "faults        what it all cost, per object class\r\n",
    "layout        the running layout as JSON, for the visualizer\r\n",
    "trace [on|off] residency events as they happened, one per line\r\n",
    "stream [N]    declare the model's access order, or advance it by N\r\n",
    "mem           physical memory map, and what is left\r\n",
    "dev           console, timer and interrupt controller\r\n",
    "ticks         timer ticks since boot\r\n",
    "help          this\r\n",
);

/// The physical memory map.
///
/// The two non-usable entries are the point: the kernel image and the
/// device tree blob are memory the machine has and MLOS may not hand out.
/// Everything the object manager will ever do starts from this number
/// being honest.
fn mem(out: &mut impl Write, facts: &Facts<'_>) {
    for region in facts.info.regions {
        let (base, len, kind) = (region.base, region.len, region.kind);
        let _ = writeln!(out, "  {base:#012x} + {len:#x} {kind:?}");
    }
    let (base, len) = facts.image;
    let _ = writeln!(out, "  image  {base:#x} + {len:#x}");
    let _ = writeln!(
        out,
        "  usable {} MiB of {} MiB, {} cpu(s)",
        facts.info.usable_bytes() >> 20,
        facts.total >> 20,
        facts.info.cpu_count
    );
}

/// What the device tree said about the devices in use.
fn dev(out: &mut impl Write, facts: &Facts<'_>) {
    let _ = writeln!(
        out,
        "  console  pl011 @ {:#x}, irq {}",
        facts.uart, facts.uart_irq
    );
    let _ = writeln!(out, "  timer    generic, irq {}", facts.timer_irq);
    match facts.gic {
        Some((dist, redist)) => {
            let _ = writeln!(out, "  gic      v3, dist {dist:#x}, redist {redist:#x}");
        }
        None => {
            let _ = out.write_str("  gic      none\r\n");
        }
    }
}
