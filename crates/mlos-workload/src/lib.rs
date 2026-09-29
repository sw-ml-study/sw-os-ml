//! A decode loop, as an access trace: sessions of different lengths,
//! round-robin, each sweeping the weights and re-reading its own KV.
//!
//! Invariant: the access order follows from the arithmetic, never from
//! convenience. Design and history: docs/notes/mlos-workload.md.

#![forbid(unsafe_code)]

mod model;
mod real;
mod shape;

use mlos_objtab::SessionId;
use mlos_synth::{LAYERS, TILES, model as weights};
use mlos_trace::{Access, Header};

pub use mlos_synth::kv::{BYTES as KV_BYTES, block, meta as kv_meta};
pub use real::{Context, MINICPM, MODEL_ID, Real};
pub use shape::{Shape, Stream};

/// What to call this workload in a trace header.
pub const MODEL: &str = "synth-8x16+kv";

/// Several sessions decoding at once, for different numbers of tokens.
#[derive(Clone, Copy)]
pub struct Decode {
    /// How many sessions share the model, decoding round-robin.
    pub sessions: u16,
    /// How many rounds the longest-running session lasts.
    pub rounds: u16,
}

impl Decode {
    /// `sessions` sessions over `rounds` rounds.
    #[must_use]
    pub const fn of(sessions: u16, rounds: u16) -> Self {
        Self { sessions, rounds }
    }

    /// How long session `which` keeps generating: spread evenly, so the
    /// shortest stops after a fraction of the run and the longest lasts
    /// all of it.
    #[must_use]
    pub const fn length(&self, which: u16) -> u16 {
        match self.sessions {
            0 => 0,
            count => self.rounds * (which + 1) / count,
        }
    }

    /// The trace header naming what this is of.
    #[must_use]
    pub const fn header(&self) -> Header<'static> {
        Header {
            model: MODEL,
            source: "mlos-workload::Decode",
        }
    }

    /// The whole access sequence: round-robin over sessions still
    /// generating, and within a turn every weight tile, layer by layer,
    /// then that session's KV prefix for the layer.
    #[must_use]
    pub fn trace(&self) -> Vec<Access> {
        let mut out = Vec::new();
        for round in 0..self.rounds {
            for session in 0..self.sessions {
                if self.length(session) <= round {
                    continue; // this one has stopped generating
                }
                let by = SessionId(session + 1);
                for layer in 0..LAYERS {
                    let tiles = (0..TILES).map(|tile| weights::tile(layer, tile));
                    let cache = (0..=round).map(|at| block(session + 1, layer, at));
                    out.extend(tiles.chain(cache).map(|object| Access {
                        session: by,
                        object,
                    }));
                }
            }
        }
        out
    }
}
