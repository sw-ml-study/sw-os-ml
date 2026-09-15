//! MLOS's built artifacts, as a `sw-ml-study.system-layout` document.
//!
//! The static half of the visualization work: what the build produced and
//! where it will sit, with no emulator in the path. That separation is
//! deliberate -- a contract that only validates after a VM boots is a
//! contract whose failures are hard to read, and everything here is
//! decided at link time and at image-build time anyway.
//!
//! Three spaces. The block device the weights are stored on, guest RAM as
//! the kernel image divides it, and the object arena. The first and third
//! are what the runtime emitter fills in: the disk says what exists, the
//! arena says what is resident, and the whole argument of `docs/PRD.md` is
//! about the ratio between them.
//!
//! The sibling repos: sw-mlpl turns this into geometry, demo-extensions
//! draws it, and SWTOS emits the same shape for a flash image. See
//! `../sw-mlpl/docs/storage-layout-viz.md` for the contract itself.

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

/// How much of it the guest gets.
///
/// Here rather than in the VMM arguments because the map and the machine
/// have to agree: an emitter claiming 512 MiB while QEMU is handed 1 GiB
/// would draw a free region that is half the size of the real one, and
/// nothing would report the discrepancy. `mlos-cli` reads this.
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
    // Empty, and stated rather than omitted. An edge runs from a stored
    // object to the arena region holding its bytes, and statically no
    // object is resident -- so there is nothing yet to draw a line
    // between. Step 009 fills this in.
    doc.edges.clear();
    Ok(doc)
}

/// What to stamp into `provenance`, so a rendered picture is traceable.
///
/// Public because the runtime emitter runs inside the guest, which has no
/// git and no way to know what built it; `mlos runtime` passes this in
/// through the boot arguments.
///
/// `--dirty` matters more than the sha: a layout emitted from an
/// uncommitted tree is one nobody else can reproduce, and saying so is
/// cheaper than discovering it later.
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
