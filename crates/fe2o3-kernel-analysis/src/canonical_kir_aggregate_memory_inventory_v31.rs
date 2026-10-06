//! Borrowed exact-owner memory census before an optimizer selects allocations.
//! The original census is shared with aggregate mutation and independent replay.
use super::*;

/// One supported typed allocation in the exact borrowed inventory.
/// Closed uses alone do not establish initialization, SSA replacement or source
/// lifetime/Drop semantics; those remain independent obligations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirAggregateMemoryAllocationV31 {
    operation: usize,
    closed_uses: bool,
}
impl CanonicalKirAggregateMemoryAllocationV31 {
    /// Dense operation ordinal under `CanonicalKirAggregateMemoryInventoryV31::inventory`.
    pub const fn operation(self) -> usize {
        self.operation
    }
    /// All address uses fit the existing exact private layout/access grammar.
    /// This does not assert that any read is initialized or safely promotable.
    pub const fn has_closed_supported_uses(self) -> bool {
        self.closed_uses
    }
}

/// Original typed event. `None` explicitly includes unmodeled operations and
/// cannot be interpreted as an inert operation by a semantic consumer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalKirAggregateMemoryEventV31 {
    /// No event in the closed aggregate layout grammar; inspect the actual op.
    None,
    /// A new dynamic lifetime of the named allocation kills its initialized bits.
    Allocate { allocation: usize },
    /// A checked constant typed projection of the named allocation.
    Project { allocation: usize },
    /// A read of the named census leaf, with its exact actual result ValueId.
    Read { leaf: usize, output: ValueId },
    /// A write to the named census leaf, with its exact actual input ValueId.
    Write { leaf: usize, value: ValueId },
}

/// Unreserved retained storage of a memory inventory. The caller keeps the
/// borrowed canonical inventory and this owner paid for every query.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirAggregateMemoryStorageV31 {
    retained: usize,
}
impl CanonicalKirAggregateMemoryStorageV31 {
    /// Actual retained owner and vector capacity, excluding derivation scratch.
    pub const fn retained_storage(self) -> usize {
        self.retained
    }
}

/// Immutable original-derived allocation, leaf and event census, including
/// allocations not selected by a particular aggregate optimization round.
/// All indices are scoped to this exact borrowed inventory, never identities
/// across stages. A chain consumer must replay exact occurrence transport and
/// join original allocation/layout/offset/type keys before reusing any index.
/// No SSA replacement, source equivalence or executable authority is granted.
pub struct CanonicalKirAggregateMemoryInventoryV31<'inventory, 'graph> {
    inventory: &'inventory Inventory<'graph>,
    census: census::Census,
}

struct MemoryCapture<'i, 'g> {
    inventory: &'i Inventory<'g>,
}
impl<'i, 'g> MemoryCapture<'i, 'g> {
    fn inventory(self) -> &'i Inventory<'g> {
        self.inventory
    }
}

