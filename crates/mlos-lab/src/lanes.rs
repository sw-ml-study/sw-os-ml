//! The disk trace as per-session lanes, scheduled in the kernel.
//!
//! Invariant: a lane is a session's accesses in trace order, cut into
//! tokens where it touches the model's first tile; the scheduler sees
//! positions, waits and next objects, nothing else, and the merged order
//! names every access exactly once. Design: docs/notes/mlos-lab.md.

use mlos_abi::{Error, ObjectId, Result};
use mlos_objtab::SessionId;
use mlos_sched::{At, Schedule, Waiting};
use mlos_session::MAX_SESSIONS;
use mlos_trace::Access;

/// One lane: the session, where its run starts in the grouped objects,
/// how long it is, and its latency ceiling.
pub type Lane = (SessionId, u32, u32, u32);

/// Lanes over the room's static buffers, with a cursor each.
pub struct Lanes<'a> {
    /// Every access's object, grouped by session, each session contiguous.
    pub objects: &'a [ObjectId],
    /// For each grouped access, its token and index within the token.
    pub shape: &'a [(u32, u32)],
    /// The sessions, in order of first appearance.
    pub lanes: &'a [Lane],
    cursors: [u32; MAX_SESSIONS],
    due: [u32; MAX_SESSIONS],
    clock: u32,
}

impl Waiting for Lanes<'_> {
    fn sessions(&self) -> usize {
        self.lanes.len()
    }

    fn at(&self, session: usize) -> Option<At> {
        let (_, start, len, ceiling) = *self.lanes.get(session)?;
        let cursor = self.cursors[session];
        let (token, index) = *self.shape.get((start + cursor) as usize)?;
        (cursor < len).then_some(At {
            token,
            index,
            waited: self.clock - self.due[session] - index,
            ceiling,
        })
    }

    fn next_of(&self, session: usize) -> Option<ObjectId> {
        self.at(session)?;
        let (_, start, _, _) = self.lanes[session];
        self.objects
            .get((start + self.cursors[session]) as usize)
            .copied()
    }
}

impl<'a> Lanes<'a> {
    /// Groups `trace` into the `count` lanes already named in `room.2`
    /// (session, 0, 0, ceiling), in that order, cutting a new token
    /// wherever a session touches `first`. `NoBudget` if the buffers are
    /// too small.
    pub fn build(
        trace: &[Access],
        first: ObjectId,
        room: (&'a mut [ObjectId], &'a mut [(u32, u32)], &'a mut [Lane]),
        count: usize,
    ) -> Result<Self> {
        let (objects, shape, lanes) = room;
        let mut filled = 0usize;
        for lane in lanes[..count].iter_mut() {
            filled = fill(trace, lane, first, (objects, shape), filled)?;
        }
        Ok(Self {
            objects: &objects[..filled],
            shape: &shape[..filled],
            lanes: &lanes[..count],
            cursors: [0; MAX_SESSIONS],
            due: [0; MAX_SESSIONS],
            clock: 0,
        })
    }

    /// Runs `schedule` over the lanes, writing the merged order into
    /// `objects` and `sessions`. Returns how many accesses were ordered,
    /// which is every one.
    pub fn order(
        &mut self,
        schedule: &mut dyn Schedule,
        into: (&mut [ObjectId], &mut [SessionId]),
    ) -> usize {
        let (objects, sessions) = into;
        let mut n = 0usize;
        while let Some(s) = schedule.pick(self)
            && let Some(object) = self.next_of(s)
            && n < objects.len()
        {
            objects[n] = object;
            sessions[n] = self.lanes[s].0;
            n += 1;
            self.clock += 1;
            let (_, start, len, _) = self.lanes[s];
            let cursor = self.cursors[s] + 1;
            self.cursors[s] = cursor;
            let done = cursor >= len || self.shape[(start + cursor) as usize].1 == 0;
            self.due[s] = if done { self.clock } else { self.due[s] };
        }
        n
    }
}

/// Copies one session's accesses into the grouped buffers starting at
/// `filled`, numbering tokens from where the session touches `first`,
/// and records the lane's start and length. Returns the new fill point.
fn fill(
    trace: &[Access],
    lane: &mut Lane,
    first: ObjectId,
    into: (&mut [ObjectId], &mut [(u32, u32)]),
    mut filled: usize,
) -> Result<usize> {
    let (objects, shape) = into;
    lane.1 = filled as u32;
    let (mut token, mut index) = (0u32, 0u32);
    for access in trace.iter().filter(|a| a.session == lane.0) {
        if access.object == first && lane.2 != 0 {
            token += 1;
            index = 0;
        }
        *objects.get_mut(filled).ok_or(Error::NoBudget)? = access.object;
        shape[filled] = (token, index);
        filled += 1;
        index += 1;
        lane.2 += 1;
    }
    Ok(filled)
}
