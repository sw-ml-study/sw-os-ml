//! Folding a device tree walk into a machine description.

use mlos_fdt::{Event, reg_pair};
use mlos_hal::{MemoryKind, MemoryRegion};

use crate::regions::Regions;

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
    fn prop(&mut self, name: &str, value: &[u8]) {
        let cell = || -> Option<u32> { Some(u32::from_be_bytes(value.get(..4)?.try_into().ok()?)) };
        match (self.depth, name) {
            (1, "#address-cells") => self.address_cells = cell().unwrap_or(2),
            (1, "#size-cells") => self.size_cells = cell().unwrap_or(2),
            (2, "reg") if self.node.starts_with("memory@") => self.memory(value),
            (2, "reg") if self.node.starts_with("pl011@") => {
                let first = reg_pair(value, self.address_cells, self.size_cells, 0);
                self.uart_base = first.map(|(base, _)| base as usize);
            }
            _ => {}
        }
    }

    /// Records every `(base, len)` pair a `memory` node lists.
    fn memory(&mut self, value: &[u8]) {
        for index in 0.. {
            let Some((base, len)) = reg_pair(value, self.address_cells, self.size_cells, index)
            else {
                return;
            };
            if !self.regions.push(MemoryRegion {
                base,
                len,
                kind: MemoryKind::Usable,
            }) {
                return; // map full; keep the regions we already have
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
            regions: Regions::new(),
            cpu_count: 0,
            uart_base: None,
        }
    }
}
