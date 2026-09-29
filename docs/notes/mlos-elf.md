# mlos-elf

A minimal ELF64 section-header reader: where the kernel's sections land in
memory, read from the linked file. Host-side tooling layer. Linked from
`crates/mlos-elf/src/lib.rs` and `read.rs`.

## Why it exists

The layout emitter must not guess. `docs/plan.md`'s visualization steps
promise that every figure comes from the artifact that produced it, and
the kernel's `.text`/`.rodata`/`.data`/`.bss` extents are decided by
`linker/aarch64.ld` at link time. Reading them back out of the linked file
is the only way to report them and still be right after the next commit
changes their sizes.

## Section headers only

Symbols, relocs and program headers are all readable the same way and none
of them are needed yet; a reader that stops at what is used is one that
can be checked by eye. Only little-endian ELF64 is accepted, because that
is the only kind MLOS links.

`sections` returns the non-allocated sections (`.debug_*`, `.symtab`) too,
with `addr` zero; filtering them is the caller's business, since which
sections matter depends on what the caller is mapping.

A section name with a missing NUL terminator yields the rest of the string
table rather than an error: a section whose name is odd is not a reason to
refuse to draw the map.

## Bounds-checked reads

Every field read is bounds-checked rather than trusting the file, because
the file is an input: a truncated ELF should produce a message naming the
offset, not a panic in a tool the user did not know parsed ELF.
