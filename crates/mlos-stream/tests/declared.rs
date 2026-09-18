//! What a declared stream says, and what advancing costs.

use mlos_abi::{Fields, ObjectClass, ObjectId};
use mlos_objtab::NextUse;
use mlos_stream::{PERIOD, Stream};

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

/// A stream declaring `count` tiles in order.
fn sweep(count: u16) -> Stream {
    let order: Vec<ObjectId> = (0..count).map(tile).collect();
    let mut stream = Stream::EMPTY;
    stream.declare(&order).expect("room for the declaration");
    stream
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
    let stream = sweep(8);
    assert_eq!(stream.next_after(tile(99)), NextUse::Never);
}

#[test]
fn each_declared_object_is_wanted_at_its_own_offset() {
    let stream = sweep(128);
    assert_eq!(stream.next_after(tile(5)), NextUse::At(5));
    assert_eq!(stream.next_after(tile(55)), NextUse::At(55));
    assert_eq!(stream.next_after(tile(127)), NextUse::At(127));
}

#[test]
fn the_object_on_the_cursor_is_wanted_a_whole_period_later() {
    // REGRESSION. It is being acquired NOW, so the use happening now is
    // not the one a policy needs. Answering zero would say "wanted now"
    // forever: the position never changes, the cursor runs past it, and a
    // policy computing `at - now` saturates to zero and pins exactly the
    // object it should have evicted first. Found by watching `objs` in a
    // running guest after advancing the stream.
    let mut stream = sweep(128);
    assert_eq!(stream.next_after(tile(0)), NextUse::At(128));
    stream.advance(40);
    assert_eq!(stream.next_after(tile(40)), NextUse::At(168));
}

#[test]
fn advancing_moves_the_answers_and_nothing_else() {
    let mut stream = sweep(128);
    stream.advance(40);
    assert_eq!(stream.cursor(), 40);
    // Still ahead of the cursor: unchanged.
    assert_eq!(stream.next_after(tile(85)), NextUse::At(85));
    // Behind the cursor now, so it comes round on the next sweep.
    assert_eq!(stream.next_after(tile(5)), NextUse::At(133));
}

#[test]
fn it_keeps_cycling() {
    let mut stream = sweep(16);
    for round in 0..4u32 {
        assert_eq!(stream.next_after(tile(3)), NextUse::At(round * 16 + 3));
        stream.advance(16);
    }
}

#[test]
fn a_declaration_too_large_is_refused_rather_than_truncated() {
    // A truncated declaration is a stream that quietly describes a
    // different workload, and every next-use answer after it would be
    // confidently wrong.
    let order: Vec<ObjectId> = (0..=PERIOD as u16).map(tile).collect();
    let mut stream = Stream::EMPTY;
    assert!(stream.declare(&order).is_err());
}

#[test]
fn declaring_again_replaces_and_rewinds() {
    // A new declaration is a new workload; carrying a cursor across would
    // point into a sequence that no longer exists.
    let mut stream = sweep(128);
    stream.advance(500);
    stream.declare(&[tile(7)]).expect("room");
    assert_eq!(stream.cursor(), 0);
    assert_eq!(stream.next_after(tile(7)), NextUse::At(1));
}
