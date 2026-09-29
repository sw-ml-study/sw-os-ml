# mlos-layout

The `sw-ml-study.system-layout` document: a columnar table of spaces and
regions that a visualizer draws. Host-side tooling layer. Linked from
`crates/mlos-layout/src/lib.rs`, `fill.rs`, `read.rs`, `render.rs` and
`valid.rs`.

## A cross-repo contract, not an MLOS format

The format is pinned by `../sw-mlpl/docs/storage-layout-viz.md`, emitted
first by SWTOS, parsed by sw-mlpl's array language and drawn by
demo-extensions' native3d. MLOS is the second producer. The contract facts
(schema string, version, column names and order) live in the code; this
note holds why the format is shaped the way it is.

Nothing in the crate knows what an `ML_OBJECT` is. That is the point of
the boundary the contract draws: the operating system knows storage
semantics, the visualizer knows geometry, and the only thing crossing
between them is a table. Region kinds, owners and ids are opaque strings
and numbers at this level; `mlos-image-map` supplies MLOS's vocabulary,
and SWTOS supplies a different one through the same shape.

## Why columnar

Struct-of-arrays rather than an array of objects, because that is what
sw-mlpl's `parse_json` ingests today: homogeneous numeric arrays become
numeric arrays, all-string arrays become string lists, and the layout math
then runs elementwise over whole columns.

## MLOS's extension columns

`tier`, `object`, `state`, `reuse`, `cost` and `next_use` are MLOS's
extensions. The contract permits extra columns, and this is what an ML
object store knows that a flash image does not, which is the argument for
sharing a format rather than forking one. Regions they do not apply to
carry the empty string or zero.

`region_object_id` is a string because a `u64` does not survive a JSON
number in every consumer.

The three word-count columns (`region_text_words`, `region_data_words`,
`region_bss_words`) are 24-bit image words, which MLOS has none of. They
are emitted as zeros rather than omitted, because a consumer written
against SWTOS reads them unconditionally, and a missing column is a crash
where a zero is a fact.

## Rendering from tables

Column names and their order live in `render.rs` as tables rather than as
a run of `push_str` calls, so that adding a column is one line and cannot
be added to the header without being added to the body. The contract's
own columns come first, in the order the contract lists them, and the
producer's extensions follow; a reader diffing the output against
`storage-layout-viz.md` should be able to do it by eye.

`quoted` is a hand-written JSON string escaper. Rust's `{:?}` is close but
not the same: it emits Rust's own `\u{..}` form for a control character,
which JSON does not accept, and "close" in a format three other repos
parse is not a property worth relying on.

## Tiling: padding and free are the interesting part

A consumer draws a space as a solid stack of cells, so every byte of
capacity has to belong to some region. `fill` sorts a space's regions,
inserts padding between them and free space after them, and checks the
result covers `[0, capacity)` exactly once. Anything the producer did not
account for shows up as padding or free, which is not bookkeeping but the
point: the gap between two sections is alignment cost, the gap at the end
is headroom, and a viewer that shows neither is flattering the system it
draws.

Padding and free are distinct kinds on purpose. SWTOS's emitter makes the
same distinction, and folding them together would hide exactly the number
a layout is usually being looked at to find.

Overlap is an error rather than something to reconcile: two regions
claiming a byte means the producer is wrong about its own layout, and a
picture drawn from it would be confidently misleading. Running past
capacity is likewise an error rather than a region nobody can draw.

## Validating the text, not the types

Two emitters write this format: a host-side one with `Vec`s and a `no_std`
one streaming to a console. They share no code that could be checked
once. What they do share is the bytes a consumer reads, so `validate`
checks those, and the runtime emitter gets the same gate as the static one
without either knowing about the other.

The validator is deliberately about shape and not about numbers. The
regions in a layout change whenever the kernel grows a section or a sweep
gets further, and a test pinned to those figures would be rewritten every
time instead of ever failing usefully. What must never change is that
columns stay index-aligned, that regions tile each space exactly once, and
that ids are unique and non-zero, because a consumer draws a solid stack
of cells and picks on the id, and each of those breaks silently.

`Columns::read` is line-based, not a JSON parser, and does not pretend to
be: both emitters write one column per line, which is all a validator
needs. A consumer's real parser is what decides whether the text is valid
JSON, and `tests/` puts the emitted text through one. Its `Vec` is public
because checking index alignment means walking every column whatever its
name, and an accessor that hands out exactly that is the `Vec` with extra
steps.
