# mlos-device

The device traits an MLOS platform implements: a console, an interrupt
controller, a timer. HAL layer, above the platform crates. Linked from
`crates/mlos-device/src/lib.rs`, `console.rs`, `irq.rs` and `timer.rs`.

## Why its own crate

Split out of `mlos-hal` because devices and platforms are different kinds
of thing: a console or an interrupt controller is discoverable and
pluggable, while the platform is singular and fixed at boot. The
`sw-checklist` module gate is what forced the question, but the answer
stands on its own; see AGENTS.md, "design the split BEFORE you need it".

## Nothing architecture-specific

Nothing here names a page size, a privilege level, a specific interrupt
controller, or an atomics width. That restriction is what keeps a future
`mlos-hal-riscv64` an additive change rather than a refactor
(`docs/architecture.md` s.9).

## `Console` is one method, and stays one method

A console is the first thing a kernel needs and the last thing it should
spend design budget on: formatting, line discipline and buffering all
belong above this, in code that is not architecture-specific
(`mlos-console`, `mlos-line`). `write` takes `&self` rather than
`&mut self` because the console is shared and may be written from an
interrupt handler; implementations serialise internally. It is infallible
by construction: there is no useful way to report that the console
failed, because reporting it would need the console.

## `Irq` is opaque

On aarch64 an `Irq` is a GIC INTID with SGI/PPI/SPI ranges; on x86-64 it
is an APIC vector. Code above `mlos-hal` gets them from device discovery
and passes them back, and must not do arithmetic on them, because the
arithmetic is different on every controller.

## Claim and complete are separate

The controller needs to know the handler has finished before it will
deliver another interrupt at the same priority. Merging claim and complete
into one call would silently drop nested interrupts.

## The timer's frequency is read, not assumed

The aarch64 generic timer's rate is a board property, and hard-coding it
is a classic way to get a kernel that boots on one machine and hangs on
the next. `Ticks` is meaningless without the `Hertz` read beside it.

## Deadlines are absolute

`set_deadline` takes a tick count, not a duration. A duration has to be
added to "now" by someone, and doing that in the caller races with the
timer advancing between the read and the write.
