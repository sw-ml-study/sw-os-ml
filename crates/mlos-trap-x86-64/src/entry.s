// Entry stubs for all 256 vectors, one per vector so the vector is known
// without asking the CPU.
//
// 0-31 are exceptions, and fatal: each makes the frame uniform -- vector,
// error code, then what the CPU pushed (rip, cs, rflags, rsp, ss) -- by
// pushing a zero where the CPU pushes no error code, and nothing is saved
// for a return that never happens.
//
// 32-255 are interrupts, and return: the common path saves every
// register the SysV ABI lets a Rust function clobber, calls
// `mlos_irq_dispatch(vector)`, restores, and `iretq`s. No SSE state: the
// bare target is soft-float, so the kernel never touches it.

    .altmacro
    .macro mlos_stub n
    .global mlos_trap_stub_\n
mlos_trap_stub_\n:
    // Vectors with a CPU-pushed error code: 8, 10-14, 17, 21, 29, 30.
    .if (\n == 8) || (\n >= 10 && \n <= 14) || (\n == 17) || (\n == 21) || (\n == 29) || (\n == 30)
    .else
    push 0
    .endif
    push \n
    jmp mlos_trap_common
    .endm

    .macro mlos_irq_stub n
    .global mlos_trap_stub_\n
mlos_trap_stub_\n:
    push \n
    jmp mlos_irq_common
    .endm

    .section .text.trap, "ax"
    .code64
    .set i, 0
    .rept 32
    mlos_stub %i
    .set i, i + 1
    .endr
    .rept 224
    mlos_irq_stub %i
    .set i, i + 1
    .endr

mlos_irq_common:
    // On entry: vector, then the CPU's 5-word frame, which the CPU aligned
    // to 16 bytes -- so rsp is 16-aligned here. Nine pushes and one pad
    // word keep it aligned for the call.
    push rax
    push rcx
    push rdx
    push rsi
    push rdi
    push r8
    push r9
    push r10
    push r11
    sub rsp, 8
    mov rdi, [rsp + 80]         // the vector the stub pushed
    cld
    call mlos_irq_dispatch
    add rsp, 8
    pop r11
    pop r10
    pop r9
    pop r8
    pop rdi
    pop rsi
    pop rdx
    pop rcx
    pop rax
    add rsp, 8                  // the vector
    iretq

mlos_trap_common:
    mov rdi, rsp                // frame: [vector, error, rip, cs, rflags, rsp, ss]
    and rsp, -16                // SysV alignment for the Rust side
    call mlos_trap_dispatch     // never returns

    .macro mlos_stub_addr n
    .quad mlos_trap_stub_\n
    .endm

    .section .rodata.trap, "a"
    .balign 8
    .global mlos_trap_stubs
mlos_trap_stubs:
    .set i, 0
    .rept 256
    mlos_stub_addr %i
    .set i, i + 1
    .endr
