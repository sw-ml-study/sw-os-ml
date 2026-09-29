# mlos-hal

The hardware abstraction layer: one trait, four methods, declarations
only. HAL layer. Linked from `crates/mlos-hal/src/lib.rs`, `boot.rs` and
`page.rs`.

## Why its own crate

Nothing above this crate may name a page size, a privilege level, a
specific interrupt controller, or an atomics width. That is not
tidiness. It is the concession that keeps `mlos-hal-riscv64` an additive
change rather than a refactor when RISC-V's GPU support matures
(`docs/architecture.md` s.9).

The trait is small on purpose. A wide HAL is a HAL that has started
leaking architecture into the kernel, and the leak is always found later
than the widening. Implementations live in `mlos-hal-aarch64` and
`mlos-hal-x86-64`, which is where `unsafe` belongs.

## Not `forbid(unsafe_code)`

`PageTable`'s methods are `unsafe fn` declarations, and they should be:
installing a mapping can alias memory arbitrarily, and nothing here can
check the caller's claim. There are no `unsafe` blocks in this crate.

## Static dispatch for page tables, `dyn` for the rest

`Platform::PageTable` is an associated type rather than a `dyn` object
because mapping is on the model-fault path, and a virtual call per
mapping is a cost the fault budget in `docs/architecture.md` s.4 will
not stand. The console, timer and interrupt controller are `dyn`: none of
them is hot. The whole `Platform` is held by the kernel as a single value
so a test or the host-side simulator can substitute one that touches no
hardware at all.

## Boot info is borrowed, and built once

`BootInfo::regions` is a borrowed slice rather than an owned collection
because there is no allocator when it is built; the memory map is what
the allocator is built from. It is produced once, by architecture-specific
code, from whatever the platform offers (a device tree on aarch64, ACPI
tables on x86-64) and is immutable afterwards. The kernel never learns
which it was.

`usable_bytes` is the number every residency budget in `docs/PRD.md` is a
fraction of, so it is computed once from the map rather than guessed.
`MemoryKind::Reclaimable` is worth acting on: on a machine sized by its
residency budget, tens of megabytes of firmware tables is real.
`MemoryKind::Kernel` is neither allocatable nor reclaimable, because we
are running out of it.

## Page flags are booleans, and the interface names no page size

The bit encodings of permissions differ per architecture, so encoding
them in `PageFlags` would put architecture above the HAL, which is
exactly what this crate exists to prevent. The `device` flag exists on
its own because the wrong memory type on an MMIO range is the classic way
a driver appears to work until it does not.

Sizes and level counts are the architecture's business. `PageTable`
speaks in byte ranges; an implementation is free to satisfy a range with
4 KiB leaves, 2 MiB blocks, or whatever its tables offer. `activate` has
no way to report failure: getting it wrong faults on the instruction
after the switch.
