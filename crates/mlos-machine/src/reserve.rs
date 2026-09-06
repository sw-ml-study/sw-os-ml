//! Carving occupied ranges out of the memory map.
//!
//! The device tree describes what the *machine* has. It knows nothing
//! about what a loader put into it -- the kernel image, the blob itself --
//! so those two facts have to be combined somewhere, and this is where.
//!
//! Without it, `BootInfo::usable_bytes` reports memory that is already
//! occupied, and the first allocator to trust it hands out the ground the
//! kernel is standing on.

use mlos_hal::{MemoryKind, MemoryRegion};

use crate::regions::Regions;

/// Returns the map with `[base, base + len)` re-labelled as `kind`,
/// splitting whatever regions it overlaps.
#[must_use]
pub fn reserve(map: &Regions, base: u64, len: u64, kind: MemoryKind) -> Regions {
    let end = base + len;
    let mut out = Regions::default();
    for region in map.as_slice() {
        split(&mut out, *region, base, end, kind);
    }
    out
}

/// Emits one region as up to three: the part below the reservation, the
/// overlap re-labelled, and the part above.
fn split(out: &mut Regions, region: MemoryRegion, base: u64, end: u64, kind: MemoryKind) {
    let (start, finish) = (region.base, region.base + region.len);
    if finish <= base || start >= end {
        out.push(region); // no overlap; pass it through unchanged
        return;
    }
    let part = |base, len, kind| MemoryRegion { base, len, kind };
    if start < base {
        out.push(part(start, base - start, region.kind));
    }
    let (lo, hi) = (start.max(base), finish.min(end));
    out.push(part(lo, hi - lo, kind));
    if finish > end {
        out.push(part(end, finish - end, region.kind));
    }
}
