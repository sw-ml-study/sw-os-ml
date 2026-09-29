//! Folding a device tree walk into a machine description.
//!
//! Invariant: a node's properties precede its children, so one current
//! node name is enough. Design: docs/notes/mlos-machine.md.

use crate::regions::Regions;
use mlos_fdt::{Event, reg_pair, string};

/// Walk state.
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
    pub virtio: Option<(usize, usize)>,
    /// How many `virtio_mmio@` nodes were seen.
    pub virtio_count: u32,
    /// `/chosen/bootargs`, which says which console was asked for.
    pub bootargs: Option<&'a str>,
    /// Whether the interrupt controller claimed `arm,gic-v3`. Combined
    /// with [`Self::gic_reg`] after the walk: property order is not fixed.
    pub gic_v3: bool,
    /// The interrupt controller's two `reg` ranges: distributor and
    /// redistributor on a v3, something else on a v2.
    pub gic_reg: Option<(u64, u64)>,
}

impl<'a> Scan<'a> {
    /// Folds one walk event.
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
            (2, "bootargs") if self.node == "chosen" => self.bootargs = string(value),
            (2, "interrupts") if self.node.starts_with("pl011@") && self.uart_irq.is_none() => {
                // <kind, number, flags>. Kind 0 is a shared interrupt; the
                // tree numbers from the SPI base, the GIC from 32.
                let kind = cell(0);
                self.uart_irq = (kind == Some(0)).then(|| cell(1)).flatten().map(|n| n + 32);
            }
            (2, "compatible") if self.node.starts_with("intc@") => {
                self.gic_v3 = value.split(|&b| b == 0).any(|name| name == b"arm,gic-v3");
            }
            _ => {}
        }
    }

    /// Handles a `reg` property, according to which node carries it. The
    /// console is the first `pl011@` node, not the last.
    fn reg(&mut self, value: &[u8]) {
        let (cells, sizes) = (self.address_cells, self.size_cells);
        let pair = |index| reg_pair(value, cells, sizes, index);
        if self.node.starts_with("memory@") {
            self.regions.extend_usable(&pair);
        } else if self.node.starts_with("pl011@") && self.uart_base.is_none() {
            self.uart_base = pair(0).map(|(base, _)| base as usize);
        } else if self.node.starts_with("virtio_mmio@") {
            self.virtio_count += 1;
            // The lowest window: the slots are uniform and contiguous.
            if let Some((base, size)) = pair(0) {
                let seen = self.virtio.map_or(usize::MAX, |(base, _)| base);
                if (base as usize) < seen {
                    self.virtio = Some((base as usize, size as usize));
                }
            }
        } else if self.node.starts_with("intc@") {
            // Both ranges or neither: half a controller looks initialised.
            if let (Some((first, _)), Some((second, _))) = (pair(0), pair(1)) {
                self.gic_reg = Some((first, second));
            }
        }
    }
}

impl Default for Scan<'_> {
    /// Cells start at the specification's defaults; the root node's own
    /// `#address-cells` and `#size-cells` overwrite them before any `reg`
    /// is read.
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
