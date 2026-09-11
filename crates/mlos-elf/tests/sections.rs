//! Reading section headers back out of a linked file.
//!
//! Built by hand rather than checked in as a binary fixture: a 200-byte
//! ELF written here is one a reader can verify by eye against the spec,
//! and it cannot rot into "whatever the toolchain emitted that day".

use std::{fs, path::PathBuf};

/// Section header entries are 64 bytes in ELF64.
const ENTRY: usize = 64;

/// A minimal little-endian ELF64 with `names` as its sections.
///
/// Section 0 is the reserved null entry, then one per name, then the
/// string table -- the layout a linker produces, minimally.
fn elf(names: &[&str]) -> Vec<u8> {
    let mut strtab = vec![0u8];
    let mut offsets = Vec::new();
    for name in names {
        offsets.push(strtab.len() as u32);
        strtab.extend(name.as_bytes());
        strtab.push(0);
    }

    let count = names.len() + 2;
    let table = 0x40;
    let mut bytes = vec![0u8; table + count * ENTRY];
    bytes[..6].copy_from_slice(b"\x7fELF\x02\x01");
    bytes[0x28..0x30].copy_from_slice(&(table as u64).to_le_bytes());
    bytes[0x3a..0x3c].copy_from_slice(&(ENTRY as u16).to_le_bytes());
    bytes[0x3c..0x3e].copy_from_slice(&(count as u16).to_le_bytes());
    bytes[0x3e..0x40].copy_from_slice(&((count - 1) as u16).to_le_bytes());

    for (index, offset) in offsets.iter().enumerate() {
        let at = table + (index + 1) * ENTRY;
        bytes[at..at + 4].copy_from_slice(&offset.to_le_bytes());
        let addr = 0x4020_0000u64 + (index as u64) * 0x1000;
        bytes[at + 0x10..at + 0x18].copy_from_slice(&addr.to_le_bytes());
        bytes[at + 0x20..at + 0x28].copy_from_slice(&(0x100u64).to_le_bytes());
    }

    let last = table + (count - 1) * ENTRY;
    let at = bytes.len() as u64;
    bytes[last + 0x18..last + 0x20].copy_from_slice(&at.to_le_bytes());
    bytes[last + 0x20..last + 0x28].copy_from_slice(&(strtab.len() as u64).to_le_bytes());
    bytes.extend(strtab);
    bytes
}

/// Writes `bytes` somewhere this test can read them back from.
fn written(name: &str, bytes: &[u8]) -> PathBuf {
    let path = std::env::temp_dir().join(format!("mlos-elf-{name}"));
    fs::write(&path, bytes).expect("the temp directory is writable");
    path
}

#[test]
fn names_addresses_and_sizes_come_back() {
    let path = written("good", &elf(&[".text", ".rodata", ".bss"]));
    let sections = mlos_elf::sections(&path).expect("a well-formed ELF parses");

    let named: Vec<(&str, u64, u64)> = sections
        .iter()
        .filter(|section| section.addr != 0)
        .map(|section| (section.name.as_str(), section.addr, section.size))
        .collect();
    assert_eq!(
        named,
        [
            (".text", 0x4020_0000, 0x100),
            (".rodata", 0x4020_1000, 0x100),
            (".bss", 0x4020_2000, 0x100),
        ]
    );
}

#[test]
fn the_null_section_is_kept_not_hidden() {
    let path = written("null", &elf(&[".text"]));
    let sections = mlos_elf::sections(&path).expect("a well-formed ELF parses");
    // Three: the reserved entry, `.text`, and the string table. The caller
    // filters on `addr`, so dropping entries here would shift nothing but
    // would make an index mean something different than it does in the file.
    assert_eq!(sections.len(), 3);
    assert_eq!(sections[0].addr, 0);
}

#[test]
fn a_truncated_file_is_an_error_not_a_panic() {
    let whole = elf(&[".text", ".rodata"]);
    let path = written("short", &whole[..whole.len() / 2]);
    let error = mlos_elf::sections(&path).expect_err("a half file cannot parse");
    assert!(error.to_string().contains("truncated"), "{error}");
}

#[test]
fn something_that_is_not_an_elf_is_refused() {
    let path = written("plain", b"#!/bin/sh\necho not an elf\n");
    mlos_elf::sections(&path).expect_err("a shell script is not an ELF64");
}
