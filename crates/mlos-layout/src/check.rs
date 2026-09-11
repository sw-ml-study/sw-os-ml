//! What it means for a document to satisfy the contract.
//!
//! Deliberately about SHAPE and not about numbers. The regions in a layout
//! change whenever the kernel grows a section or the model gains a tile,
//! and a test pinned to those figures would be rewritten every time
//! instead of ever failing usefully. What must never change is that the
//! columns stay index-aligned, that regions tile each space exactly once,
//! and that ids are unique -- because a consumer draws a solid stack of
//! cells and picks on the id, and each of those breaks silently.

use std::collections::HashSet;

use crate::{Doc, Space};

/// Checks a document against the contract, naming the first violation.
///
/// A `String` rather than a typed error: every one of these is a producer
/// bug found during development, and what its author needs is a sentence,
/// not something to match on.
pub fn check(doc: &Doc) -> Result<(), String> {
    let ids = named(doc)?;
    for space in &doc.spaces {
        tiles(doc, space)?;
    }
    let loose = |edge: &&crate::Edge| !ids.contains(&edge.to) || !ids.contains(&edge.from);
    match doc.edges.iter().find(loose) {
        Some(edge) => Err(format!("{:?} edge names a missing region", edge.kind)),
        None => Ok(()),
    }
}

/// Every region's id, checked for being present, unique and in a space.
fn named(doc: &Doc) -> Result<HashSet<u32>, String> {
    let mut ids = HashSet::new();
    for region in &doc.regions {
        let name = &region.name;
        if region.id == 0 {
            return Err(format!("region {name:?} has no id"));
        }
        if !ids.insert(region.id) {
            return Err(format!("region id {} is used twice", region.id));
        }
        if !doc.spaces.iter().any(|space| space.key == region.space) {
            return Err(format!("region {name:?} is in no known space"));
        }
    }
    Ok(ids)
}

/// Checks that `space`'s regions cover it exactly once, end to end.
fn tiles(doc: &Doc, space: &Space) -> Result<(), String> {
    let extents = extents(doc, space);
    let key = &space.key;
    let mut at = 0;
    for (start, length) in extents {
        if start != at {
            return Err(format!("{key}: {at:#x} uncovered; next region {start:#x}"));
        }
        at = start.saturating_add(length);
    }
    match at == space.capacity {
        true => Ok(()),
        false => Err(format!(
            "{key}: covered to {at:#x} of {:#x}",
            space.capacity
        )),
    }
}

/// One space's regions as sorted `(start, length)` pairs.
fn extents(doc: &Doc, space: &Space) -> Vec<(u64, u64)> {
    let mut extents: Vec<(u64, u64)> = doc
        .regions
        .iter()
        .filter(|region| region.space == space.key)
        .map(|region| (region.start, region.length))
        .collect();
    extents.sort_unstable();
    extents
}
