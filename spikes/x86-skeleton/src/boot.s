// PVH entry: QEMU (and Firecracker, Cloud Hypervisor) jump here in 32-bit
// protected mode, paging off, with the start-info pointer in ebx.
// Job: identity-map the first 1 GiB, enter long mode, call kmain(ebx).

    .section .note.pvh, "a", @note
    .balign 4
    .long 4                 // namesz: "Xen\0"
    .long 4                 // descsz
    .long 18                // XEN_ELFNOTE_PHYS32_ENTRY
    .asciz "Xen"
    .balign 4
    .long pvh_start

    .section .text.boot, "ax"
    .code32
    .global pvh_start
pvh_start:
    cli
    mov esp, offset stack_top
    mov esi, ebx                        // keep start_info for kmain

    mov edi, offset pml4                // zero pml4, pdpt, pd (3 pages)
    xor eax, eax
    mov ecx, 3 * 1024
    rep stosd

    mov eax, offset pdpt
    or eax, 3
    mov dword ptr [pml4], eax
    mov eax, offset pd
    or eax, 3
    mov dword ptr [pdpt], eax

    xor ecx, ecx                        // 512 x 2 MiB pages = 1 GiB
1:  mov eax, ecx
    shl eax, 21
    or eax, 0x83                        // present | writable | huge
    mov dword ptr [pd + ecx * 8], eax
    inc ecx
    cmp ecx, 512
    jne 1b

    mov eax, offset pml4
    mov cr3, eax
    mov eax, cr4
    or eax, 1 << 5                      // PAE
    mov cr4, eax
    mov ecx, 0xC0000080                 // EFER
    rdmsr
    or eax, 1 << 8                      // LME
    wrmsr
    mov eax, cr0
    or eax, 0x80000001                  // PG | PE
    mov cr0, eax

    lgdt [gdt_ptr]
    // Far jump into the 64-bit code segment through a memory far
    // pointer: LLVM's Intel parser encodes `push offset sym` with a
    // 16-bit immediate, which cannot hold an address above 64 KiB.
    jmp fword ptr [long_mode_ptr]

    .code64
long_mode:
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov ss, ax
    xor eax, eax
    mov fs, ax
    mov gs, ax
    lea rsp, [rip + stack_top]
    mov edi, esi                        // kmain(start_info)
    call kmain
2:  hlt
    jmp 2b

    .section .rodata
    .balign 8
gdt:
    .quad 0
    .quad 0x00AF9A000000FFFF            // 0x08: 64-bit code
    .quad 0x00CF92000000FFFF            // 0x10: data
gdt_end:
gdt_ptr:
    .word gdt_end - gdt - 1
    .long gdt
long_mode_ptr:
    .long long_mode
    .word 0x08

    .section .bss
    .balign 4096
pml4: .skip 4096
pdpt: .skip 4096
pd:   .skip 4096
    .balign 16
    .skip 64 * 1024
stack_top:
