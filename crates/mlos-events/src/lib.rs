//! Residency transitions, recorded as they happen.
//!
//! Invariant: recording does not format, allocate, or write to a device;
//! an overwritten event is counted, never silently lost. Design and
//! history: docs/notes/mlos-events.md.

#![no_std]
#![forbid(unsafe_code)]

mod event;
mod line;

pub use event::Event;
pub use line::{MARKER, verb, write};

/// How many events a ring holds. Overflow drops the oldest and counts it.
pub const CAPACITY: usize = 256;

/// A fixed ring of the most recent events.
pub struct Ring {
    /// Whether to record at all. Public so `trace off` is a field write.
    pub enabled: bool,
    events: [Option<Event>; CAPACITY],
    next: usize,
    seq: u32,
    dropped: u32,
}

impl Ring {
    /// An empty ring, recording.
    pub const EMPTY: Self = Self {
        enabled: true,
        events: [None; CAPACITY],
        next: 0,
        seq: 0,
        dropped: 0,
    };

    /// Records one transition, oldest-out when full. Assigns `seq`.
    pub const fn record(&mut self, mut event: Event) {
        if !self.enabled {
            return;
        }
        if self.events[self.next].is_some() {
            self.dropped = self.dropped.saturating_add(1);
        }
        self.seq = self.seq.saturating_add(1);
        event.seq = self.seq;
        self.events[self.next] = Some(event);
        self.next = (self.next + 1) % CAPACITY;
    }

    /// Every event held, oldest first.
    pub fn events(&self) -> impl Iterator<Item = &Event> {
        let (before, after) = self.events.split_at(self.next);
        after.iter().chain(before).filter_map(Option::as_ref)
    }

    /// How many events were overwritten before anything read them.
    #[must_use]
    pub const fn dropped(&self) -> u32 {
        self.dropped
    }
}

/// What happened.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// Bytes arrived in the arena. Residency went up.
    Placed,
    /// A resident object was wanted again. Nothing moved.
    Hit,
    /// There was no room, and nothing was thrown away to make some.
    Refused,
    /// A resident object was thrown away.
    Evicted,
}

impl Kind {
    /// The word a consumer matches on.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Placed => "placed",
            Self::Hit => "hit",
            Self::Refused => "refused",
            Self::Evicted => "evicted",
        }
    }
}
