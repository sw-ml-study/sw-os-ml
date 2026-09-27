//! A decode loop over a real model's shape.
//!
//! [`Decode`](crate::Decode) is the same loop over the 8x16 synthetic
//! model, and everything it argues about sessions and reuse holds here.
//! What changes is where the objects come from: not `mlos-synth`'s
//! uniform kilobyte tiles but a [`Shape`] read from emufpga's sidecar --
//! real tensors, real sizes, and the model's own declaration of which
//! are swept per token and which are read once. That is the difference
//! between a trace shaped like inference and a trace shaped like a
//! guess, which `docs/architecture.md` s.12 names as the risk.
//!
//! One token, one session, in this workload:
//!
//! 1. Per layer, the rotating streams in declared order -- q, k, v, then
//!    that session's KV blocks for the layer so far, then o, gate, up,
//!    down. Attention reads the cache between the projections that feed
//!    it and the one that consumes it, so that is where the blocks go.
//! 2. Then the rotating streams outside any layer: the output head.
//!
//! ## Context, and why it is a knob
//!
//! Measured without one, this shape is a weight sweep with a rounding
//! error of KV on it: MiniCPM5-1B's two KV heads make one token's cache
//! 24 KiB across all layers, against 1.68 GB of weights swept to produce
//! it. Forty tokens of four sessions is 3.8 MB of cache. On that trace
//! FIFO and LRU tie exactly -- a pure cycle cannot distinguish them --
//! and next-use wins by construction, which is the degenerate case the
//! M3 plan said to avoid, arriving from a real model rather than a
//! synthetic one.
//!
//! What makes KV matter is CONTEXT: sessions that arrive with a long
//! prompt already cached, and cache blocks coarse enough that a real
//! context is thousands of objects rather than millions. [`Context`]
//! is both numbers. `Context::DECODE_ONLY` is the bare loop, for
//! comparing with the synthetic model; a prefix in the thousands with
//! sixteen or thirty-two tokens per block is what a serving engine
//! actually holds, and is where recency has something to be right about.
//!
//! The resident streams -- embeddings, norms -- are read ONCE, at the
//! start, by the first session, which is exactly what the sidecar
//! declares them to be: "read once into RAM". A norm is touched every
//! layer in the arithmetic, but the sidecar's author put it in RAM for
//! good and this workload does not second-guess a declaration it was
//! given. Whether those resident bytes then survive is the policy's
//! problem, as it would be in a real system.

use mlos_abi::{Fields, ObjectClass, ObjectId};
use mlos_objtab::SessionId;
use mlos_synth::kv::block;
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
    /// `mlos-synth` defines it; a serving engine pages the cache in
    /// sixteen or thirty-two, and so does a table that has to hold it.
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
        let (shape, context) = (self.shape, self.context);
        let at = |session: u16, object| Access {
            session: SessionId(session + 1),
            object,
        };
        let resident = shape.streams.iter().filter(|s| !s.rotating);
        let mut out: Vec<Access> = resident.map(|s| at(0, Self::object(s))).collect();
        for round in 0..self.loop_.rounds {
            for session in (0..self.loop_.sessions).filter(|&s| self.loop_.length(s) > round) {
                let tokens = u32::from(context.prefix) + u32::from(round) + 1;
                let cached = tokens.div_ceil(u32::from(context.tokens_per_block.max(1)));
                for stream in shape.streams.iter().filter(|s| s.rotating) {
                    out.push(at(session, Self::object(stream)));
                    if stream.layer < shape.layers && stream.name.contains("v_proj") {
                        let blocks =
                            (0..cached).map(|b| block(session + 1, stream.layer, b as u16));
                        out.extend(blocks.map(|object| at(session, object)));
                    }
                }
            }
        }
        out
    }
}
