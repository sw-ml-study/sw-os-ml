# mlos-snapshot

The running object store, streamed out as a `sw-ml-study.system-layout`
document from inside the kernel. Objects layer, `no_std`. Linked from
`crates/mlos-snapshot/src/lib.rs`, `columns.rs`, `parts.rs` and `rows.rs`.

## The counterpart to mlos-image-map

`mlos-image-map` describes what the build produced; this crate describes
what is actually in memory, and the difference between the two files is
the whole argument of `docs/PRD.md`. A picture of a disk image is
something any operating system could be drawn from. A picture of which
parts of a model are resident, what each cost to fetch, how often each has
been wanted and when each is next needed is not.

Both documents use the same region ids, from `mlos-spaces`, so a consumer
joins them: the tile at `disk` id X and the bytes it became at `dram` id Y
are the same object, and the `backs` edge says so. The arena's unused tail
takes the first structural id of the `dram` space, which is what the
static emitter's `fill` gives its free region too, so the arena's spare
space is the same region in both documents.

## Must not disturb what it measures

Reading the table is not acquiring from it: emitting faults nothing in
and moves no counter. `tests/undisturbed.rs` holds that.

## Streaming with no allocator

The host emitter builds a `Vec` per column and joins it. There is no
allocator in the kernel, so a column is written as it is walked: open the
bracket, write a separator before every item but the first, close it.
Separator-before rather than comma-after is what keeps the JSON valid
without knowing in advance how many rows there are.

`rows` is an iterator rather than a collection for the same reason. Each
column is written by walking it again, sixteen passes over a hundred-odd
rows, which costs nothing and means no buffer has to exist anywhere.

`provenance` comes last, which looks odd and is deliberate: every column
is written with a trailing comma, and a streaming emitter with no buffer
cannot go back and remove one. A scalar at the end closes the object
without it. Key order is not significant to any JSON parser, and the two
lines that identify the format are still the first two.

Write errors are dropped rather than propagated. The sink is a console: if
it has stopped accepting bytes there is nowhere to report that to, and a
half-written document is already in front of whoever is reading it.

## No escaping

`text` quotes but does not escape. Nothing MLOS puts in a string column
needs escaping, since they are built out of decimal numbers and fixed
words, and a `no_std` escaper with nowhere to build the escaped string
would have to go character by character for no benefit. `tests/document.rs`
pins the vocabulary, so a value that would need escaping is a test failure
rather than malformed JSON in somebody else's parser.

## Row carries resolved values

`Row` carries values already resolved rather than the `ObjectMeta` it
came from. That is what lets every column be a one-line field read: the
alternative is a match per column, repeated sixteen times, each one
another chance to describe a free region as an object. The tail row is
passed into `rows` rather than built there because it is the one row that
describes no object.

Sorted order is deliberately not promised. The contract requires the
regions of a space to tile it, not to arrive in order, and sorting without
an allocator would be a quadratic pass for no gain; the consumer indexes
by `region_id` and takes geometry from `start` and `length`, neither of
which cares.

The tail row is skipped when the arena is exactly full. A zero-length
region satisfies the tiling rule and draws as nothing, which is a box in
the legend that is never on screen. When present, it makes the high-water
mark a boundary in the picture rather than a number somebody has to be
told.

## Which spaces, which rows

`sysram` is absent on purpose. A running kernel has no symbol table and
cannot say where its own `.text` ended, so claiming a RAM map here would
mean inventing one, and the static document already has a real one, drawn
from the linked image.

The disk map lists every tile, resident or not: it is about what exists,
and a tile's `state` column says whether it is also in memory. That is the
picture worth having, which parts of a model a workload has actually
touched, drawn over the whole model rather than over a fragment of it.

Arena rows have their length rounded to the arena's granularity, because
that is what the object cost: the arena hands out aligned extents, and
reporting the unrounded size would leave holes the contract does not
allow.

## Columns, in contract order

`parts.rs` is split the way the contract is: the columns every producer
emits, then the ones MLOS adds. A reader comparing it against
`../sw-mlpl/docs/storage-layout-viz.md`, or against the host emitter's
`render.rs`, should be able to do it by eye. The two emitters disagreeing
about a column name is the failure mode that would be hardest to notice,
since each is valid JSON on its own.

The image word counts are emitted as zeros rather than omitted: a
consumer written against SWTOS reads them without checking, and a missing
column is a crash where a zero is a fact. The extension columns carry
empty strings where they do not apply, so a consumer grouping by tier or
state never sees a bucket that is really "not an object".

## The `backs` edge

MLOS's version of SWTOS's catalog -> extent -> allocation chain, and what
an "explain this object" view consumes. Only weight tiles have stored
bytes: an activation is recomputed, so there is no disk extent for an edge
to run from, and drawing one would be a lie about where it came from.

## Provenance from boot arguments

`revision` is stamped into `provenance` so a rendered snapshot is
traceable. The kernel cannot know its own commit, so the host passes it in
through `/chosen/bootargs` (`mlos.rev=`), the same channel that asked for
the document in the first place. `write_for` lives here rather than in
`mlsh` because the boot argument and the `provenance` field it fills are
one decision, and splitting them across two crates means two places to
look when a snapshot comes back stamped `unknown`.

`write` returns `false` when no model has been registered. An empty
document would be a truthful picture of nothing, and the shell can say
something better.

## `of` is public for tests

`of` renders any manager, not only the live one, so a test can build its
own arena and object table rather than reaching for the kernel's statics.
That matters more than it looks: the live manager is a `static` shared by
every test in a binary, and a test suite that mutates it cannot run its
cases in parallel or in isolation.
