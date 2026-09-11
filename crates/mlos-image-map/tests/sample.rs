//! The committed sample, checked as text.
//!
//! `examples/viz/storage-layout.json` is what the other three repos
//! develop against, so what matters is not that the emitter could produce
//! a good document but that the file actually in this repository is one.
//! It is parsed here the way a consumer parses it -- out of the text --
//! rather than through the types that wrote it, because a bug that only
//! shows up in rendering would be invisible to any test that skipped it.
//!
//! Shape, not figures. The numbers move whenever the kernel grows or the
//! model gains a tile; the invariants do not.

use std::{collections::HashSet, fs, path::PathBuf};

/// The sample, as the other repos read it.
fn sample() -> String {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/viz/storage-layout.json");
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// Every `"name": [...]` column, as raw items.
fn columns(text: &str) -> Vec<(String, Vec<String>)> {
    text.lines()
        .filter_map(|line| {
            let (name, rest) = line.trim().split_once("\": [")?;
            let items = rest.trim_end_matches([',', ']']);
            let values = if items.is_empty() {
                Vec::new()
            } else {
                items.split(", ").map(str::to_owned).collect()
            };
            Some((name.trim_start_matches('"').to_owned(), values))
        })
        .collect()
}

/// One column by name.
fn column(text: &str, name: &str) -> Vec<String> {
    columns(text)
        .into_iter()
        .find(|(found, _)| found == name)
        .unwrap_or_else(|| panic!("no {name} column"))
        .1
}

/// A numeric column.
fn numbers(text: &str, name: &str) -> Vec<u64> {
    column(text, name)
        .iter()
        .map(|item| item.parse().unwrap_or_else(|_| panic!("{name}: {item:?}")))
        .collect()
}

#[test]
fn it_identifies_itself() {
    let text = sample();
    assert!(
        text.contains("\"schema\": \"sw-ml-study.system-layout\""),
        "{text:.200}"
    );
    assert!(text.contains("\"version\": 1"));
    assert!(text.contains("\"producer\": \"mlos\""));
    assert!(text.contains("\"revision\""));
}

#[test]
fn every_column_is_index_aligned() {
    let text = sample();
    let regions = column(&text, "region_id").len();
    let spaces = column(&text, "spaces").len();
    let edges = column(&text, "rel_kind").len();
    assert!(regions > 0 && spaces > 0);

    for (name, values) in columns(&text) {
        let want = match name.split('_').next() {
            Some("region") => regions,
            Some("spaces" | "space") => spaces,
            _ => edges,
        };
        assert_eq!(values.len(), want, "column {name} is the wrong length");
    }
}

#[test]
fn the_contract_columns_are_all_present() {
    let names: Vec<String> = columns(&sample())
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    // Exactly the list in ../sw-mlpl/docs/storage-layout-viz.md, plus the
    // extensions MLOS adds. A consumer written against SWTOS reads the
    // first group unconditionally.
    for required in [
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
        "region_tier",
        "region_object_id",
        "region_state",
    ] {
        assert!(
            names.contains(&required.to_owned()),
            "{required} is missing"
        );
    }
}

#[test]
fn regions_tile_every_space_exactly_once() {
    let text = sample();
    let keys = column(&text, "spaces");
    let capacity = numbers(&text, "space_capacity");
    let (space, start) = (
        column(&text, "region_space"),
        numbers(&text, "region_start"),
    );
    let length = numbers(&text, "region_length");

    for (key, capacity) in keys.iter().zip(capacity) {
        let mut extents: Vec<(u64, u64)> = (0..space.len())
            .filter(|index| space[*index] == *key)
            .map(|index| (start[index], length[index]))
            .collect();
        extents.sort_unstable();
        let covered = extents.iter().fold(0, |at, (start, length)| {
            assert_eq!(*start, at, "{key} has a hole or an overlap at {at:#x}");
            at + length
        });
        assert_eq!(covered, capacity, "{key} is not covered to its capacity");
    }
}

#[test]
fn ids_are_unique_and_never_zero() {
    let ids = numbers(&sample(), "region_id");
    assert!(!ids.contains(&0), "zero is reserved for 'no region'");
    let unique: HashSet<&u64> = ids.iter().collect();
    assert_eq!(unique.len(), ids.len(), "picking needs ids to be unique");
}

#[test]
fn padding_is_not_folded_into_free() {
    let kinds = column(&sample(), "region_kind");
    // Alignment cost is the number a memory map is usually being read to
    // find. SWTOS's emitter keeps the same distinction.
    assert!(kinds.iter().any(|kind| kind == "\"padding\""), "{kinds:?}");
    assert!(kinds.iter().any(|kind| kind == "\"free\""));
}

#[test]
fn object_columns_are_empty_exactly_where_there_is_no_object() {
    let text = sample();
    let (object, tier) = (
        column(&text, "region_object_id"),
        column(&text, "region_tier"),
    );
    for (object, tier) in object.iter().zip(&tier) {
        // Structural regions carry the empty string in every object-only
        // column. A tier on something that is not an object would be a
        // colour mode showing a fact that does not exist.
        assert_eq!(object == "\"\"", tier == "\"\"", "{object} / {tier}");
    }
    assert!(object.iter().any(|id| id != "\"\""), "no objects at all");
}
