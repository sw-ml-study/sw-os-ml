//! One line of input, in a fixed buffer.

/// The longest command line. Nothing here takes arguments yet, so this is
/// generous; it is fixed because there is no allocator, and it will still
/// be fixed when there is -- a shell that can be made to allocate by
/// holding down a key is a shell with a denial of service in it.
const CAPACITY: usize = 96;

/// A line being typed.
pub struct Line {
    bytes: [u8; CAPACITY],
    len: usize,
}

impl Line {
    /// Appends a byte. `false` if the line is full, so the caller knows
    /// not to echo something that was not accepted.
    pub fn push(&mut self, byte: u8) -> bool {
        let Some(slot) = self.bytes.get_mut(self.len) else {
            return false;
        };
        *slot = byte;
        self.len += 1;
        true
    }

    /// Removes the last byte. `false` if there was nothing to remove, so
    /// backspace at the prompt does not erase the prompt.
    pub fn backspace(&mut self) -> bool {
        self.len = match self.len.checked_sub(1) {
            Some(len) => len,
            None => return false,
        };
        true
    }

    /// The line so far, or `""` if it is not valid UTF-8.
    ///
    /// Only printable ASCII is ever pushed, so the failure cannot happen
    /// -- but returning `""` rather than unwrapping keeps a future change
    /// to that rule from turning into a panic with no console.
    #[must_use]
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.len]).unwrap_or("")
    }
}

impl Default for Line {
    fn default() -> Self {
        Self {
            bytes: [0; CAPACITY],
            len: 0,
        }
    }
}
