//! Stable region ids: the same object gets the same id in every document.
//!
//! ```text
//!  31  28 27  24 23      16 15       8 7      0
//! +------+------+----------+----------+--------+
//! | space| class|  layer   |  tensor  |  tile  |
//! +------+------+----------+----------+--------+
//! ```
//!
//! Invariant: an id fits in 31 bits, and the model field is not in it, so
//! a second model is refused rather than collided. Design:
//! docs/notes/mlos-spaces.md.

use mlos_abi::ObjectId;

use crate::Where;

/// The stable id of an object's region in `place`. `None` when the object
/// cannot be named in 31 bits: a second model, or a layer or tensor past
/// 255.
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
