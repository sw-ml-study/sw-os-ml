//! The name of a piece of model state.
//!
//! Invariant: the bit layout is fixed because gateware decodes it with a
//! shift and a mask; the `const` assertions below pin it. Design and
//! history: docs/notes/mlos-abi.md.

use crate::class::ObjectClass;

/// Bit position of the class byte.
const CLASS_SHIFT: u32 = 56;
/// Bit position of the model (or session) field.
const MODEL_SHIFT: u32 = 40;
/// Bit position of the layer field.
const LAYER_SHIFT: u32 = 24;
/// Bit position of the tensor field.
const TENSOR_SHIFT: u32 = 8;

/// The addressing fields of an [`ObjectId`], decoded as one group, the
/// way gateware slices the word.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Fields {
    /// Which model -- or, for a session-scoped class, which session. See
    /// [`ObjectClass::is_session_scoped`].
    pub model: u16,
    /// Which layer within the model.
    pub layer: u16,
    /// Which tensor within the layer.
    pub tensor: u16,
    /// Which tile within the tensor. Zero for tensor-granular objects.
    pub tile: u8,
}

/// The name of a piece of model state. The inner `u64` is public: a raw
/// value from userspace is arbitrary until [`class`](Self::class) accepts
/// it, and no accessor may imply otherwise.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct ObjectId(pub u64);

impl ObjectId {
    /// Builds a well-formed id.
    #[must_use]
    pub const fn new(class: ObjectClass, fields: Fields) -> Self {
        Self(
            ((class as u64) << CLASS_SHIFT)
                | ((fields.model as u64) << MODEL_SHIFT)
                | ((fields.layer as u64) << LAYER_SHIFT)
                | ((fields.tensor as u64) << TENSOR_SHIFT)
                | (fields.tile as u64),
        )
    }

    /// Decodes the class, or `None` if the class byte is not defined. An
    /// all-zero word is rejected here rather than read as object 0.
    #[must_use]
    pub const fn class(self) -> Option<ObjectClass> {
        ObjectClass::from_u8((self.0 >> CLASS_SHIFT) as u8)
    }

    /// Decodes the addressing fields. Infallible: only the class byte can
    /// be malformed.
    #[must_use]
    pub const fn fields(self) -> Fields {
        Fields {
            model: (self.0 >> MODEL_SHIFT) as u16,
            layer: (self.0 >> LAYER_SHIFT) as u16,
            tensor: (self.0 >> TENSOR_SHIFT) as u16,
            tile: self.0 as u8,
        }
    }
}

/// The id is one machine word, and gateware assumes it.
const _: () = assert!(size_of::<ObjectId>() == 8);
/// The fields cover the word exactly: 8 + 16 + 16 + 16 + 8 = 64.
const _: () = assert!(CLASS_SHIFT == MODEL_SHIFT + 16);
const _: () = assert!(MODEL_SHIFT == LAYER_SHIFT + 16);
const _: () = assert!(LAYER_SHIFT == TENSOR_SHIFT + 16);
const _: () = assert!(TENSOR_SHIFT == 8);
