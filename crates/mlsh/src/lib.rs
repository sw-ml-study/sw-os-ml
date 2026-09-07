//! The MLOS inspector shell.
//!
//! Closes `docs/PRD.md` gate G1, which asks the proof of concept to reach
//! a shell. Not a Unix shell, and never will be: MLOS has no filesystem to
//! navigate and no processes to list. What it has is a memory map it had
//! to work for, some devices it discovered, and -- at M2 -- an object
//! table. Those are what a shell here is for.
//!
//! Input arrives through [`queue`] rather than directly from the interrupt
//! handler, because running a command inside a handler holds the interrupt
//! active and stops the timer.

#![no_std]

mod commands;
mod line;

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
    /// Ticks so far. Borrowed rather than copied, because it keeps
    /// changing and the shell should report the count at the moment it
    /// was asked, not at the moment boot handed these over.
    pub ticks: &'a AtomicU32,
}

/// A line reader and a dispatcher.
#[derive(Default)]
pub struct Shell {
    line: line::Line,
}

impl Shell {
    /// Writes the prompt.
    pub fn prompt(&self, out: &mut impl Write) {
        let _ = out.write_str("mlsh> ");
    }

    /// Runs until the machine is switched off, which it never is.
    ///
    /// `wfi` rather than a spin: everything that happens from here begins
    /// with an interrupt, so there is nothing to poll for and no reason to
    /// burn a core doing it.
    pub fn run(mut self, out: &mut impl Write, facts: &Facts<'_>, idle: fn()) -> ! {
        let _ = out.write_str("\r\n");
        self.prompt(out);
        loop {
            self.pump(out, facts);
            idle();
        }
    }

    /// Drains the input queue, echoing and dispatching.
    ///
    /// Called from the idle loop, so everything it does happens with
    /// interrupts enabled and the timer still running.
    pub fn pump(&mut self, out: &mut impl Write, facts: &Facts<'_>) {
        while let Some(byte) = mlos_queue::pop() {
            self.feed(byte, out, facts);
        }
    }

    /// Handles one byte.
    ///
    /// Echo happens here rather than in the driver because echo is a
    /// property of the line being edited: a backspace has to erase a
    /// character the terminal already drew, and only something that knows
    /// whether the line is empty can decide whether to.
    fn feed(&mut self, byte: u8, out: &mut impl Write, facts: &Facts<'_>) {
        match byte {
            b'\r' | b'\n' => {
                let _ = out.write_str("\r\n");
                commands::dispatch(self.line.as_str(), out, facts);
                self.line = line::Line::default();
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
