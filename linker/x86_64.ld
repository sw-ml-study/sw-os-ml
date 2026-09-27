/* MLOS kernel image layout, x86-64.
 *
 * Loaded by QEMU's PVH path (saga mlos-x86-64): physical == virtual at
 * 1 MiB, the conventional load address below which legacy structures and
 * firmware live. The entry is `_start` in mlos-hal-x86-64's entry.s.
 */

ENTRY(_start)

KERNEL_BASE = 0x100000;

SECTIONS
{
    . = KERNEL_BASE;
    __kernel_start = .;

    .text : ALIGN(4096) {
        KEEP(*(.text.boot))
        *(.text .text.*)
    }

    .rodata : ALIGN(4096) { *(.rodata .rodata.*) }

    /* The PVH note is how QEMU finds the 32-bit entry. Kept explicitly:
     * the /DISCARD/ below drops every other note, and without this one
     * QEMU refuses the ELF ("Error loading uncompressed kernel without
     * PVH ELF Note"). */
    .note.pvh : ALIGN(4) { KEEP(*(.note.pvh)) }
    .data   : ALIGN(4096) { *(.data .data.*) }

    .bss : ALIGN(4096) {
        __bss_start = .;
        /* The boot page tables first: the far end of .bss from the stack,
         * so an overflow is not the thing that takes them out. */
        *(.bss.boot)
        *(.bss .bss.*)
        *(COMMON)
        . = ALIGN(16);
        __bss_end = .;
    }

    /* Boot stack, as linker/aarch64.ld has it and for the same reason:
     * 256 KiB, because `Manager` is built on the stack before it is moved
     * into a static, and a 64 KiB stack ran off the bottom on `model` --
     * through the page tables, here, so the guest triple-faulted. Memory
     * is identity-mapped in large pages, so there is no guard page; a 4 KiB
     * guard needs 4 KiB mappings for the image, which is later work. */
    . = ALIGN(4096);
    __stack_bottom = .;
    . += 0x40000;
    __stack_top = .;

    . = ALIGN(4096);
    __kernel_end = .;

    /DISCARD/ : { *(.comment) *(.note*) *(.eh_frame*) }
}
