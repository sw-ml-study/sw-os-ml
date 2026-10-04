// kheap.h -- size of k's heap and object table, and what happens when full.
//
// k's allocator (b.c) is a buddy system over one static array `_` of 64-byte
// blocks: size class i hands out blocks of 64<<i bytes, and starts with one
// free block at byte offset 64*(2^i - 1). An object's handle indexes `O`,
// whose entries hold the block pointer and its class.
//
// Default (KHEAP undefined): kbm's original layout, unchanged -- 2^24 blocks
// (1 GiB), 30 classes, 4096 handles, no checks. Classes 24..29 start beyond
// the array; k simply never gets that far with 1 GiB.
//
// -DKHEAP=n (requires -DKSYS): 2^n blocks (64<<n bytes) and n classes, so every
// class's block lies inside the array. Exhausting the heap or the handle table
// prints "wsfull" and exits instead of writing past the end. The array is
// also 64-byte aligned: k recovers pointers by masking with ...ffc0.
//   KHEAP=24 1 GiB   20 64 MiB   18 16 MiB   16 4 MiB   12 256 KiB
// -DKOBJ=n: 2^n handles (default 12 = 4096; the handle field is 12 bits).
// The table costs 8 bytes per handle.
// -DKHEAP_ATTR='...': extra attribute on the heap array, e.g. to place it in
// external PSRAM under ESP-IDF: -DKHEAP_ATTR='__attribute__((section(".ext_ram.bss")))'.
#ifndef KHEAP_H
#define KHEAP_H

#ifndef KOBJ
#define KOBJ 12
#endif
#ifndef KHEAP_ATTR
#define KHEAP_ATTR
#endif
#if KOBJ > 12
#error "KOBJ > 12: k's handle field is 12 bits"
#endif

#ifdef KHEAP
#ifndef KSYS
#error "KHEAP requires KSYS: k_sys reports wsfull and exits"
#endif
extern U k_sys(U, U, U, U, U, U, U);
static U kfull(void) {
  k_sys(1, 2, (U)"wsfull\n", 7, 0, 0, 0);
  k_sys(60, 1, 0, 0, 0, 0, 0);
  for (;;) {}  // a bare-metal k_sys exit may return
}
#define KH KHEAP
#define KC KHEAP
#define KALIGN __attribute((aligned(64)))
#define KOOM(i) P((i)>=KC,kfull())
#define KFULL kfull()
#define KOBJCHK P(o>>KOBJ,kfull())
#else
#define KH 24
#define KC 30
#define KALIGN
#define KOOM(i)
#define KFULL 0
#define KOBJCHK
#endif

#endif
