//! The contract, mutation-checked.
//!
//! Every test here builds a document that passes and then breaks it one
//! way. A checker that has only ever been shown valid input proves
//! nothing: the reason to write this down is that a dropped column, a
//! one-region gap and a reused id are all invisible until something
//! downstream draws the wrong picture, and each of them must fail HERE.

use mlos_layout::{Doc, Edge, Region, Space, fill};

/// A two-space document that satisfies the contract.
fn good() -> Doc {
    let mut regions = vec![region(1, "flash", 0, 64), region(2, "flash", 128, 64)];
    let mut ids = 100..;
    fill("flash", 256, &mut ids, &mut regions).expect("the fixture tiles");

    let mut arena = Vec::new();
    fill("arena", 512, &mut ids, &mut arena).expect("an empty space is all free");
    regions.extend(arena);

    Doc {
        spaces: vec![space("flash", 256), space("arena", 512)],
        regions,
        edges: Vec::new(),
    }
}

fn space(key: &str, capacity: u64) -> Space {
    Space {
        key: key.to_owned(),
        name: key.to_owned(),
        block: 8,
        capacity,
    }
}

fn region(id: u32, space: &str, start: u64, length: u64) -> Region {
    Region {
        id,
        space: space.to_owned(),
        kind: "image".to_owned(),
        name: format!("region {id}"),
        owner: "kernel".to_owned(),
        start,
        length,
        tier: String::new(),
        object: String::new(),
        state: "never".to_owned(),
    }
}

#[test]
fn a_well_formed_document_passes() {
    good().check().expect("the fixture satisfies the contract");
}

#[test]
fn fill_keeps_padding_distinct_from_free() {
    let doc = good();
    let kinds: Vec<&str> = doc
        .regions
        .iter()
        .filter(|region| region.space == "flash")
        .map(|region| region.kind.as_str())
        .collect();
    // The gap between two placed regions is alignment cost; the gap at the
    // end is headroom. Folding them together would hide the first.
    assert!(kinds.contains(&"padding"), "got {kinds:?}");
    assert!(kinds.contains(&"free"), "got {kinds:?}");
}

#[test]
fn a_gap_fails() {
    let mut doc = good();
    doc.regions.retain(|region| region.kind != "padding");
    doc.check().expect_err("a hole in a space must not pass");
}

#[test]
fn an_overlap_fails() {
    let mut doc = good();
    doc.regions.push(region(9, "flash", 0, 8));
    doc.check()
        .expect_err("two regions on one byte must not pass");
}

#[test]
fn a_stretched_capacity_fails() {
    let mut doc = good();
    doc.spaces[0].capacity *= 2;
    doc.check()
        .expect_err("regions that stop short must not pass");
}

#[test]
fn a_reused_id_fails() {
    let mut doc = good();
    let taken = doc.regions[0].id;
    doc.regions[1].id = taken;
    doc.check().expect_err("picking needs ids to be unique");
}

#[test]
fn a_region_in_no_space_fails() {
    let mut doc = good();
    doc.regions[0].space = "nowhere".to_owned();
    doc.check().expect_err("region_space must join to a space");
}

#[test]
fn a_dangling_edge_fails() {
    let mut doc = good();
    doc.edges.push(Edge {
        kind: "backs".to_owned(),
        from: 1,
        to: 4242,
    });
    doc.check()
        .expect_err("an edge must name regions that exist");
}

#[test]
fn every_column_is_index_aligned() {
    let doc = good();
    let text = doc.render("test", "abc123");
    let regions = doc.regions.len();
    for (name, values) in columns(&text) {
        let want = match name.split('_').next() {
            Some("region") => regions,
            Some("space") | Some("spaces") => doc.spaces.len(),
            _ => doc.edges.len(),
        };
        assert_eq!(values.len(), want, "column {name} is the wrong length");
    }
    // A column the contract names but the emitter forgot is the failure
    // this exists to catch, so check presence as well as length.
    let names: Vec<&str> = columns(&text).into_iter().map(|(name, _)| name).collect();
    for required in [
        "spaces",
        "space_block",
        "region_id",
        "region_start",
        "rel_kind",
    ] {
        assert!(
            names.contains(&required),
            "{required} is missing from {names:?}"
        );
    }
}

/// Every `"name": [...]` column in a rendered document, as raw text.
fn columns(text: &str) -> Vec<(&str, Vec<&str>)> {
    text.lines()
        .filter_map(|line| {
            let (name, rest) = line.trim().split_once("\": [")?;
            let items = rest.trim_end_matches(&[',', ']'][..]);
            let values = if items.is_empty() {
                Vec::new()
            } else {
                items.split(", ").collect()
            };
            Some((name.trim_start_matches('"'), values))
        })
        .collect()
}
