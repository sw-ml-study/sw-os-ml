// Exception entry stubs. One per vector, so the vector is known without
// asking the CPU; each makes the frame uniform -- vector, error code, then
// what the CPU pushed (rip, cs, rflags, rsp, ss) -- by pushing a zero
// where the CPU pushes no error code. Fatal-fault path only: nothing is
// saved for a return that never happens.

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

    .section .text.trap, "ax"
    .code64
    .set i, 0
    .rept 32
    mlos_stub %i
    .set i, i + 1
    .endr

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
    .rept 32
    mlos_stub_addr %i
    .set i, i + 1
    .endr
