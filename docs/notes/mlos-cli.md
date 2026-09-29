# mlos-cli

`mlos`: build, run and diagnose MLOS from the host. Host-side tooling
layer, the one place the VMM launch decisions live. Linked from
`crates/mlos-cli/src/main.rs`, `image.rs`, `run.rs`, `vmm.rs`, `x86.rs`
and `doctor.rs`.

## Why a binary and not scripts

It replaces `scripts/boot.sh` and `scripts/image.sh`, which were fine
until more than one thing needed them and a stale image started producing
confusing results. Every command that runs a build fails loudly rather
than continuing with a stale artifact: a stale image boots happily and
looks almost right.

## The `Image`, not the ELF (aarch64)

`image::build` objcopies the kernel to a flat arm64 `Image`. QEMU jumps
straight to an ELF's entry point and skips the arm64 boot protocol, so
`x0` arrives as zero instead of a device tree pointer, which is how MLOS
spent M1 step 004 not knowing where its own device tree was.

The objcopy is the `llvm-objcopy` that ships with the active toolchain,
found through `rustc --print sysroot`, rather than one on `PATH`: the
toolchain's own is guaranteed to match the LLVM that produced the object
files, and a mismatched system objcopy fails in ways that look like a
linker bug. The sysroot path is trimmed because `rustc` ends it in a
newline, and a path with one in it silently does not exist.

## The ELF, not an image (x86-64)

`x86::build` returns the ELF itself: PVH is found through an ELF note, so
QEMU needs the program headers an objcopy would strip. The two
architectures therefore hand QEMU different kinds of file, on purpose.

## Which hypervisor

`run::HOSTS` is `hvf`, `tcg`, `vz`. The first two are QEMU accelerators;
`vz` is a different program entirely, Apple's Virtualization.framework
driven through vfkit. They share a list because from the outside they are
the same question: which thing runs MLOS. `vmm.rs` is its own module for
the same reason in reverse: QEMU and vfkit are not variants of one thing,
and only the question is shared.

`vz` produces no output yet. Virtualization.framework offers a virtio
console and no PL011, and MLOS drives only the latter. The VM runs;
nothing says so. Requirement N2 closes when the virtio-console driver
lands. vfkit also does not expose `VZLinuxBootLoader`'s command line, so
nothing under `vz` can carry `mlsh.run=`, which is why `mlos runtime`
uses QEMU. vfkit insists on an initrd even though `VZLinuxBootLoader`
does not, so it gets a single zero byte that MLOS never looks at.

## The QEMU command line

- `-m` comes from `mlos_image_map::RAM_BYTES` rather than a literal: the
  layout emitter draws guest RAM at that size, and a VMM handed a
  different number would make every free region in the picture wrong with
  nothing to report the disagreement.
- `gic-version=3` is pinned rather than left to QEMU: the default differs
  by accelerator (TCG gives a GICv2, HVF a v3), so without it the two hand
  the guest different interrupt controllers, and only one is the one MLOS
  drives.
- `-serial mon:stdio` multiplexes the guest console with the QEMU
  monitor, which is what makes `Ctrl-A x` work. vfkit has no monitor.
- `virtio-mmio.force-legacy=false` is required, not cosmetic, for both
  the virtio console and the model disk: QEMU's `virt` defaults
  virtio-mmio to version 1, the legacy layout with a different queue
  convention, and MLOS speaks only version 2. Without it the magic value
  matches, the version does not, and every slot probes as absent.
- `console=hvc0` is what tells MLOS to prefer the virtio console, the same
  `console=` convention Linux uses.
- `-cpu` is `host` under HVF and `cortex-a72` under TCG.

## The model disk

`image::disk` writes one raw image holding the synthetic model's weights
and then the replay trace, attached read-only over virtio-blk.

Laid out by id: tile `(layer, tensor)` at sector
`(layer * TILES + tensor) * TILE_BYTES / 512`, filled with
`0xA0 | (layer ^ tensor)`. The low nibble is the same pattern the stub
provider fabricates, on purpose, so when the guest reads it back from a
real device the bytes being right is not the news. The high nibble is:
the stub writes `layer ^ tensor` alone, so `0xA0` in a byte can only have
come off the disk, and provenance becomes checkable rather than merely
arithmetic.

The trace follows the weights at `mlos_synth::disk::TRACE_AT`, as an
eight-byte little-endian length then the text. One device rather than
two, because a second virtio-blk would mean teaching `probe` to tell
block devices apart: machinery in service of a layout decision that is
free, since the model occupies a fixed extent and what follows is spare.

Read-only because weights are immutable, which is the property that lets
one copy serve every session. A writable model disk would be a tier that
has to be invalidated.

## Interactive versus capture

`run::run` is interactive: `mlsh` is on the other end, and a piped stdin
would not reach it, because a receive FIFO only sees keystrokes from a
real terminal. Quit with `Ctrl-A x` under QEMU, `Ctrl-C` under vfkit.
`--debug` halts the CPU with a gdb stub on `:1234`; load the ELF in gdb
for symbols, since the image has none.

`run::capture` is the non-interactive counterpart, for tests and for
checking a change still boots. No keystroke reaches the guest (a file is
not a terminal), so a shell session is driven through the boot arguments,
which become `/chosen/bootargs` and can carry `mlsh.run=model;sweep;layout`.
That is what makes a runtime snapshot reproducible instead of something a
person has to sit and type. `--run SCRIPT` opens the same door to the
caller, and is how the kernel/simulator comparison drives four replays in
one boot; it is what makes a headless boot a measurement rather than a
smoke test.

Three decisions in `capture`:

