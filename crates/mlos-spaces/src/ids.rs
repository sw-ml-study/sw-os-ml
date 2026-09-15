//! Stable region ids.
//!
//! The contract says `region_id` is assigned by the producer and stays the
//! same for the same thing across snapshots: it is what native3d picks on,
//! and what cross-highlights a stored tile against the arena region
//! holding its bytes. A positional index would break the moment a region
//! is inserted.
//!
//! ```text
//!  31  28 27  24 23      16 15       8 7      0
//! +------+------+----------+----------+--------+
//! | space| class|  layer   |  tensor  |  tile  |
//! +------+------+----------+----------+--------+
//! ```
//!
//! The whole thing fits in 31 bits, which matters because a JSON number is
//! safe to 2^53 in some consumers and to 2^31 in others.
//!
//! The model field is deliberately NOT in the id. One model fits; a second
//! would collide, so [`object`] refuses rather than truncating -- a loud
//! failure at the point a second model is registered is worth more than a
//! picture that quietly shows two models on top of each other.

use mlos_abi::ObjectId;

use crate::Where;

/// The stable id of an object's region in `place`.
///
/// `None` when the object cannot be named in 31 bits -- a second model, or
/// a layer or tensor past 255. Every one of those is a real change to the
/// model rather than bad input, and each wants this scheme widened
/// deliberately rather than wrapped around silently.
#[must_use]
pub fn object(place: Where, id: ObjectId) -> Option<u32> {
    let class = id.class()?;
    let fields = id.fields();
    if fields.model != 1 || fields.layer > 0xff || fields.tensor > 0xff {
        return None;
    }
    Some(
        (place as u32) << 28
            | (class as u32) << 24
            | (fields.layer as u32) << 16
            | (fields.tensor as u32) << 8
            | u32::from(fields.tile),
    )
}
