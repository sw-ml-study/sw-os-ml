# mlos-k-ffi

The K bridge links the portable `kbm-fork/ksrc` C core into the ARM64 MLOS
kernel. The C build is deliberately freestanding and passes only integers and
pointers through `k_sys`, so the Rust kernel owns the device boundary.

K runs as an `mlsh` command on the boot core. Its read callback drains the
interrupt-driven MLOS input queue and waits with `wfi` when the queue is
empty; its write callback uses the boot-published terminal. The first slice
uses a 256 MiB K heap (`KHEAP=22`) so it fits the existing 512 MiB guest.

The AArch64 FP/SIMD enable is scoped around K. MLOS interrupt and shell code
remains soft-float, so it does not touch the vector registers while K runs.
The exit syscall currently returns to K; a later trampoline step should make
`\\` return directly to `mlsh`.
