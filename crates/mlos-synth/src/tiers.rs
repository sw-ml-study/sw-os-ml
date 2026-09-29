//! The tiers that are not memory, described entirely by what they charge.
//!
//! Neither touches a device: reads are pattern-filled. Design and
//! history: docs/notes/mlos-synth.md.

use mlos_abi::Result;
use mlos_objtab::{CostNs, ProviderId};
use mlos_provider::{Cost, Located, Provider};

/// A place objects come from, described entirely by what it charges.
pub struct Tier {
    id: ProviderId,
    cost: Cost,
}

/// Cold storage, NVMe-shaped: slow to start, then fast.
pub static BACKING: Tier = Tier {
    id: ProviderId(2),
    cost: Cost {
        latency: CostNs(3_000_000),
        bytes_per_ms: 1_000_000,
    },
};

/// Recomputation: no bytes anywhere, no seek, but real work per byte.
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
    /// whose bytes it got.
    fn read(&self, object: Located, _offset: u32, into: &mut [u8]) -> Result<u32> {
        let fields = object.id.fields();
        into.fill((fields.layer as u8) ^ (fields.tensor as u8));
        Ok(into.len() as u32)
    }

    fn cost(&self, _object: Located) -> Cost {
        self.cost
    }
}
