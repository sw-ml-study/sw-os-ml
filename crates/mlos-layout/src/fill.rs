//! Making a space's regions tile it exactly.
//!
//! Invariant: every byte of a space belongs to exactly one region; gaps
//! become `padding` (between) or `free` (after), never nothing. Design:
//! docs/notes/mlos-layout.md.

use std::io;

use crate::Region;

/// Sorts `regions`, inserts padding between them and free after them, and
/// checks that the result covers `[0, capacity)` exactly once. Overlap and
/// overrun are errors, not reconciled.
pub fn fill(
    space: &str,
    capacity: u64,
    ids: &mut impl Iterator<Item = u32>,
    regions: &mut Vec<Region>,
) -> io::Result<()> {
    regions.sort_by_key(|region| region.start);
    let mut tiled = Vec::with_capacity(regions.len() * 2 + 1);
    let mut at = 0;

    for region in regions.drain(..) {
        overlap(space, &region, at)?;
        if region.start > at {
            tiled.push(gap(space, ids, at, region.start - at, "padding"));
        }
        at = region.start.saturating_add(region.length);
        tiled.push(region);
    }

    tail(space, capacity, ids, &mut tiled, at)?;
    *regions = tiled;
    Ok(())
}

/// Closes a space off: headroom becomes free, and running past its
/// capacity is an error rather than a region nobody can draw.
fn tail(
    space: &str,
    capacity: u64,
    ids: &mut impl Iterator<Item = u32>,
    tiled: &mut Vec<Region>,
    at: u64,
) -> io::Result<()> {
    if at > capacity {
        return Err(io::Error::other(format!(
            "{space}: regions reach {at:#x}, past its {capacity:#x} capacity"
        )));
    }
    if at < capacity {
        tiled.push(gap(space, ids, at, capacity - at, "free"));
    }
    Ok(())
}

/// Refuses a region that starts before the previous one ended.
fn overlap(space: &str, region: &Region, at: u64) -> io::Result<()> {
    if region.start >= at {
        return Ok(());
    }
    let (name, start) = (&region.name, region.start);
    Err(io::Error::other(format!(
        "{space}: {name} at {start:#x} overlaps the region before it"
    )))
}

/// An unaccounted-for extent, named after what it is.
fn gap(
    space: &str,
    ids: &mut impl Iterator<Item = u32>,
    start: u64,
    length: u64,
    kind: &str,
) -> Region {
    Region {
        id: ids.next().unwrap_or_default(),
        space: space.to_owned(),
        kind: kind.to_owned(),
        name: format!("{kind} at {start:#x}"),
        owner: String::new(),
        start,
        length,
        tier: String::new(),
        object: String::new(),
        state: "fixed".to_owned(),
        reuse: 0,
        cost: 0,
        next_use: String::new(),
    }
}
