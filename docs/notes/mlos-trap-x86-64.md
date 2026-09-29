# mlos-trap-x86-64

x86-64 exceptions and interrupts: the IDT, one entry stub per vector, and
the report a fatal fault prints. Platform layer, the counterpart of
`mlos-trap-aarch64`. Linked from `crates/mlos-trap-x86-64/src/lib.rs`,
`idt.rs`, `trap.rs` and `entry.s`.

## Why its own crate

The IDT, `lidt` and `cr2` are `unsafe`, and `unsafe` is confined to the
HAL and driver crates. The aarch64 twin has the same contract, and
`describe` prints in the same shape as `mlos_trap_aarch64::describe`, so
a fault reads the same on both architectures.

## Exceptions are fatal, interrupts return

Vectors 0-31 are exceptions and fatal: the entry saves nothing it would
need to return, captures what the CPU says about why it stopped, and hands
that to a reporter that never returns. If no reporter is installed the
CPU parks with interrupts off. The gates are interrupt gates, so
interrupts stay off in the handler, which is what a fatal-fault path
wants: nothing may run between the fault and the report.

Vectors 32-255 are interrupts and do return: the common path saves every
register the SysV ABI lets a Rust function clobber, calls the installed
`on_irq` with the vector, restores, and `iretq`s. No SSE state is saved
because the bare target is soft-float and the kernel never touches it.
The end-of-interrupt is the handler's business, not this crate's: which
controller to acknowledge, and whether the vector needs it at all (the
spurious one does not), is knowledge this crate does not have.

## One stub per vector

There are 256 stubs so the vector is known without asking the CPU. The
exception stubs make the frame uniform (vector, error code, then what the
CPU pushed: `rip`, `cs`, `rflags`, `rsp`, `ss`) by pushing a zero where
the CPU pushes no error code. The vectors that carry one are 8, 10-14,
17, 21, 29 and 30. `mlos_trap_dispatch` receives a pointer to that frame
and reads it as seven words.

The interrupt path keeps the stack 16-byte aligned for the call: the CPU
aligned its five-word frame to 16, the stub pushed one word, and the
common path pushes nine registers and one pad word.

## Handlers as atomic function pointers

The reporter and `on_irq` are stored as `AtomicPtr` and are plain `fn`s,
for the same reason as on aarch64: a handler runs at an arbitrary moment
and must not depend on anything it might have borrowed. `install`
requires both to be safe to call from an interrupt: no allocation, no lock the
interrupted code could hold, no assumption about which stack they are on.

## Reading `CR2`

`Trap::capture` reads `CR2` immediately, because it holds the address a
page fault was trying to reach only until the next fault overwrites it.
For every other vector the value is stale, and the report says so by
printing it unconditionally: `cr2` means something only under `#PF`.
