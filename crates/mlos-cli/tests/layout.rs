//! The runtime snapshot, end to end, through a real boot.
//!
//! Ignored by default: it builds a kernel, boots a VM under TCG, drives
//! the shell and reads back what it printed. Far too slow for the ordinary
//! `cargo test` gate, and it needs QEMU. CI runs it explicitly:
//!
//! ```text
//! cargo test -p mlos-cli -- --ignored
//! ```
//!
//! What it covers that the fast tests cannot: that boot arguments reach
//! the shell at all, that a document survives the console round trip, and
//! that the two emitters -- one on the host, one in the guest, sharing no
//! code -- agree about which region is which.

use std::{fs, process::Command};

/// The binary cargo built for this test.
const MLOS: &str = env!("CARGO_BIN_EXE_mlos");

/// The workspace root, so the tool writes where it expects to.
const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

/// Runs `mlos <verb>` and returns the file it wrote.
fn emit(verb: &str) -> String {
    let out = Command::new(MLOS)
        .args([verb])
        .current_dir(ROOT)
        .output()
        .expect("mlos runs");
    assert!(
        out.status.success(),
        "mlos {verb} failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let path = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    fs::read_to_string(format!("{ROOT}/{path}")).expect("the file it said it wrote")
}

/// Every region id in a document, with what it names.
fn named(text: &str) -> Vec<(u64, String)> {
    let found = mlos_layout::Columns::read(text);
    let ids = found.numbers("region_id").expect("an id column");
    let names = found.strings("region_name").expect("a name column");
    ids.into_iter()
        .zip(names.into_iter().map(str::to_owned))
        .collect()
}

#[test]
#[ignore = "boots a VM; run with --ignored"]
fn a_booted_mlos_emits_a_layout_the_host_validator_accepts() {
    let running = emit("runtime");
    // `mlos runtime` validates before writing, so reaching here already
    // means it passed -- but the file on disk is what other repositories
    // read, and checking that is not the same statement.
    mlos_layout::validate(&running).expect("a booted MLOS emits a valid document");
    assert!(running.contains("\"producer\": \"mlos\""));
    // The revision is stamped by the host and carried into the guest
    // through `/chosen/bootargs`; `unknown` means that channel broke.
    assert!(
        !running.contains("\"revision\": \"unknown\""),
        "{running:.400}"
    );
}

#[test]
#[ignore = "boots a VM; run with --ignored"]
fn the_guest_and_the_host_agree_about_which_region_is_which() {
    let (built, running) = (emit("layout"), emit("runtime"));
    let (built, running) = (named(&built), named(&running));

    let shared: Vec<&(u64, String)> = built
        .iter()
        .filter(|(id, _)| running.iter().any(|(other, _)| other == id))
        .collect();
    // 128 weight tiles plus the arena's free region, when there is one.
    assert!(shared.len() >= 128, "only {} ids in both", shared.len());
    for (id, name) in shared {
        let theirs = running
            .iter()
            .find(|(other, _)| other == id)
            .expect("a shared id");
        assert_eq!(*name, theirs.1, "id {id:#x} names two different things");
    }
}

#[test]
#[ignore = "boots a VM; run with --ignored"]
fn running_the_model_changes_what_the_snapshot_says() {
    // The pair's whole reason for existing: the build artifact says what
    // exists, the snapshot says what is resident, and they must differ.
    let (built, running) = (emit("layout"), emit("runtime"));
    let state = |text: &str| {
        mlos_layout::Columns::read(text)
            .strings("region_state")
            .expect("a state column")
            .iter()
            .filter(|found| **found == "resident")
            .count()
    };
    assert_eq!(state(&built), 0, "a build artifact holds nothing resident");
    assert!(state(&running) > 0, "the sweep faulted nothing in");
}
