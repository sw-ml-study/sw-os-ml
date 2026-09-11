//! The bare-target aliases say the same thing, and say the right thing.
//!
//! `.cargo/config.toml` spells the exclusions out four times, and the one
//! previous time this gate had a hole it was exactly that: two lists that
//! were supposed to agree, did not, and four crates were built for the
//! bare targets while never being linted there. An unused import shipped
//! through the gap.
//!
//! So the rule is checked rather than remembered. A crate is host-only
//! when its `lib.rs` does not say `#![no_std]`; every host-only crate must
//! be excluded, every bare-target crate must not be, and all four aliases
//! must agree. Adding a crate and forgetting the config fails here, with a
//! message naming the line to edit.

use std::{collections::BTreeSet, fs, path::PathBuf};

/// The workspace root, from this crate's manifest.
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The `k*` alias lines, by name.
fn aliases() -> Vec<(String, String)> {
    let text = fs::read_to_string(root().join(".cargo/config.toml")).expect("cargo config");
    text.lines()
        .filter_map(|line| line.split_once('='))
        .map(|(name, value)| (name.trim().to_owned(), value.trim().to_owned()))
        .filter(|(name, _)| name.starts_with('k'))
        .collect()
}

/// What one alias excludes.
fn excluded(command: &str) -> BTreeSet<String> {
    let mut words = command.split_whitespace().peekable();
    let mut names = BTreeSet::new();
    while let Some(word) = words.next() {
        if word == "--exclude" {
            names.insert(words.peek().copied().unwrap_or_default().to_owned());
        }
    }
    names
}

/// Every crate whose root module does not say `#![no_std]`.
///
/// `main.rs` as well as `lib.rs`, because `mlos-kernel` is the one crate
/// with no library at all -- and it is the crate the bare-target build
/// exists for, so misreading it as host-only would be the worst possible
/// way to be wrong.
fn host_only() -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for entry in fs::read_dir(root().join("crates")).expect("crates/") {
        let path = entry.expect("a directory entry").path();
        let bare = ["src/lib.rs", "src/main.rs"].iter().any(|root| {
            fs::read_to_string(path.join(root)).is_ok_and(|text| text.contains("#![no_std]"))
        });
        if !bare {
            names.insert(
                path.file_name()
                    .expect("a crate name")
                    .to_string_lossy()
                    .into_owned(),
            );
        }
    }
    names
}

#[test]
fn all_four_aliases_exclude_the_same_crates() {
    let aliases = aliases();
    assert_eq!(aliases.len(), 4, "expected kbuild/kclippy for two targets");
    let first = excluded(&aliases[0].1);
    for (name, command) in &aliases {
        assert_eq!(
            excluded(command),
            first,
            "{name} excludes a different set; that is the drift this test exists for"
        );
    }
}

#[test]
fn the_exclusions_are_exactly_the_host_only_crates() {
    let (name, command) = aliases().remove(0);
    assert_eq!(
        excluded(&command),
        host_only(),
        "{name} in .cargo/config.toml is out of step with the workspace: a crate \
         without #![no_std] cannot link for a bare target and must be excluded, \
         and one with it must not be"
    );
}

#[test]
fn the_kernel_is_never_excluded() {
    // It is the whole point of the bare-target build. Excluding it would
    // leave the aliases passing while checking nothing that matters.
    for (name, command) in aliases() {
        assert!(
            !excluded(&command).contains("mlos-kernel"),
            "{name} excludes the kernel"
        );
    }
}
