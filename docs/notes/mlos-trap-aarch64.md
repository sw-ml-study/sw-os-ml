# mlos-trap-aarch64

aarch64 exception vectors: faults are captured, reported and stop; IRQs
save the interrupted context, dispatch, and return. Platform layer.
Linked from `crates/mlos-trap-aarch64/src/lib.rs`, `trap.rs` and
`vectors.rs`.

## Why the crate is empty on other architectures

`#![cfg(target_arch = "aarch64")]` makes the crate compile to nothing
elsewhere, so the workspace-wide gate can sweep every crate without a
hand-maintained exclude list. The crate says where it applies; a list in
`.cargo/config.toml` would say it somewhere else and then drift, which is
what happened before the line existed.

## Why a vector table at all

Before this crate, `VBAR_EL1` was zero and every exception branched into
unmapped nothing. Diagnosing that took a disassembler and a register
dump. The point of a vector table is that the machine tells you instead.

## Reporting only, except for IRQs

No fault handler resumes. A fault that reaches these vectors today is a
bug, and stopping at it is the correct response: `report` reads the
syndrome first, before anything else can overwrite it, hands it to
whoever registered, and parks if nobody did, since there is no way to
report and no state worth returning to.

The IRQ path is the one exception and returns. That is the whole
difference between the two: an interrupt is not a bug, and the work it
interrupted is still worth finishing. `irq` is entered with the
interrupted context already on the stack (the `save!` macro) and
`x19`-`x28` the compiler's problem.

## Handlers are raw function pointers in atomics

`REPORTER` and `HANDLER` are `AtomicUsize` rather than `static mut`, and
hold a plain `fn` rather than a closure, because a handler runs on an
arbitrary stack at an arbitrary moment and must not depend on anything it
might have borrowed. `install` is the only writer, so the `transmute`
back to a function pointer is sound.

## Unmask last, and separately

Everything up to `unmask` can be got wrong quietly, but an unmasked
interrupt with a half-built controller behind it fires immediately and
repeatedly, which is much harder to read than a machine that simply
never ticks. So the unmask is its own call, made after the vector table
is installed and the controller is up.

## Which vector fired is diagnostic on its own

`VECTOR_NAMES` names all sixteen entries. A fault from `CurrentSpx` is
the kernel's own bug; the same fault from `Lower64` is a userspace one,
and long before there is userspace, seeing `Lower64` at all would mean
something is very wrong. Exception class `0x25` is a data abort from the
current EL: overwhelmingly the one a kernel meets first, and what a wrong
page table produces.

`describe` lives with `Trap` rather than in the kernel because it is
entirely about this type: which registers matter, and what their bits
mean.

## The frame

`FRAME` is `x0`-`x18`, `x29`, `x30`, `ELR_EL1` and `SPSR_EL1`: 23 values,
rounded to 24 slots because the stack pointer must stay 16-byte aligned.
Callee-saved registers are absent on purpose. The handler is `extern
"C"`, so the compiler already preserves them, and saving them twice costs
every interrupt for the benefit of none. The macros hard-code the frame
size, and the `const` assertion is what keeps `FRAME` honest about it.
`restore!` writes `ELR_EL1` and `SPSR_EL1` back before restoring `x0`
and `x1`, so `eret` returns to exactly the instruction that was
interrupted.

## The table

Sixteen entries of 128 bytes, the whole table aligned to 2048 because
`VBAR_EL1` has no bits for anything finer; the linker script places
`.text.vectors` on that boundary. `vector_table` is naked and in its own
section because the compiler must emit nothing before the first entry:
`VBAR_EL1` addresses the table itself, and anything at offset 0 that is
not entry 0 is taken as entry 0.

Every fault entry does the same two things, load its own index and
branch. Keeping the assembly to the irreducible minimum is deliberate:
code reached only when something has already gone wrong is code that
gets tested least.

## Lessons

**A zero `VBAR_EL1` is a silent machine.** Step 001 saw a stack push
fault and the machine spin at `0x200` with no way to say why. Install the
vectors before anything can trap, and before any other bring-up step that
could fault.
