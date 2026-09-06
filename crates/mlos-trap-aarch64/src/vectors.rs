//! The vector table.
//!
//! Sixteen entries, each 128 bytes, the whole table aligned to 2048 --
//! `VBAR_EL1` has no bits for anything finer. The linker script places
//! `.text.vectors` on that boundary.
//!
//! Every entry does the same two things: load its own index and branch.
//! That is all 128 bytes needs to hold, and keeping the assembly to the
//! irreducible minimum is deliberate -- code reached only when something
//! has already gone wrong is code that gets tested least.

use crate::{irq, report};

/// Caller-saved state, plus the two registers `eret` consumes.
///
/// `x0`-`x18`, `x29`, `x30`, `ELR_EL1` and `SPSR_EL1`: 23 values, rounded
/// to 24 slots because the stack pointer must stay 16-byte aligned.
/// Callee-saved registers are absent on purpose -- the handler is
/// `extern "C"`, so the compiler already preserves them, and saving them
/// twice costs every interrupt for the benefit of none.
const FRAME: usize = 192;

/// The macros hard-code the frame size; this is what keeps the constant
/// above honest about it.
const _: () = assert!(FRAME == 192 && FRAME % 16 == 0);

/// Pushes the interrupted context.
macro_rules! save {
    () => {
        concat!(
            "sub sp, sp, #",
            stringify!(192),
            "\n",
            "stp x0,  x1,  [sp, #0]\n",
            "stp x2,  x3,  [sp, #16]\n",
            "stp x4,  x5,  [sp, #32]\n",
            "stp x6,  x7,  [sp, #48]\n",
            "stp x8,  x9,  [sp, #64]\n",
            "stp x10, x11, [sp, #80]\n",
            "stp x12, x13, [sp, #96]\n",
            "stp x14, x15, [sp, #112]\n",
            "stp x16, x17, [sp, #128]\n",
            "stp x18, x29, [sp, #144]\n",
            "mrs x0, elr_el1\n",
            "mrs x1, spsr_el1\n",
            "stp x30, x0,  [sp, #160]\n",
            "str x1, [sp, #176]\n",
        )
    };
}

/// Pops it again, restoring `ELR_EL1` and `SPSR_EL1` last but one so
/// `eret` returns to exactly the instruction that was interrupted.
macro_rules! restore {
    () => {
        concat!(
            "ldr x1, [sp, #176]\n",
            "ldp x30, x0, [sp, #160]\n",
            "msr elr_el1, x0\n",
            "msr spsr_el1, x1\n",
            "ldp x18, x29, [sp, #144]\n",
            "ldp x16, x17, [sp, #128]\n",
            "ldp x14, x15, [sp, #112]\n",
            "ldp x12, x13, [sp, #96]\n",
            "ldp x10, x11, [sp, #80]\n",
            "ldp x8,  x9,  [sp, #64]\n",
            "ldp x6,  x7,  [sp, #48]\n",
            "ldp x4,  x5,  [sp, #32]\n",
            "ldp x2,  x3,  [sp, #16]\n",
            "ldp x0,  x1,  [sp, #0]\n",
            "add sp, sp, #",
            stringify!(192),
            "\n",
        )
    };
}

/// A fault entry: aligned to its slot, tagged with its index, reported.
macro_rules! vector {
    ($index:literal) => {
        concat!(".balign 0x80\n", "mov x0, #", $index, "\n", "b {report}\n")
    };
}

/// An IRQ entry. Unlike a fault, this one has to come back: it saves the
/// interrupted context, dispatches, restores, and `eret`s.
macro_rules! interrupt {
    () => {
        concat!(
            ".balign 0x80\n",
            save!(),
            "bl {irq}\n",
            restore!(),
            "eret\n"
        )
    };
}

/// The table `VBAR_EL1` points at.
///
/// Naked and in its own section: the compiler must emit nothing before
/// the first entry, because `VBAR_EL1` addresses the table itself and
/// anything at offset 0 that is not entry 0 is taken as entry 0.
#[unsafe(naked)]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.vectors")]
pub extern "C" fn vector_table() -> ! {
    core::arch::naked_asm!(
        vector!(0), interrupt!(), vector!(2), vector!(3),    // Current EL, SP0
        vector!(4), interrupt!(), vector!(6), vector!(7),    // Current EL, SPx
        vector!(8), interrupt!(), vector!(10), vector!(11),  // Lower EL, AArch64
        vector!(12), interrupt!(), vector!(14), vector!(15), // Lower EL, AArch32
        report = sym report,
        irq = sym irq,
    )
}
