//! The disk space: the model's weights where they are actually stored.
//!
//! Also where MLOS's region vocabulary is defined, because this is the
//! only module that turns an [`ObjectClass`] into a word a viewer colours
//! by. Both matches here are exhaustive with no wildcard arm, so adding a
//! class or a tier to the ABI is a compile error in this file rather than
//! a region that silently renders as nothing.

use std::{fs, io, path::Path};

use mlos_abi::ObjectId;
use mlos_layout::{Region, Space, fill};
use mlos_objtab::ObjectMeta;
use mlos_spaces::{NextUseText, Where, kind, object, state, tier};
use mlos_synth::{LAYERS, TILES, disk::Disk, model};
use mlos_virtio_blk::SECTOR;

use crate::space;

/// The disk image as a space, with one region per stored tile.
///
/// Capacity is the image file's real size, not the model's: if they
/// disagree, the difference shows up as free space or as an error from
/// `fill`, and either is better than an emitter that assumes.
pub fn disk(path: &Path) -> io::Result<(Space, Vec<Region>)> {
    let capacity = fs::metadata(path)?.len();
    let meta = model::weights();
    let mut regions = Vec::with_capacity(usize::from(LAYERS) * usize::from(TILES));
    for layer in 0..LAYERS {
        for tensor in 0..TILES {
            regions.push(region(model::tile(layer, tensor), &meta)?);
        }
    }

    fill(
        Where::Disk.key(),
        capacity,
        &mut Where::Disk.ids(),
        &mut regions,
    )?;
    let named = space(
        Where::Disk,
        "virtio-blk model image",
        SECTOR as u64,
        capacity,
    );
    Ok((named, regions))
}

/// One stored object, placed where `Disk::sector_of` puts it.
///
/// `state` comes from the same function the runtime emitter uses, rather
/// than being written as the constant it happens to be here. Nothing has
/// run, so every object reads `never` -- and that is the difference the
/// two documents exist to show, which makes it worth deriving rather than
/// asserting.
fn region(id: ObjectId, meta: &ObjectMeta) -> io::Result<Region> {
    let named = |what: &str| io::Error::other(format!("{what} for object {:#018x}", id.0));
    let fields = id.fields();
    Ok(Region {
        id: object(Where::Disk, id).ok_or_else(|| named("no stable region id"))?,
        space: Where::Disk.key().to_owned(),
        kind: kind(id.class().ok_or_else(|| named("no class"))?).to_owned(),
        name: format!("layer {} tile {}", fields.layer, fields.tensor),
        owner: format!("model {}", fields.model),
        start: Disk::sector_of(id) * SECTOR as u64,
        length: u64::from(meta.size),
        tier: tier(meta.tier).to_owned(),
        object: id.0.to_string(),
        state: state(meta).to_owned(),
        reuse: u64::from(meta.reuse_count),
        cost: u64::from(meta.reload_cost.0),
        next_use: NextUseText(meta.next_use).to_string(),
    })
}
