//! What the kernel knows about one piece of model state.
//!
//! Invariant: a policy holds no state this record does not own, so the
//! same policy code runs in the kernel and the simulator. Design:
//! docs/notes/mlos-objtab.md.

/// How an object's numbers are stored. A choice the kernel can make, not
/// a fixed property.
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
    Stream = 4,
    /// No bytes stored at all -- recomputed, regenerated or dropped.
    Archive = 5,
}

/// When an object will next be wanted. Three-way, not a number: a policy
/// may act on `At` with certainty and on `Probability` only as a hint.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum NextUse {
    /// Not known to be wanted again.
    #[default]
    Never,
    /// Wanted again at exactly this position in a declared stream. A
    /// position from the stream's origin, not a distance from the cursor,
    /// so advancing the stream does not invalidate it.
    At(u32),
    /// Wanted with this likelihood, as a fraction of `u16::MAX`.
    Probability(u16),
}

/// A cost in nanoseconds.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub struct CostNs(pub u32);

impl CostNs {
    /// Cannot be done at any price. Eviction may choose "expensive" and
    /// never this.
    pub const IMPOSSIBLE: Self = Self(u32::MAX);
}

/// Whether an object can change, and how.
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mutability {
    /// Weights. Identical for every session, so one copy serves all.
    Immutable = 1,
    /// Shared until written, then private.
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
    /// Where it goes back to when it is evicted. Set once at registration
    /// and never changed; `tier` is what moves.
    pub home: Tier,
    /// Who can produce it.
    pub provider: ProviderId,
    /// Where its home is, as that provider understands "where". Opaque to
    /// the table; distinct from [`Self::resident_at`], and unchanged by
    /// eviction.
    pub handle: u64,
    /// Where it is in memory right now, or zero if it is not.
    pub resident_at: u64,
    /// When it is next wanted.
    pub next_use: NextUse,
    /// How often it has been wanted, for frequency-aware caching.
    pub reuse_count: u16,
    /// When it became resident, on a monotonic acquire counter.
    pub placed_tick: u32,
    /// When it was last wanted, on the same counter.
    pub used_tick: u32,
    /// What fetching it again would cost.
    pub reload_cost: CostNs,
    /// What recomputing it would cost, or [`CostNs::IMPOSSIBLE`].
    pub recompute_cost: CostNs,
    /// How many live leases refer to it.
    pub share_count: u16,
    /// Whether it can change.
    pub mutability: Mutability,
    /// Whose it is.
    pub owner: SessionId,
}
