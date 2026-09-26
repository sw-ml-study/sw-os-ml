//! What a declared stream says, and what advancing costs.

use mlos_abi::{Fields, ObjectClass, ObjectId};
use mlos_objtab::NextUse;
use mlos_stream::{NEVER, Stream, chain};

/// Tile `n` of the synthetic model's sweep.
fn tile(n: u16) -> ObjectId {
    ObjectId::new(
        ObjectClass::WeightTile,
        Fields {
            model: 1,
            layer: n / 16,
            tensor: n % 16,
            tile: 0,
        },
    )
}

/// A declaration and its chain, which a stream borrows rather than owns.
struct Declared {
    objects: Vec<ObjectId>,
    next: Vec<u32>,
}

impl Declared {
    fn of(objects: Vec<ObjectId>) -> Self {
        let mut next = vec![0; objects.len()];
        let mut seen = vec![(ObjectId(0), 0u32); objects.len().max(1)];
        chain(&objects, &mut next, &mut seen).expect("buffers sized from the declaration");
        Self { objects, next }
    }

    fn sweep(count: u16) -> Self {
        Self::of((0..count).map(tile).collect())
    }

    fn stream(&self) -> Stream<'_> {
        let mut stream = Stream::EMPTY;
        stream.declare(&self.objects, &self.next).expect("room");
        stream
    }
}

#[test]
fn an_undeclared_stream_knows_nothing() {
    // Which is what the table said before streams existed, so a workload
    // that declares nothing sees no change at all.
    let stream = Stream::EMPTY;
    assert_eq!(stream.next_after(tile(0)), NextUse::Never);
    assert_eq!(stream.cursor(), 0);
}

#[test]
fn an_object_it_has_never_heard_of_is_never() {
    // An honest answer rather than a maximum: nothing has declared a
    // future for it, which is not the same as it having none.
    let held = Declared::sweep(8);
    assert_eq!(held.stream().next_after(tile(99)), NextUse::Never);
}

#[test]
fn positions_are_one_based_ticks() {
    // Because `ObjectMeta::used_tick` has counted that way since M2 and a
    // policy compares the two. An earlier version answered in zero-based
    // indices, and an object wanted by the very next access read as
    // `Never` -- the most evictable thing in the table. Belady was being
    // told to throw away exactly what it was about to need.
    let held = Declared::sweep(128);
    let stream = held.stream();
    assert_eq!(stream.next_after(tile(5)), NextUse::At(6));
    assert_eq!(stream.next_after(tile(127)), NextUse::At(128));
}

#[test]
fn the_object_on_the_cursor_is_wanted_at_its_next_occurrence() {
    // REGRESSION. It is being acquired NOW, so the use happening now is
    // not the one a policy needs. Answering zero would say "wanted now"
    // forever: a policy computing `at - now` saturates to zero and pins
    // exactly the object it should have evicted first. Found by watching
    // `objs` in a running guest after advancing the stream.
    let held = Declared::of(vec![tile(0), tile(1), tile(0), tile(2)]);
    let mut stream = held.stream();
    assert_eq!(stream.next_after(tile(0)), NextUse::At(3));
    stream.advance(2);
    // On the cursor again, and its next occurrence is now behind it.
    assert_eq!(stream.next_after(tile(0)), NextUse::Never);
}

#[test]
fn running_off_the_end_is_never_rather_than_coming_round_again() {
    // REGRESSION, and the one that cost 27 reads. A stream used to be
    // cyclic -- declare one period, run the cursor past it -- on the
    // reasoning that every token reads the same objects in the same
    // order. True of weights, false of a KV cache, which accumulates. The
    // kernel answered `Never` for every KV block while the simulator,
    // reading the whole trace, knew better.
    let held = Declared::sweep(16);
    let mut stream = held.stream();
    assert_eq!(stream.next_after(tile(3)), NextUse::At(4));
    stream.advance(16);
    assert_eq!(stream.next_after(tile(3)), NextUse::Never);
}

#[test]
fn an_accumulating_tail_is_answered_exactly() {
    // The shape a decode loop actually has: the same weights re-swept,
    // and a growing set of blocks that did not exist on the first pass.
    // Nothing here has a period, which is why nothing here may assume one.
    let (w0, w1) = (tile(0), tile(1));
    let (k0, k1) = (tile(200), tile(201));
    let held = Declared::of(vec![w0, w1, k0, w0, w1, k0, k1]);
    let mut stream = held.stream();
    // Asked as the workload runs, each object sitting on the cursor.
    // Weights come round, and so does the block that already existed.
    assert_eq!(stream.next_after(w0), NextUse::At(4));
    stream.advance(2);
    assert_eq!(stream.next_after(k0), NextUse::At(6));
    // The block that did not exist on the first pass has no second one,
    // and saying so is the answer a cyclic stream could not give.
    stream.advance(4);
    assert_eq!(stream.next_after(k1), NextUse::Never);
}

#[test]
fn a_chain_shorter_than_the_declaration_is_refused() {
    // A short chain is a stream that quietly describes a different
    // workload, and every next-use answer after it would be confidently
    // wrong.
    let held = Declared::sweep(8);
    let mut stream = Stream::EMPTY;
    assert!(stream.declare(&held.objects, &held.next[..4]).is_err());
}

#[test]
fn declaring_again_replaces_and_rewinds() {
    // A new declaration is a new workload; carrying a cursor across would
    // point into a sequence that no longer exists.
    let held = Declared::sweep(128);
    let mut stream = held.stream();
    stream.advance(500);
    let again = Declared::of(vec![tile(7), tile(7)]);
    stream.declare(&again.objects, &again.next).expect("room");
    assert_eq!(stream.cursor(), 0);
    assert_eq!(stream.next_after(tile(7)), NextUse::At(2));
}

#[test]
fn the_chain_points_at_the_next_occurrence_and_then_stops() {
    let objects = vec![tile(0), tile(1), tile(0), tile(0)];
    let mut next = vec![0; objects.len()];
    let mut seen = vec![(ObjectId(0), 0u32); 4];
    chain(&objects, &mut next, &mut seen).expect("room");
    assert_eq!(next, vec![2, NEVER, 3, NEVER]);
}

#[test]
fn too_many_distinct_objects_is_refused_rather_than_silently_partial() {
    let objects: Vec<ObjectId> = (0..8).map(tile).collect();
    let mut next = vec![0; objects.len()];
    let mut seen = vec![(ObjectId(0), 0u32); 4];
    assert!(chain(&objects, &mut next, &mut seen).is_err());
}
