//! The PVH rules, on the host, against bytes shaped like QEMU's.

use mlos_hal::{MemoryKind, MemoryRegion};
use mlos_pvh::{ENTRY_LEN, HEADER_LEN, MAGIC, StartInfo, regions, user_args, virtio_slots};

/// A start info with the given version, command line and memory map.
fn header(version: u32, cmdline: u64, memmap: u64, entries: u32) -> Vec<u8> {
    let mut bytes = vec![0; HEADER_LEN];
    bytes[0..4].copy_from_slice(&MAGIC.to_le_bytes());
    bytes[4..8].copy_from_slice(&version.to_le_bytes());
    bytes[24..32].copy_from_slice(&cmdline.to_le_bytes());
    bytes[40..48].copy_from_slice(&memmap.to_le_bytes());
    bytes[48..52].copy_from_slice(&entries.to_le_bytes());
    bytes
}

/// One e820 record.
fn record(base: u64, len: u64, kind: u32) -> Vec<u8> {
    let mut bytes = vec![0; ENTRY_LEN];
    bytes[0..8].copy_from_slice(&base.to_le_bytes());
    bytes[8..16].copy_from_slice(&len.to_le_bytes());
    bytes[16..20].copy_from_slice(&kind.to_le_bytes());
    bytes
}

#[test]
fn a_version_one_header_is_read() {
    let info = StartInfo::parse(&header(1, 0x2000, 0x3000, 4)).expect("valid");
    assert_eq!(
        (info.version, info.cmdline, info.memmap, info.memmap_entries),
        (1, 0x2000, 0x3000, 4)
    );
}

#[test]
fn a_bad_magic_a_version_without_a_map_or_a_short_slice_is_refused() {
    let mut bad = header(1, 0, 0, 0);
    bad[0] ^= 1;
    assert_eq!(StartInfo::parse(&bad), None);
    assert_eq!(StartInfo::parse(&header(0, 0, 0, 0)), None);
    assert_eq!(StartInfo::parse(&header(1, 0, 0, 0)[..40]), None);
}

#[test]
fn e820_types_map_to_memory_kinds_and_unknown_is_not_usable() {
    let map = [
        record(0, 0x9fc00, 1),
        record(0xf0000, 0x10000, 2),
        record(0x10_0000, 0x1000, 3),
        record(0x20_0000, 0x1000, 99),
        record(0x30_0000, 0, 1),
    ]
    .concat();
    let mut seen = Vec::new();
    regions(&map, |region| seen.push(region));
    let kinds: Vec<_> = seen.iter().map(|r: &MemoryRegion| r.kind).collect();
    assert_eq!(
        kinds,
        [
            MemoryKind::Usable,
            MemoryKind::Reserved,
            MemoryKind::Reclaimable,
            MemoryKind::Reserved
        ]
    );
    assert_eq!((seen[0].base, seen[0].len), (0, 0x9fc00));
}

#[test]
fn virtio_slots_come_from_the_command_line_lowest_first() {
    let line =
        "mlsh.run=x virtio_mmio.device=512@0xfeb00e00:12 virtio_mmio.device=512@0xfeb00c00:11";
    assert_eq!(virtio_slots(line), Some((0xfeb0_0c00, 512, 2)));
    assert_eq!(
        virtio_slots("virtio_mmio.device=4K@0x10000:5"),
        Some((0x10000, 4096, 1))
    );
    assert_eq!(virtio_slots("console=ttyS0 virtio_mmio.device=junk"), None);
    assert_eq!(virtio_slots(""), None);
}

#[test]
fn the_loaders_appended_entries_are_not_part_of_mlsh_run() {
    let line = "mlos.rev=abc mlsh.run=mem;dev virtio_mmio.device=512@0xfeb00e00:12";
    assert_eq!(user_args(line), "mlos.rev=abc mlsh.run=mem;dev");
    // Before mlsh.run= they are ordinary words, left in place.
    let early = "virtio_mmio.device=512@0xfeb00e00:12 mlsh.run=model 32";
    assert_eq!(user_args(early), early);
    assert_eq!(user_args("console=ttyS0"), "console=ttyS0");
}
