# mlos-virtio

virtio over MMIO: the transport and the split virtqueue that every virtio
device driver rides on. Drivers layer. Linked from
`crates/mlos-virtio/src/lib.rs`, `regs.rs` and `queue.rs`.

## Why its own crate

Apple's Virtualization.framework offers no PL011, so without virtio MLOS
boots under it and says nothing (`docs/architecture.md` s.8.1). A console
is what MLOS needed first, but the transport is also the first piece of
the provider machinery M2 needed: a block device and a filesystem are the
same transport with a different device id. That is why the transport is
separated from the console that happens to use it, and why each device
gets a crate of its own (`mlos-virtio-console`, `mlos-virtio-blk`) rather
than a module here. Keeping the two apart is what stops the transport
crate growing a module every time a device is added.

## Version 2 only

Version 1 of virtio-mmio is a different memory layout with a different
queue-address convention. Supporting both would double `regs.rs` to serve
hardware that no longer ships.

## Probing, not indexing

QEMU lays out 32 virtio-mmio slots whether or not anything is plugged
into them, and an empty slot reads a device id of zero rather than
failing. `Device::probe` is therefore how a driver finds what is actually
there: check the magic and version, then treat id 0 as an empty slot.

## Only `VIRTIO_F_VERSION_1` is accepted

Every feature accepted is a behaviour the driver then has to implement,
and neither the console nor the block driver needs any of them.
`VIRTIO_F_VERSION_1` is bit 32, so it lives in the second feature word,
which is the whole reason the feature registers are selected a word at a
time. The handshake order in `negotiate` is the specification's: announce
ourselves, read what is offered, answer, then check the device still
agrees.

## No interrupt registers

`regs::reg` names only the registers the drivers use. The interrupt
registers are absent because transmit spins for completion rather than
waiting for one; they arrive with receive, which needs a handler anyway.

## Split, not packed; synchronous, not interrupt-driven

The split virtqueue is what every device supports, and the layout is
simple enough to reason about without a specification open. Modern virtio
lets the three rings live at three separate addresses, which spares the
alignment arithmetic the legacy contiguous layout needs.

The queue is synchronous: submit one buffer, spin until the device returns
it. Console output is not hot, and a driver that cannot block is a driver
that needs an interrupt handler before it can print anything, which is
the wrong order to build things in.

`SIZE` is eight descriptors: far more than a synchronous driver uses, and
a power of two because the ring index wraps by masking.

## Always descriptor 0

`submit` always publishes the chain beginning at descriptor 0, because
every driver here submits one operation at a time and waits for it. The
device follows `next` from there, so a chain of three (a block read) is
submitted exactly like a chain of one (a console write). Chaining exists
for the block device: a read is a header the device reads, a buffer it
writes, and a status byte it writes, three descriptors describing one
operation.

## The fence before the notify

The device reads the descriptor and the available ring from memory, so
both must be visible before it is told to look. `submit` fences after
writing the ring slot and again after advancing the index, before the
`QUEUE_NOTIFY` write. Without the fences the device can be pointed at a
descriptor that has not been written yet, which fails intermittently and
only under load. Every read of the used ring is volatile for the mirror
reason: the device writes it by DMA, and the compiler has no reason to
expect it to change.
