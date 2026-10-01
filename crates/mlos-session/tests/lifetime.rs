//! A session is created, holds, is refused, and is destroyed.

use mlos_abi::Error;
use mlos_session::{Contract, SessionId, Sessions};

#[test]
fn ids_start_at_one_and_fill_the_lowest_gap() {
    let mut held = Sessions::<4>::EMPTY;
    assert_eq!(held.create(Contract::NONE), Ok(SessionId(1)));
    assert_eq!(held.create(Contract::NONE), Ok(SessionId(2)));
    held.destroy(SessionId(1)).expect("known");
    assert_eq!(held.create(Contract::NONE), Ok(SessionId(1)));
}

#[test]
fn a_full_table_refuses_and_that_is_admission_control() {
    let mut held = Sessions::<2>::EMPTY;
    held.create(Contract::NONE).expect("one");
    held.create(Contract::NONE).expect("two");
    assert_eq!(held.create(Contract::NONE), Err(Error::Refused));
    assert_eq!(
        held.adopt(SessionId(9), Contract::NONE),
        Err(Error::Refused)
    );
}

#[test]
fn an_adopted_id_cannot_be_taken_twice_and_zero_is_nobody() {
    let mut held = Sessions::<4>::EMPTY;
    held.adopt(SessionId(3), Contract::NONE).expect("free");
    assert_eq!(
        held.adopt(SessionId(3), Contract::NONE),
        Err(Error::Refused)
    );
    assert_eq!(
        held.adopt(SessionId(0), Contract::NONE),
        Err(Error::Refused)
    );
    assert_eq!(held.destroy(SessionId(7)), Err(Error::BadObject));
}

#[test]
fn the_ceiling_refuses_before_anything_is_counted() {
    let mut held = Sessions::<4>::EMPTY;
    let promised = Contract {
        resident_ceiling: 1024,
        ..Contract::NONE
    };
    let id = held.create(promised).expect("room");
    held.charge(id, 600).expect("under");
    assert_eq!(held.charge(id, 600), Err(Error::Refused));
    assert_eq!(held.get(id).map(|s| s.resident), Some(600));
    held.credit(id, 600);
    held.charge(id, 1024).expect("exactly the ceiling fits");
    // Nobody's objects, and a stranger's, are not counted against anyone.
    held.charge(SessionId(0), 1 << 30).expect("uncounted");
    held.charge(SessionId(42), 1 << 30).expect("uncounted");
}
