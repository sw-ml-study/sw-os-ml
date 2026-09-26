//! The only `unsafe` in the spike: port I/O, MSRs and CPUID.

use core::arch::asm;
use core::fmt;

/// A 16550 UART at an I/O port base.
pub struct Serial(u16);

impl Serial {
    /// COM1, which QEMU wires to `-serial`.
    pub fn com1() -> Self {
        Serial(0x3F8)
    }

    fn put(&mut self, byte: u8) {
        // SAFETY: COM1's line-status and data ports; reads and writes
        // have no side effects beyond the UART itself.
        unsafe {
            while inb(self.0 + 5) & 0x20 == 0 {}
            outb(self.0, byte);
        }
    }
}

impl fmt::Write for Serial {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        s.bytes().for_each(|b| self.put(b));
        Ok(())
    }
}

/// True once EFER.LMA says the CPU is really in long mode.
pub fn efer_lma() -> bool {
    let (lo, _hi): (u32, u32);
    // SAFETY: EFER (0xC0000080) exists on every x86-64 CPU; reading it is harmless.
    unsafe { asm!("rdmsr", in("ecx") 0xC000_0080u32, out("eax") lo, out("edx") _hi) };
    lo & (1 << 10) != 0
}

/// The hypervisor leaf's signature: "KVMKVMKVM", "TCGTCGTCGTCG", or none.
pub fn cpuid_vendor() -> &'static str {
    let r = core::arch::x86_64::__cpuid(0x4000_0000);
    match r.ebx {
        0x4b4d_564b => "KVM",
        0x5447_4354 => "TCG",
        _ => "unknown",
    }
}

/// Ends the VM via QEMU's `isa-debug-exit` device at port 0xf4.
pub fn exit_qemu(code: u32) -> ! {
    // SAFETY: port 0xf4 is the debug-exit device the harness attaches;
    // without it the write is ignored and we fall through to `hlt`.
    unsafe { asm!("out dx, eax", in("dx") 0xf4u16, in("eax") code) };
    loop {
        // SAFETY: halting with interrupts off parks the CPU.
        unsafe { asm!("hlt") };
    }
}

unsafe fn inb(port: u16) -> u8 {
    let v: u8;
    // SAFETY: caller guarantees `port` is a device register safe to read.
    unsafe { asm!("in al, dx", in("dx") port, out("al") v) };
    v
}

unsafe fn outb(port: u16, v: u8) {
    // SAFETY: caller guarantees `port` is a device register safe to write.
    unsafe { asm!("out dx, al", in("dx") port, in("al") v) };
}
