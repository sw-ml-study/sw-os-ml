//! The entry point.

/// Where the loader jumps.
///
/// Naked, because a compiler-emitted prologue would push to a stack that
/// does not exist yet. Step 001 established that empirically: the pushed
/// `str x30, [sp, #-0x10]!` faulted into the zero vector table at
/// `VBAR_EL1 + 0x200` before any of our code retired, under both TCG and
/// HVF. Nothing here may touch the stack until `sp` is set.
///
/// In order: park every core but the boot core, install the boot stack,
/// zero `.bss`, and branch to the kernel.
///
/// `x0` holds the device tree pointer on entry and is never clobbered, so
/// the kernel receives it as its first argument. Only `x1` and `x2` are
/// used as scratch.
///
/// Secondary cores park rather than spin into the kernel: MLOS is
/// single-core until there is a scheduler for them to enter, and a core
/// running the boot path twice corrupts what the first one built.
#[unsafe(naked)]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.boot")]
pub extern "C" fn _start() -> ! {
    core::arch::naked_asm!(
        "mrs  x1, mpidr_el1", // this core's id
        "and  x1, x1, #0xff", // Aff0; boot core is 0 on virt
        "cbnz x1, 3f",
        "adrp x1, __stack_top", // sp before anything else
        "add  x1, x1, :lo12:__stack_top",
        "mov  sp, x1",
        "adrp x1, __bss_start",
        "add  x1, x1, :lo12:__bss_start",
        "adrp x2, __bss_end",
        "add  x2, x2, :lo12:__bss_end",
        "1:  cmp  x1, x2", // zero .bss, 8 bytes at a time
        "    b.hs 2f",
        "    str  xzr, [x1], #8",
        "    b    1b",
        "2:  b    mlos_main", // x0 still holds the DTB pointer
        "3:  wfe",            // secondary cores stop here
        "    b    3b",
    )
}
