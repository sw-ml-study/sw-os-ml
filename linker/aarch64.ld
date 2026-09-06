/* MLOS kernel image layout, aarch64.
 *
 * QEMU's `virt` machine places RAM at 0x4000_0000. The image starts 2 MiB
 * above that: QEMU's own loader and, on the UEFI path, firmware structures
 * occupy the bottom of RAM, and 2 MiB is one aarch64 block-descriptor
 * mapping, so the kernel begins on a boundary the page tables can map with
 * a single entry once step 004 turns the MMU on.
 */

ENTRY(_start)

KERNEL_BASE = 0x40200000;

SECTIONS
{
    . = KERNEL_BASE;
    __kernel_start = .;

    /* .text.boot first and KEEP: the entry point must be at the image
     * base, because a bare `-kernel` boot jumps to the load address. */
    .text : ALIGN(4096) {
        KEEP(*(.text.boot))
        *(.text .text.*)
    }

    .rodata : ALIGN(4096) { *(.rodata .rodata.*) }
    .data   : ALIGN(4096) { *(.data .data.*) }

    .bss : ALIGN(4096) {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        . = ALIGN(16);
        __bss_end = .;
    }

    . = ALIGN(4096);
    __kernel_end = .;

    /DISCARD/ : { *(.comment) *(.note*) *(.eh_frame*) }
}
