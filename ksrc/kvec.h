// kvec.h -- portable stand-ins for the AVX-512 builtins k uses.
//
// Included by a.h/z.h only when the compiler is NOT targeting AVX-512F, so the
// original x86 BareMetal build is unchanged (byte-identical k.app).
// Everything here is plain GCC/Clang vector-extension C: it compiles for any
// ISA, and clang lowers the 64-byte vectors to NEON on aarch64 or SSE/AVX2 on
// x86-64. Each helper states the AVX-512 operation it replaces; the golden
// transcripts in test/golden/ are the conformance check.
//
// Macro arguments must not contain bare commas (k's macros split on them).
// Types come from _.h: V = 64 x u8, i6 = 16 x u32, e6 = 16 x f32, U = u64.
#ifndef KVEC_H
#define KVEC_H
// With KVEC_BG_ONLY defined (z.h via ksys.h), only bg is emitted.

// vpmovb2m: bit i = top bit of byte i.
UV(bg,Ur=0;for(int k=0;k<64;k++)r|=(U)((unsigned char)a[k]>>7)<<k;r)
#ifndef KVEC_BG_ONLY
// vpmovd2m: bit i = top bit of dword i.
UV(bi,i6 d=(i6)a;Ur=0;for(int k=0;k<16;k++)r|=(U)(d[k]>>31)<<k;r)
// vpshufb (per 128-bit lane): r[i] = b[i]&0x80 ? 0 : a[lane(i) + (b[i]&15)].
VF(a4,V r;for(int k=0;k<64;k++)r[k]=b[k]&128?0:a[(k&~15)|(b[k]&15)];r)
// vpermb: r[i] = a[b[i]&63].
VF(A0,V r;for(int k=0;k<64;k++)r[k]=a[b[k]&63];r)
// vpermd: r[i] = a[b[i]&15] on dwords.
VF(A2,i6 d=(i6)a;i6 x=(i6)b;i6 r;for(int k=0;k<16;k++)r[k]=d[x[k]&15];(V)r)
// Shift bytes toward higher indices, zero fill. AVX-512 builds this from
// vpermi2b (i=0: 1 byte, i=1: 2 bytes) and valignd (i=2..5: 4,8,16,32 bytes).
// Used by k's prefix scans.
Vg(S6,int s=i<2?i+1:4<<(i-2);V r;for(int k=0;k<64;k++)r[k]=k<s?0:a[k-s];r)
// vsqrtps. elementwise_sqrt lowers to vector sqrt (fsqrt on NEON); a libm
// sqrtf would be unavailable: k builds with -fno-builtin and no libc.
#if __clang__
VE(_q,B(elementwise_sqrt)(x))
#else // GCC: no elementwise builtins
VE(_q,e6 r;for(int k=0;k<16;k++)r[k]=__builtin_sqrtf(x[k]);r)
#endif
// pclmulqdq(x, ~0): carry-less multiply by all-ones = prefix XOR of the bits.
_F(X9,x^=x<<1;x^=x<<2;x^=x<<4;x^=x<<8;x^=x<<16;x^=x<<32;-a^x)

#endif // !KVEC_BG_ONLY

#endif
