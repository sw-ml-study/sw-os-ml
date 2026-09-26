/* Loaded by QEMU's PVH path: physical == virtual at 1 MiB. */
ENTRY(pvh_start)

SECTIONS
{
    . = 0x100000;
    .text   : ALIGN(4096) { KEEP(*(.text.boot)) *(.text .text.*) }
    .rodata : ALIGN(4096) { *(.rodata .rodata.*) }
    /* The PVH note must survive: QEMU finds the 32-bit entry through it.
     * (sw-os-ml's linker/x86_64.ld currently discards .note* -- it would
     * have to keep this one.) */
    .note.pvh : ALIGN(4) { KEEP(*(.note.pvh)) }
    .data   : ALIGN(4096) { *(.data .data.*) }
    .bss    : ALIGN(4096) { *(.bss .bss.*) *(COMMON) }
    /DISCARD/ : { *(.comment) *(.eh_frame*) }
}
