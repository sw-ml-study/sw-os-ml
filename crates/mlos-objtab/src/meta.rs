//! What the kernel knows about one piece of model state.
//!
//! Every field here exists to answer a question a page-based operating
//! system cannot ask. A page table entry says where a page is and whether
//! it is dirty. This says how expensive the object would be to get back,
//! when it will next be wanted, and how many sessions are waiting on it
//! -- and those are the inputs to every decision in `docs/PRD.md`.

/// How an object's numbers are stored.
///
/// Not decoration: precision is a *choice the kernel can make*. Demoting
/// cold KV from FP16 to Q4 is rung 2 of the degradation ladder, and it is
/// the reason an ML workload can be made cheaper under pressure where an
/// ordinary process cannot.
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Precision {
    /// IEEE half.
    Fp16 = 1,
    /// Brain float.
    Bf16 = 2,
    /// 8-bit quantized.
    Q8 = 3,
    /// 4-bit quantized.
    Q4 = 4,
    /// 3-bit quantized.
    Q3 = 5,
    /// Ternary.
    Ternary = 6,
}

/// Where an object currently lives, ordered by cost of access.
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Tier {
    /// Device memory or pinned DRAM. Leases held; never evicted silently.
    Hot = 1,
    /// DRAM. Evictable under pressure.
    Warm = 2,
    /// A block store. Fetched on fault.
    Cold = 3,
    /// Never resident: consumed as it passes.
    ///
    /// The tier a page-based system cannot express. A streamed object is
    /// not "in memory" in a way you could point at; it is a scheduled flow
    /// that compute is arranged around.
    Stream = 4,
    /// No bytes stored at all -- recomputed, regenerated or dropped.
    Archive = 5,
}

/// When an object will next be wanted.
///
/// The field that does not exist in any page-based operating system, and
/// the one that makes this whole design worth building. Three-way rather
/// than a number because the two kinds of knowledge differ in kind, not
/// degree: a dense layer sweep yields an exact distance, an MoE router
/// yields a distribution. A policy may act on `Distance` with certainty
/// and on `Probability` only as a hint, and collapsing them to one number
/// would silently license the wrong decision.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum NextUse {
    /// Not known to be wanted again.
    #[default]
    Never,
    /// Wanted again in exactly this many steps of a declared stream.
    Distance(u32),
    /// Wanted with this likelihood, as a fraction of `u16::MAX`.
    Probability(u16),
}

/// A cost in nanoseconds.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub struct CostNs(pub u32);

impl CostNs {
    /// Cannot be done at any price.
    ///
    /// Weights have no recompute cost: there is no computation that
    /// produces them. Eviction must be able to tell "expensive" from
    /// "impossible", because it may choose the first and never the second.
    pub const IMPOSSIBLE: Self = Self(u32::MAX);
}

/// Whether an object can change, and how.
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mutability {
    /// Weights. Identical for every session, so one copy serves all.
    Immutable = 1,
    /// Shared until written, then private. How a model fork stays cheap.
    CowOverlay = 2,
    /// KV, activations, adapters.
    Mutable = 3,
}

/// Which provider can produce this object.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct ProviderId(pub u8);

/// Which session owns it, or zero for objects shared across all of them.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct SessionId(pub u16);

/// Everything the kernel records about an object.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ObjectMeta {
    /// Bytes, at the current precision.
    pub size: u32,
    /// How the numbers are stored.
    pub precision: Precision,
    /// Where it is now.
    pub tier: Tier,
    /// Who can produce it.
    pub provider: ProviderId,
    /// Where its *home* is, as that provider understands "where".
    ///
    /// Opaque on purpose: a DRAM address, a block number, a recipe for
    /// recomputing it. The table records which provider to ask and what
    /// to tell it; only the provider knows what the number means.
    ///
    /// Distinct from [`Self::resident_at`], and both are needed. The home
    /// is where the object comes from and does not change when it is
    /// evicted; the residency is where it happens to be now. Collapsing
    /// them would mean an object could only be fetched once.
    pub handle: u64,
    /// Where it is in memory right now, or zero if it is not.
    pub resident_at: u64,
    /// When it is next wanted.
    pub next_use: NextUse,
    /// How often it has been wanted, for frequency-aware caching.
    pub reuse_count: u16,
    /// What fetching it again would cost.
    pub reload_cost: CostNs,
    /// What recomputing it would cost, or [`CostNs::IMPOSSIBLE`].
    pub recompute_cost: CostNs,
    /// How many live leases refer to it.
    ///
    /// The number parameter-major scheduling is built on: four sessions
    /// waiting on one layer should cause one read, not four.
    pub share_count: u16,
    /// Whether it can change.
    pub mutability: Mutability,
    /// Whose it is.
    pub owner: SessionId,
}
