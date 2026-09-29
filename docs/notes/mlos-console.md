# mlos-console

Choosing a console: PL011 or virtio, decided once at boot from the device
tree and `/chosen/bootargs`. Kernel-side glue above the drivers. Linked
from `crates/mlos-console/src/lib.rs` and `input.rs`.

## Why a choice at all

MLOS can talk to two kinds of console, and which one exists is a property
of the hypervisor rather than of the architecture: QEMU's `virt` has a
PL011, Apple's Virtualization.framework has only a virtio console. A
kernel that drives one of them boots under one of them and says nothing
under the other.

The choice is made once, at boot, from what the device tree describes and
what `/chosen/bootargs` asks for, using the same `console=` convention
Linux uses, so a host that already knows how to ask gets what it asked
for.

## An enum, not `dyn Console`

`Terminal` is `Copy`, so it can be published into a static for interrupt
handlers to find without a lifetime or an allocation, and the fault
reporter can hold one on an arbitrary stack. A trait object would need
one or the other.

## PL011 first, unless asked otherwise

`Terminal::open` prefers the PL011 unless `/chosen/bootargs` names an
`hvc` console, because the PL011 needs no setup and works before anything
else does, which is what you want when the thing you are debugging is the
console. `console=hvc0` overrides that, and is also how the virtio path
gets exercised on a machine that has both. If the virtio console cannot
be brought up, the PL011 is the fallback.

## Probing the virtio slots

QEMU lays out 32 identical virtio-mmio windows whether or not anything is
plugged into them, and an empty one reads a device id of zero rather than
failing. `Terminal::virtio` probes every slot the device tree describes
and takes the first that reports the console id.

## Receive is PL011-only

`Terminal::read` returns `None` from a virtio console, and that means "not
implemented", not "nothing arrived". Receive needs a second queue and an
interrupt, and nothing can be typed at MLOS under Virtualization.framework
until it exists. The gap is recorded in `docs/status.md`.
