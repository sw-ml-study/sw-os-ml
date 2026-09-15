//! The layout documents, end to end, through a real boot.
//!
//! Ignored by default: it builds a kernel, boots a VM under TCG, drives
//! the shell and reads back what it printed. Far too slow for the ordinary
//! `cargo test` gate, and it needs QEMU. CI runs it explicitly:
//!
//! ```text
//! cargo test -p mlos-cli -- --ignored
//! ```
//!
//! What this covers that the fast tests cannot: that boot arguments reach
//! the shell at all, that a document survives the console round trip, and
//! that the two emitters -- one on the host, one in the guest, sharing no
//! code -- agree about which region is which.
//!
//! Emitted ONCE and shared. `mlos layout` and `mlos runtime` write to
//! fixed paths under `build/`, so a test per boot would have five tests
//! writing the same three files at once; they passed serially and failed
//! in parallel, which is the worst way for a test to be wrong.

use std::{fs, process::Command, sync::OnceLock};

/// The binary cargo built for this test.
const MLOS: &str = env!("CARGO_BIN_EXE_mlos");

/// The workspace root, so the tool writes where it expects to.
const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

/// The static document, the runtime one, and the event stream.
struct Emitted {
    built: String,
    running: String,
    events: String,
}

/// Runs `mlos <verb>` and returns the file it named.
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
    read(&path)
}

/// One file under the workspace root.
fn read(path: &str) -> String {
    fs::read_to_string(format!("{ROOT}/{path}")).expect("a file the tool wrote")
}

/// Both documents and the stream, from one run of each command.
fn artifacts() -> &'static Emitted {
    static ONCE: OnceLock<Emitted> = OnceLock::new();
    ONCE.get_or_init(|| {
        let built = emit("layout");
        let running = emit("runtime");
        Emitted {
            built,
            running,
            events: read("build/runtime-events.jsonl"),
        }
    })
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

/// The `region` field of one event line.
fn region(line: &str) -> u64 {
    line.split("\"region\":")
        .nth(1)
        .and_then(|rest| rest.split(',').next())
        .and_then(|value| value.parse().ok())
        .expect("a region id")
}

#[test]
#[ignore = "boots a VM; run with --ignored"]
fn a_booted_mlos_emits_a_layout_the_host_validator_accepts() {
    let running = &artifacts().running;
    // `mlos runtime` validates before writing, so reaching here already
    // means it passed -- but the file on disk is what other repositories
    // read, and checking that is not the same statement.
    mlos_layout::validate(running).expect("a booted MLOS emits a valid document");
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
    let (built, running) = (named(&artifacts().built), named(&artifacts().running));
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
    let resident = |text: &str| {
        mlos_layout::Columns::read(text)
            .strings("region_state")
            .expect("a state column")
            .iter()
            .filter(|found| **found == "resident")
            .count()
    };
    assert_eq!(resident(&artifacts().built), 0, "a build holds nothing");
    assert!(
        resident(&artifacts().running) > 0,
        "the sweep faulted nothing in"
    );
}

#[test]
#[ignore = "boots a VM; run with --ignored"]
fn a_booted_mlos_emits_an_event_stream_beside_the_snapshot() {
    let events = &artifacts().events;
    let lines: Vec<&str> = events.lines().collect();
    assert!(lines.len() > 30, "only {} events:\n{events}", lines.len());

    // The marker is stripped, so what is left is plain JSON Lines. Every
    // line stands alone, which is what lets a truncated capture still
    // yield everything before the cut.
    for line in &lines {
        assert!(line.starts_with('{') && line.ends_with('}'), "{line:?}");
        assert!(
            !line.contains("@ev"),
            "the marker should be stripped: {line:?}"
        );
    }
    // A boot that faults, re-uses and runs out produces all three.
    for kind in ["\"placed\"", "\"hit\"", "\"refused\""] {
        assert!(events.contains(kind), "no {kind} event in the stream");
    }
    // Losing events silently would make a replay rebuild the wrong
    // picture with no way to know, so the count is always stated.
    assert!(events.contains("\"dropped\""), "no dropped count");
}

#[test]
#[ignore = "boots a VM; run with --ignored"]
fn every_event_names_a_region_in_the_snapshot_beside_it() {
    // Both files come from one boot, so an event naming a region the
    // snapshot does not have would mean the two emitters disagree about
    // the id scheme -- the one thing they must share.
    let ids = mlos_layout::Columns::read(&artifacts().running)
        .numbers("region_id")
        .expect("an id column");

    let stream = artifacts().events.lines();
    for line in stream.filter(|line| !line.contains("\"dropped\"")) {
        // A refused object never became resident, so it has no arena
        // region -- but its id is still the name of where it would have
        // gone, and the stored region of the same object does exist.
        let (region, stored) = (region(line), region(line) & 0x0fff_ffff | (1 << 28));
        assert!(
            ids.contains(&region) || ids.contains(&stored),
            "{line}\nnames a region in neither space"
        );
    }
}
