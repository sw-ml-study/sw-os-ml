//! The events and the snapshots must tell the same story.
//!
//! Two emitters describe the same system from two angles: one says what is
//! resident, the other says what happened. Nothing forces them to agree,
//! and they are written in different crates against different data --
//! which means they can drift apart and both keep passing every test that
//! looks at one of them alone.
//!
//! So: take a snapshot before a sweep, take one after, and replay the
//! events onto the first. If the result is not the second, one of the two
//! is lying, and this is the only test that can say so.

mod fixture;

use std::collections::BTreeMap;

use mlos_layout::Columns;

/// A budget a quarter the size of the model's weights.
const BUDGET: usize = 32 * 1024;

/// The arena's allocation granularity.
const GRAIN: u64 = 16;

/// What a region looks like to anything that draws one.
type Region = (String, u64, u64, String, u64);

/// Every region in a document, by id.
fn regions(text: &str) -> BTreeMap<u64, Region> {
    let found = Columns::read(text);
    let column = |name: &str| found.strings(name).expect(name);
    let ids = found.numbers("region_id").expect("region_id");
    let (space, state) = (column("region_space"), column("region_state"));
    let start = found.numbers("region_start").expect("region_start");
    let length = found.numbers("region_length").expect("region_length");
    let reuse = found.numbers("region_reuse").expect("region_reuse");

    (0..ids.len())
        .map(|at| {
            let region = (
                space[at].to_owned(),
                start[at],
                length[at],
                state[at].to_owned(),
                reuse[at],
            );
            (ids[at], region)
        })
        .collect()
}

/// One field of one event line.
///
/// A four-line parser rather than a JSON crate, because what is being
/// tested is that the emitter's own text can be read back -- and a lenient
/// parser that repaired a malformed line would hide exactly that.
fn field(line: &str, key: &str) -> String {
    let rest = line.split(&format!("\"{key}\":")).nth(1).expect(key);
    let value = rest.split([',', '}']).next().expect("a value");
    value.trim().trim_matches('"').to_owned()
}

/// Applies the event stream to `before`, producing what `after` should be.
///
/// The rules are the object manager's, restated: a placement adds an arena
/// region and makes the object resident, a hit is a use and nothing more,
/// a refusal changes nothing at all. If restating them here is wrong, the
/// comparison fails and says which region disagreed.
fn replay(before: &BTreeMap<u64, Region>, events: &str, capacity: u64) -> BTreeMap<u64, Region> {
    let mut after = before.clone();
    let stored = |region: u64| region & 0x0fff_ffff | (1 << 28);

    for line in events.lines().filter(|line| !line.contains("\"dropped\"")) {
        let region: u64 = field(line, "region").parse().expect("a region id");
        let used = |at: &mut BTreeMap<u64, Region>, id: u64| {
            if let Some(found) = at.get_mut(&id) {
                found.3 = "resident".to_owned();
                found.4 += 1;
            }
        };
        match field(line, "event").as_str() {
            "placed" => {
                let bytes: u64 = field(line, "bytes").parse().expect("a byte count");
                let offset: u64 = field(line, "offset").parse().expect("an offset");
                let length = bytes.next_multiple_of(GRAIN);
                let reuse = after.get(&stored(region)).map_or(0, |found| found.4) + 1;
                let dram = (
                    "dram".to_owned(),
                    offset,
                    length,
                    "resident".to_owned(),
                    reuse,
                );
                after.insert(region, dram);
                used(&mut after, stored(region));
            }
            "hit" => {
                used(&mut after, region);
                used(&mut after, stored(region));
            }
            _ => {} // refused: nothing moved, which is the point of recording it
        }
    }
    tail(&mut after, capacity);
    after
}

/// Recomputes the arena's free region, which the events do not mention.
///
/// Derived rather than recorded: it is not a thing that happens, it is
/// what is left over after the things that did.
fn tail(after: &mut BTreeMap<u64, Region>, capacity: u64) {
    let free = 2u64 << 28;
    after.remove(&free);
    let used: u64 = after
        .values()
        .filter(|region| region.0 == "dram")
        .map(|region| region.2)
        .sum();
    if used < capacity {
        let region = (
            "dram".to_owned(),
            used,
            capacity - used,
            "free".to_owned(),
            0,
        );
        after.insert(free, region);
    }
}

#[test]
fn replaying_the_stream_reproduces_the_later_snapshot() {
    let mut manager = fixture::with(BUDGET, 0);
    let before = fixture::document(&manager);
    fixture::sweep(&mut manager);
    fixture::sweep(&mut manager); // again, so the stream carries hits too
    let after = fixture::document(&manager);
    let events = fixture::events(&manager);

    let predicted = replay(&regions(&before), &events, BUDGET as u64);
    let actual = regions(&after);

    // The free region's state word is the one thing replay cannot know --
    // the emitter calls a structural region "fixed" and this calls it
    // "free" -- so compare everything else and check the tail separately.
    let ignore_state = |map: &BTreeMap<u64, Region>| {
        map.iter()
            .map(|(id, region)| (*id, (region.0.clone(), region.1, region.2, region.4)))
            .collect::<BTreeMap<_, _>>()
    };
    assert_eq!(
        ignore_state(&predicted),
        ignore_state(&actual),
        "the events and the snapshot disagree"
    );
    for (id, region) in &actual {
        if region.3 == "fixed" {
            continue;
        }
        assert_eq!(
            predicted[id].3, region.3,
            "region {id:#x} is in the wrong state"
        );
    }
}

#[test]
fn a_run_with_nothing_to_report_replays_to_itself() {
    // The degenerate case, which is the one a replay is most likely to get
    // wrong: an empty stream must leave the picture exactly as it was.
    let manager = fixture::with(BUDGET, 0);
    let before = regions(&fixture::document(&manager));
    let predicted = replay(&before, &fixture::events(&manager), BUDGET as u64);
    assert_eq!(
        predicted.keys().collect::<Vec<_>>(),
        before.keys().collect::<Vec<_>>()
    );
}

#[test]
fn the_stream_accounts_for_every_object_that_became_resident() {
    // A weaker claim than the replay, and worth making separately: if the
    // ring silently dropped events, the replay could still agree by
    // accident on a run where nothing was lost.
    let mut manager = fixture::with(BUDGET, 0);
    fixture::sweep(&mut manager);
    let events = fixture::events(&manager);
    let placed = events
        .lines()
        .filter(|line| line.contains("\"placed\""))
        .count();

    let after = regions(&fixture::document(&manager));
    let resident = after
        .values()
        .filter(|region| region.0 == "dram" && region.3 == "resident");
    assert_eq!(placed, resident.count(), "in:\n{events}");
    // The count is always stated, and here it is zero: nothing was lost.
    // Checked field by field rather than as one long substring, so adding
    // a field to the line does not fail a test about dropped events.
    let lost = events
        .lines()
        .find(|line| line.contains("\"dropped\""))
        .expect("a dropped line, even when nothing was dropped");
    assert!(lost.contains("\"bytes\":0"), "{lost}");
}
