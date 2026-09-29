//! The words MLOS's regions are described by.
//!
//! Contract: sw-mlpl's palette is keyed on these exact strings. Every
//! match is exhaustive with no wildcard arm. Design:
//! docs/notes/mlos-spaces.md.

use core::fmt;

use mlos_abi::ObjectClass;
use mlos_objtab::{NextUse, ObjectMeta, Tier};

/// What a viewer colours an object by: the Purpose mode.
#[must_use]
pub const fn kind(class: ObjectClass) -> &'static str {
    match class {
        ObjectClass::WeightTile => "weight-tile",
        ObjectClass::Scale => "scale",
        ObjectClass::Expert => "expert",
        ObjectClass::KvBlock => "kv-block",
        ObjectClass::Activation => "activation",
        ObjectClass::EmbedBlock => "embed-block",
        ObjectClass::RagBlock => "rag-block",
        ObjectClass::Adapter => "adapter",
    }
}

/// Where an object currently lives: the Location mode.
#[must_use]
pub const fn tier(tier: Tier) -> &'static str {
    match tier {
        Tier::Hot => "hot",
        Tier::Warm => "warm",
        Tier::Cold => "cold",
        Tier::Stream => "stream",
        Tier::Archive => "archive",
    }
}

/// Whether an object is in memory: the State mode. `never` and `evicted`
/// both have `resident_at == 0`; only the use count tells them apart.
#[must_use]
pub const fn state(meta: &ObjectMeta) -> &'static str {
    match (meta.resident_at, meta.reuse_count) {
        (0, 0) => "never",
        (0, _) => "evicted",
        _ => "resident",
    }
}

/// When an object will next be wanted, written out as one of three
/// shapes: `never`, `at N`, `probability P`.
pub struct NextUseText(pub NextUse);

impl fmt::Display for NextUseText {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            NextUse::Never => out.write_str("never"),
            NextUse::At(position) => write!(out, "at {position}"),
            NextUse::Probability(chance) => write!(out, "probability {chance}"),
        }
    }
}
