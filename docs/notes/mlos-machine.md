# mlos-machine

The machine, as the device tree describes it: memory, CPUs, console,
interrupt controller, virtio slots, boot arguments, and where the blob
itself sits. Platform layer, architecture-neutral. Linked from
`crates/mlos-machine/src/lib.rs`, `regions.rs`, `reserve.rs` and
`scan.rs`.

## Why its own crate

It answers exactly the questions MLOS asks at boot and stops. Nothing in
it is aarch64-specific, because a device tree is not. It lives outside
`mlos-hal-aarch64` for that reason and because that crate had no module
budget left, which is the `sw-checklist` gate doing what AGENTS.md says
it should: forcing the split at the point the concern actually separates.

## Boot arguments are read to the end

`rest` returns everything after `key` to the end of the string, not to
the next space, because the values differ in what they need:
`mlos.rev=abc123` is one token, while `mlsh.run=model;trace off;sweep` is
three commands and two of them take an argument. Whitespace-splitting
here would keep `model;trace` and throw the rest away. So the rule is the
permissive one and the caller narrows it, with `split_whitespace().next()`
for a setting that is a single token. The cost is that a setting whose
value has spaces in it must come last. That is a real constraint, and it
is stated wherever such a setting is written rather than only here.

`rest` lives here rather than in the shell because boot arguments are a
device-tree property and this crate is what reads one. `/chosen/bootargs`
is read because it says which console was asked for, in the same
`console=` form Linux uses.

## The tree describes the machine, not what the loader put in it

The device tree knows nothing about the kernel image or about the blob
itself, so those facts have to be combined with the map somewhere, and
`reserve` is where. Without it, `BootInfo::usable_bytes` reports memory
that is already occupied, and the first allocator to trust it hands out
the ground the kernel is standing on. `Machine::reserving` combines the
image extent with the map because that is this type's job, not its
caller's.

The blob's own extent is recorded as `Reclaimable` as it is parsed.
Firmware structures are real memory, several kilobytes here and much more
on a machine with ACPI, and on a system whose whole premise is accounting
for resident bytes, writing them off permanently would be an odd place to
start.

## A fixed-capacity map

`Regions` is fixed because it is built before there is an allocator; the
map is what the allocator gets built from. `MAX_REGIONS` is sixteen: QEMU
`virt` reports one DRAM region, and carving the kernel image and the
device tree blob out of it can turn that one into five. Sixteen leaves
room for a machine with a split map without pretending this is a growable
collection. A push that does not fit returns `false` and drops the region
rather than growing silently or panicking at boot.

`extend_usable` takes a decoder closure rather than raw `reg` bytes: how
many cells a `reg` uses is the parent node's business, and the map should
not have to know about device trees to be filled from one.

## The walk keeps one current node

Property events always arrive between a node's own `Node` event and its
first child's, because the specification requires every property to
precede every subnode. That is what makes a single "current node name"
correct rather than a bug waiting for a deeper tree. The same ordering is
why `Scan::default` can start the cell counts at the specification's
defaults: the root node's own `#address-cells` and `#size-cells`
overwrite them before any `reg` is read.

`reg` is handled by one function rather than three because `reg` means
"where this device is" regardless of the device, and the cells that
decode it come from the same parent either way.

## The console is the first `pl011@`, not the last

A machine can have more than one. Two `-serial` backends make QEMU
instantiate a second PL011 at `0x9040000`, and overwriting as the walk
goes picks whichever comes last, which is not the console. The exact
answer is `/chosen/stdout-path`, but that node comes after the UARTs in
QEMU's tree, so honouring it needs candidates resolved at the end of the
walk rather than a single field. Recorded as a gap in `docs/status.md`;
first-wins is right for every tree QEMU emits.

The console's `interrupts` property is `<kind, number, flags>`. Kind 0 is
a shared interrupt, whose numbering starts at 32: the device tree counts
from the start of the SPI range, the GIC does not, hence the `+ 32`.

## virtio: the lowest window and the count

QEMU lays out 32 identical `virtio_mmio@` slots whether or not anything
is plugged in, so the count matters more than any single address; finding
a device means probing them. The slots are uniform and contiguous, so one
base (the lowest seen) and one size describe all of them.

## The interrupt controller: `compatible` and `reg` are combined after the walk

`gic_v3` and `gic_reg` are separate fields because a device tree does not
order a node's properties: in QEMU's own blob `reg` comes before
`compatible`, so deciding what the ranges mean while reading them reads
the wrong answer. They are combined once the walk is over.

For a GICv3 the two `reg` ranges are the distributor and the
redistributor, in that order: one system-wide, one per-CPU. A GICv2 puts
a CPU interface in the second range instead, which is a different device
at a different offset, hence the `compatible` check; only a v3 layout is
understood. Both ranges are recorded or neither: half an interrupt
controller is worse than none, because it looks initialised.
