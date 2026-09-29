# mlos-kernel-x86-64

The x86-64 half of `mlos-kernel`: what `boot.rs`, `banner.rs` and
`handlers.rs` are for aarch64. Kernel layer. Linked from
`crates/mlos-kernel-x86-64/src/lib.rs`, `banner.rs`, `handlers.rs` and
`machine.rs`.

## Why its own crate

The x86-64 port was developed in its own lane (saga `mlos-x86-64`,
`lanes/x86/`) while the aarch64 kernel was still moving, and two
architectures editing one file is a merge conflict waiting to happen. So
the x86-64 boot path is a crate of its own, and `mlos-kernel` links it
with one line. What turns out to be common moves back into shared crates
later.

## The boot sequence

The console is initialised first, so anything after it can print. Then
the trap crate is installed, so a fault anywhere later is reported rather
than silent. Then the machine is described from what the PVH loader
handed over (`machine`) and reported (`banner`); interrupts are armed
(`handlers`); and `mlsh` runs on COM1, woken by its receive interrupt.

`Ctrl-D` ends the guest through QEMU's `isa-debug-exit` with the report
bits, so a test has an answer that is not text. The bit layout is
duplicated in `mlos-cli/src/x86.rs`; the two must agree. `REACHED`
(`0x40`) is set on every report so that QEMU's own failure statuses can
never be mistaken for one of the kernel's. `TRAPPED` is added when the
trap reporter, not `Ctrl-D`, ended the guest, and the reporter exits with
whatever boot bits had been recorded by then, so a trap during the banner
still says how far boot got.

A machine whose start info cannot be read is a machine the kernel cannot
run on: there is no memory map, so the kernel says so and waits for
`Ctrl-D` rather than limp on with a guessed one.

## The banner

The banner is the first line and names the architecture, `MLOS x86-64`,
the same shape as the aarch64 kernel's `MLOS aarch64`. It then reports
what the entry found (long mode read back from `EFER`, which page size
the identity map used) and what the loader said (the memory map, how
many records did not fit, where the virtio slots came from). Each line is
a fact that was checked, not assumed, which is why `long mode NO` is a
possible output.

## Describing the machine

`machine::discover` is the x86-64 counterpart of
`mlos_machine::Machine::probe`. The memory map comes from
`hvm_start_info`; the virtio-mmio slots and `mlsh.run=` come from the
command line, both parsed by `mlos-pvh`. So `BootInfo` is filled from two
sources here where aarch64 fills it from one device tree.

The kernel image and the loader's own structures (the start info, the
memory map, the command line) occupy RAM the map calls free, and are
carved out with the same `reserve` the aarch64 path uses, so `mem`
reports the same shape on both. The loader's structures are marked
Reclaimable, like the aarch64 DTB: needed until read, free afterwards.

The carve-out is done only inside Usable regions. `reserve` re-labels
whatever it overlaps, and QEMU puts the loader's structures in the BIOS
area when ACPI is off, which the map already calls Reserved. Re-labelling
that Reclaimable would one day hand firmware memory to an allocator.

Memory-map records that do not fit in `Regions` are counted and reported,
never silently lost: a dropped record is memory the kernel does not know
about, and the banner says how many there were.

The command line handed to `mlsh` is the user's part only, without the
`virtio_mmio.device=` entries QEMU appends, because `mlsh.run=` reads to
the end of the line. See `docs/notes/mlos-pvh.md`.

## Interrupts

Two sources, as on aarch64: the LAPIC timer, private to this CPU, at the
2 Hz the shell counts; and COM1's receive line, through the IOAPIC. Twice
a second is slow enough to read and fast enough that a few seconds of
capture shows time passing. The vectors (`0x30` for the timer, `0x34` for
COM1) are clear of the exceptions (0-31) and of where the silenced PIC was
parked (`0xe0` up).

`arm`'s order is load-bearing: the IDT is already in, the PIC is silenced
and the LAPIC enabled before anything is routed to them, the TSC and
timer rates are measured before the timer needs one, and `sti` is last.
Without a LAPIC (never on `microvm`, but not assumed) interrupts are not
armed, the clock rate is zero, `mlsh` is told there is no timer, and COM1
is polled as it was before interrupts existed.

Every handler but the spurious vector's ends with an EOI, or the LAPIC
delivers nothing lower again. The LAPIC base is kept in an atomic, zero
until armed, so the handler can reach it without borrowing anything.

## Idle and `Ctrl-D`

`idle` is what the shell runs between keystrokes. With interrupts armed
it uses `wait_unless`, so a byte that arrived since the shell last looked
is not slept through. `Ctrl-D` ends the guest on the call after it
arrives, not the one it arrives in: the shell drains the queue between
calls, so everything typed before `Ctrl-D` runs first.
