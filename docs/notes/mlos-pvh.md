# mlos-pvh

What the PVH loader hands an x86-64 guest, parsed from bytes: the
`hvm_start_info` header, its e820 memory map, and the kernel command
line. Platform layer, safe code. Linked from `crates/mlos-pvh/src/lib.rs`,
`cmdline.rs` and `memmap.rs`.

## Why its own crate

On x86-64 this is the device tree's counterpart (`docs/plan.md`, saga
`mlos-x86-64`): `hvm_start_info` carries the memory map, and the command
line carries both `mlsh.run=` and, on QEMU `microvm`, one
`virtio_mmio.device=` per transport. So `BootInfo` is filled from two
sources here where aarch64 fills it from one.

The crate parses byte slices and nothing else, with `unsafe` forbidden.
Turning a physical address into a slice is `mlos-hal-x86-64`'s job, and
keeping that step out of here is what lets every rule in this crate be
tested on the host: the tests hand it bytes, the kernel hands it the
loader's memory, and the code is the same.

## Refusing to guess

`StartInfo::parse` returns `None` if the magic is wrong or the version
predates the memory map (version 1, which is what QEMU's PVH loader
writes). A machine without a map is one whose RAM the kernel would be
guessing at, and MLOS does not guess: the kernel reports the invalid
start info and waits for `Ctrl-D` rather than run.

`regions` maps e820 type 1 to `Usable` and type 3 (ACPI tables) to
`Reclaimable`; everything else, reserved, NVS, unusable, and types this
code has never heard of, is `Reserved`. Unknown is not usable. The safe
mistake is to waste memory, not to hand out memory firmware still owns.
Zero-length records are dropped and a trailing partial record is ignored,
since neither describes memory.

`virtio_slots` skips a malformed `virtio_mmio.device=` entry rather than
trusting it: a slot at a misread address is worse than a missing one.

## The command line

QEMU `microvm` announces each virtio-mmio transport on the command line
instead of in a device tree, in the convention Linux reads:
`virtio_mmio.device=SIZE@BASE:IRQ`, SIZE in bytes with an optional `K`
or `M` suffix, BASE in hex. `virtio_slots` reduces the entries to the
shape `mlos_lab::set_slots` takes, the lowest base, its size, and the
count, which is what the device tree's `virtio_mmio@` nodes give on
aarch64.

`mlsh.run=` takes the rest of the line, because its commands have spaces
in them, and QEMU appends the `virtio_mmio.device=` entries after
whatever the user passed. Without `user_args`, the last shell command
would be handed the device list as its argument. The line is cut at the
first entry that follows `mlsh.run=`; a line without one is returned
whole.