- The console log is named per process. A fixed name means two captures
  running at once overwrite each other's, which is exactly what parallel
  boot tests do, and it looks like a kernel that sometimes does not print.
- The emulator's own complaints go to a file beside the log, because when
  it refuses to start the console stays empty and the reason is the only
  thing that would help.
- Having exited is the signal, not having printed nothing. A guest with
  no console prints nothing and is fine (`vz` is exactly that), but a
  guest that had already exited never got as far as trying.

## `mlsh.run=` goes last

It takes the rest of the boot string, because its commands take arguments
and arguments have spaces. Anything appended after it would be eaten as
script. `runtime`, `boot` and the x86-64 `boot` all order it last.

## Why `runtime` uses TCG, and one boot

`mlos runtime` produces a file other repositories render, and a snapshot
that came out differently on somebody else's machine would be worse than
no snapshot. TCG is deterministic, which is the property that matters
here and the same reason the boot tests use it.

Three artifacts come from one boot: the snapshot, the events that led to
it, and the access trace those events record. Two boots would not be the
same run and nothing downstream could tell. The trace is derived from the
events rather than recorded separately, because every acquire is already
in the stream and a second recorder is a second thing to disagree with
the first.

The document goes through the same validator the static emitter uses.
Two emitters that share no code still have to produce one format, and
this is the only place that can tell: the guest has no allocator to check
itself with, and the file is what other repositories read.

`RUNTIME_SECONDS` is generous (12) because TCG is slow: a sweep faults
thirty-odd tiles off a virtio-blk device before the layout is printed,
and a snapshot that timed out half way through would be a valid document
describing a system that never existed.

## Argument parsing rejects, it does not ignore

A tolerant parser turns a typo into a confusing failure somewhere further
down. `options` rejects any option or positional argument it does not
recognise, and `accelerator` checks the host name against `run::HOSTS`
before it can reach QEMU. `takes_host` is false for verbs that accept no
positional argument, so `mlos doctor oops` is told it passed an unexpected
argument rather than that it chose a bad accelerator; those verbs are
validated in one place in `dispatch` rather than each remembering to.

`--console` and `--run` take a value, and `options` drops the value rather
than returning it: `run::boot` reads both back positionally. What the
parser owes them is that neither value reaches the accelerator check,
since `virtio` and a shell script are both things QEMU would object to,
confusingly, on the user's behalf.

## `doctor`

Earns its place because the host requirements differ sharply between a
Mac and the Linux/NVIDIA box, and "why will it not boot" should be
answerable by a command rather than by rereading `docs/architecture.md`
s.8. It never fails: a missing tool is the answer, not an error.

The tools are a table so that adding a check is a row rather than a code
change, and each row carries the flag that makes the tool report a
version: `ffmpeg` wants a single dash, and reporting "missing" because the
flag was wrong is exactly what `doctor` must not do. Accelerators come
from `run::HOSTS` rather than a copy, so the two cannot drift apart.
`probe` returns all of a tool's output, not the first line: `rustup target
list --installed` and `qemu -accel help` both answer with a list, and
checking only the first line reports everything after it as missing.

## The x86-64 lane

`x86.rs` is its own module, reached through one hook in `main`, so the
x86-64 lane (saga `mlos-x86-64`, `lanes/x86/`) and the aarch64 lane never
edit the same lines. Folding the two into one `--arch`-aware path is a
later refactor, once both have merged.

Without `--arch`, `build` and `run` target the host's own architecture,
x86-64 on a Linux PC and aarch64 on Apple Silicon, which is the one it can
accelerate. Every other verb (`doctor`, `layout`, `runtime`, `help`) is
architecture-neutral or aarch64-only for now and keeps the shared path.
`--arch aarch64` takes that path explicitly, stripped and handed back to
`main`'s dispatch, so both architectures are selectable wherever `mlos`
runs.

The QEMU machine is `microvm`, for virtio-mmio, which MLOS speaks, rather
than `q35`'s PCI. `acpi=off`, because with ACPI on `microvm` describes its
virtio-mmio slots in the DSDT and leaves them off the command line, and
the command line is where MLOS reads them (`mlos-pvh`). COM1 is the
console, multiplexed with the QEMU monitor. `-cpu max` so the identity map
can use 1 GiB pages; the 2 MiB fallback is exercised by the boot test with
QEMU's default CPU. The `isa-debug-exit` device is how the guest reports
before it has a console: QEMU exits with `(code << 1) | 1`, and the bits
are `mlos-kernel-x86-64`'s. The model disk is the one the aarch64 guest
gets.

With `--capture`, the x86-64 `boot` kills the guest at the deadline: it
waits for input that a headless boot never sends, so still running is the
normal end. Without `--capture` there is no deadline and it waits for the
guest. The report after the guest ends goes on its own line, because the
guest's last output is usually a prompt.

## Lessons

**A pipe nobody reads is worse than no signal.** A CI runner reported
`missing "MLOS aarch64" in:` with nothing after the colon for ten days,
while QEMU had been saying `failed to find romfile "efi-virtio.rom"` down
a pipe nobody read. `capture` now keeps the emulator's stderr and reports
it when the guest exits with an empty console.

**Let no stray argument reach QEMU.** A stray `#` once arrived as an
accelerator name, and what the user saw was `invalid accelerator #` from
a program they had never typed. That is why `accelerator` exists and why
`options` rejects rather than ignores.

**A far-future `Instant` overflows.** The x86-64 `boot` once used a
far-future deadline to mean "no deadline"; it overflowed `Instant`,
panicked, and left QEMU running behind the shell. Found recording
`demos/x86-64.tape`. The deadline is now an `Option`.

**Trim what `rustc` prints.** `rustc --print sysroot` ends in a newline,
and a path with one in it silently does not exist.
