//! AArch64 FP/SIMD ownership around K.

#[cfg(target_arch = "aarch64")]
pub fn enable() {
    // SAFETY: MLOS interrupt code is soft-float and never touches vectors.
    unsafe {
        core::arch::asm!(
            "mrs x0, cpacr_el1", "orr x0, x0, #(3 << 20)",
            "msr cpacr_el1, x0", "isb", out("x0") _,
        );
    }
}

#[cfg(not(target_arch = "aarch64"))]
pub fn enable() {}

#[cfg(target_arch = "aarch64")]
pub fn disable() {
    // SAFETY: K has returned before the kernel resumes soft-float work.
    unsafe {
        core::arch::asm!(
            "mrs x0, cpacr_el1", "bic x0, x0, #(3 << 20)",
            "msr cpacr_el1, x0", "isb", out("x0") _,
        );
    }
}

#[cfg(not(target_arch = "aarch64"))]
pub fn disable() {}
