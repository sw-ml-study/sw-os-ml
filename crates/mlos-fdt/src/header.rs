//! The 40-byte device tree header.

/// Reads a big-endian `u32` at a byte offset, or `None` if out of range.
fn be32(bytes: &[u8], at: usize) -> Option<u32> {
    let field = bytes.get(at..at + 4)?;
    Some(u32::from_be_bytes(field.try_into().ok()?))
}

/// What the header says about the rest of the blob.
pub struct Header {
    /// Total blob length, including this header.
    pub total_size: usize,
    /// Byte offset of the structure block.
    pub struct_offset: usize,
    /// Length of the structure block.
    pub struct_size: usize,
    /// Byte offset of the strings block.
    pub strings_offset: usize,
    /// Length of the strings block.
    pub strings_size: usize,
}

impl Header {
    /// `0xd00dfeed`, big-endian, at offset 0.
    pub const MAGIC: u32 = 0xd00d_feed;

    /// Version 16 introduced the layout this reader assumes.
    pub const MIN_COMPATIBLE: u32 = 16;

    /// Parses and sanity-checks a header.
    ///
    /// Checks `last_comp_version`, not `version`: a newer blob that still
    /// declares itself backward compatible with 16 is one we can read, and
    /// refusing it would reject a future QEMU for no reason.
    pub fn parse(blob: &[u8]) -> Option<Self> {
        if be32(blob, 0)? != Self::MAGIC || be32(blob, 24)? > Self::MIN_COMPATIBLE {
            return None;
        }
        Some(Self {
            total_size: be32(blob, 4)? as usize,
            struct_offset: be32(blob, 8)? as usize,
            struct_size: be32(blob, 36)? as usize,
            strings_offset: be32(blob, 12)? as usize,
            strings_size: be32(blob, 32)? as usize,
        })
    }
}
