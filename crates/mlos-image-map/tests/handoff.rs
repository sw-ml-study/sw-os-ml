//! The handoff document must describe the data that is actually emitted.
//!
//! `docs/layout-handoff.md` tells three other repositories what words to
//! expect, and one of them turns that list into a colour palette that
//! ERRORS on a word it does not have a row for. So a kind added here and
//! not written down there is not a documentation lapse; it is a rendering
//! failure in somebody else's repository, and the person who has to debug
//! it is not the person who caused it.
//!
//! Checked against the committed samples rather than against the source,
//! because the samples are what the other repositories actually pin.

use std::{fs, path::PathBuf};

use mlos_layout::Columns;

/// One file from the repository root.
fn at(path: &str) -> String {
    let full = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path);
    fs::read_to_string(&full).unwrap_or_else(|error| panic!("{}: {error}", full.display()))
}

/// Every distinct value of `column` across both published documents.
fn published(column: &str) -> Vec<String> {
    let mut seen: Vec<String> = ["storage-layout.json", "runtime-layout.json"]
        .iter()
        .flat_map(|name| {
            let text = at(&format!("examples/viz/{name}"));
            Columns::read(&text)
                .strings(column)
                .unwrap_or_else(|why| panic!("{name}: {why}"))
                .into_iter()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .filter(|value| !value.is_empty())
        .collect();
    seen.sort_unstable();
    seen.dedup();
    seen
}

#[test]
fn every_word_the_samples_use_is_written_down() {
    let handoff = at("docs/layout-handoff.md");
    for column in [
        "region_kind",
        "region_space",
        "region_state",
        "region_tier",
        "region_next_use",
    ] {
        for value in published(column) {
            assert!(
                handoff.contains(&value),
                "{column} emits {value:?}, which docs/layout-handoff.md never mentions"
            );
        }
    }
}

#[test]
fn the_palette_rows_it_asks_for_are_the_ones_actually_missing() {
    // sw-mlpl's palette as of their storage-layout-viz work. Copied rather
    // than read across repository boundaries on purpose: this is a record
    // of what was handed over, and it should fail when OUR data outgrows
    // it rather than quietly follow their edits.
    let theirs = [
        "header", "catalog", "image", "free", "padding", "text", "data", "bss", "state", "stack",
        "kernel",
    ];
    let handoff = at("docs/layout-handoff.md");
    for kind in published("region_kind") {
        if theirs.contains(&kind.as_str()) {
            continue;
        }
        // In the fenced list itself, not merely somewhere in the prose
        // around it: the list is what someone copies into a palette, and
        // a kind discussed but not listed is a kind they will not add.
        let section = handoff.split("### Add now").nth(1).unwrap_or_default();
        let listed = section.split("```").nth(1).unwrap_or_default();
        assert!(
            listed.contains(&kind),
            "{kind:?} is not in the 'Add now' list"
        );
    }
}

#[test]
fn the_checksums_it_publishes_match_the_files_beside_it() {
    // The other repositories pin these. A sample regenerated without the
    // document being updated hands them a checksum for a file that no
    // longer exists.
    let handoff = at("docs/layout-handoff.md");
    for name in [
        "storage-layout.json",
        "runtime-layout.json",
        "runtime-events.jsonl",
    ] {
        let digest = sha256(at(&format!("examples/viz/{name}")).as_bytes());
        assert!(
            handoff.contains(&digest),
            "{name} is {digest}, which docs/layout-handoff.md does not carry"
        );
    }
}

/// SHA-256, so the one figure three repositories pin is checked rather
/// than retyped.
///
/// Written out because this workspace has no dependencies and one test is
/// not a reason to acquire the first. The algorithm is fixed and short.
fn sha256(message: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut hash: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut padded = message.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend(((message.len() as u64) * 8).to_be_bytes());

    for block in padded.chunks(64) {
        let mut w = [0u32; 64];
        for (at, word) in block.chunks(4).enumerate() {
            w[at] = u32::from_be_bytes(word.try_into().expect("four bytes"));
        }
        for at in 16..64 {
            let s0 = w[at - 15].rotate_right(7) ^ w[at - 15].rotate_right(18) ^ (w[at - 15] >> 3);
            let s1 = w[at - 2].rotate_right(17) ^ w[at - 2].rotate_right(19) ^ (w[at - 2] >> 10);
            w[at] = w[at - 16]
                .wrapping_add(s0)
                .wrapping_add(w[at - 7])
                .wrapping_add(s1);
        }
        let mut v = hash;
        for at in 0..64 {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let choose = (v[4] & v[5]) ^ (!v[4] & v[6]);
            let one = v[7]
                .wrapping_add(s1)
                .wrapping_add(choose)
                .wrapping_add(K[at])
                .wrapping_add(w[at]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let most = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let two = s0.wrapping_add(most);
            v = [
                one.wrapping_add(two),
                v[0],
                v[1],
                v[2],
                v[3].wrapping_add(one),
                v[4],
                v[5],
                v[6],
            ];
        }
        for (at, word) in v.iter().enumerate() {
            hash[at] = hash[at].wrapping_add(*word);
        }
    }
    hash.iter().map(|word| format!("{word:08x}")).collect()
}
