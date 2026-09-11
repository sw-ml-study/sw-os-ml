//! The disk space: the model's weights where they are actually stored.
//!
//! Also where MLOS's region vocabulary is defined, because this is the
//! only module that turns an [`ObjectClass`] into a word a viewer colours
//! by. Both matches here are exhaustive with no wildcard arm, so adding a
//! class or a tier to the ABI is a compile error in this file rather than
//! a region that silently renders as nothing.

use std::{fs, io, path::Path};

use mlos_abi::{ObjectClass, ObjectId};
use mlos_layout::{Region, Space, fill};
use mlos_objtab::{ObjectMeta, Tier};
use mlos_synth::{LAYERS, TILES, disk::Disk, model};
use mlos_virtio_blk::SECTOR;

use crate::ids::{self, Where};

/// The disk image as a space, with one region per stored tile.
///
/// Capacity is the image file's real size, not the model's: if they
/// disagree, the difference shows up as free space or as an error from
/// `fill`, and either is better than an emitter that assumes.
pub fn space(path: &Path) -> io::Result<(Space, Vec<Region>)> {
    let capacity = fs::metadata(path)?.len();
    let meta = model::weights();
    let mut regions = Vec::with_capacity(usize::from(LAYERS) * usize::from(TILES));
    for layer in 0..LAYERS {
        for tensor in 0..TILES {
            regions.push(region(model::tile(layer, tensor), &meta)?);
        }
    }

    fill("disk", capacity, &mut Where::Disk.ids(), &mut regions)?;
    Ok((
        Space {
            key: Where::Disk.key().to_owned(),
            name: "virtio-blk model image".to_owned(),
            block: SECTOR as u64,
            capacity,
        },
        regions,
    ))
}

/// One stored object, placed where `Disk::sector_of` puts it.
fn region(id: ObjectId, meta: &ObjectMeta) -> io::Result<Region> {
    let named = |what: &str| io::Error::other(format!("{what} for object {:#018x}", id.0));
    let fields = id.fields();
    Ok(Region {
        id: ids::object(Where::Disk, id).ok_or_else(|| named("no stable region id"))?,
        space: Where::Disk.key().to_owned(),
        kind: kind(id.class().ok_or_else(|| named("no class"))?).to_owned(),
        name: format!("layer {} tile {}", fields.layer, fields.tensor),
        owner: format!("model {}", fields.model),
        start: Disk::sector_of(id) * SECTOR as u64,
        length: u64::from(meta.size),
        tier: tier(meta.tier).to_owned(),
        object: id.0.to_string(),
        // Nothing has run. The static file describes what exists; the
        // runtime file describes what is resident, and the difference
        // between the two is the whole subject.
        state: "never".to_owned(),
    })
}

/// What a viewer colours an object by.
fn kind(class: ObjectClass) -> &'static str {
    match class {
        ObjectClass::WeightTile => "weight-tile",
        ObjectClass::Scale => "scale",
        ObjectClass::Expert => "expert",
        ObjectClass::KvBlock => "kv-block",
        ObjectClass::Activation => "activation",
        ObjectClass::EmbedBlock => "embed-block",
        ObjectClass::RagBlock => "rag-block",
        ObjectClass::Adapter => "adapter",
    }
}

/// Where an object currently lives, as a word.
fn tier(tier: Tier) -> &'static str {
    match tier {
        Tier::Hot => "hot",
        Tier::Warm => "warm",
        Tier::Cold => "cold",
        Tier::Stream => "stream",
        Tier::Archive => "archive",
    }
}
