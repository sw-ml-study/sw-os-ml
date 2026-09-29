# mlos-pl011

The Arm PL011 UART: polled transmit, interrupt-driven receive. Drivers
layer. Linked from `crates/mlos-pl011/src/lib.rs`, `tx.rs` and `rx.rs`.

## Why its own crate

Not a module of `mlos-hal-aarch64`, because a PrimeCell UART is not an
aarch64 thing: it turns up wherever Arm IP does, and a RISC-V board with
one would want exactly this code.

## No initialisation

`Pl011::at` does nothing but record the base address. The loader has
already configured the baud rate and enabled the transmitter, and
reprogramming it during bring-up is a good way to lose the console at
exactly the moment it becomes useful. Nothing verifies that a PL011 lives
at the address either, because the only way to ask is to touch it; the
caller of `at` is asserting it, and every `unsafe` block in the crate
rests on that assertion.

## Split by direction

Transmit and receive have almost nothing in common beyond a base address,
so they are separate modules.

Transmit is polled: printing is not hot, and a kernel that cannot print
until interrupts work cannot report why they do not. The reads of the
flag register are volatile because the device, not the compiler, decides
what a read means; an optimised-away reload of `FR` would spin forever on
a full FIFO.

Receive is interrupt-driven: polling for a keystroke means either burning
a core or missing it. The receive-timeout interrupt (`IMSC` bit 6) is
enabled alongside the receive interrupt because without it a lone
keystroke waits for enough friends to reach the FIFO trigger level, which
for a person typing is forever.

## Errors on receive are discarded

The upper bits of `DR` carry framing and parity errors. They are dropped:
a research kernel on an emulated UART has no better answer than "ignore
it".

## Clearing and draining

Enabling the receive interrupt clears any pending interrupt first. The
FIFO may already hold something from before we were listening, and
unmasking on top of a latched interrupt fires immediately, before there is
a handler to drain it.

Acknowledging an interrupt and draining the FIFO are separate operations
and both are required: clearing without draining re-raises immediately,
and draining without clearing leaves the controller believing the line is
still asserted. The drain loops because a receive timeout can deliver
several bytes at once, and stopping after one leaves the rest latched.

## `drain_echo` is not a line discipline

It has no notion of a line, a buffer or a cursor; it is a bring-up
convenience that proves the interrupt path works. The shell's reader
(`mlos-line`, `mlos-queue`) is the line discipline.

## CRLF

The `Console` impl inserts a carriage return before every newline.
Terminals want CRLF and the kernel should not care, so the translation
lives in the driver.
