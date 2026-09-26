//! Where each declared access turns up next.
//!
//! One pass over a declared sequence, producing for every position the
//! position of the next access to the same object. That is the whole of
//! what a policy needs to know about the future, precomputed once, so
//! asking costs an indexed read rather than a search.
//!
//! **This function is why the kernel and the simulator agree.** The
//! simulator used to compute the same chain itself, from the whole trace,
//! with its own copy of the algorithm; the kernel scanned a declaration
//! instead. Two implementations of "when is this next wanted" is two
//! answers, and step 009 measured the difference at 27 reads. There is
//! now one implementation and both sides call it -- `mlos-sim` wrapping
//! it in `Vec`s it can afford, the kernel in statics it can.
//!
//! No allocation, no map. The caller supplies the output and a scratch
//! table of objects seen so far, which is linear-searched: a decode loop
//! touches a few hundred distinct objects against thousands of accesses,
//! so the scan is short and a hash table would be machinery the kernel
//! would have to justify.

use mlos_abi::{Error, ObjectId, Result};

/// No further use in this declaration.
///
/// Not a large number standing in for infinity: it means the declaration
/// says nothing more about this object, which a policy must be free to
/// treat differently from "wanted very far away".
pub const NEVER: u32 = u32::MAX;

/// Fills `into[i]` with the next position after `i` holding the same
/// object as `objects[i]`, or [`NEVER`].
///
/// Backwards, because that is the direction the answer falls out in:
/// walking from the end, the last position seen for an object IS its next
/// use from any earlier point.
///
/// `seen` needs one slot per DISTINCT object, not per access. `NoBudget`
/// if either buffer is too small -- refused rather than truncated,
/// because a partial chain produces confident wrong numbers.
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
