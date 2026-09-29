//! The MLOS inspector shell.
//!
//! Invariant: input arrives through the queue and commands run from the
//! idle loop, never inside the interrupt handler, which would hold the
//! interrupt active and stop the timer. Design and history:
//! docs/notes/mlsh.md.

#![no_std]

mod acquire;
mod commands;
mod objects;
mod report;

use core::{fmt::Write, sync::atomic::AtomicU32};

use mlos_hal::BootInfo;

pub use mlos_queue::push;

/// What the shell can report on: everything boot learned, plus a way to
/// read the tick count, which changes.
pub struct Facts<'a> {
    /// The memory map and CPU count.
    pub info: BootInfo<'a>,
    /// Total physical bytes, before anything was reserved.
    pub total: u64,
    /// The kernel image's base and length.
    pub image: (u64, u64),
    /// Console base address.
    pub uart: usize,
    /// Console interrupt.
    pub uart_irq: u32,
    /// Timer interrupt.
    pub timer_irq: u32,
    /// GICv3 distributor and redistributor.
    pub gic: Option<(u64, u64)>,
    /// Which kind of console is in use.
    pub console: &'static str,
    /// The lowest virtio-mmio window and how many slots follow it.
    pub virtio: Option<(usize, usize)>,
    /// How many virtio-mmio slots the device tree describes.
    pub virtio_count: u32,
    /// Reads a free-running counter, and how fast it runs. Fine enough to
    /// time a sweep, which the 2 Hz tick is not.
    pub clock: (fn() -> u64, u32),
    /// `/chosen/bootargs`, verbatim. `mlsh.run=a;b;c` runs those verbs at
    /// boot and takes the rest of the string, so it must come last;
    /// `mlos.rev=<sha>` stamps a layout document's provenance.
    pub bootargs: &'a str,
    /// Ticks so far, borrowed so the count is read when asked for.
    pub ticks: &'a AtomicU32,
    /// What differs by platform beyond addresses and numbers.
    pub platform: Platform,
}

/// What differs by platform beyond addresses and numbers: how `dev`
/// names the timer and interrupt controller, and whether `mem peek` can
/// read. One value, so a kernel that has nothing new to say sets one
/// field to [`Platform::GENERIC`].
#[derive(Clone, Copy)]
pub struct Platform {
    /// What the timer is and what `timer_irq` counts in, as `dev` prints
    /// it before the number: `generic, irq` on aarch64, `lapic, vector` on
    /// x86-64, where the LAPIC timer has a vector and no IRQ line.
    pub timer: &'static str,
    /// The interrupt controller, named, where there is no GIC to describe
    /// by its addresses; empty otherwise.
    pub irqchip: &'static str,
    /// Reads the eight bytes at an address, for `mem peek`, or `None`.
    /// Supplied by the kernel because it is `unsafe` underneath and the
    /// shell is not where `unsafe` lives. An unmapped address faults.
    pub peek: Option<fn(u64) -> u64>,
}

impl Platform {
    /// The aarch64 kernel as it was before these fields existed: the
    /// generic timer by IRQ, the GIC described by its addresses, no peek.
    pub const GENERIC: Self = Self {
        timer: "generic, irq",
        irqchip: "",
        peek: None,
    };
}

/// A line reader and a dispatcher.
#[derive(Default)]
pub struct Shell {
    line: mlos_line::Line,
}

impl Shell {
    /// Writes the prompt.
    pub fn prompt(&self, out: &mut impl Write) {
        let _ = out.write_str("mlsh> ");
    }

    /// Runs until the machine is switched off, which it never is. `idle`
    /// is `wfi`: everything from here begins with an interrupt.
    pub fn run(mut self, out: &mut impl Write, facts: &Facts<'_>, idle: fn()) -> ! {
        let _ = out.write_str("\r\n");
        // Boot-script verbs run before the prompt, echoed as if typed, so
        // a captured console reads like a session.
        for verb in mlos_machine::rest(facts.bootargs, "mlsh.run=").split(';') {
            if verb.is_empty() {
                continue;
            }
            let _ = writeln!(out, "mlsh> {verb}\r");
            commands::dispatch(verb, out, facts);
        }
        self.prompt(out);
        loop {
            self.pump(out, facts);
            idle();
        }
    }

    /// Drains the input queue, echoing and dispatching. Called from the
    /// idle loop, with interrupts enabled.
    pub fn pump(&mut self, out: &mut impl Write, facts: &Facts<'_>) {
        while let Some(byte) = mlos_queue::pop() {
            self.feed(byte, out, facts);
        }
    }

    /// Handles one byte. Echo happens here, not in the driver: only the
    /// line knows whether a backspace has anything to erase.
    fn feed(&mut self, byte: u8, out: &mut impl Write, facts: &Facts<'_>) {
        match byte {
            b'\r' | b'\n' => {
                let _ = out.write_str("\r\n");
                commands::dispatch(self.line.as_str(), out, facts);
                self.line = mlos_line::Line::default();
                self.prompt(out);
            }
            0x7f | 0x08 if self.line.backspace() => {
                let _ = out.write_str("\x08 \x08");
            }
            0x20..=0x7e if self.line.push(byte) => {
                let _ = out.write_str(core::str::from_utf8(&[byte]).unwrap_or(""));
            }
            _ => {}
        }
    }
}
