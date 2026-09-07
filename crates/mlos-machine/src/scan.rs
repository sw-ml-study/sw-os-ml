//! Folding a device tree walk into a machine description.

use crate::regions::Regions;
use mlos_fdt::{Event, reg_pair};

/// Walk state.
///
/// Property events always arrive between a node's own `Node` event and its
/// first child's, because the specification requires every property to
/// precede every subnode. That is what makes a single "current node name"
/// correct rather than a bug waiting for a deeper tree.
pub struct Scan<'a> {
    depth: usize,
    node: &'a str,
    address_cells: u32,
    size_cells: u32,
    /// Memory regions, as the tree reports them.
    pub regions: Regions,
    /// How many `cpu@` nodes appeared under `/cpus`.
    pub cpu_count: u32,
    /// Base address of the PL011, if the tree has one.
    pub uart_base: Option<usize>,
    /// The console's interrupt number, if the tree gives one.
    pub uart_irq: Option<u32>,
    /// The lowest `virtio_mmio@` window, and how many follow it.
    ///
    /// QEMU lays out 32 identical slots whether or not anything is
    /// plugged in, so the count matters more than any single address:
    /// finding the console means probing them.
    pub virtio: Option<(usize, usize)>,
    /// How many `virtio_mmio@` nodes were seen.
    pub virtio_count: u32,
    /// `/chosen/bootargs`, which says which console was asked for.
    pub bootargs: Option<&'a str>,
    /// Whether the interrupt controller claimed `arm,gic-v3`.
    ///
    /// Kept apart from [`Self::gic_reg`] because a device tree does not
    /// order a node's properties: in QEMU's own blob `reg` comes *before*
    /// `compatible`, so deciding what the ranges mean while reading them
    /// reads the wrong answer. They are combined once the walk is over.
    pub gic_v3: bool,
    /// The interrupt controller's two `reg` ranges, whatever they mean.
    ///
    /// For a GICv3 these are the distributor and the redistributor, in
    /// that order: one system-wide, one per-CPU. A GICv2 puts a CPU
    /// interface in the second range instead, which is a different device
    /// at a different offset -- hence the `compatible` check.
    pub gic_reg: Option<(u64, u64)>,
}

impl<'a> Scan<'a> {
    pub fn event(&mut self, event: Event<'a>) {
        match event {
            Event::Node { name } => {
                self.depth += 1;
                self.node = name;
                // /cpus/cpu@N -- root is depth 1, /cpus is 2, cpu@N is 3.
                if self.depth == 3 && name.starts_with("cpu@") {
                    self.cpu_count += 1;
                }
            }
            Event::EndNode => self.depth = self.depth.saturating_sub(1),
            Event::Prop { name, value } => self.prop(name, value),
        }
    }

    /// Folds one property, ignoring everything not asked about.
    fn prop(&mut self, name: &str, value: &'a [u8]) {
        let cell = |index: usize| -> Option<u32> {
            let at = index * 4;
            Some(u32::from_be_bytes(value.get(at..at + 4)?.try_into().ok()?))
        };
        match (self.depth, name) {
            (1, "#address-cells") => self.address_cells = cell(0).unwrap_or(2),
            (1, "#size-cells") => self.size_cells = cell(0).unwrap_or(2),
            (2, "reg") => self.reg(value),
            (2, "interrupts") if self.node.starts_with("pl011@") && self.uart_irq.is_none() => {
                // <kind, number, flags>. Kind 0 is a shared interrupt,
                // whose numbering starts at 32 -- the device tree counts
                // from the start of the SPI range, the GIC does not.
                let kind = cell(0);
                self.uart_irq = (kind == Some(0)).then(|| cell(1)).flatten().map(|n| n + 32);
            }
            (2, "bootargs") if self.node == "chosen" => {
                self.bootargs = core::str::from_utf8(value)
                    .ok()
                    .map(|args| args.trim_end_matches('\0'));
            }
            (2, "compatible") if self.node.starts_with("intc@") => {
                self.gic_v3 = value.split(|&b| b == 0).any(|name| name == b"arm,gic-v3");
            }
            _ => {}
        }
    }

    /// Handles a `reg` property, according to which node carries it.
    ///
    /// One function rather than three because `reg` means "where this
    /// device is" regardless of the device, and the cells that decode it
    /// come from the same parent either way.
    ///
    /// The console is the FIRST `pl011@` node, not the last. A machine can
    /// have more than one -- two `-serial` backends make QEMU instantiate
    /// a second at 0x9040000 -- and overwriting as the walk goes picks
    /// whichever comes last, which is not the console. The exact answer is
    /// `/chosen/stdout-path`, but that node comes *after* the UARTs in
    /// QEMU's tree, so honouring it needs candidates resolved at the end
    /// of the walk rather than a single field. Recorded as a gap in
    /// `docs/status.md`; first-wins is right for every tree QEMU emits.
    fn reg(&mut self, value: &[u8]) {
        let (cells, sizes) = (self.address_cells, self.size_cells);
        let pair = |index| reg_pair(value, cells, sizes, index);
        if self.node.starts_with("memory@") {
            self.regions.extend_usable(&pair);
        } else if self.node.starts_with("pl011@") && self.uart_base.is_none() {
            self.uart_base = pair(0).map(|(base, _)| base as usize);
        } else if self.node.starts_with("virtio_mmio@") {
            self.virtio_count += 1;
            // The lowest window: the slots are uniform and contiguous, so
            // one base and one size describe all of them.
            if let Some((base, size)) = pair(0) {
                let seen = self.virtio.map_or(usize::MAX, |(base, _)| base);
                if (base as usize) < seen {
                    self.virtio = Some((base as usize, size as usize));
                }
            }
        } else if self.node.starts_with("intc@") {
            // Both ranges or neither: half an interrupt controller is
            // worse than none, because it looks initialised.
            if let (Some((first, _)), Some((second, _))) = (pair(0), pair(1)) {
                self.gic_reg = Some((first, second));
            }
        }
    }
}

impl Default for Scan<'_> {
    /// Cells start at the specification's defaults; the root node's own
    /// `#address-cells` and `#size-cells` overwrite them before any `reg`
    /// is read, because a node's properties precede its children.
    fn default() -> Self {
        Self {
            depth: 0,
            node: "",
            address_cells: 2,
            size_cells: 1,
            regions: Regions::default(),
            cpu_count: 0,
            uart_base: None,
            uart_irq: None,
            virtio: None,
            virtio_count: 0,
            bootargs: None,
            gic_v3: false,
            gic_reg: None,
        }
    }
}
