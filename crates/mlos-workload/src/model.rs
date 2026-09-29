//! What the object table would know about this workload's objects: the
//! half a trace does not carry.
//!
//! Invariant: an object this workload never generates is `None`, not a
//! guess. Design: docs/notes/mlos-workload.md.

use mlos_abi::{ObjectClass, ObjectId};
use mlos_objtab::ObjectMeta;
use mlos_sim::Model;
use mlos_synth::model as weights;

use crate::{Decode, Real};
use mlos_synth::kv;

impl Model for Decode {
    /// Weight tiles come from the synthetic model; KV blocks from this
    /// workload. Anything else is `None`: the trace and the workload are
    /// not a matched pair.
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
    /// The whole workload as trace text, ready for a disk or a file. The
    /// one rendering both the simulator and the kernel replay.
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
    /// Weights are the shape's streams, sized by the shape; KV blocks are
    /// sized by what one token adds to one layer of this model's cache.
    /// Both are priced by the byte.
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
/// milliseconds to the first byte, then a gigabyte a second, as
/// `mlos-synth`'s disk tier charges.
fn fetching(size: u32) -> mlos_objtab::CostNs {
    mlos_objtab::CostNs(3_000_000u32.saturating_add(size))
}
