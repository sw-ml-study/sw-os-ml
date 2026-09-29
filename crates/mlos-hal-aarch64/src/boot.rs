//! The entry point, and the arm64 `Image` header that gets us the tree.
//!
//! Invariant: nothing here touches the stack until `sp` is set, and `x0`
//! reaches `mlos_main` unclobbered. Design and history:
//! docs/notes/mlos-hal-aarch64.md.

/// The arm64 Linux `Image` header, at image offset 0. `text_offset` is
/// 2 MiB with the placement flag clear, so the loader puts us at DRAM
/// base + 2 MiB: where `linker/aarch64.ld` links for.
#[unsafe(naked)]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.boot")]
pub extern "C" fn _start() -> ! {
    core::arch::naked_asm!(
        "b    {entry}",             // code0
        ".long 0",                  // code1
        ".quad 0x200000",           // text_offset: DRAM base + 2 MiB
        ".quad __image_size",       // image_size
        ".quad 0x2",                // flags: little-endian, 4 KiB pages
        ".quad 0",                  // res2
        ".quad 0",                  // res3
        ".quad 0",                  // res4
        ".ascii \"ARM\\x64\"",      // magic, 0x644d5241 little-endian
        ".long 0",                  // res5
        entry = sym primary_entry,
    )
}

/// Where the header branches: park every core but the boot core, install
/// the boot stack, zero `.bss`, branch to the kernel. Naked, because a
/// prologue would push to a stack that does not exist yet. `x0` is never
/// clobbered; only `x1` and `x2` are scratch.
#[unsafe(naked)]
#[unsafe(link_section = ".text.entry")]
extern "C" fn primary_entry() -> ! {
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
