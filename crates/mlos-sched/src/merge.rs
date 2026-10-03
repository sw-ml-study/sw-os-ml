//! Driving a schedule over per-session streams into one trace.
//!
//! Invariant: every access of every session appears exactly once, in
//! the order the schedule chose; the scheduler never sees more than
//! positions, waits and next objects. Design: docs/notes/mlos-sched.md.

extern crate alloc;

use alloc::vec::Vec;

use mlos_abi::ObjectId;
use mlos_objtab::SessionId;
use mlos_trace::Access;

use crate::lanes::Lanes;
use crate::{Schedule, Waiting};

/// One session's lane: who it is, how long it will wait, and its tokens,
/// each the objects one step asks for in order.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Lane {
    /// The session.
    pub session: SessionId,
    /// `Contract::latency_ceiling`, in acquires; zero for none.
    pub ceiling: u32,
    /// Its stream, token by token.
    pub tokens: Vec<Vec<ObjectId>>,
}

/// The trace `schedule` produces from `lanes`.
pub fn merge(schedule: &mut dyn Schedule, lanes: &[Lane]) -> Vec<Access> {
    merge_timed(schedule, lanes).0
}

/// The trace, and each lane's worst token period: acquires from the end
/// of the previous token (or the start) to the end of this one. The same
/// figure the kernel's driver records as `Delivered::worst_period`.
pub fn merge_timed(schedule: &mut dyn Schedule, lanes: &[Lane]) -> (Vec<Access>, Vec<u32>) {
    let mut held = Lanes::over(lanes);
    let mut out = Vec::new();
    let mut worst = alloc::vec![0u32; lanes.len()];
    while let Some(s) = schedule.pick(&held)
        && let Some(object) = held.next_of(s)
    {
        let session = lanes[s].session;
        out.push(Access { session, object });
        if let Some(period) = held.advance(s) {
            worst[s] = worst[s].max(period);
        }
    }
    (out, worst)
}
