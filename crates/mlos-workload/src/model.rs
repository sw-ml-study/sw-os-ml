//! What the object table would know about this workload's objects.
//!
//! The half a trace deliberately does not carry. A trace names objects
//! and says nothing about their size or cost, because those are
//! properties of the model rather than of the workload -- so the
//! simulator needs both, and the trace header names which model it must
//! be paired with.

use mlos_abi::{ObjectClass, ObjectId};
use mlos_objtab::ObjectMeta;
use mlos_sim::Model;
use mlos_synth::model as weights;

use crate::{Decode, Real};
use mlos_synth::kv;

impl Model for Decode {
    /// Weight tiles come from the synthetic model; KV blocks from this
    /// workload, which is what produced them.
    ///
    /// Anything else is `None` rather than a guess. A trace naming an
    /// object this workload never generates means the two are not a
    /// matched pair, and the simulator counts that separately from a
    /// residency decision for exactly that reason.
    fn meta(&self, id: ObjectId) -> Option<ObjectMeta> {
        match id.class()? {
            ObjectClass::WeightTile => Some(weights::weights()),
            ObjectClass::Activation => Some(weights::activations()),
            ObjectClass::KvBlock => Some(kv::meta(id.fields().model)),
            _ => None,
        }
    }
}

impl Decode {
    /// The whole workload as trace text, ready for a disk or a file.
    ///
    /// Rendered once by the host and written where the guest will read
    /// it, so the kernel replays the SAME accesses the simulator measured
    /// rather than its own idea of them. Two generators agreeing is a
    /// thing to be checked; one generator and a file is a thing that
    /// cannot disagree. Beside the `Model` impl because both are this
    /// workload as another crate consumes it.
    #[must_use]
    pub fn text(&self) -> String {
        let held = self.trace();
        let mut text = String::new();
        let header = mlos_trace::Header {
            model: crate::MODEL,
            source: "mlos-workload::Decode",
        };
        let _ = mlos_trace::render(&mut text, header, &held);
        text
    }
}

impl Model for Real<'_> {
    /// Weights are the shape's streams, sized by the shape; KV blocks
    /// are sized by what one token adds to one layer of THIS model's
    /// cache. Both priced by the byte, because a precomputed scalar
    /// sized for a kilobyte tile would let a 400 MB head arrive for the
    /// price of one -- the unit error `tests/verdict.rs` had to correct.
    fn meta(&self, id: ObjectId) -> Option<ObjectMeta> {
        let fields = id.fields();
        match id.class()? {
            ObjectClass::WeightTile if fields.model == crate::MODEL_ID => {
                let stream = self
                    .shape
                    .streams
                    .iter()
                    .find(|s| s.layer == fields.layer && s.tensor == fields.tensor)?;
                let mut meta = weights::weights();
                meta.size = stream.bytes;
                meta.precision = mlos_objtab::Precision::Fp16;
                meta.reload_cost = fetching(meta.size);
                Some(meta)
            }
            ObjectClass::KvBlock => {
                let mut meta = kv::meta(fields.model);
                meta.size = self.shape.kv_block_bytes() * u32::from(self.context.tokens_per_block);
                Some(meta)
            }
            _ => None,
        }
    }
}

/// What fetching `size` bytes costs from the backing tier: three
/// milliseconds to the first byte, then a gigabyte a second. The same
/// figure `mlos-synth`'s disk tier charges, applied per object.
fn fetching(size: u32) -> mlos_objtab::CostNs {
    mlos_objtab::CostNs(3_000_000u32.saturating_add(size))
}
