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
        /* Two KEEPs, in this order, because the arm64 Image header must be
         * at image offset 0. `.text.boot` holds only the header; the code
         * it branches to lives in `.text.entry`. One section for both
         * would leave their order up to the linker. */
        KEEP(*(.text.boot))
        KEEP(*(.text.entry))
        /* VBAR_EL1 has no bits below 11, so the vector table must start on
         * a 2048-byte boundary. Aligning it here rather than inside the
         * table means the assembly does not have to pad itself. */
        . = ALIGN(2048);
        KEEP(*(.text.vectors))
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

    /* Boot stack. The entry point sets SP to __stack_top before it does
     * anything else, because the loader leaves SP undefined -- see step
     * 001, where a compiler-emitted prologue faulted on that. 64 KiB is
     * generous for bring-up and shrinks once there is anything to measure.
     * In .bss, so it costs image bytes but not file bytes. */
    . = ALIGN(4096);
    __stack_bottom = .;
    . += 0x10000;
    __stack_top = .;

    . = ALIGN(4096);
    __kernel_end = .;

    /* The Image header's image_size: the MEMORY footprint, not the file
     * size. It includes .bss and the boot stack, neither of which occupies
     * file bytes, so the loader knows how much RAM to leave alone. */
    __image_size = __kernel_end - __kernel_start;

    /DISCARD/ : { *(.comment) *(.note*) *(.eh_frame*) }
}
