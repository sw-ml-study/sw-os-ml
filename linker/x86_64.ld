/* MLOS kernel image layout, x86-64.
 *
 * Placeholder. x86-64 is milestone M6 (docs/plan.md); this script exists so
 * the target builds and stays building, not because the layout is settled.
 * 1 MiB is the conventional load address below which legacy structures and
 * firmware live.
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
