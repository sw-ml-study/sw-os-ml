//! Every region the running system has, as a stream.
//!
//! An iterator rather than a collection because there is no allocator to
//! collect into. Each column is written by walking this again -- sixteen
//! passes over a hundred-odd rows, which costs nothing and means no buffer
//! has to exist anywhere.
//!
//! [`Row`] carries values already resolved rather than the `ObjectMeta` it
//! came from. That is what lets every column be a one-line field read: the
//! alternative is a match per column, repeated sixteen times, each one
//! another chance to describe a free region as an object.
//!
//! Sorted order is deliberately NOT promised. The contract requires the
//! regions of a space to tile it, not to arrive in order, and sorting
//! without an allocator would be a quadratic pass for no gain -- the
//! consumer indexes by `region_id` and takes geometry from `start` and
//! `length`, neither of which cares.

use mlos_objman::{Arena, Manager};
use mlos_objtab::{NextUse, ObjectMeta};
use mlos_spaces::{Where, kind, state, tier};
use mlos_synth::{LAYERS, TILES, disk::Disk, model};
use mlos_virtio_blk::SECTOR;

use mlos_abi::ObjectId;

/// The arena's allocation granularity.
pub const GRAIN: u64 = Arena::ALIGN as u64;

/// One region, ready to be written out.
#[derive(Clone, Copy)]
pub struct Row {
    /// Which space it is in.
    pub place: Where,
    /// Its stable id.
    pub id: u32,
    /// Offset within the space.
    pub start: u64,
    /// Length in bytes.
    pub length: u64,
    /// Its class, or `free` -- the Purpose colour mode.
    pub kind: &'static str,
    /// Whose it is.
    pub owner: &'static str,
    /// Where it lives, or empty when it is not an object.
    pub tier: &'static str,
    /// Resident, evicted, never, or `fixed` for what is not an object.
    pub state: &'static str,
    /// The raw `ObjectId`. Zero means this region holds no object.
    pub object: u64,
    /// Which layer and tensor, for naming.
    pub at: (u16, u16),
    /// When it is next wanted.
    pub next_use: NextUse,
    /// How many times it has been wanted.
    pub reuse: u64,
    /// What getting it back would cost, in nanoseconds.
    pub cost: u64,
}

/// Every region: what is stored, what is resident, and `tail` after them.
///
/// The tail is passed in rather than built here because it is the one row
/// that describes no object -- see `tail` in the crate root.
pub fn rows<'m, const N: usize>(
    manager: &'m Manager<'static, N>,
    base: u64,
    tail: Row,
) -> impl Iterator<Item = Row> + 'm {
    stored(manager)
        .chain(placed(manager, base))
        // Skipped when the arena is exactly full. A zero-length region
        // satisfies the tiling rule and draws as nothing, which is a box
        // in the legend that is never on screen.
        .chain(core::iter::once(tail).filter(|row| row.length > 0))
}

/// The model's weight tiles, where the disk image keeps them.
///
/// Every tile, resident or not: the disk map is about what EXISTS, and a
/// tile's `state` column says whether it is also in memory. That is the
/// picture worth having -- which parts of a model a workload has actually
/// touched, drawn over the whole model rather than over a fragment of it.
fn stored<'m, const N: usize>(manager: &'m Manager<'static, N>) -> impl Iterator<Item = Row> + 'm {
    (0..LAYERS).flat_map(move |layer| {
        (0..TILES).filter_map(move |tensor| {
            let id = model::tile(layer, tensor);
            let meta = *manager.table.get(id)?;
            let start = Disk::sector_of(id) * SECTOR as u64;
            object(Where::Disk, id, &meta, (start, u64::from(meta.size)))
        })
    })
}

/// Everything actually in the arena, at the address it was given.
///
/// Length is rounded to the arena's granularity, because that is what the
/// object cost. A bump allocator hands out aligned extents, and reporting
/// the unrounded size would leave holes the contract does not allow.
fn placed<'m, const N: usize>(
    manager: &'m Manager<'static, N>,
    base: u64,
) -> impl Iterator<Item = Row> + 'm {
    (0..LAYERS).flat_map(move |layer| {
        let tiles = (0..TILES).map(move |tensor| model::tile(layer, tensor));
        tiles
            .chain(core::iter::once(model::activation(layer)))
            .filter_map(move |id| {
                let meta = *manager.table.get(id)?;
                let at = meta.resident_at.checked_sub(base)?;
                let extent = (at, u64::from(meta.size).next_multiple_of(GRAIN));
                object(Where::Dram, id, &meta, extent)
            })
    })
}

/// One object's region in `place`, with every column resolved.
fn object(place: Where, id: ObjectId, meta: &ObjectMeta, extent: (u64, u64)) -> Option<Row> {
    let fields = id.fields();
    Some(Row {
        place,
        id: mlos_spaces::object(place, id)?,
        start: extent.0,
        length: extent.1,
        kind: kind(id.class()?),
        owner: "model 1",
        tier: tier(meta.tier),
        state: state(meta),
        object: id.0,
        at: (fields.layer, fields.tensor),
        next_use: meta.next_use,
        reuse: u64::from(meta.reuse_count),
        cost: u64::from(meta.reload_cost.0),
    })
}
