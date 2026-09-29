//! `virtio_mmio.device=` entries on the kernel command line.
//!
//! Invariant: an entry is `virtio_mmio.device=SIZE@BASE:IRQ`, SIZE in bytes
//! with an optional `K`/`M` suffix, BASE in hex; a malformed one is skipped,
//! never guessed at. Design: docs/notes/mlos-pvh.md.

/// The lowest transport's base and size, and how many transports there
/// are: the shape `mlos_lab::set_slots` takes. `None` if the command line
/// names none; malformed entries are skipped.
#[must_use]
pub fn virtio_slots(cmdline: &str) -> Option<(usize, usize, u32)> {
    let mut found: Option<(usize, usize, u32)> = None;
    for (base, size) in cmdline.split_ascii_whitespace().filter_map(entry) {
        found = Some(match found {
            Some((low, low_size, count)) if low < base => (low, low_size, count + 1),
            Some((_, _, count)) => (base, size, count + 1),
            None => (base, size, 1),
        });
    }
    found
}

/// The command line up to the first `virtio_mmio.device=` entry that
/// follows `mlsh.run=`, which QEMU appended after the user's arguments and
/// `mlsh.run=` would otherwise read as its own. A line without one is
/// returned whole.
#[must_use]
pub fn user_args(cmdline: &str) -> &str {
    let Some(run) = cmdline.find("mlsh.run=") else {
        return cmdline;
    };
    match cmdline[run..].find(" virtio_mmio.device=") {
        Some(end) => &cmdline[..run + end],
        None => cmdline,
    }
}

/// One `virtio_mmio.device=SIZE@BASE:IRQ` word, as `(base, size)`.
fn entry(word: &str) -> Option<(usize, usize)> {
    let (size, rest) = word.strip_prefix("virtio_mmio.device=")?.split_once('@')?;
    let base = rest.split(':').next()?.strip_prefix("0x")?;
    let (digits, scale) = match size.as_bytes().last()? {
        b'K' | b'k' => (&size[..size.len() - 1], 1 << 10),
        b'M' | b'm' => (&size[..size.len() - 1], 1 << 20),
        _ => (size, 1),
    };
    let size = digits.parse::<usize>().ok()?.checked_mul(scale)?;
    Some((usize::from_str_radix(base, 16).ok()?, size))
}
