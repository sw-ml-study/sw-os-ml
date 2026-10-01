//! Replay an access trace against a policy, and count what it cost.
//!
//! Invariants: every policy in a comparison gets the same budget, and
//! the same trace and policy give the same counts every time. Design and
//! history: docs/notes/mlos-sim.md.

#![forbid(unsafe_code)]

mod foresight;
mod resident;
mod run;
mod view;

use mlos_abi::ObjectId;
use mlos_arena::Arena;
use mlos_objtab::ObjectMeta;
use mlos_policy::Policy;
use mlos_session::{Contract, MAX_SESSIONS, Sessions};
use mlos_trace::Trace;

use run::Run;

pub use foresight::{Foresight, wanted_at};
pub use resident::Resident;

/// Where an object's size and costs come from: the model the trace
/// header names.
pub trait Model {
    /// What the table would know about `id`, or `None` if it is not in
    /// this model -- which means the trace and the model do not match.
    fn meta(&self, id: ObjectId) -> Option<ObjectMeta>;
}

/// Replays `trace` against one policy under `budget` bytes. The budget
/// is a real `mlos-arena` over a buffer this long, so fragmentation
/// counts.
pub fn replay(trace: &Trace<'_>, model: &dyn Model, budget: u64, policy: &dyn Policy) -> Outcome {
    let seen = Foresight::read(trace.accesses);
    let run = Run {
        model,
        policy,
        seen: &seen,
    };
    let mut bytes = vec![0u8; usize::try_from(budget).expect("a budget that fits in memory")];
    let mut resident = Resident::new(Arena::new(&mut bytes));
    let mut outcome = Outcome::default();
    let mut sessions = Sessions::<MAX_SESSIONS>::EMPTY;
    let named = trace.accesses.iter().map(|access| access.session);
    outcome.sessions = sessions.adopt_each(named, Contract::NONE);
    for (at, access) in trace.accesses.iter().enumerate() {
        run.step(&mut resident, &mut outcome, access.object, at as u32 + 1);
    }
    outcome
}

/// Replays `trace` against every policy under the same budget; a
/// comparison with different budgets cannot be expressed here.
pub fn compare<'a>(
    trace: &Trace<'_>,
    model: &dyn Model,
    budget: u64,
    policies: &[&'a dyn Policy],
) -> Vec<(&'a str, Outcome)> {
    policies
        .iter()
        .map(|policy| (policy.name(), replay(trace, model, budget, *policy)))
        .collect()
}

/// What one policy did with one trace.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Outcome {
    /// Acquires that found the object already resident. The good case.
    pub hits: u64,
    /// Acquires that had to go to a provider. The number to minimise.
    pub reads: u64,
    /// Bytes those reads moved.
    pub bytes: u64,
    /// Objects thrown away to make room.
    pub evicted: u64,
    /// Acquires that could not be served at all. Part of the cost, not a
    /// failure.
    pub refused: u64,
    /// What the reads would have cost, in nanoseconds, as providers charge.
    pub cost: u64,
    /// Accesses naming objects this model does not have. Zero for a
    /// matched trace and model; non-zero means the two files disagree.
    pub mismatched: u64,
    /// Distinct sessions the trace named, each created for the replay.
    pub sessions: u64,
}

impl Outcome {
    /// Acquires served without going to a provider, per thousand of
    /// those asked. Comparable across traces of different lengths.
    #[must_use]
    pub const fn hit_per_mille(&self) -> u64 {
        match self.hits + self.reads + self.refused {
            0 => 0,
            asked => self.hits * 1000 / asked,
        }
    }
}
