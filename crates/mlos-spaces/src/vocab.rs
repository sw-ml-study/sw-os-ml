//! The words MLOS's regions are described by.
//!
//! One definition each, shared by both emitters, because sw-mlpl's palette
//! is keyed on these strings and a second spelling of "weight-tile" would
//! render as nothing at all. Every match here is exhaustive with no
//! wildcard arm, so adding a class or a tier to the ABI is a compile error
//! in this file rather than a region that silently loses its colour.

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

/// Whether an object is in memory: the State mode.
///
/// Three-way, and the middle one is the interesting one. An object that
/// has been wanted before and is not here now was *thrown away*, which is
/// a different fact from never having been asked for -- and it is the fact
/// a residency policy will be judged on. Both are `resident_at == 0`, so
/// nothing but the use count can tell them apart.
#[must_use]
pub const fn state(meta: &ObjectMeta) -> &'static str {
    match (meta.resident_at, meta.reuse_count) {
        (0, 0) => "never",
        (0, _) => "evicted",
        _ => "resident",
    }
}

/// When an object will next be wanted, written out.
///
/// A string with three shapes rather than a number, for the reason
/// [`NextUse`]'s own documentation gives: a dense layer sweep yields an
/// exact position in a declared stream and an MoE router yields a
/// distribution, and those differ in kind rather than in degree. A policy may act on a distance
/// with certainty and on a probability only as a hint, so collapsing them
/// into one column would licence a consumer to draw them the same way.
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
