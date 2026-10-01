//! Each session's own stream, token by token, for a scheduler to merge.
//!
//! Invariant: within a token the order is the arithmetic's -- the
//! layer's weights, its cache read between the value and output
//! projections, then the next layer -- and a merged process-major order
//! is byte for byte what the generators produced by hand before.
//! Design: docs/notes/mlos-workload.md.

use mlos_abi::ObjectId;
use mlos_synth::kv::block;
use mlos_synth::{LAYERS, TILES, model as weights};

use crate::{Decode, Real};

impl Decode {
    /// Session `session`'s tokens over the synthetic model: every tile of
    /// every layer, then that layer's cache so far, for as many rounds as
    /// the session lasts.
    #[must_use]
    pub fn tokens(&self, session: u16) -> Vec<Vec<ObjectId>> {
        (0..self.length(session))
            .map(|round| {
                let mut token = Vec::new();
                for layer in 0..LAYERS {
                    token.extend((0..TILES).map(|tile| weights::tile(layer, tile)));
                    token.extend((0..=round).map(|at| block(session + 1, layer, at)));
                }
                token
            })
            .collect()
    }
}

impl Real<'_> {
    /// Session `session`'s tokens over the real shape: the rotating
    /// streams in declared order with the layer's cache after its value
    /// projection. Session zero's first token begins with the resident
    /// streams, read once for everyone.
    #[must_use]
    pub fn tokens(&self, session: u16) -> Vec<Vec<ObjectId>> {
        (0..self.loop_.length(session))
            .map(|round| self.token(session, round))
            .collect()
    }

    /// One token of `session` at `round`.
    fn token(&self, session: u16, round: u16) -> Vec<ObjectId> {
        let (shape, context) = (self.shape, self.context);
        let per = u32::from(context.tokens_per_block.max(1));
        let cached = (u32::from(context.prefix) + u32::from(round) + 1).div_ceil(per);
        let mut token = Vec::new();
        if session == 0 && round == 0 {
            token.extend(
                shape
                    .streams
                    .iter()
                    .filter(|s| !s.rotating)
                    .map(Self::object),
            );
        }
        for stream in shape.streams.iter().filter(|s| s.rotating) {
            token.push(Self::object(stream));
            if stream.layer < shape.layers && stream.name.contains("v_proj") {
                token.extend((0..cached).map(|b| block(session + 1, stream.layer, b as u16)));
            }
        }
        token
    }
}
