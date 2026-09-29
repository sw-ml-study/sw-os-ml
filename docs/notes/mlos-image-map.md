# mlos-image-map

MLOS's built artifacts as a `sw-ml-study.system-layout` document, plus the
host side of lifting the runtime document and event stream off a captured
console. Host-side tooling layer. Linked from
`crates/mlos-image-map/src/lib.rs`, `memory.rs`, `objects.rs` and
`runtime.rs`.

## The static half

What the build produced and where it will sit, with no emulator in the
path. That separation is deliberate: a contract that only validates after
a VM boots is a contract whose failures are hard to read, and everything
here is decided at link time and at image-build time anyway.

Three spaces: the block device the weights are stored on, guest RAM as the
kernel image divides it, and the object arena. The first and third are
what the runtime emitter (`mlos-snapshot`) fills in: the disk says what
exists, the arena says what is resident, and the whole argument of
`docs/PRD.md` is about the ratio between them.

The sibling repos: sw-mlpl turns this into geometry, demo-extensions draws
it, and SWTOS emits the same shape for a flash image. The contract itself
is `../sw-mlpl/docs/storage-layout-viz.md`.

The static document's edge table is empty, and stated rather than omitted.
An edge runs from a stored object to the arena region holding its bytes,
and statically no object is resident, so there is nothing yet to draw a
line between.

## Every figure is read back, never restated

`linker/aarch64.ld` decides where `.text` ends and `.bss` begins, and it
will decide differently after the next commit; an emitter carrying its own
copy of those numbers would be wrong without being noticed, which is
exactly the failure a memory map is supposed to expose. So section extents
come from the linked ELF via `mlos-elf`, and `text_offset` and
`image_size` come from the arm64 `Image` header, the header the loader
reads, so these are the numbers that decide where the kernel actually
lands.

The boot stack is deduced rather than read: it is whatever lies between
the end of `.bss` and the end of the image, one page-aligned step on (the
`ALIGN(4096)` the linker script puts before `__stack_bottom`). It has no
section of its own to look up, and the alternative would be carrying a
copy of its size here, where nothing would notice it going stale.

`RAM_BYTES` lives here rather than in the VMM arguments because the map
and the machine have to agree: an emitter claiming 512 MiB while QEMU is
handed 1 GiB would draw a free region that is half the size of the real
one, and nothing would report the discrepancy. `mlos-cli` reads this.

The disk space's capacity is the image file's real size, not the model's:
if they disagree, the difference shows up as free space or as an error
from `fill`, and either is better than an emitter that assumes.

## Section vocabulary

`text`, `data`, `bss` and `stack` are sw-mlpl's existing palette words,
used verbatim so MLOS's RAM map renders through the palette SWTOS already
needs. `rodata` is the one addition, and it is not MLOS-specific: every
OS map wants it.

## The arena is its own space

Physically the arena is a static inside the kernel image, which `sysram`
already accounts for as part of `.data`. It gets its own space anyway
because residency is the thing being looked at, and a 32 KiB box inside a
512 MiB one is not a picture of anything. Statically it is entirely free.

## State derived, not asserted

A stored tile's `state` comes from the same `mlos_spaces::state` the
runtime emitter uses, rather than being written as the constant it happens
to be here. Nothing has run, so every object reads `never`, and that is
the difference the two documents exist to show, which makes it worth
deriving rather than asserting.

## Provenance

`revision` is stamped into `provenance` so a rendered picture is
traceable. It is public because the runtime emitter runs inside the guest,
which has no git and no way to know what built it; `mlos runtime` passes
it in through the boot arguments. `--dirty` matters more than the sha: a
layout emitted from an uncommitted tree is one nobody else can reproduce,
and saying so is cheaper than discovering it later.

## Lifting the runtime document off the console

MLOS has no filesystem, so the guest writes its layout to the console and
the host cuts it back out. Crude, and honest about being crude: a serial
line is the only channel out of the machine, and inventing a file system
to avoid admitting that would be a much larger lie than a pair of
brace-delimited markers.

The document is found by structure rather than by a sentinel: a line that
is exactly `{` opens it and a line that is exactly `}` closes it, and
nothing else `mlsh` prints begins a line with a brace. The event stream
that shares the console is marked (`@ev`) and extracted by prefix match,
so neither collides with the other.

When the document is missing, the whole console text goes in the error,
not just a summary of it. A guest that did not print a layout usually did
not get as far as the shell, and the reason is somewhere in what it did
print, so putting it in front of whoever ran the command saves them
running it again to look.

## The boot script

`SCRIPT` sweeps before the snapshot on purpose. A layout of an arena
nothing has been put in is a picture of an empty box, and the whole point
of the runtime document is what residency looks like under pressure; the
arena is a quarter the size of the model, so the sweep stops part way and
the boundary that leaves is the interesting line in the image.

It sweeps twice, because one sweep produces no hits. The second walks the
same tiles, finds the first 32 already resident, and stops at the same
place, so the stream carries all three kinds of event rather than two,
and a consumer can tell a re-use from a fetch without having to be told
that the missing kind exists.

## The replay size

`REPLAY` (2 sessions, 16 rounds) is smaller than the verdict's 4x40: the
guest parses the whole trace into a static array, and 25,200 accesses is
400 KiB of `.bss` plus half a megabyte of text to read off a virtual disk
under TCG. What the kernel-versus-simulator comparison needs is that the
two agree exactly, which a shorter trace shows as well as a longer one.

It lives here rather than beside the disk writer that renders it, because
this is the number the boot test reads back to build the simulator's side,
and a test cannot import from a binary crate.

## The trace is not the stream

`trace` derives an access trace from an event stream: what the workload
asked for, which is what a policy is replayed against, as opposed to what
this particular run's object manager did about it, which is what the
stream itself says. `mlos-trace` explains why the two must not be the same
file.

`save` is shared by both emitted files and both callers; a second copy
would be a second place for the directory creation to be forgotten.
