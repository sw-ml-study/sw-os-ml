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

use crate::{Decode, kv};

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