pub(super) fn memory_inventory_headers_v31() -> Result<usize> {
    type Output<'i, 'g> = (
        CanonicalKirAggregateMemoryInventoryV31<'i, 'g>,
        CanonicalKirAggregateMemoryStorageV31,
    );
    type Capture<'a, 'w, 'i, 'g> = (MemoryCapture<'i, 'g>, &'a mut Meter<'a, 'w>);
    type Scope<'a, 'w, 'i, 'g> = (
        MemoryCapture<'i, 'g>,
        Capture<'a, 'w, 'i, 'g>,
        std::panic::AssertUnwindSafe<Capture<'a, 'w, 'i, 'g>>,
        std::thread::Result<Result<Output<'i, 'g>>>,
        [Result<Output<'i, 'g>>; 2],
        std::panic::AssertUnwindSafe<Result<Output<'i, 'g>>>,
        std::thread::Result<()>,
        [Option<Box<dyn std::any::Any + Send>>; 2],
        Result<()>,
    );
    type Frame<'a, 'w, 'i, 'g> = (
        &'i Inventory<'g>,
        &'a mut Meter<'a, 'w>,
        Output<'i, 'g>,
        Result<Output<'i, 'g>>,
        Result<census::Census>,
        [usize; 4],
        Scope<'a, 'w, 'i, 'g>,
    );
    add(
        size_of::<Frame<'_, '_, '_, '_>>(),
        std::mem::align_of::<Frame<'_, '_, '_, '_>>(),
    )
}

impl<'i, 'g> CanonicalKirAggregateMemoryInventoryV31<'i, 'g> {
    /// Derives the same exact typed census used by aggregate planning and pair
    /// replay, without selecting allocations or invoking a second SSA planner.
    /// Scratch is settled on success/refusal/unwind; returned credit is unreserved.
    pub fn derive(
        inventory: &'i Inventory<'g>,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, CanonicalKirAggregateMemoryStorageV31)> {
        let capture = MemoryCapture { inventory };
        let run = move |meter: &mut Meter<'_, '_>| {
            let inventory = capture.inventory();
            meter.reserve(memory_inventory_headers_v31()?)?;
            let census = census::derive(inventory, meter)?;
            let retained = [
                bytes(&census.allocations)?,
                bytes(&census.cells)?,
                bytes(&census.events)?,
            ]
            .into_iter()
            .try_fold(size_of::<Self>(), add)?;
            Ok((
                Self { inventory, census },
                CanonicalKirAggregateMemoryStorageV31 { retained },
            ))
        };
        assert_eq!(
            std::mem::size_of_val(&run),
            size_of::<MemoryCapture<'_, '_>>()
        );
        assert_eq!(
            std::mem::align_of_val(&run),
            std::mem::align_of::<MemoryCapture<'_, '_>>()
        );
        resources::scoped(budget, run)
    }

    /// The exact borrowed endpoint whose dense operation and leaf indices apply.
    pub const fn inventory(&self) -> &'i Inventory<'g> {
        self.inventory
    }
    /// Number of supported typed allocations, whether closed or escaped.
    pub fn allocation_count(&self) -> usize {
        self.census.allocations.len()
    }
    /// Returns one allocation with its complete-use status before SSA planning.
    pub fn allocation(&self, index: usize) -> Option<CanonicalKirAggregateMemoryAllocationV31> {
        self.census
            .allocations
            .get(index)
            .map(|row| CanonicalKirAggregateMemoryAllocationV31 {
                operation: row.operation,
                closed_uses: row.eligible,
            })
    }
    /// Number of observed typed leaves, not an expansion by aggregate byte size.
    pub fn leaf_count(&self) -> usize {
        self.census.cells.len()
    }
    /// Exact allocation operation, leaf layout, byte offset and logical type.
    /// Unlike a selected SSA witness this includes unselected/escaped leaves.
    pub fn leaf(&self, index: usize) -> Option<CanonicalKirAggregateSsaMemorySlotV18> {
        self.census
            .cells
            .get(index)
            .map(|cell| CanonicalKirAggregateSsaMemorySlotV18 {
                allocation: self.census.allocations[cell.address.allocation].operation,
                layout: cell.address.layout,
                offset: cell.address.offset,
                ty: cell.ty,
            })
    }
    /// Actual checked alignment of the original leaf projection.
    pub fn leaf_alignment(&self, index: usize) -> Option<u32> {
        self.census
            .cells
            .get(index)
            .map(|cell| cell.address.alignment)
    }
    /// Complete original operation roster; each operation has one event row.
    pub fn event_count(&self) -> usize {
        self.census.events.len()
    }
    /// Event at one dense original operation ordinal. Callers independently
    /// model all `None` operations and every non-closed allocation interaction.
    pub fn event(&self, operation: usize) -> Option<CanonicalKirAggregateMemoryEventV31> {
        use CanonicalKirAggregateMemoryEventV31 as Event;
        self.census.events.get(operation).map(|event| match *event {
            census::Event::None => Event::None,
            census::Event::Allocate(a) => Event::Allocate {
                allocation: self.census.allocations[a].operation,
            },
            census::Event::Project(a) => Event::Project {
                allocation: self.census.allocations[a].operation,
            },
            census::Event::Read(leaf) => Event::Read {
                leaf,
                output: self.inventory.operations()[operation].operation.results[0].id,
            },
            census::Event::Write(leaf, value) => Event::Write { leaf, value },
        })
    }
    /// Always false: this is an original typed census, not proof or admission.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}
