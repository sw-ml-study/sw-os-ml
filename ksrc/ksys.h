// ksys.h -- k's OS boundary as one C-ABI function, for -DKSYS builds.
//
// The original z.h binds k's eight OS entry points (exit, read, write, close,
// open, fstat, mmap, munmap) with per-OS inline assembly. With KSYS defined
// they all become calls to a single function the host provides:
//
//     U k_sys(U nr, U a, U b, U c, U d, U e, U f);
//
// `nr` uses x86-64 Linux numbering, the numbering kbm's s.asm already speaks:
//   60 exit   0 read   1 write   3 close   2 open   5 fstat   9 mmap  11 munmap
// fstat must fill the x86-64 Linux `struct stat` layout (z.c reads st_mode as
// the low half of word 3 and st_size as word 6). A host that does not support
// a call returns (U)-38 (-ENOSYS); k then treats files as unavailable.
//
// Only integers and pointers cross this boundary, so a k object compiled with
// FP/SIMD enabled can be linked into a softfloat kernel (sw-os-ml): the two
// AArch64 ABIs differ only in how FP arguments are passed.
#ifndef KSYS_H
#define KSYS_H

extern U k_sys(U, U, U, U, U, U, U);
// The originals are variadic `(Ux,...)` functions. That only works where
// every argument is 64 bits; on ILP32 targets (32-bit ARM) pointers and ints
// are 4 bytes and va_arg(U) misreads them. So each entry point becomes a
// macro that casts each argument to U and makes one fixed-arity, prototyped
// call, so the arguments are passed correctly on any ABI. z.h's O(f,i) list then expands to
// nothing; the numbers live here.
// KCALL(nr, args...) casts each of 1..6 arguments to U explicitly (pointers
// included) and calls k_sys with the rest zero.
#define KC1(n,p1) k_sys(n,(U)(p1),0,0,0,0,0)
#define KC2(n,p1,p2) k_sys(n,(U)(p1),(U)(p2),0,0,0,0)
#define KC3(n,p1,p2,p3) k_sys(n,(U)(p1),(U)(p2),(U)(p3),0,0,0)
#define KC4(n,p1,p2,p3,p4) k_sys(n,(U)(p1),(U)(p2),(U)(p3),(U)(p4),0,0)
#define KC5(n,p1,p2,p3,p4,p5) k_sys(n,(U)(p1),(U)(p2),(U)(p3),(U)(p4),(U)(p5),0)
#define KC6(n,p1,p2,p3,p4,p5,p6) k_sys(n,(U)(p1),(U)(p2),(U)(p3),(U)(p4),(U)(p5),(U)(p6))
#define KPICK(p1,p2,p3,p4,p5,p6,N,...) N
#define KCALL(n,...) KPICK(__VA_ARGS__,KC6,KC5,KC4,KC3,KC2,KC1,_)(n,__VA_ARGS__)
#define _k(...) KCALL(60, __VA_ARGS__)
#define _w(...) KCALL(0, __VA_ARGS__)
#define w_(...) KCALL(1, __VA_ARGS__)
#define _d(...) KCALL(3, __VA_ARGS__)
#define d_(...) KCALL(2, __VA_ARGS__)
#define _n(...) KCALL(5, __VA_ARGS__)
#define m_(...) KCALL(9, __VA_ARGS__)
#define _m(...) KCALL(11, __VA_ARGS__)
#define O(f,i)

// Cycle counter for k's \t timing.
#if __x86_64
AS(ut,"rdtsc;shl $32,%rdx;or %rdx,%rax;")
#elif __aarch64__
AS(ut,"mrs x0,cntvct_el0\nmov x1,100\nmul x0,x0,x1\n")
#elif __riscv && __riscv_xlen==64
AS(ut,"rdtime a0\n")
#elif __riscv && __riscv_xlen==32
// 64-bit cycle count from two 32-bit halves; re-read if the high half rolled.
ZU ut(void){unsigned hi,lo,hi2;do{__asm__ volatile("rdcycleh %0":"=r"(hi));
 __asm__ volatile("rdcycle %0":"=r"(lo));__asm__ volatile("rdcycleh %0":"=r"(hi2));}
 while(hi!=hi2);return (U)hi<<32|lo;}
#elif __XTENSA__
// CCOUNT: 32-bit cycle counter (wraps after ~18 s at 240 MHz; fine for \t).
ZU ut(void){unsigned kcc;__asm__ volatile("rsr %0, ccount":"=a"(kcc));return kcc;}
#elif __arm__
// No user-readable cycle counter is guaranteed on 32-bit ARM Linux;
// \t timings read 0. A host can provide one through k_sys later.
ZU ut(void){return 0;}
#else
#error "ksys.h: no cycle counter for this architecture"
#endif

#if __AVX512F__
UV(bg,B(ia32_cvtb2mask512)(a))
#else
#define KVEC_BG_ONLY
#include"kvec.h"
#undef KVEC_BG_ONLY
#endif

#endif
