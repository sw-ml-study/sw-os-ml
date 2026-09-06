//! Pins the ABI's wire layout.
//!
//! These tests exist to fail. FPGA gateware decodes `ObjectId` by shifting
//! and masking at fixed bit positions (`docs/design.md` s.8.2), so moving a
//! field here silently desynchronises software from a bitstream. A literal
//! expected word is the cheapest way to make that impossible to do by
//! accident.

use mlos_abi::{Error, Fields, ObjectClass, ObjectId};

/// The exact word the layout must produce. If this test fails, either the
/// change was a mistake, or it is an intentional ABI break that needs the
/// gateware rebuilt -- there is no third case.
#[test]
fn layout_is_pinned_to_exact_bit_positions() {
    let id = ObjectId::new(
        ObjectClass::WeightTile,
        Fields {
            model: 0x0007,
            layer: 0x0011,
            tensor: 0x0003,
            tile: 0x03,
        },
    );

    //          class model layer tensor tile
    //             01  0007  0011   0003   03
    assert_eq!(id.0, 0x0100_0700_1100_0303);
}

/// Every field must survive a round trip at its maximum value, which is
/// what catches a shift that overlaps its neighbour.
#[test]
fn fields_do_not_bleed_into_each_other() {
    let fields = Fields {
        model: u16::MAX,
        layer: u16::MAX,
        tensor: u16::MAX,
        tile: u8::MAX,
    };
    let id = ObjectId::new(ObjectClass::Adapter, fields);

    assert_eq!(id.fields(), fields);
    assert_eq!(id.class(), Some(ObjectClass::Adapter));
    assert_eq!(id.0, 0x08FF_FFFF_FFFF_FFFF);
}

/// A zeroed word is what uninitialised memory and a lazy caller both hand
/// us. It must not decode to a real object.
#[test]
fn zero_is_not_a_valid_object() {
    assert_eq!(ObjectId::default().class(), None);
    assert_eq!(ObjectId(0).class(), None);
    assert_eq!(ObjectClass::from_u8(0), None);
}

/// Undefined class bytes are rejected rather than truncated into range.
#[test]
fn undefined_classes_are_rejected() {
    for byte in 9u8..=255 {
        assert_eq!(ObjectClass::from_u8(byte), None, "byte {byte} decoded");
    }
    assert_eq!(ObjectId(0xFF00_0000_0000_0000).class(), None);
}

/// KV and activations are per-session; everything else is per-model. This
/// is the distinction that stops one session's KV aliasing another's.
#[test]
fn only_kv_and_activations_are_session_scoped() {
    let session = [ObjectClass::KvBlock, ObjectClass::Activation];
    for byte in 1u8..=8 {
        let class = ObjectClass::from_u8(byte).expect("1..=8 are defined");
        assert_eq!(class.is_session_scoped(), session.contains(&class));
    }
}

/// Error discriminants are ABI. Zero is success and never an error.
#[test]
fn error_codes_round_trip_and_exclude_zero() {
    for code in 1i32..=8 {
        let err = Error::from_i32(code).expect("1..=8 are defined");
        assert_eq!(err.as_i32(), code);
    }
    assert_eq!(Error::from_i32(0), None);
    assert_eq!(Error::from_i32(9), None);
    assert_eq!(Error::Refused.as_i32(), 5);
}
