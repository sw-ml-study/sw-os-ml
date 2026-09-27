//! `virtio_mmio.device=` entries on the kernel command line.
//!
//! QEMU `microvm` announces each virtio-mmio transport this way instead of
//! in a device tree -- the same convention Linux reads -- as
//! `virtio_mmio.device=SIZE@BASE:IRQ`, SIZE in bytes with an optional
//! `K`/`M` suffix, BASE in hex.

/// The lowest transport's base and size, and how many transports there
/// are: the shape `mlos_lab::set_slots` takes, the same as the device
/// tree's `virtio_mmio@` nodes give on aarch64.
///
/// `None` if the command line names none. Malformed entries are skipped
/// rather than trusted: a slot at a misread address is worse than a
/// missing one.
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

/// The command line up to where the loader's own additions begin.
///
/// `mlsh.run=` takes the REST of the line, because its commands have
/// spaces in them -- and QEMU `microvm` appends `virtio_mmio.device=`
/// entries AFTER whatever the user passed. Without this, the last shell
/// command would be handed the device list as its argument. Cut at the
/// first entry that follows `mlsh.run=`; a line without one is whole.
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
