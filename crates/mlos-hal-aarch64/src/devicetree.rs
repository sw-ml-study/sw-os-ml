//! What the device tree says this machine is.
//!
//! Answers exactly three questions -- where is memory, how many CPUs,
//! where is the console -- and stops. Anything more waits until something
//! needs it.

use mlos_fdt::{Event, Fdt, reg_pair};
use mlos_hal::{MemoryKind, MemoryRegion};

/// How many memory regions we will record.
///
/// QEMU `virt` reports one. Eight leaves room for a machine with a split
/// map without putting a variable-length structure somewhere there is no
/// allocator to build it.
pub const MAX_REGIONS: usize = 8;

/// The machine, as the device tree describes it.
#[derive(Clone, Copy)]
pub struct Machine {
    /// Memory regions, `region_count` of them valid.
    pub regions: [MemoryRegion; MAX_REGIONS],
    /// How many of `regions` were filled.
    pub region_count: usize,
    /// How many `cpu@` nodes appeared under `/cpus`.
    pub cpu_count: u32,
    /// Base address of the PL011, if the tree has one.
    pub uart_base: Option<usize>,
}

/// Walk state.
///
/// Property events always arrive between a node's own `Node` event and
/// its first child's, because the specification requires every property
/// to precede every subnode. That is what makes a single "current node
/// name" correct rather than a bug waiting for a deeper tree.
struct Scan<'a> {
    depth: usize,
    node: &'a str,
    address_cells: u32,
    size_cells: u32,
    machine: Machine,
}

impl<'a> Scan<'a> {
    /// Folds one walk event into the state.
    fn event(&mut self, event: Event<'a>) {
        match event {
            Event::Node { name } => {
                self.depth += 1;
                self.node = name;
                // /cpus/cpu@N -- root is depth 1, /cpus is 2, cpu@N is 3.
                if self.depth == 3 && name.starts_with("cpu@") {
                    self.machine.cpu_count += 1;
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
                self.machine.uart_base = first.map(|(base, _)| base as usize);
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
            let Some(slot) = self.machine.regions.get_mut(self.machine.region_count) else {
                return; // more regions than MAX_REGIONS; keep the first ones
            };
            *slot = MemoryRegion {
                base,
                len,
                kind: MemoryKind::Usable,
            };
            self.machine.region_count += 1;
        }
    }
}

impl Machine {
    /// Reads the device tree the loader left at `dtb`.
    ///
    /// # Safety
    ///
    /// `dtb` must be what the arm64 boot protocol put in `x0`: a device
    /// tree blob whose declared length is readable. A pointer to anything
    /// else is rejected by the header check, not believed.
    pub unsafe fn probe(dtb: *const u8) -> Option<Self> {
        // SAFETY: forwarded from this function's contract.
        let fdt = unsafe { Fdt::from_ptr(dtb) }?;
        let empty = MemoryRegion {
            base: 0,
            len: 0,
            kind: MemoryKind::Reserved,
        };
        let mut scan = Scan {
            depth: 0,
            node: "",
            // Specification defaults, overwritten by the root's own cells.
            address_cells: 2,
            size_cells: 1,
            machine: Self {
                regions: [empty; MAX_REGIONS],
                region_count: 0,
                cpu_count: 0,
                uart_base: None,
            },
        };
        fdt.walk(|event| scan.event(event))?;
        Some(scan.machine)
    }
}
