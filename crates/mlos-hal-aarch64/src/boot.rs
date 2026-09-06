//! The entry point, and the header that gets us booted properly.

/// The arm64 Linux `Image` header, at image offset 0.
///
/// Not decoration. Step 004 found `x0` arriving as zero: QEMU jumps
/// straight to an ELF's entry point, skipping the arm64 boot protocol that
/// puts the device tree pointer there. It *does* place a device tree in
/// RAM regardless -- a probe found `d00dfeed` at `0x4000_0000` -- but
/// finding it by scanning for a magic number is a hack we would carry
/// forever. This header is the documented way to be handed it.
///
/// It is wanted anyway: `Virtualization.framework` boots an uncompressed
/// raw arm64 image and nothing else (`docs/architecture.md` s.8.1), which
/// is step 010.
///
/// Layout is fixed by the protocol:
///
/// ```text
///   0  code0        branch past the header
///   4  code1        0
///   8  text_offset  where to load, relative to the base of DRAM
///  16  image_size   memory footprint, including .bss and the stack
///  24  flags        bit 0 endianness, bits 1-2 page size, bit 3 placement
///  32  res2 res3 res4
///  56  magic        "ARM\x64"
///  60  res5         PE/COFF offset, unused
/// ```
///
/// `text_offset` is `0x20_0000` and the placement flag is clear, so the
/// loader must put us at DRAM base + 2 MiB -- which is the `0x4020_0000`
/// the linker script links for. Setting the "load me anywhere" flag
/// instead would let the loader pick, and position-dependent code linked
/// at a fixed address would then land somewhere it was not linked for.
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

/// Where the header branches.
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
/// `x0` holds the device tree pointer and is never clobbered, so the
/// kernel receives it as its first argument. Only `x1` and `x2` are used
/// as scratch.
///
/// Secondary cores park rather than spin into the kernel: MLOS is
/// single-core until there is a scheduler for them to enter, and a core
/// running the boot path twice corrupts what the first one built.
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
