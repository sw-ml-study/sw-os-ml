//! The two memory spaces: guest RAM, and the arena inside it.
//!
//! Every figure is read back from the linked kernel rather than restated.
//! `linker/aarch64.ld` decides where `.text` ends and `.bss` begins, and
//! it will decide differently after the next commit; an emitter carrying
//! its own copy of those numbers would be wrong without being noticed,
//! which is exactly the failure a memory map is supposed to expose.

use std::{fs, io, path::Path};

use mlos_layout::{Region, Space, fill};

use crate::{RAM_BASE, RAM_BYTES, ids::Where};

/// Page size, and the block a RAM map is drawn in.
const PAGE: u64 = 4096;

/// Kernel sections worth drawing, and what a viewer colours them by.
///
/// `text`, `data`, `bss` and `stack` are sw-mlpl's existing palette words,
/// used verbatim so MLOS's RAM map renders through the palette SWTOS
/// already needs. `rodata` is the one addition, and it is not MLOS-
/// specific -- every OS map wants it.
const SECTIONS: [(&str, &str); 4] = [
    (".text", "text"),
    (".rodata", "rodata"),
    (".data", "data"),
    (".bss", "bss"),
];

/// Guest RAM, from the bottom of the machine's DRAM to the top of what
/// `mlos run` gives it.
///
/// The boot stack is deduced rather than read: it is whatever lies between
/// the end of `.bss` and the end of the image, one page-aligned step on --
/// the `ALIGN(4096)` the linker script puts before `__stack_bottom`. It
/// has no section of its own to look up, and the alternative would be
/// carrying a copy of its size here, where nothing would notice it going
/// stale.
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
    Ok((Where::Sysram.space("guest RAM", PAGE, RAM_BYTES), regions))
}

/// The object arena, as a space of its own.
///
/// Physically it is a static inside the kernel image, which `sysram`
/// already accounts for as part of `.data`. It gets its own space anyway
/// because residency is the thing being looked at, and a 32 KiB box inside
/// a 512 MiB one is not a picture of anything. Statically it is entirely
/// free; step 009 fills it with what is actually resident.
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
    Ok((Where::Dram.space("object arena", block, capacity), regions))
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
    }
}

/// `text_offset` and `image_size` from the arm64 `Image` header.
///
/// The header the loader reads, so these are the numbers that decide where
/// the kernel actually lands -- not a guess at them.
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
