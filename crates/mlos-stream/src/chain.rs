//! Where each declared access turns up next, precomputed in one pass.
//!
//! Invariant: this is the one implementation of "when is this next
//! wanted"; the kernel and the simulator both call it. Design and
//! history: docs/notes/mlos-stream.md.

use mlos_abi::{Error, ObjectId, Result};

/// No further use in this declaration. A distinct answer, not a large
/// distance.
pub const NEVER: u32 = u32::MAX;

/// Fills `into[i]` with the next position after `i` holding the same
/// object as `objects[i]`, or [`NEVER`]. `seen` needs one slot per
/// distinct object. `NoBudget` if either buffer is too small; nothing is
/// truncated.
pub fn chain(objects: &[ObjectId], into: &mut [u32], seen: &mut [(ObjectId, u32)]) -> Result<()> {
    let into = into.get_mut(..objects.len()).ok_or(Error::NoBudget)?;
    let mut distinct = 0usize;
    for (at, object) in objects.iter().enumerate().rev() {
        let at = at as u32;
        match seen[..distinct].iter_mut().find(|(held, _)| held == object) {
            Some((_, position)) => {
                into[at as usize] = *position;
                *position = at;
            }
            None => {
                into[at as usize] = NEVER;
                *seen.get_mut(distinct).ok_or(Error::NoBudget)? = (*object, at);
                distinct += 1;
            }
        }
    }
    Ok(())
}
