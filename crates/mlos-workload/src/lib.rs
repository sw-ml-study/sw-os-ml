//! A decode loop, as an access trace.
//!
//! The workload the M3 comparison is actually decided on, and the reason
//! it exists is that the recorded trace cannot decide anything. A dense
//! sweep re-reads nothing within a pass, so LRU evicts precisely what it
//! is about to need and misses everything; known-next-use is optimal on
//! it by construction. Both facts follow from the shape of the workload
//! before any policy is written, and a result that could not have come
//! out otherwise is not a measurement.
//!
//! A real decode loop has two halves that pull in opposite directions:
//!
//! - **Weights are swept cyclically.** Every token reads every tile, in
//!   the same order. This is LRU's WORST case, and worth understanding
//!   rather than just asserting: in a cycle longer than the budget, the
//!   tile LRU just touched is the one it will want LAST, so it evicts
//!   exactly what it is about to need.
//! - **KV accumulates and is re-read.** Each token appends a block per
//!   layer and reads every earlier block for that layer.
//!
//! That was the first design, and it did not work. Re-reading the whole
//! prefix every token is ITSELF a cyclic sweep: every block is read once
//! per round, in order, so after one pass LRU's recency order is exactly
//! insertion order and it evicts what FIFO evicts. Measured, they tied at
//! 192 reads each on the KV half alone.
//!
//! **A purely cyclic workload can never distinguish FIFO from LRU**, and
//! that is worth stating as a property rather than as an accident. The
//! two differ only when something placed EARLY is used RECENTLY, which a
//! uniform scan never produces.
//!
//! What produces it in real serving is concurrency. Several sessions
//! decode at once, they sit at different positions, and they do not all
//! finish together. A session that has stopped generating still holds KV
//! that will never be read again; an active session holds early blocks it
//! reads every round. LRU distinguishes those and FIFO cannot -- it
//! evicts the active session's early blocks while a finished session's
//! later blocks survive purely for being younger.
//!
//! So this workload is round-robin over sessions of different lengths.
//! That is also nearer the system MLOS is for: the premise in
//! `docs/PRD.md` is many sessions wanting the same weights, and a
//! single-session trace could not express it.
//!
//! **This is a model of a decode loop, not a recording of one.** Weaker
//! provenance than the step-001 trace, and it has to be said: what keeps
//! it honest is that the access ORDER follows from the structure rather
//! than from anyone's preference, and that M3 step 010 replaces the
//! sizes with a real model's.

#![forbid(unsafe_code)]

mod kv;
mod model;

use mlos_objtab::SessionId;
use mlos_synth::{LAYERS, TILES, model as weights};
use mlos_trace::{Access, Header};

pub use kv::{BYTES as KV_BYTES, block, meta as kv_meta};

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

    /// How long session `which` keeps generating.
    ///
    /// Spread evenly, so the shortest stops after a fraction of the run
    /// and the longest lasts all of it. Sessions that finish are what
    /// make recency mean something: their KV is never read again, and a
    /// policy that notices keeps the active sessions' blocks instead.
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

    /// The whole access sequence.
    ///
    /// Round-robin over sessions still generating. Within a session's
    /// turn: every weight tile, layer by layer, then that session's KV
    /// prefix for the layer -- the order the arithmetic needs them, since
    /// the projections come from the weights and attention reads the
    /// cache. An order chosen for convenience would be a different
    /// workload wearing this one's name.
    ///
    /// Returned rather than written into a caller's slice, unlike
    /// `mlos-trace`: nothing replays a GENERATED trace in the kernel, and
    /// a length the caller had to predict is a length that can disagree
    /// with the loop that fills it.
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
