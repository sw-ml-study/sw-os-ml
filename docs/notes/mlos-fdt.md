# mlos-fdt

A minimal reader for the flattened device tree: it walks the blob, it does
not build a tree. Boot layer. Linked from `crates/mlos-fdt/src/lib.rs`,
`header.rs`, `cursor.rs` and `reg.rs`.

## Why a walker and not a tree

It answers the three questions MLOS asks at boot -- where is memory, how
many CPUs, and where is the console -- and no more. Building a tree needs
an allocator, and the memory map is what the allocator gets built *from*.

## Architecture-neutral on purpose

aarch64 and RISC-V both boot this way, so this is one of the few places a
future `mlos-hal-riscv64` costs nothing (`docs/architecture.md` s.9).
Nothing in the crate names an architecture.

## The blob is untrusted

It arrives from firmware: it is the first untrusted input the kernel ever
sees, and a parser that indexes without checking turns a malformed one
into arbitrary memory access. Every read is bounds-checked and every
malformed input yields `None`, never a panic and never an out-of-range
access. `Fdt::from_ptr` reads the 40-byte fixed header first to learn the
blob's length, so a pointer to something that is not a blob is rejected
after 40 bytes rather than believed.

`Fdt::walk` returns `None` on a malformed blob, having already reported
whatever it read successfully: a caller that got what it needed before the
damage can proceed.

## Version check

`Header::parse` checks `last_comp_version`, not `version`: a newer blob
that still declares itself backward compatible with 16 is one this reader
can read, and refusing it would reject a future QEMU for no reason.

## Strings are ASCII or the blob is wrong

`Cursor::cstr` rejects non-UTF-8 rather than replacing it. Node names in a
device tree are ASCII by specification, so anything else means the blob
is not what it claims to be.

## `reg` needs the parent's cell widths

A `reg` value is a flat run of big-endian cells whose widths come from the
*parent* node's `#address-cells` and `#size-cells`. That indirection is why
`reg_pair` takes both widths rather than reading them off the bytes.
`cells` refuses more than two cells: a device tree may in principle use
wider addresses, but nothing that would fit in a `u64` does, and silently
truncating an address is worse than admitting we cannot read it. A value
too short for the requested pair is `None`, so a truncated or
mis-specified property is rejected rather than read as zeros.

## Lessons

**Strip the NUL yourself.** Property strings are NUL-terminated, and the
terminator is inside the value's declared length, so a caller that
compares the raw bytes to a string literal is comparing against a trailing
zero and always losing. `string` strips the terminator explicitly rather
than trusting `trim_ascii_end`, which trims whitespace and leaves a NUL
exactly where it was. That was a real bug and an invisible one:
`/chosen/bootargs` came back as `console=hvc0\0`, every `contains` and
`starts_with` still matched, and nothing noticed until a NUL was written
into a JSON document and somebody else's parser refused it.
