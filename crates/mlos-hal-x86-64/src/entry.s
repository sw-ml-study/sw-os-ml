// PVH entry for MLOS x86-64. Assembled by `global_asm!` in boot.rs.
//
// Arrives in 32-bit protected mode, paging off, interrupts off, with the
// hvm_start_info pointer in ebx. Leaves in long mode, on a 64 KiB stack,
// with .bss zeroed and the first 1 GiB identity-mapped, calling
// `mlos_main(start_info)`. Nothing else: everything a device tree told
// the aarch64 kernel is read from start_info later, in Rust.

    // The note QEMU finds the entry through. linker/x86_64.ld keeps it.
    .section .note.pvh, "a", @note
    .balign 4
    .long 4                     // namesz: "Xen\0"
    .long 4                     // descsz
    .long 18                    // XEN_ELFNOTE_PHYS32_ENTRY
    .asciz "Xen"
    .balign 4
    .long _start

    .section .text.boot, "ax"
    .code32
    .global _start
_start:
    cli
    cld
    mov esi, ebx                // start_info survives in esi

    // Zero .bss first: the page tables and the stack live there.
    mov edi, offset __bss_start
    mov ecx, offset __bss_end
    sub ecx, edi
    xor eax, eax
    rep stosb
    mov esp, offset mlos_boot_stack_top

    // PML4[0] -> PDPT. The rest of the map depends on the CPU.
    mov eax, offset mlos_pdpt
    or eax, 3                   // present | writable
    mov dword ptr [mlos_pml4], eax

    mov eax, 0x80000001         // pdpe1gb is CPUID.80000001h:EDX[26]
    cpuid
    bt edx, 26
    jnc 2f

    // 1 GiB page: PDPT[0] maps 0..1 GiB directly.
    mov dword ptr [mlos_pdpt], 0x83   // present | writable | page size
    mov byte ptr [mlos_gigabyte_pages], 1
    jmp 3f

    // Fallback: PDPT[0] -> PD of 512 x 2 MiB pages.
2:  mov eax, offset mlos_pd
    or eax, 3
    mov dword ptr [mlos_pdpt], eax
    xor ecx, ecx
1:  mov eax, ecx
    shl eax, 21
    or eax, 0x83
    mov dword ptr [mlos_pd + ecx * 8], eax
    inc ecx
    cmp ecx, 512
    jne 1b

3:  mov eax, offset mlos_pml4
    mov cr3, eax
    mov eax, cr4
    or eax, 1 << 5              // PAE
    mov cr4, eax
    mov ecx, 0xC0000080         // EFER
    rdmsr
    or eax, 1 << 8              // LME
    wrmsr
    mov eax, cr0
    or eax, 0x80000001          // PG | PE
    mov cr0, eax

    lgdt [mlos_gdt_ptr]
    // Through a memory far pointer: LLVM's Intel parser encodes
    // `push offset sym` with a 16-bit immediate, which cannot hold an
    // address above 64 KiB (found in spikes/x86-skeleton).
    jmp fword ptr [mlos_long_mode_ptr]

    .code64
mlos_long_mode:
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov ss, ax
    xor eax, eax
    mov fs, ax
    mov gs, ax
    mov edi, esi                // mlos_main(start_info), zero-extended
    call mlos_main
4:  hlt                         // mlos_main does not return; if it does,
    jmp 4b                      // park rather than run off the end

    .section .rodata.boot, "a"
    .balign 8
mlos_gdt:
    .quad 0
    .quad 0x00AF9A000000FFFF    // 0x08: 64-bit code
    .quad 0x00CF92000000FFFF    // 0x10: data
mlos_gdt_end:
mlos_gdt_ptr:
    .word mlos_gdt_end - mlos_gdt - 1
    .long mlos_gdt
mlos_long_mode_ptr:
    .long mlos_long_mode
    .word 0x08

    .section .bss.boot, "aw", @nobits
    .balign 4096
mlos_pml4: .skip 4096
mlos_pdpt: .skip 4096
mlos_pd:   .skip 4096
    .global mlos_gigabyte_pages
mlos_gigabyte_pages: .skip 1
    .balign 16
    .skip 64 * 1024
mlos_boot_stack_top:
