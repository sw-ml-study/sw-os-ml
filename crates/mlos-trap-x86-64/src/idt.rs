//! The interrupt descriptor table: 32 gates, one per exception vector,
//! each pointing at a stub in `entry.s`.

use core::arch::{asm, global_asm};

global_asm!(include_str!("entry.s"));

unsafe extern "C" {
    /// The stubs' addresses, vector order, emitted by `entry.s`.
    static mlos_trap_stubs: [u64; 32];
}

/// The kernel code selector: the 64-bit code segment of the GDT that
/// `mlos-hal-x86-64`'s entry loads.
const KERNEL_CS: u64 = 0x08;

/// Present, DPL 0, 64-bit interrupt gate: interrupts stay off in the
/// handler, which is what a fatal-fault path wants.
const INTERRUPT_GATE: u64 = 0x8e;

/// 32 gates of 16 bytes each, as pairs of words. Written once, by
/// [`load`], before `lidt` makes the CPU read it.
static mut IDT: [[u64; 2]; 32] = [[0; 2]; 32];

/// Fills the IDT and loads it.
///
/// # Safety
///
/// Once, on the boot CPU, interrupts off, nothing else touching `IDT`.
pub unsafe fn load() {
    // SAFETY: read-only data emitted by `entry.s`, never written.
    let stubs = unsafe { &mlos_trap_stubs };
    let idt = (&raw mut IDT).cast::<[u64; 2]>();
    for (vector, &stub) in stubs.iter().enumerate() {
        let low = (stub & 0xffff) | KERNEL_CS << 16 | INTERRUPT_GATE << 40;
        let gate = [low | (stub >> 16 & 0xffff) << 48, stub >> 32];
        // SAFETY: `vector` < 32, inside `IDT`; written only here, once,
        // before `lidt` tells the CPU the table exists.
        unsafe { idt.add(vector).write(gate) };
    }
    let base = (&raw const IDT) as u64;
    let pointer: [u16; 5] = [
        (core::mem::size_of::<[[u64; 2]; 32]>() - 1) as u16,
        base as u16,
        (base >> 16) as u16,
        (base >> 32) as u16,
        (base >> 48) as u16,
    ];
    // SAFETY: `pointer` is a valid 10-byte IDTR image of a filled table
    // that lives for 'static.
    unsafe { asm!("lidt [{}]", in(reg) &pointer, options(readonly, nostack)) };
}
