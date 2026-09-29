# mlos-spaces

MLOS's names for its layout spaces, its stable region ids and its region
vocabulary, shared by both layout emitters. Objects layer, `no_std`.
Linked from `crates/mlos-spaces/src/lib.rs`, `ids.rs` and `vocab.rs`.

## Why its own crate

`no_std`, and that is the whole reason it exists apart from `mlos-layout`.
Two emitters produce the same contract: a host-side one reading build
artifacts (`mlos-image-map`), and one inside the kernel reading the live
object table (`mlos-snapshot`). They cannot share a renderer, since the
kernel has no allocator and streams to a `Write`, but they must share the
id scheme and the vocabulary, because a consumer joins the two files on
`region_id` and colours both from one palette. Two copies of that would
agree right up until the moment they mattered.

The contract itself is `../sw-mlpl/docs/storage-layout-viz.md`.

## Spaces

`disk` and `dram` appear in both the static and the runtime document;
`sysram` only in the static one, because a running kernel has no symbols
and cannot say where its own `.text` ended.

## Stable ids

The contract says `region_id` is assigned by the producer and stays the
same for the same thing across snapshots: it is what native3d picks on,
and what cross-highlights a stored tile against the arena region holding
its bytes. A positional index would break the moment a region is inserted,
so the id is built from the object's own fields (the bit layout is in
`ids.rs`).

The whole id fits in 31 bits, which matters because a JSON number is safe
to 2^53 in some consumers and to 2^31 in others.

The model field is deliberately not in the id. One model fits; a second
would collide, so `object` refuses rather than truncating. A loud failure
at the point a second model is registered is worth more than a picture
that quietly shows two models on top of each other. The same goes for a
layer or tensor past 255: every one of those is a real change to the model
rather than bad input, and each wants the scheme widened deliberately
rather than wrapped around silently.

Class zero is not a valid `ObjectClass`, so the class-zero range of each
space is free for regions that are not objects: padding, free space,
kernel sections. `Where::ids` hands them out in issue order, and both
emitters take them from there in the same order, which is what keeps the
arena's free region the same region in both documents.

## Vocabulary

One definition of each word, shared by both emitters, because sw-mlpl's
palette is keyed on these strings and a second spelling of "weight-tile"
would render as nothing at all. Every match is exhaustive with no wildcard
arm, so adding a class or a tier to the ABI is a compile error in
`vocab.rs` rather than a region that silently loses its colour.

`state` is three-way, and the middle one is the interesting one. An object
that has been wanted before and is not here now was thrown away, which is
a different fact from never having been asked for, and it is the fact a
residency policy will be judged on. Both are `resident_at == 0`, so
nothing but the use count can tell them apart.

`NextUseText` writes a string with three shapes rather than a number, for
the reason `NextUse`'s own documentation gives: a dense layer sweep yields
an exact position in a declared stream and an MoE router yields a
distribution, and those differ in kind rather than in degree. A policy may
act on a distance with certainty and on a probability only as a hint, so
collapsing them into one column would licence a consumer to draw them the
same way.
