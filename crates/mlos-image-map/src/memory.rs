//! The two memory spaces: guest RAM, and the arena inside it.
//!
//! Invariant: section extents come from the linked ELF and the `Image`
//! header, never from a copy kept here. Design: docs/notes/mlos-image-map.md.

use std::{fs, io, path::Path};

use mlos_layout::{Region, Space, fill};

use mlos_spaces::Where;

use crate::{RAM_BASE, RAM_BYTES, space};

/// Page size, and the block a RAM map is drawn in.
const PAGE: u64 = 4096;

/// Kernel sections worth drawing, and the palette word each is coloured
/// by. The words are sw-mlpl's; `rodata` is the one MLOS added.
const SECTIONS: [(&str, &str); 4] = [
    (".text", "text"),
    (".rodata", "rodata"),
    (".data", "data"),
    (".bss", "bss"),
];

/// Guest RAM, from the bottom of the machine's DRAM to the top of what
/// `mlos run` gives it. The boot stack is deduced: from the end of `.bss`
/// rounded to a page (the linker's `ALIGN(4096)`) to the end of the image.
pub fn sysram(elf: &Path, image: &Path) -> io::Result<(Space, Vec<Region>)> {
    let (offset, size) = header(image)?;
    let ids = &mut Where::Sysram.ids();
    let mut regions = vec![region(ids, "reserved", "loader reserve", 0, offset)];
    let mut end = offset;
    for section in mlos_elf::sections(elf)? {
        let Some((_, kind)) = SECTIONS.iter().find(|(name, _)| *name == section.name) else {
            continue;
        };
        let start = section.addr.saturating_sub(RAM_BASE);
        end = start + section.size;
        regions.push(region(ids, kind, &section.name, start, section.size));
    }
    let (top, stack) = (offset + size, end.next_multiple_of(PAGE));
    regions.push(region(ids, "stack", "boot stack", stack, top - stack));
    fill(Where::Sysram.key(), RAM_BYTES, ids, &mut regions)?;
    Ok((space(Where::Sysram, "guest RAM", PAGE, RAM_BYTES), regions))
}

/// The object arena, as a space of its own, entirely free. Physically it
/// is inside `.data`, which `sysram` already accounts for.
pub fn dram() -> io::Result<(Space, Vec<Region>)> {
    let capacity = mlos_lab::ARENA_BYTES as u64;
    let mut regions = Vec::new();
    fill(
        Where::Dram.key(),
        capacity,
        &mut Where::Dram.ids(),
        &mut regions,
    )?;
    let block = u64::from(mlos_objman::Arena::ALIGN);
    Ok((space(Where::Dram, "object arena", block, capacity), regions))
}

/// One structural region -- something the kernel put there, not an object.
fn region(
    ids: &mut impl Iterator<Item = u32>,
    kind: &str,
    name: &str,
    at: u64,
    len: u64,
) -> Region {
    Region {
        id: ids.next().unwrap_or_default(),
        space: Where::Sysram.key().to_owned(),
        kind: kind.to_owned(),
        name: name.to_owned(),
        owner: "kernel".to_owned(),
        start: at,
        length: len,
        tier: String::new(),
        object: String::new(),
        state: "fixed".to_owned(),
        reuse: 0,
        cost: 0,
        next_use: String::new(),
    }
}

/// `text_offset` and `image_size` from the arm64 `Image` header: the
/// numbers the loader itself uses.
fn header(image: &Path) -> io::Result<(u64, u64)> {
    let bytes = fs::read(image)?;
    let at = |offset: usize| -> io::Result<u64> {
        bytes
            .get(offset..offset + 8)
            .and_then(|field| field.try_into().ok())
            .map(u64::from_le_bytes)
            .ok_or_else(|| io::Error::other("image is too short to hold an arm64 header"))
    };
    Ok((at(8)?, at(16)?))
}
