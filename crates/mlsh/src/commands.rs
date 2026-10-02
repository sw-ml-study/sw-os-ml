//! The verbs.
//!
//! Invariant: a verb that needs a model is refused in `dispatch`, in one
//! place, before it runs. Design: docs/notes/mlsh.md.

use core::{fmt::Write, sync::atomic::Ordering};

use crate::Facts;

/// Runs one line.
pub fn dispatch(line: &str, out: &mut impl Write, facts: &Facts<'_>) {
    let (verb, args) = line.trim().split_once(' ').unwrap_or((line.trim(), ""));
    let ready = !NEEDS_MODEL.contains(&verb) || mlos_lab::with(|_| ()).is_some();
    match verb {
        _ if !ready => _ = writeln!(out, "  no model registered (try `model`)"),
        "" => {}
        "help" | "?" => _ = out.write_str(HELP),
        "mem" => mem(out, facts, args),
        "dev" => dev(out, facts),
        "sweep" => crate::objects::sweep(out, facts.clock),
        "arena" => crate::report::arena(out),
        "faults" => crate::report::faults(out),
        "metrics" => crate::report::metrics(out),
        "layout" => _ = mlos_snapshot::write_for(out, facts.bootargs),
        "stream" => crate::objects::stream(out, args),
        "replay" => crate::objects::replay(out, args),
        "session" => crate::session::session(out, args),
        "trace" => _ = mlos_lab::with(|held| mlos_events::verb(out, &mut held.events, args)),
        "model" => crate::objects::model(out, args),
        "get" => crate::acquire::get(out, args),
        "evict" | "release" => crate::acquire::let_go(out, args, verb == "evict"),
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

/// Verbs that need a model to already exist; `dispatch` checks them once.
const NEEDS_MODEL: [&str; 13] = [
    "sweep", "get", "evict", "objs", "arena", "faults", "layout", "trace", "stream", "replay",
    "session", "release", "metrics",
];

/// What `help` prints.
const HELP: &str = concat!(
    "model [KiB]   register the model; optionally set the arena budget\r\n",
    "sweep         acquire every tile in order, faulting them in\r\n",
    "get L T       acquire one tile, and say if it had to fault\r\n",
    "evict L T     throw one tile out, returning its bytes to the arena\r\n",
    "release L T   let go of the pin `get` took; shared objects are dearer to evict\r\n",
    "objs [all]    what the table knows: tier, residency, use count\r\n",
    "arena         how full memory is, and what would still fit\r\n",
    "faults        what it all cost, per object class\r\n",
    "metrics       Rm, Ps, Ks, Ss: the numbers docs/PRD.md says matter\r\n",
    "layout        the running layout as JSON, for the visualizer\r\n",
    "trace [on|off] residency events as they happened, one per line\r\n",
    "stream [N]    declare the model's access order, or advance it by N\r\n",
    "replay POLICY replay the recorded workload under one policy\r\n",
    "session [new [KIB] [WAIT] | end ID]  list, create (ceilings: KiB, acquires), or end\r\n",
    "mem           physical memory map, and what is left\r\n",
    "mem peek ADDR read 8 bytes at ADDR; unmapped faults, on purpose\r\n",
    "dev           console, timer and interrupt controller\r\n",
    "ticks         timer ticks since boot\r\n",
    "help          this\r\n",
);

/// The physical memory map, including the reserved entries MLOS may not
/// hand out. `mem peek ADDR` reads eight bytes at ADDR instead; an
/// unmapped address faults, on purpose.
fn mem(out: &mut impl Write, facts: &Facts<'_>, args: &str) {
    if let Some(addr) = args.strip_prefix("peek") {
        let addr = u64::from_str_radix(addr.trim().trim_start_matches("0x"), 16);
        let _ = match (facts.platform.peek, addr) {
            (Some(peek), Ok(addr)) => writeln!(out, "  {addr:#x}: {:#018x}", peek(addr)),
            (None, _) => writeln!(out, "  mem peek: not provided on this machine"),
            (_, Err(_)) => writeln!(out, "  mem peek ADDR   (ADDR in hex)"),
        };
        return;
    }
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

/// The devices in use, as `Facts` describes them. A timer the platform
/// has not described is reported as absent, not as irq 0.
fn dev(out: &mut impl Write, facts: &Facts<'_>) {
    let (kind, uart, irq) = (facts.console, facts.uart, facts.uart_irq);
    let _ = writeln!(out, "  console  {kind} @ {uart:#x}, irq {irq}");
    let _ = match facts.timer_irq {
        0 => writeln!(out, "  timer    none"),
        irq => writeln!(out, "  timer    {} {irq}", facts.platform.timer),
    };
    let _ = match (facts.gic, facts.platform.irqchip) {
        (Some((dist, redist)), _) => {
            writeln!(out, "  gic      v3, dist {dist:#x}, redist {redist:#x}")
        }
        (None, "") => writeln!(out, "  gic      none"),
        (None, chip) => writeln!(out, "  irqchip  {chip}"),
    };
}
