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
    match line.trim() {
        "" => {}
        "help" | "?" => help(out),
        "mem" => mem(out, facts),
        "dev" => dev(out, facts),
        "model" => crate::objects::model(out),
        "sweep" => crate::objects::sweep(out),
        "faults" => crate::objects::faults(out),
        "ticks" => {
            let ticks = facts.ticks.load(Ordering::Relaxed);
            let _ = writeln!(out, "{ticks} timer ticks since boot");
        }
        other => {
            let _ = writeln!(out, "no such command: {other}   (try `help`)");
        }
    }
}

/// Lists what there is to ask for.
fn help(out: &mut impl Write) {
    let _ = out.write_str(concat!(
        "model  register the synthetic model across three tiers\r\n",
        "sweep  acquire every tile in order, faulting them in\r\n",
        "faults what that cost, per object class\r\n",
        "mem    physical memory map, and what is left\r\n",
        "dev    console, timer and interrupt controller\r\n",
        "ticks  timer ticks since boot\r\n",
        "help   this\r\n",
    ));
}

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
