//! Residency transitions, recorded as they happen.
//!
//! A snapshot says what is resident now. This says what happened to get
//! there -- what came in, what it cost, and what had to be refused -- and
//! it is the part of the shared visualization demo that is MLOS's alone:
//! SWTOS's flash is static once built, whereas the whole subject of an ML
//! object store is churn.
//!
//! **Recording does not format.** The fault path pushes a `Copy` struct
//! into a ring and returns; turning it into JSON happens later, in the
//! shell, off the path being measured. An emitter that writes to a
//! console from inside `service` would be timing its own console driver
//! and calling the result a fault cost.
//!
//! The ring is owned by the `Manager` rather than being a global, so a
//! test can have one of its own. That matters here more than it usually
//! would: the thing under test is a side effect, and a global would make
//! every case in a test binary share it.
//!
//! **There is no wall clock in an event, and that is deliberate.** The
//! only clock the shell has is the 2 Hz timer, which cannot resolve a
//! fault; and elapsed time under TCG is not the timing of any real
//! machine. So ordering comes from `seq`, which is exact, and duration
//! comes from `cost`, which is the modelled figure a provider charges --
//! the same axis M3's policy comparison is measured on.

#![no_std]
#![forbid(unsafe_code)]

mod event;
mod line;

pub use event::Event;
pub use line::{MARKER, verb, write};

/// How many events a ring holds.
///
/// A sweep of the synthetic model is 128 acquires and so at most 128
/// events, which leaves room to spare. Overflow drops the OLDEST and is
/// counted, never silent: a consumer replaying a stream with a hole in it
/// would rebuild the wrong picture and have no way to know.
pub const CAPACITY: usize = 256;

/// A fixed ring of the most recent events.
pub struct Ring {
    /// Whether to record at all.
    ///
    /// Public so `trace off` is a field write. Recording costs a handful
    /// of stores, so this exists to prove that rather than because the
    /// cost is known to matter.
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

    /// Records one transition, oldest-out when full.
    ///
    /// A handful of stores and a modulo. Nothing is formatted, nothing is
    /// allocated, and nothing is written to a device -- which is what
    /// makes it cheap enough to leave on while the thing being measured
    /// is the fault path itself.
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
    ///
    /// Not a failure. Until M3 there is no policy to choose a victim, so
    /// refusing is the honest outcome -- and it is the most interesting
    /// event in the stream, because it is the moment the system ran out of
    /// the resource it exists to manage.
    Refused,
    /// A resident object was thrown away. Reserved for M3.
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
