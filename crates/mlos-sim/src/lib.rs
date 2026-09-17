//! Replay an access trace against a policy, and count what it cost.
//!
//! The harness every number in M3 comes out of, which makes its own
//! correctness the first thing to worry about. Two properties carry that
//! weight:
//!
//! **The budget is a property of the RUN, not of the policy.** A
//! comparison where one policy got more memory is not a comparison, and
//! it is the easiest mistake to make and the hardest to see afterwards.
//! [`compare`] takes one budget and applies it to every policy, so the
//! mistake is not expressible rather than merely discouraged.
//!
//! **The same trace and policy give the same counts every time.** Nothing
//! here consults a clock, a hash seed or an iteration order that could
//! vary. A test says so, because a harness that were nondeterministic
//! would make every later number arguable and there would be no way to
//! tell from the numbers themselves.
//!
//! `docs/design.md` s.10 calls this the host-side level of the test
//! pyramid. It is std, and the policies it runs are not: they are the
//! `no_std` crates the kernel links, which is what lets step 009 replay
//! the same trace in the kernel and require the counts to match.

#![forbid(unsafe_code)]

mod resident;
mod run;
mod view;

use mlos_abi::ObjectId;
use mlos_objtab::ObjectMeta;
use mlos_policy::Policy;
use mlos_trace::Trace;

use run::Run;

pub use resident::Resident;

/// Where an object's size and costs come from.
///
/// A trace names objects and says nothing about how big they are, on
/// purpose -- size is a property of the model, not of the workload. This
/// is the model, and the trace header names which one it must be.
pub trait Model {
    /// What the table would know about `id`, or `None` if it is not in
    /// this model -- which means the trace and the model do not match.
    fn meta(&self, id: ObjectId) -> Option<ObjectMeta>;
}

/// Replays `trace` against one policy under `budget` bytes.
pub fn replay(trace: &Trace<'_>, model: &dyn Model, budget: u64, policy: &dyn Policy) -> Outcome {
    let run = Run {
        model,
        budget,
        policy,
    };
    let mut resident = Resident::default();
    let mut outcome = Outcome::default();
    for (at, access) in trace.accesses.iter().enumerate() {
        run.step(&mut resident, &mut outcome, access.object, at as u32 + 1);
    }
    outcome
}

/// Replays `trace` against every policy under the SAME budget.
///
/// One budget argument for all of them, which is the enforcement: a
/// comparison where the policies had different budgets cannot be
/// expressed through this function at all.
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
    /// Acquires that could not be served at all.
    ///
    /// Not a failure: demand paging refuses by design and the cost of
    /// refusing is part of what the comparison measures. A policy with
    /// many refusals and few reads has not won.
    pub refused: u64,
    /// What the reads would have cost, in nanoseconds, as providers charge.
    pub cost: u64,
    /// Accesses naming objects this model does not have.
    ///
    /// Always zero for a matched trace and model, and its own counter
    /// rather than a refusal because it means something completely
    /// different: a refusal is a residency decision, and this is the two
    /// files not being about the same thing.
    pub mismatched: u64,
}

impl Outcome {
    /// Acquires served without going to a provider, per thousand.
    ///
    /// The ratio to compare policies on. A raw read count is only
    /// comparable between runs of the same length, and two traces of
    /// different lengths are exactly what step 010 will bring.
    #[must_use]
    pub const fn hit_per_mille(&self) -> u64 {
        match self.hits + self.reads + self.refused {
            0 => 0,
            asked => self.hits * 1000 / asked,
        }
    }
}
