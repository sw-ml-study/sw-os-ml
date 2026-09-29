//! Checking a rendered document against the contract, as text.
//!
//! Invariant: columns are index-aligned, ids are unique and non-zero,
//! edges name real regions, and regions tile each space exactly once.
//! Shape only, never numbers. Design: docs/notes/mlos-layout.md.

use std::collections::HashSet;

use crate::{Columns, SCHEMA};

/// Columns the contract requires of every producer.
const REQUIRED: [&str; 17] = [
    "spaces",
    "space_name",
    "space_block",
    "space_capacity",
    "region_id",
    "region_space",
    "region_kind",
    "region_name",
    "region_owner",
    "region_start",
    "region_length",
    "region_text_words",
    "region_data_words",
    "region_bss_words",
    "rel_kind",
    "rel_from",
    "rel_to",
];

/// Checks a rendered document, naming the first violation.
pub fn validate(text: &str) -> Result<(), String> {
    let found = Columns::read(text);
    if !text.contains(SCHEMA) {
        return Err(format!("no {SCHEMA:?} in the document"));
    }
    for name in REQUIRED {
        found.strings(name)?;
    }
    aligned(&found)?;
    refs(&found)?;
    tiles(&found)
}

/// Every column is as long as the others in its group.
fn aligned(found: &Columns) -> Result<(), String> {
    let (regions, spaces) = (found.strings("region_id")?, found.strings("spaces")?);
    let edges = found.strings("rel_kind")?;
    for (name, values) in &found.0 {
        let (name, length) = (name.as_str(), values.len());
        let want = match name.split('_').next() {
            Some("region") => regions.len(),
            Some("spaces" | "space") => spaces.len(),
            _ => edges.len(),
        };
        if length != want {
            return Err(format!("column {name} has {length} entries, not {want}"));
        }
    }
    Ok(())
}

/// Ids are present, unique, and every edge names one that exists.
fn refs(found: &Columns) -> Result<(), String> {
    let ids = found.numbers("region_id")?;
    let seen: HashSet<u64> = ids.iter().copied().collect();
    if seen.len() != ids.len() {
        return Err("a region id is used twice; picking needs them unique".to_owned());
    }
    if seen.contains(&0) {
        return Err("id zero is reserved for 'no region'".to_owned());
    }
    let ends = found
        .numbers("rel_from")?
        .into_iter()
        .chain(found.numbers("rel_to")?);
    match ends.filter(|end| !seen.contains(end)).count() {
        0 => Ok(()),
        loose => Err(format!(
            "{loose} edge endpoints name regions that do not exist"
        )),
    }
}

/// Every space is covered by its regions exactly once, end to end.
fn tiles(found: &Columns) -> Result<(), String> {
    let space = found.strings("region_space")?;
    let start = found.numbers("region_start")?;
    let length = found.numbers("region_length")?;
    let keys = found.strings("spaces")?;
    for (key, capacity) in keys.iter().zip(found.numbers("space_capacity")?) {
        let mut extents: Vec<(u64, u64)> = (0..space.len())
            .filter(|index| space[*index] == *key)
            .map(|index| (start[index], length[index]))
            .collect();
        extents.sort_unstable();
        let covered = extents
            .iter()
            .try_fold(0, |at, (start, length)| match *start == at {
                true => Ok(at + length),
                false => Err(format!("{key}: {at:#x} uncovered; next region {start:#x}")),
            })?;
        if covered != capacity {
            return Err(format!("{key}: covered to {covered:#x} of {capacity:#x}"));
        }
    }
    Ok(())
}
