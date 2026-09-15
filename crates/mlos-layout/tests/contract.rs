//! The contract, mutation-checked.
//!
//! Every test builds a document that passes and then breaks it one way. A
//! validator that has only ever been shown valid input proves nothing: a
//! dropped column, a one-region gap and a reused id are all invisible
//! until something downstream draws the wrong picture, and each of them
//! must fail HERE.
//!
//! The mutations are applied to the rendered TEXT, because that is what
//! the validator reads and what a consumer parses. Mutating the types
//! instead would test a code path no other repository ever sees.

use mlos_layout::{Columns, Doc, Edge, Region, Space, fill, validate};

/// A two-space document that satisfies the contract.
fn good() -> String {
    let mut regions = vec![region(1, "flash", 0, 64), region(2, "flash", 128, 64)];
    let mut ids = 100..;
    fill("flash", 256, &mut ids, &mut regions).expect("the fixture tiles");

    let mut arena = Vec::new();
    fill("arena", 512, &mut ids, &mut arena).expect("an empty space is all free");
    regions.extend(arena);

    Doc {
        spaces: vec![space("flash", 256), space("arena", 512)],
        regions,
        edges: vec![Edge {
            kind: "backs".to_owned(),
            from: 1,
            to: 2,
        }],
    }
    .render("test", "abc123")
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
        reuse: 0,
        cost: 0,
        next_use: String::new(),
    }
}

/// Rewrites one column's items, leaving the rest of the text alone.
///
/// Textual rather than a re-render, so the mutation cannot be tidied up by
/// the emitter on its way out.
fn put(text: &str, name: &str, items: &[String]) -> String {
    let line = |line: &str| match line.trim().starts_with(&format!("\"{name}\": [")) {
        true => format!("  \"{name}\": [{}],", items.join(", ")),
        false => line.to_owned(),
    };
    text.lines().map(line).collect::<Vec<_>>().join("\n") + "\n"
}

/// One column's items.
fn get(text: &str, name: &str) -> Vec<String> {
    Columns::read(text)
        .0
        .into_iter()
        .find(|(key, _)| key == name)
        .expect("a column that is there")
        .1
}

#[test]
fn a_well_formed_document_passes() {
    validate(&good()).expect("the fixture satisfies the contract");
}

#[test]
fn fill_keeps_padding_distinct_from_free() {
    // The gap between two placed regions is alignment cost; the gap at the
    // end is headroom. Folding them together hides the first, which is
    // usually the number a memory map is being read to find.
    let kinds = get(&good(), "region_kind");
    assert!(kinds.iter().any(|kind| kind == "\"padding\""), "{kinds:?}");
    assert!(kinds.iter().any(|kind| kind == "\"free\""));
}

#[test]
fn a_dropped_column_fails() {
    let text = good();
    let without: String = text
        .lines()
        .filter(|line| !line.contains("\"region_owner\""))
        .collect::<Vec<_>>()
        .join("\n");
    validate(&without).expect_err("a missing column must not pass");
}

#[test]
fn a_short_column_fails() {
    let text = good();
    let mut owners = get(&text, "region_owner");
    owners.pop();
    validate(&put(&text, "region_owner", &owners)).expect_err("columns must be index-aligned");
}

#[test]
fn a_gap_fails() {
    let text = good();
    let mut lengths = get(&text, "region_length");
    lengths[0] = "8".to_owned();
    validate(&put(&text, "region_length", &lengths)).expect_err("a hole must not pass");
}

#[test]
fn an_overlap_fails() {
    let text = good();
    let mut starts = get(&text, "region_start");
    starts[1] = "0".to_owned();
    validate(&put(&text, "region_start", &starts)).expect_err("two regions on one byte");
}

#[test]
fn a_stretched_capacity_fails() {
    let text = good();
    let mut capacity = get(&text, "space_capacity");
    capacity[0] = "999999".to_owned();
    validate(&put(&text, "space_capacity", &capacity)).expect_err("regions stopping short");
}

#[test]
fn a_reused_id_fails() {
    let text = good();
    let mut ids = get(&text, "region_id");
    ids[1] = ids[0].clone();
    validate(&put(&text, "region_id", &ids)).expect_err("picking needs unique ids");
}

#[test]
fn a_zero_id_fails() {
    let text = good();
    let mut ids = get(&text, "region_id");
    ids[0] = "0".to_owned();
    validate(&put(&text, "region_id", &ids)).expect_err("zero means 'no region'");
}

#[test]
fn a_dangling_edge_fails() {
    let text = good();
    validate(&put(&text, "rel_to", &["4242".to_owned()])).expect_err("an edge to nowhere");
}

#[test]
fn a_document_that_is_not_this_format_fails() {
    validate("{ \"schema\": \"something.else\" }").expect_err("wrong schema");
}
