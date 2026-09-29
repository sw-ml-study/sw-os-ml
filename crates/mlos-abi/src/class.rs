//! What kind of model state an object holds.
//!
//! Invariant: discriminants are decoded in gateware and are never reused
//! or renumbered; zero is not a class, so an all-zero id fails to decode.
//! Design: docs/notes/mlos-abi.md.

/// The class of an [`ObjectId`](crate::ObjectId). Discriminants are part
/// of the ABI; zero is deliberately not a class.
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ObjectClass {
    /// Dense layer weights. Immutable, shareable across sessions.
    WeightTile = 1,
    /// Quantization scales. Tiny and hot.
    Scale = 2,
    /// One MoE expert's weights. Immutable, selected by the router.
    Expert = 3,
    /// A block of one session's KV cache. Mutable, session-scoped.
    KvBlock = 4,
    /// Intermediate activations. Transient, session-scoped, recomputable.
    Activation = 5,
    /// A block of an embedding table. Immutable, randomly accessed.
    EmbedBlock = 6,
    /// A retrieved document block. Immutable, semantically addressed.
    RagBlock = 7,
    /// A LoRA-scale adapter. Small, mutable.
    Adapter = 8,
}

impl ObjectClass {
    /// Every class, in discriminant order. The one list per-class
    /// accounting indexes by; a new class is added here and nowhere else.
    pub const ALL: [Self; 8] = [
        Self::WeightTile,
        Self::Scale,
        Self::Expert,
        Self::KvBlock,
        Self::Activation,
        Self::EmbedBlock,
        Self::RagBlock,
        Self::Adapter,
    ];

    /// Its position in [`Self::ALL`], for indexing a per-class array.
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize - 1
    }

    /// Decodes a class byte, rejecting anything this ABI does not define.
    #[must_use]
    pub const fn from_u8(value: u8) -> Option<Self> {
        Some(match value {
            1 => Self::WeightTile,
            2 => Self::Scale,
            3 => Self::Expert,
            4 => Self::KvBlock,
            5 => Self::Activation,
            6 => Self::EmbedBlock,
            7 => Self::RagBlock,
            8 => Self::Adapter,
            _ => return None,
        })
    }

    /// Whether this class is scoped to a session rather than to a model.
    /// Session-scoped classes read [`Fields::model`](crate::Fields::model)
    /// as a session id; the class is the only thing that disambiguates.
    #[must_use]
    pub const fn is_session_scoped(self) -> bool {
        matches!(self, Self::KvBlock | Self::Activation)
    }
}
