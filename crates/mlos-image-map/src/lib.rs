//! MLOS's built artifacts, as a `sw-ml-study.system-layout` document,
//! with no emulator in the path.
//!
//! Invariant: every figure is read from the artifact that decided it,
//! never restated here. Design and history: docs/notes/mlos-image-map.md.

#![forbid(unsafe_code)]

mod memory;
mod objects;
pub mod runtime;

use std::{
    io,
    path::{Path, PathBuf},
    process::Command,
};

use mlos_layout::{Doc, Space};
use mlos_spaces::Where;

/// Where QEMU's `virt` machine puts DRAM.
pub const RAM_BASE: u64 = 0x4000_0000;

/// How much of it the guest gets. `mlos-cli` hands QEMU this same number,
/// so the map and the machine cannot disagree.
pub const RAM_BYTES: u64 = 512 << 20;

/// Where `mlos layout` writes.
pub const OUT: &str = "build/storage-layout.json";

/// Writes the layout of `image` and `disk` to [`OUT`], and says where.
///
/// The kernel ELF is the file `image` was objcopied from, beside it.
pub fn emit(image: &Path, disk: &Path) -> io::Result<PathBuf> {
    let text = document(image, disk)?.render(mlos_spaces::PRODUCER, &revision());
    mlos_layout::validate(&text).map_err(io::Error::other)?;
    runtime::save(OUT, &text)
}

/// A space, named -- the host side of [`Where`], which cannot name a
/// `Space` itself because the contract's types are host-only.
fn space(place: Where, name: &str, block: u64, capacity: u64) -> Space {
    Space {
        key: place.key().to_owned(),
        name: name.to_owned(),
        block,
        capacity,
    }
}

/// Every space and region, assembled.
fn document(image: &Path, disk: &Path) -> io::Result<Doc> {
    let built = [
        objects::disk(disk)?,
        memory::dram()?,
        memory::sysram(&image.with_file_name("mlos-kernel"), image)?,
    ];

    let mut doc = Doc::default();
    for (space, regions) in built {
        doc.spaces.push(space);
        doc.regions.extend(regions);
    }
    // Empty, stated rather than omitted: statically no object is resident.
    doc.edges.clear();
    Ok(doc)
}

/// What to stamp into `provenance`: `git describe --dirty`, or `unknown`.
/// Public so `mlos runtime` can pass it to the guest.
#[must_use]
pub fn revision() -> String {
    Command::new("git")
        .args(["describe", "--always", "--dirty", "--abbrev=12"])
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| "unknown".to_owned())
}
