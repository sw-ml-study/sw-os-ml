//! Writing the columnar document out.
//!
//! Column names and their order live here as tables rather than as a run
//! of `push_str` calls, so that adding a column is one line and cannot be
//! added to the header without being added to the body. The contract's own
//! columns come first, in the order the contract lists them, and the
//! producer's extensions follow -- a reader diffing this against
//! `storage-layout-viz.md` should be able to do it by eye.

use crate::{Doc, Region, SCHEMA, Space, VERSION};

/// A column of strings: its name, and how to read it out of one row.
type Text<T> = (&'static str, fn(&T) -> &str);

/// A column of numbers, likewise.
type Nums<T> = (&'static str, fn(&T) -> u64);

/// The space columns that are lists of strings.
const SPACE_TEXT: [Text<Space>; 2] = [("spaces", |s| &s.key), ("space_name", |s| &s.name)];

/// The space columns that are numbers.
const SPACE_NUMS: [Nums<Space>; 2] = [
    ("space_block", |s| s.block),
    ("space_capacity", |s| s.capacity),
];

/// The region columns that are lists of strings, contract first.
const REGION_TEXT: [Text<Region>; 7] = [
    ("region_space", |r| &r.space),
    ("region_kind", |r| &r.kind),
    ("region_name", |r| &r.name),
    ("region_owner", |r| &r.owner),
    ("region_tier", |r| &r.tier),
    ("region_object_id", |r| &r.object),
    ("region_state", |r| &r.state),
];

/// The region columns that are numbers, contract first.
///
/// The three word counts are 24-bit image words, which MLOS has none of:
/// they are emitted as zeros rather than omitted, because a consumer
/// written against SWTOS reads them unconditionally and a missing column
/// is a crash where a zero is a fact.
const REGION_NUMS: [Nums<Region>; 6] = [
    ("region_id", |r| u64::from(r.id)),
    ("region_start", |r| r.start),
    ("region_length", |r| r.length),
    ("region_text_words", |_| 0),
    ("region_data_words", |_| 0),
    ("region_bss_words", |_| 0),
];

/// The whole document.
pub fn document(doc: &Doc, producer: &str, revision: &str) -> String {
    let (spaces, regions) = (&doc.spaces, &doc.regions);
    let mut out = format!(
        "{{\n  \"schema\": \"{SCHEMA}\",\n  \"version\": {VERSION},\n  \
         \"provenance\": {{ \"producer\": {}, \"revision\": {} }},\n\n",
        quoted(producer),
        quoted(revision)
    );
    for (name, of) in SPACE_TEXT {
        out += &column(name, spaces.iter().map(|row| quoted(of(row))));
    }
    for (name, of) in SPACE_NUMS {
        out += &column(name, spaces.iter().map(|row| of(row).to_string()));
    }
    out.push('\n');
    for (name, of) in REGION_NUMS {
        out += &column(name, regions.iter().map(|row| of(row).to_string()));
    }
    for (name, of) in REGION_TEXT {
        out += &column(name, regions.iter().map(|row| quoted(of(row))));
    }
    out.push('\n');
    out += &edges(doc);
    out.truncate(out.trim_end().trim_end_matches(',').len());
    out + "\n}\n"
}

/// The edge table. Three columns of length E, which may be zero.
fn edges(doc: &Doc) -> String {
    column("rel_kind", doc.edges.iter().map(|edge| quoted(&edge.kind)))
        + &column(
            "rel_from",
            doc.edges.iter().map(|edge| edge.from.to_string()),
        )
        + &column("rel_to", doc.edges.iter().map(|edge| edge.to.to_string()))
}

/// One column, rendered items and all.
fn column(name: &str, items: impl Iterator<Item = String>) -> String {
    format!(
        "  \"{name}\": [{}],\n",
        items.collect::<Vec<_>>().join(", ")
    )
}

/// A JSON string, quotes included.
///
/// Rust's `{:?}` is close but not the same -- it emits Rust's own
/// `\u{..}` form for a control character, which JSON does not accept --
/// and "close" in a format three other repos parse is not a property
/// worth relying on.
fn quoted(value: &str) -> String {
    let body: String = value
        .chars()
        .map(|character| match character {
            '"' => "\\\"".to_owned(),
            '\\' => "\\\\".to_owned(),
            '\n' => "\\n".to_owned(),
            '\t' => "\\t".to_owned(),
            '\r' => "\\r".to_owned(),
            control if control < ' ' => format!("\\u{:04x}", control as u32),
            other => other.to_string(),
        })
        .collect();
    format!("\"{body}\"")
}
