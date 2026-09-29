# mlos-virtio-console

A virtio console, transmit only. Drivers layer. Linked from
`crates/mlos-virtio-console/src/lib.rs`.

## Why its own crate

Like every other virtio device. `mlos-virtio` is the transport and the
virtqueue; what rides on them is a driver, and keeping the two apart is
what stops the transport crate growing a module every time a device is
added.

## Transmit only

That is what closes requirement N2: MLOS needs to be able to *say*
something under Virtualization.framework before being able to listen there
is worth anything. Receive needs the second queue (queue 0 of port 0) and
an interrupt, and until it exists nothing can be typed at MLOS under
Virtualization.framework; `mlos-console` reports `None` for a read and
`docs/status.md` records the gap.

## Static rings and a synchronous buffer

The rings and the 256-byte output buffer are one `static`: there is no
allocator, and the device reads them by physical address, so they must
not move. Identity-mapped, so the address this code sees is the one the
device is given.

`write` sends a bufferful at a time and waits for the device before the
next chunk is staged. That is what makes one static buffer safe to reuse,
and it is the whole justification for `Rings` being `Sync`: written only
by the boot core, one submission at a time, each waiting for the device.

## CRLF

`fmt::Write` inserts a carriage return before every newline. Terminals
want CRLF and the kernel should not care, so the translation lives in the
driver rather than in every caller that prints a line.
