//! The committed samples, checked as text.
//!
//! `examples/viz/` is what the other three repositories develop against,
//! so what matters is not that the emitters COULD produce a good document
//! but that the files actually in this repository are ones. They are read
//! the way a consumer reads them -- out of the text -- rather than through
//! the types that wrote them, because a bug that only appeared in
//! rendering would be invisible to any test that skipped it.
//!
//! Shape, not figures. The numbers move whenever the kernel grows or a
//! sweep gets further; the invariants do not.

use std::{fs, path::PathBuf};

use mlos_layout::{Columns, validate};

/// One committed sample, as the other repositories read it.
fn sample(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/viz")
        .join(name);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// Both of them, by the name they are published under.
const PUBLISHED: [&str; 2] = ["storage-layout.json", "runtime-layout.json"];

#[test]
fn both_samples_satisfy_the_contract() {
    for name in PUBLISHED {
        validate(&sample(name)).unwrap_or_else(|why| panic!("{name}: {why}"));
    }
}

#[test]
fn both_samples_identify_themselves() {
    for name in PUBLISHED {
        let text = sample(name);
        assert!(
            text.contains("\"schema\": \"sw-ml-study.system-layout\""),
            "{name}"
        );
        assert!(text.contains("\"version\": 1"), "{name}");
        assert!(text.contains("\"producer\": \"mlos\""), "{name}");
        // A revision naming a dirty tree is one nobody else can reproduce.
        // Saying so is cheaper than someone else discovering it.
        let revision = text.split("\"revision\": \"").nth(1).unwrap_or_default();
        let revision = revision.split('"').next().unwrap_or_default();
        assert!(
            !revision.is_empty() && revision != "unknown",
            "{name}: {revision:?}"
        );
        assert!(
            !revision.ends_with("-dirty"),
            "{name} was emitted from a dirty tree"
        );
    }
}

#[test]
fn the_extension_columns_are_in_both() {
    // Both documents carry the same column set, so a consumer can switch
    // between them without discovering a column is missing.
    for name in PUBLISHED {
        let found = Columns::read(&sample(name));
        for column in [
            "region_tier",
            "region_object_id",
            "region_state",
            "region_next_use",
        ] {
            found
                .strings(column)
                .unwrap_or_else(|why| panic!("{name}: {why}"));
        }
        for column in ["region_reuse", "region_cost"] {
            found
                .numbers(column)
                .unwrap_or_else(|why| panic!("{name}: {why}"));
        }
    }
}

#[test]
fn the_two_documents_join_on_region_id() {
    // The property the whole pair exists for. A tile has the same id in
    // both files, so a consumer showing the static model can highlight the
    // parts of it that are resident in the runtime snapshot.
    let (built, running) = (sample("storage-layout.json"), sample("runtime-layout.json"));
    let (built, running) = (Columns::read(&built), Columns::read(&running));
    let named = |found: &Columns| {
        let ids = found.numbers("region_id").expect("an id column");
        let names = found.strings("region_name").expect("a name column");
        let spaces = found.strings("region_space").expect("a space column");
        ids.into_iter()
            .zip(
                names
                    .into_iter()
                    .map(str::to_owned)
                    .zip(spaces.into_iter().map(str::to_owned)),
            )
            .collect::<std::collections::HashMap<_, _>>()
    };
    let (built, running) = (named(&built), named(&running));

    let shared: Vec<&u64> = built.keys().filter(|id| running.contains_key(id)).collect();
    assert!(shared.len() > 100, "only {} ids in both", shared.len());
    for id in shared {
        assert_eq!(
            built[id], running[id],
            "id {id:#x} names two different things"
        );
    }
}

#[test]
fn the_runtime_sample_shows_residency_the_static_one_cannot() {
    // If these agreed, the runtime document would be pointless.
    let running = Columns::read(&sample("runtime-layout.json"));
    let states = running.strings("region_state").expect("a state column");
    assert!(
        states.contains(&"resident"),
        "nothing is resident; did the sweep run?"
    );
    assert!(
        states.contains(&"never"),
        "everything is resident, which cannot fit"
    );

    let built = Columns::read(&sample("storage-layout.json"));
    let quiet = built.strings("region_state").expect("a state column");
    assert!(
        !quiet.contains(&"resident"),
        "the build artifact cannot hold a resident object"
    );
}

#[test]
fn the_runtime_sample_has_edges_and_the_static_one_does_not() {
    let running = Columns::read(&sample("runtime-layout.json"));
    assert!(!running.strings("rel_kind").expect("edges").is_empty());
    let built = Columns::read(&sample("storage-layout.json"));
    // Nothing is resident in a build artifact, so there is nothing for an
    // edge to run to. Emitted as an empty column rather than omitted.
    assert!(
        built
            .strings("rel_kind")
            .expect("an edge column")
            .is_empty()
    );
}

#[test]
fn padding_is_not_folded_into_free() {
    // Alignment cost is usually the number a memory map is read to find.
    // SWTOS's emitter keeps the same distinction.
    let kinds = Columns::read(&sample("storage-layout.json"));
    let kinds = kinds.strings("region_kind").expect("a kind column");
    assert!(kinds.contains(&"padding"), "{kinds:?}");
    assert!(kinds.contains(&"free"));
}
