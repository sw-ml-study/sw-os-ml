# mlos-uart16550

The 16550 UART reached through x86 port I/O: the x86-64 guest's console,
COM1 on QEMU `microvm`. Driver layer, the PL011's counterpart. Linked from
`crates/mlos-uart16550/src/lib.rs`, `port.rs`, `rx.rs` and `tx.rs`.

## Why its own crate

COM1 at `0x3f8` is where every PC has had its first serial port since the
8250 the 16550 descends from, and it is what `microvm` wires the guest's
serial console to. The driver is shaped like the PL011's, polled
transmit in `tx`, receive in `rx`, so that `mlos-console` can later hold
either behind one `Terminal`.

Port I/O is what makes this crate x86-specific: `in` and `out` exist only
on x86, so the crate is empty anywhere but a bare x86-64 target, and a
Linux x86-64 host must not pick it up either. A 16550 behind MMIO, as on
some Arm and RISC-V boards, would be a second access method in this crate,
not a second driver.

## Safe port wrappers

`port::read` and `port::write` are safe functions wrapping `unsafe`
instructions, where `mlos-hal-x86-64`'s `inb`/`outb` are `unsafe fn`. The
difference is who is handing over the port number. The HAL's callers can
name any port, and a port write can do anything the hardware behind it
does; this crate's callers are the driver's own methods, which only ever
compute `base + offset` for a UART register, and reading or writing one
affects nothing but the UART. That is the invariant every `SAFETY:` in
`port.rs` relies on, and it holds because `port` is private.

## `init`, and what it does not touch

The PL011 driver can assume the loader configured the UART; this one
cannot. PVH has no firmware, so nothing set a baud rate before the kernel,
and `init` sets 115200 8N1 with interrupts off.

The FIFO control register is left alone on purpose. Enabling or resetting
the FIFO discards whatever has already arrived, and input that was sent
before the kernel looked, a test's scripted input or a fast typist, is
exactly what must not be lost. Without the FIFO the 16550 holds one byte
and QEMU flow-controls the rest, which is enough.

`init` leaves `OUT2` clear in the modem control register. On a PC `OUT2`
gates the UART's interrupt onto the ISA line, and until the interrupt
controllers are armed there is nowhere for it to go.
`enable_receive_interrupt` sets `OUT2` along with the receive-data
interrupt enable, keeping DTR and RTS as `init` left them.

## Polled transmit

Transmit is polled, like the PL011's, and for the same reason: a kernel
that cannot print until interrupts work cannot say why they do not.
Newlines go out as CRLF because terminals want it, and the kernel should
not have to care.

## Receive

`read` takes one byte if the line status register says one is waiting.
`enable_receive_interrupt` makes each arrival raise IRQ 4, COM1's line by
PC convention, which the kernel routes through the IOAPIC so it can sleep
until a key is typed. A byte already waiting raises the interrupt as soon
as it is enabled, so input typed before the kernel was listening is not
stranded in the holding register.
