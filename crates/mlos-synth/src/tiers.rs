//! The tiers that are not memory.
//!
//! One type, not two. An earlier version had a `Backing` provider and a
//! `Recompute` provider with identical bodies and different constants,
//! which is duplication wearing a costume: what distinguishes a block
//! store from a recomputation, as far as the object manager is concerned,
//! is entirely the cost.
//!
//! Neither touches a real device yet -- a block store arrives in step
//! 006. What they are for is the *shape*: tiers with genuinely different
//! costs, so a fault has somewhere to come from and the numbers a policy
//! will later compare are real rather than placeholders.

use mlos_abi::Result;
use mlos_objtab::{CostNs, ProviderId};
use mlos_provider::{Cost, Located, Provider};

/// A place objects come from, described entirely by what it charges.
pub struct Tier {
    id: ProviderId,
    cost: Cost,
}

/// Cold storage: slow to start, then fast.
///
/// NVMe-shaped -- three milliseconds to the first byte, then about a
/// gigabyte a second. The accuracy matters less than the shape: latency
/// dominates for a small object, transfer for a large one, and that
/// crossover is what an eviction policy has to find.
pub static BACKING: Tier = Tier {
    id: ProviderId(2),
    cost: Cost {
        latency: CostNs(3_000_000),
        bytes_per_ms: 1_000_000,
    },
};

/// Recomputation: no bytes anywhere.
///
/// The tier an ordinary operating system does not have. An activation
/// reaches it by being thrown away and comes back by being calculated
/// again, which is why `recompute_cost` sits beside `reload_cost` in the
/// object table. No seek, but real work: cheaper than storage for a small
/// object and worse for a large one.
pub static RECOMPUTE: Tier = Tier {
    id: ProviderId(3),
    cost: Cost {
        latency: CostNs(20_000),
        bytes_per_ms: 40_000,
    },
};

impl Provider for Tier {
    fn id(&self) -> ProviderId {
        self.id
    }

    /// Fills with a pattern derived from the object, so a reader can tell
    /// whose bytes it got -- and notice if it got nobody's.
    fn read(&self, object: Located, _offset: u32, into: &mut [u8]) -> Result<u32> {
        let fields = object.id.fields();
        into.fill((fields.layer as u8) ^ (fields.tensor as u8));
        Ok(into.len() as u32)
    }

    fn cost(&self, _object: Located) -> Cost {
        self.cost
    }
}
