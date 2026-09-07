//! The table itself.

use mlos_abi::ObjectId;

use crate::{ObjectMeta, probe};

/// One slot.
///
/// `Removed` is distinct from `Vacant` on purpose: linear probing walks
/// until it finds a vacancy, so turning a removed slot into a vacancy
/// would cut the chain and hide every object that probed past it.
#[derive(Clone, Copy)]
enum Slot {
    /// Never used. A probe that reaches one has proved the object absent.
    Vacant,
    /// Used, then removed. A probe passes through.
    Removed,
    /// In use.
    Live(ObjectId, ObjectMeta),
}

/// A fixed-capacity map from [`ObjectId`] to [`ObjectMeta`].
///
/// Fixed because this lives in the kernel and there is no allocator, and
/// because a table that can grow can grow on the fault path -- which is
/// the one place that must not allocate.
pub struct Table<const N: usize> {
    slots: [Slot; N],
}

impl<const N: usize> Table<N> {
    /// An empty table.
    ///
    /// A constant rather than a constructor so it can initialise a
    /// `static` -- the kernel's table has to exist before there is
    /// anything to allocate it with.
    pub const EMPTY: Self = Self {
        slots: [Slot::Vacant; N],
    };

    /// Records an object, replacing any entry already under that id.
    ///
    /// `false` if the table is full, which drops the registration rather
    /// than evicting something the caller did not ask to lose.
    pub fn insert(&mut self, id: ObjectId, meta: ObjectMeta) -> bool {
        let mut reusable = None;
        for at in probe::sequence(id, N) {
            match self.slots[at] {
                Slot::Live(found, _) if found == id => {
                    self.slots[at] = Slot::Live(id, meta);
                    return true;
                }
                Slot::Removed if reusable.is_none() => reusable = Some(at),
                Slot::Vacant => {
                    self.slots[reusable.unwrap_or(at)] = Slot::Live(id, meta);
                    return true;
                }
                _ => {}
            }
        }
        false
    }

    /// Looks an object up. The fast path: an exact match or a proof of
    /// absence, with no allocation and no call out of the kernel.
    #[must_use]
    pub fn get(&self, id: ObjectId) -> Option<&ObjectMeta> {
        for at in probe::sequence(id, N) {
            match &self.slots[at] {
                Slot::Live(found, meta) if *found == id => return Some(meta),
                Slot::Vacant => return None,
                _ => {}
            }
        }
        None
    }

    /// Looks an object up for modification -- residency changes, next-use
    /// updates, lease counting.
    pub fn get_mut(&mut self, id: ObjectId) -> Option<&mut ObjectMeta> {
        for at in probe::sequence(id, N) {
            match &self.slots[at] {
                Slot::Live(found, _) if *found == id => {
                    let Slot::Live(_, meta) = &mut self.slots[at] else {
                        unreachable!("just matched Live")
                    };
                    return Some(meta);
                }
                Slot::Vacant => return None,
                _ => {}
            }
        }
        None
    }

    /// Forgets an object. `true` if it was there.
    pub fn remove(&mut self, id: ObjectId) -> bool {
        for at in probe::sequence(id, N) {
            match self.slots[at] {
                Slot::Live(found, _) if found == id => {
                    self.slots[at] = Slot::Removed;
                    return true;
                }
                Slot::Vacant => return false,
                _ => {}
            }
        }
        false
    }
}
