//! What kind of model state an object holds.
//!
//! The class is what makes a model fault more useful than a page fault:
//! it selects which residency policy applies, and it is the axis every
//! metric in `docs/PRD.md` s.5.2 is broken down by. "40,000 faults" is
//! meaningless; "38,000 of them cold KV blocks" is a diagnosis.

/// The class of an [`ObjectId`](crate::ObjectId).
///
/// Discriminants are part of the ABI and are never reused or renumbered:
/// they are decoded in FPGA gateware (`docs/design.md` s.8.2), where a
/// renumbering means a bitstream rebuild.
///
/// Zero is deliberately not a class, so an all-zero `ObjectId` -- the
/// value uninitialised memory and a lazy caller both produce -- fails to
/// decode instead of naming object 0 of model 0.
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ObjectClass {
    /// Dense layer weights. Immutable, large, consumed sequentially,
    /// shareable across every session using the model.
    WeightTile = 1,
    /// Quantization scales. Tiny, hot, effectively always resident.
    Scale = 2,
    /// One MoE expert's weights. Immutable, selected probabilistically by
    /// the router -- the class that makes prefetch a distribution rather
    /// than a guess.
    Expert = 3,
    /// A block of one session's KV cache. Mutable, grows with context,
    /// partly compressible, partly droppable.
    KvBlock = 4,
    /// Intermediate activations. Transient, and usually cheaper to
    /// recompute than to reload -- the class that makes `DISCARD` a
    /// legitimate eviction verb.
    Activation = 5,
    /// A block of an embedding table. Immutable, randomly accessed.
    EmbedBlock = 6,
    /// A retrieved document block. Immutable, semantically addressed.
    RagBlock = 7,
    /// A LoRA-scale adapter. Small, and the only weights that are mutable
    /// before training enters scope.
    Adapter = 8,
}

impl ObjectClass {
    /// Every class, in discriminant order.
    ///
    /// Exists so accounting can be per-class without anywhere keeping a
    /// second list that drifts from this one. Adding a class here is the
    /// only edit a new class needs.
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
    ///
    /// Session-scoped classes reuse [`Fields::model`](crate::Fields::model)
    /// as a session id; the class is what disambiguates the two readings.
    /// Getting this wrong would let one session's KV alias another's, so
    /// it is a single function rather than a convention.
    #[must_use]
    pub const fn is_session_scoped(self) -> bool {
        matches!(self, Self::KvBlock | Self::Activation)
    }
}
