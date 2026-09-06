//! Carving occupied ranges out of a memory map.
//!
//! Pure logic over a fixed array, so it is testable on the host without a
//! device tree, a VM, or an architecture. The boot output only ever shows
//! one shape; these cover the ones a different machine would produce.

use mlos_hal::{MemoryKind::*, MemoryRegion};
use mlos_machine::{MAX_REGIONS, Regions, reserve};

fn map(parts: &[(u64, u64, mlos_hal::MemoryKind)]) -> Regions {
    let mut regions = Regions::default();
    for &(base, len, kind) in parts {
        assert!(regions.push(MemoryRegion { base, len, kind }));
    }
    regions
}

fn shape(regions: &Regions) -> Vec<(u64, u64, mlos_hal::MemoryKind)> {
    regions
        .as_slice()
        .iter()
        .map(|r| (r.base, r.len, r.kind))
        .collect()
}

/// The case that actually happens at boot: the kernel sits in the middle
/// of DRAM, so one region becomes three.
#[test]
fn a_reservation_inside_a_region_splits_it_in_three() {
    let dram = map(&[(0x4000_0000, 0x2000_0000, Usable)]);
    let out = reserve(&dram, 0x4020_0000, 0x2_0000, Kernel);

    assert_eq!(
        shape(&out),
        [
            (0x4000_0000, 0x20_0000, Usable),
            (0x4020_0000, 0x2_0000, Kernel),
            (0x4022_0000, 0x1FDE_0000, Usable),
        ]
    );
}

/// Reserving must not invent or lose bytes, only re-label them.
#[test]
fn total_bytes_are_conserved() {
    let dram = map(&[(0x4000_0000, 0x2000_0000, Usable)]);
    let once = reserve(&dram, 0x4020_0000, 0x2_0000, Kernel);
    let twice = reserve(&once, 0x4800_0000, 0x10_0000, Reclaimable);

    let total = |r: &Regions| r.as_slice().iter().map(|x| x.len).sum::<u64>();
    assert_eq!(total(&twice), 0x2000_0000);
    assert_eq!(total(&once), total(&dram));
}

/// Edges: a reservation covering a whole region relabels it without
/// splitting; one that misses entirely changes nothing.
#[test]
fn exact_and_disjoint_reservations_do_not_split() {
    let dram = map(&[(0x1000, 0x1000, Usable), (0x4000, 0x1000, Usable)]);

    let exact = reserve(&dram, 0x1000, 0x1000, Kernel);
    assert_eq!(
        shape(&exact),
        [(0x1000, 0x1000, Kernel), (0x4000, 0x1000, Usable)]
    );

    let elsewhere = reserve(&dram, 0x9000, 0x1000, Kernel);
    assert_eq!(shape(&elsewhere), shape(&dram));
}

/// A reservation spanning several regions relabels each overlap and
/// leaves the parts outside alone.
#[test]
fn a_reservation_can_span_regions() {
    let dram = map(&[(0x0, 0x1000, Usable), (0x1000, 0x1000, Usable)]);
    let out = reserve(&dram, 0x800, 0x1000, Reclaimable);

    assert_eq!(
        shape(&out),
        [
            (0x0, 0x800, Usable),
            (0x800, 0x800, Reclaimable),
            (0x1000, 0x800, Reclaimable),
            (0x1800, 0x800, Usable),
        ]
    );
}

/// A full map drops what will not fit rather than growing or panicking.
/// Losing a region at boot is bad; a panic with no console is worse.
#[test]
fn a_full_map_refuses_rather_than_panics() {
    let mut regions = Regions::default();
    for index in 0..MAX_REGIONS as u64 {
        assert!(regions.push(MemoryRegion {
            base: index << 20,
            len: 0x1000,
            kind: Usable
        }));
    }
    assert!(!regions.push(MemoryRegion {
        base: 0xFFFF_0000,
        len: 0x1000,
        kind: Usable
    }));
    assert_eq!(regions.as_slice().len(), MAX_REGIONS);
}

/// The interrupt controller is discovered the same way the console is.
/// Checked against the real QEMU blob rather than at boot, so a wrong
/// answer shows up as a test failure and not as a machine that hangs.
#[test]
fn the_gic_is_found_in_a_real_device_tree() {
    let blob = include_bytes!("../../mlos-fdt/tests/qemu-virt.dtb");
    // SAFETY: a slice we own, not a raw pointer from firmware.
    let machine = unsafe { mlos_machine::Machine::probe(blob.as_ptr()) }.expect("valid blob");

    assert_eq!(machine.gic, Some((0x0800_0000, 0x080A_0000)));
    assert_eq!(machine.uart_base, Some(0x0900_0000));
}

/// The console's interrupt comes from the tree too. QEMU gives the PL011
/// SPI 1, which the GIC numbers 33 -- the tree counts from the start of
/// the shared range and the controller does not, and getting that offset
/// wrong is an interrupt that never arrives.
#[test]
fn the_console_interrupt_is_found_and_rebased() {
    let blob = include_bytes!("../../mlos-fdt/tests/qemu-virt.dtb");
    // SAFETY: a slice we own, not a raw pointer from firmware.
    let machine = unsafe { mlos_machine::Machine::probe(blob.as_ptr()) }.expect("valid blob");
    assert_eq!(machine.uart_irq, Some(33));
}
