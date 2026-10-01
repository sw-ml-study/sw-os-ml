//! The same decode loop as `Decode`, over a real model's shape.
//!
//! Invariant: the sidecar's declaration of what rotates and what is read
//! once is followed, never second-guessed. Design and history:
//! docs/notes/mlos-workload.md.

use mlos_abi::{Fields, ObjectClass, ObjectId};
use mlos_objtab::SessionId;
use mlos_sched::ProcessMajor;
use mlos_trace::{Access, Header};

use crate::{Decode, Shape, Stream};

/// The sidecar for MiniCPM5-1B, a 24-layer Llama at F16.
pub const MINICPM: &str = include_str!("../shapes/minicpm5-1b.names.tsv");

/// The model id a real shape's objects carry. The synthetic model is 1.
pub const MODEL_ID: u16 = 2;

/// How much cache a session brings, and how it is cut into objects.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Context {
    /// Tokens already cached when a session starts decoding: its prompt.
    pub prefix: u16,
    /// Tokens per KV object. One is a block per position, as
    /// `mlos-synth` defines it.
    pub tokens_per_block: u16,
}

impl Context {
    /// No prompt, one token per block: the bare decode loop.
    pub const DECODE_ONLY: Self = Self {
        prefix: 0,
        tokens_per_block: 1,
    };
}

/// Several sessions decoding at once over a real shape.
#[derive(Clone, Copy)]
pub struct Real<'a> {
    /// What the model is made of.
    pub shape: &'a Shape<'a>,
    /// How the sessions are spread; the same arithmetic as `Decode`.
    pub loop_: Decode,
    /// What each session brings with it.
    pub context: Context,
}

impl<'a> Real<'a> {
    /// `sessions` sessions over `rounds` rounds of `shape`, each
    /// arriving with `context`.
    #[must_use]
    pub const fn of(shape: &'a Shape<'a>, sessions: u16, rounds: u16, context: Context) -> Self {
        Self {
            shape,
            loop_: Decode::of(sessions, rounds),
            context,
        }
    }

    /// The trace header naming what this is of.
    #[must_use]
    pub fn header(&self) -> Header<'a> {
        Header {
            model: self.shape.name,
            source: "mlos-workload::Real",
        }
    }

    /// The object a stream is, in the table.
    #[must_use]
    pub fn object(stream: &Stream<'_>) -> ObjectId {
        ObjectId::new(
            ObjectClass::WeightTile,
            Fields {
                model: MODEL_ID,
                layer: stream.layer,
                tensor: stream.tensor,
                tile: 0,
            },
        )
    }

    /// The whole access sequence.
    ///
    /// The resident streams once, by the first session; then per round,
    /// per session still generating, the rotating sweep with the layer's
    /// cache read between its value projection and its output projection.
    /// The cache spans `ceil(tokens / tokens_per_block)` blocks once this
    /// token is in it.
    #[must_use]
    pub fn trace(&self) -> Vec<Access> {
        let streams: Vec<_> = (0..self.loop_.sessions)
            .map(|s| (SessionId(s + 1), self.tokens(s)))
            .collect();
        mlos_sched::merge(&mut ProcessMajor::default(), &streams)
    }
}
