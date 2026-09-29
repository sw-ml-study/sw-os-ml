# mlos-virtio-blk

A virtio block device: read-only, one sector per request. Drivers layer.
Linked from `crates/mlos-virtio-blk/src/lib.rs`, `range.rs` and
`request.rs`.

## Why it exists

This is what turns "three tiers" from a claim into a fact: with it, a cold
object's bytes genuinely live somewhere other than memory and have to
cross a queue to be used. It is its own crate rather than a module of
`mlos-virtio` for the reason given in `docs/notes/mlos-virtio.md`: the
transport is shared, and each device that rides on it is a driver.

## Why read-only

MLOS treats a block store as where weights *come from*, and weights are
immutable, the one property that makes an object shareable across every
session using the model. A tier that can be written to is a tier that has
to be invalidated, and nothing here wants that yet.

## Static rings and buffers

The rings, the request header, the sector buffer and the status byte are
one `static`. The device reads them by physical address, so they must not
move, and there is no allocator to promise that any other way. `Rings` is
`Sync` because there is one request at a time, on the boot core, each
waiting for the device before the next begins; that same discipline is
what makes reusing the buffers sound.

## One sector per request

`read_sector` moves exactly one sector because the static buffer is one
sector, and a bigger buffer would only move the limit rather than remove
it. A caller wanting more asks more than once, which is what `read_at`
does: it turns a byte range into a sequence of sector reads. `range.rs` is
separate because that is arithmetic over a device, not a device
operation, and has no business knowing how any single read works.

`read_at` accepts sector-aligned offsets only, which every caller has:
objects are placed on sector boundaries precisely so the driver never has
to read-modify-return a partial sector.

## Three descriptors, and which are writable

A read is a header the device reads, a buffer it writes, and a status
byte it writes to say how it went. That is the shape virtio uses
everywhere and the reason the queue supports chaining. Merging them would
be smaller and wrong: the device must not be able to write into the
header that told it what to do, so the header descriptor carries no
`WRITE` while the data and status descriptors do.

## Sectors are always 512 bytes

Virtio fixes the sector at 512 bytes whatever the underlying device's
block size, so a 4 KiB-sector disk still counts in 512s. A driver that
assumed otherwise would read the wrong place on real hardware while
working perfectly against a file.

## The status byte

The status byte is primed to `0xff` before every request, not zero: zero
is `Ok`, so a device that never wrote the status byte would look like one
that succeeded. Any value the specification does not define is treated as
`Error` rather than a panic: a device that answers something undefined
has failed, whatever it meant.
