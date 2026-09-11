//! Where an ELF64 file's sections land in memory.
//!
//! Exists because the layout emitter must not guess. `docs/plan.md`'s
//! visualization steps promise that every figure comes from the artifact
//! that produced it, and the kernel's `.text`/`.rodata`/`.data`/`.bss`
//! extents are decided by `linker/aarch64.ld` at link time. Reading them
//! back out of the linked file is the only way to report them and still be
//! right after the next commit changes their sizes.
//!
//! Section headers only, and deliberately nothing else. Symbols, relocs
//! and program headers are all readable the same way and none of them are
//! needed yet; a reader that stops at what is used is one that can be
//! checked by eye.

#![forbid(unsafe_code)]

mod read;

use std::{fs, io, path::Path};

use read::{u16_at, u32_at, u64_at};

/// Where the section header table is, how big its entries are, how many
/// there are, and which one holds the names.
const TABLE: (usize, usize, usize, usize) = (0x28, 0x3a, 0x3c, 0x3e);

/// One section, reduced to what a memory map needs.
#[derive(Clone, Debug)]
pub struct Section {
    /// Its name, `.text` and friends.
    pub name: String,
    /// Where it is loaded. Zero for sections that are not allocated.
    pub addr: u64,
    /// How many bytes it occupies in memory.
    pub size: u64,
}

/// Every section in the ELF64 file at `path`.
///
/// Includes the non-allocated ones (`.debug_*`, `.symtab`) with `addr`
/// zero; filtering them is the caller's business, since which sections
/// matter depends on what the caller is mapping.
pub fn sections(path: &Path) -> io::Result<Vec<Section>> {
    let bytes = fs::read(path)?;
    let (offset, stride, count, names) = table(&bytes)?;
    let strtab = at(&bytes, offset, stride, names)?;
    let start = strtab.3 as usize;
    let text = bytes
        .get(start..start.saturating_add(strtab.2 as usize))
        .ok_or_else(|| io::Error::other("ELF string table out of range"))?
        .to_vec();

    (0..count)
        .map(|index| {
            let (name, addr, size, _) = at(&bytes, offset, stride, index)?;
            Ok(Section {
                name: label(&text, name),
                addr,
                size,
            })
        })
        .collect()
}

/// Reads the section header table's own geometry, rejecting anything that
/// is not a little-endian 64-bit ELF -- the only kind MLOS links.
fn table(bytes: &[u8]) -> io::Result<(usize, usize, usize, usize)> {
    let head = bytes.get(..6).ok_or_else(|| io::Error::other("not an ELF"));
    if head? != b"\x7fELF\x02\x01" {
        return Err(io::Error::other("not a little-endian ELF64 file"));
    }
    Ok((
        u64_at(bytes, TABLE.0)? as usize,
        usize::from(u16_at(bytes, TABLE.1)?),
        usize::from(u16_at(bytes, TABLE.2)?),
        usize::from(u16_at(bytes, TABLE.3)?),
    ))
}

/// One section header's name offset, address, size and file offset.
fn at(
    bytes: &[u8],
    offset: usize,
    stride: usize,
    index: usize,
) -> io::Result<(u32, u64, u64, u64)> {
    let base = offset
        .checked_add(index.saturating_mul(stride))
        .ok_or_else(|| io::Error::other("ELF section index out of range"))?;
    Ok((
        u32_at(bytes, base)?,
        u64_at(bytes, base + 0x10)?,
        u64_at(bytes, base + 0x20)?,
        u64_at(bytes, base + 0x18)?,
    ))
}

/// The NUL-terminated name at `at` in the section-name string table.
///
/// A missing terminator yields the rest of the table rather than an error:
/// a section whose name is odd is not a reason to refuse to draw the map.
fn label(text: &[u8], at: u32) -> String {
    let rest = text.get(at as usize..).unwrap_or_default();
    let end = rest
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(rest.len());
    String::from_utf8_lossy(&rest[..end]).into_owned()
}
