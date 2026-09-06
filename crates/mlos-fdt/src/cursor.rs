//! A bounds-checked reader over a device tree's structure block.
//!
//! Everything in a DTB is big-endian and four-byte aligned, and every read
//! here is checked. This blob arrives from firmware: it is the first
//! untrusted input the kernel ever sees, and a parser that indexes without
//! checking turns a malformed one into arbitrary memory access.

/// A position in a byte slice.
pub struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    /// Starts at the beginning of `data`.
    #[must_use]
    pub const fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    /// Reads one big-endian `u32`.
    pub fn u32(&mut self) -> Option<u32> {
        let bytes = self.data.get(self.pos..self.pos + 4)?;
        self.pos += 4;
        Some(u32::from_be_bytes(bytes.try_into().ok()?))
    }

    /// Reads `len` bytes, then advances to the next four-byte boundary.
    pub fn bytes(&mut self, len: usize) -> Option<&'a [u8]> {
        let value = self.data.get(self.pos..self.pos.checked_add(len)?)?;
        self.pos = self.pos.checked_add(len)?.next_multiple_of(4);
        Some(value)
    }

    /// Reads a NUL-terminated string, then advances past the padding.
    ///
    /// Rejects non-UTF-8 rather than replacing it: node names in a device
    /// tree are ASCII by specification, so anything else means the blob is
    /// not what it claims to be.
    pub fn cstr(&mut self) -> Option<&'a str> {
        let rest = self.data.get(self.pos..)?;
        let len = rest.iter().position(|&b| b == 0)?;
        self.pos = self.pos.checked_add(len + 1)?.next_multiple_of(4);
        core::str::from_utf8(&rest[..len]).ok()
    }
}
