//! A minimal reader for the flattened device tree format.
//!
//! Enough to answer the three questions MLOS asks at boot -- where is
//! memory, how many CPUs, and where is the console -- and no more. It
//! walks; it does not build a tree, because building one needs an
//! allocator and the memory map is what the allocator gets built *from*.
//!
//! Architecture-neutral on purpose. aarch64 and RISC-V both boot this way,
//! so this is one of the few places a future `mlos-hal-riscv64` costs
//! nothing (`docs/architecture.md` s.9).
//!
//! The blob is firmware-supplied and therefore untrusted. Every read is
//! bounds-checked and every malformed input yields `None`, never a panic
//! and never an out-of-range access.

#![no_std]

mod cursor;
mod header;
mod reg;

use cursor::Cursor;
pub use header::Header;
pub use reg::{reg_pair, string};

/// Structure-block tokens.
mod token {
    /// A node begins; a NUL-terminated name follows.
    pub const BEGIN_NODE: u32 = 1;
    /// The innermost open node ends.
    pub const END_NODE: u32 = 2;
    /// A property; length, name offset and value follow.
    pub const PROP: u32 = 3;
    /// Padding, to be skipped.
    pub const NOP: u32 = 4;
    /// End of the structure block.
    pub const END: u32 = 9;
}

/// What [`Fdt::walk`] reports, in document order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Event<'a> {
    /// A node opened. `name` is `""` for the root.
    Node {
        /// The node's name, including any `@unit-address` suffix.
        name: &'a str,
    },
    /// The innermost open node closed.
    EndNode,
    /// A property of the innermost open node.
    Prop {
        /// Property name.
        name: &'a str,
        /// Raw value. Big-endian cells, or a NUL-terminated string.
        value: &'a [u8],
    },
}

/// A parsed device tree blob.
pub struct Fdt<'a> {
    structs: &'a [u8],
    strings: &'a [u8],
}

impl<'a> Fdt<'a> {
    /// Reads the blob a loader left in memory.
    ///
    /// # Safety
    ///
    /// `ptr` must point at a device tree blob whose declared `total_size`
    /// bytes are all readable. The header is read first to learn that
    /// length, so a pointer to something that is not a blob is rejected
    /// after 40 bytes rather than believed.
    pub unsafe fn from_ptr(ptr: *const u8) -> Option<Self> {
        // SAFETY: the caller guarantees a blob is here; 40 bytes is the
        // fixed header length, read before trusting any field in it.
        let head = unsafe { core::slice::from_raw_parts(ptr, 40) };
        let total = Header::parse(head)?.total_size;
        // SAFETY: `total_size` came from a header that passed its magic
        // and version checks, and the caller guarantees that many bytes.
        Self::new(unsafe { core::slice::from_raw_parts(ptr, total) })
    }

    /// Reads a blob already in a slice.
    pub fn new(blob: &'a [u8]) -> Option<Self> {
        let header = Header::parse(blob)?;
        Some(Self {
            structs: blob
                .get(header.struct_offset..)?
                .get(..header.struct_size)?,
            strings: blob
                .get(header.strings_offset..)?
                .get(..header.strings_size)?,
        })
    }

    /// Walks the tree, reporting every node and property in order.
    ///
    /// Returns `None` on a malformed blob, having already reported
    /// whatever it read successfully -- a caller that got what it needed
    /// before the damage can proceed.
    pub fn walk(&self, mut visit: impl FnMut(Event<'a>)) -> Option<()> {
        let mut cursor = Cursor::new(self.structs);
        loop {
            match cursor.u32()? {
                token::BEGIN_NODE => visit(Event::Node {
                    name: cursor.cstr()?,
                }),
                token::END_NODE => visit(Event::EndNode),
                token::PROP => visit(self.property(&mut cursor)?),
                token::NOP => {}
                token::END => return Some(()),
                _ => return None,
            }
        }
    }

    /// Reads one property: length, name offset, then the value.
    fn property(&self, cursor: &mut Cursor<'a>) -> Option<Event<'a>> {
        let len = cursor.u32()? as usize;
        let name_at = cursor.u32()? as usize;
        let value = cursor.bytes(len)?;
        let rest = self.strings.get(name_at..)?;
        let end = rest.iter().position(|&b| b == 0)?;
        Some(Event::Prop {
            name: core::str::from_utf8(&rest[..end]).ok()?,
            value,
        })
    }
}
