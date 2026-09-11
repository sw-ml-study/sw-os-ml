//! Stable region ids, and which space a region is in.
//!
//! The contract says `region_id` is assigned by the producer and stays the
//! same for the same thing across snapshots: it is what native3d picks on
//! and what cross-highlights a disk extent against the arena region
//! holding its bytes. A positional index would break the moment a region
//! is inserted, so the id is derived from what the region *is*.
//!
//! ```text
//!  31  28 27  24 23      16 15       8 7      0
//! +------+------+----------+----------+--------+
//! | space| class|  layer   |  tensor  |  tile  |
//! +------+------+----------+----------+--------+
//! ```
//!
//! Class zero is not a valid [`ObjectClass`], so structural regions --
//! kernel sections, padding, free space -- take the class-zero range of
//! their space and count up from it. Nothing collides, id zero is never
//! issued, and the whole thing fits in 31 bits, which matters because a
//! JSON number is only safe to 2^53 in some consumers and to 2^31 in
//! others.
//!
//! The model field is deliberately NOT in the id. One model fits; a second
//! would collide, so [`object`] refuses rather than truncating -- a loud
//! failure at the point a second model is registered is worth more than a
//! picture that quietly shows two models on top of each other.

use mlos_abi::ObjectId;
use mlos_layout::Space;

/// Which space a region lives in.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Where {
    /// The block device holding the model's weights.
    Disk = 1,
    /// The object arena: where a faulted-in object is placed.
    Dram = 2,
    /// Guest RAM as the machine presents it.
    Sysram = 3,
}

impl Where {
    /// The `spaces` key, and what `region_space` joins on.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Disk => "disk",
            Self::Dram => "dram",
            Self::Sysram => "sysram",
        }
    }

    /// Ids for this space's structural regions, in issue order.
    pub fn ids(self) -> impl Iterator<Item = u32> {
        (self as u32) << 28..
    }

    /// This space, with a display name and a block size.
    #[must_use]
    pub fn space(self, name: &str, block: u64, capacity: u64) -> Space {
        Space {
            key: self.key().to_owned(),
            name: name.to_owned(),
            block,
            capacity,
        }
    }
}

/// The stable id of an object's region in `place`.
///
/// `None` when the object cannot be named in 31 bits -- a second model, or
/// a layer or tensor past 255. Every one of those is a real change to the
/// synthetic model rather than bad input, and each wants this scheme
/// widened deliberately rather than wrapped around silently.
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
