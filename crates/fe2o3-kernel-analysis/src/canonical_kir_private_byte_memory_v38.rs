//! Sparse physical byte initialization. Original source values, dynamic
//! lifetimes and pointer provenance remain separate consumer obligations.
use super::*;
use crate::CanonicalKirInventoryV18 as Inventory;
use fe2o3_kernel_ir::StorageLayoutIdV1 as LayoutId;

#[path = "canonical_kir_private_byte_cfg_v38.rs"]
mod cfg;
#[path = "canonical_kir_private_byte_geometry_v38.rs"]
mod geometry;
#[path = "canonical_kir_private_byte_intervals_v38.rs"]
mod intervals;

/// Structural sparsity ceiling, independent of the existing work/storage ledger.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirPrivateByteLimitsV38 {
    /// Maximum retained partition boundaries, not allocated object bytes.
    pub max_boundaries: usize,
}

macro_rules! obligations {
    ($( $(#[$doc:meta])* $variant:ident, )*) => {
        /// An unresolved requirement. No unknown requirement is a successful proof.
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub enum CanonicalKirPrivateByteObligationV38 {
            $( $(#[$doc])* $variant, )*
        }
        impl CanonicalKirPrivateByteObligationV38 {
            const ALL: &'static [Self] = &[$(Self::$variant,)*];
            /// Fixed number of flags scanned by the allocation-free visitor.
            pub const COUNT: usize = Self::ALL.len();
        }
        const _: () = assert!(CanonicalKirPrivateByteObligationV38::COUNT <= u16::BITS as usize);
    };
}

// One declaration generates both the enum and visitation census. New flags
// cannot be added without entering the visitor's source of truth.
obligations! {
    /// Exact source of the physical address is not established.
    Address,
    /// Object or projection formation bounds require a separate argument.
    Bounds,
    /// Required access or formation alignment is not established.
    Alignment,
    /// Current dynamic allocation generation cannot be inferred from this SSA value.
    Currentness,
    /// A typed read is not definitely initialized on every incoming path.
    Initialization,
    /// The operation is not reachable from its function entry.
    Reachability,
    /// Memory is external to the private-allocation initialization analysis.
    ExternalMemory,
    /// Complete copy-boundary closure exceeds the explicit structural ceiling.
    CopyBoundaryClosure,
    /// A read/write, ordering, volatility, tag or type requirement is unmodeled.
    Operation,
    /// A projected variant view must remain valid after every intervening write.
    ActiveView,
    /// The selected variant and its tag/encoding need an authenticated contract.
    TagContract,
}
use CanonicalKirPrivateByteObligationV38 as Obligation;

/// A physical byte interval in one exact canonical Alloca occurrence.
/// The occurrence is not a dynamic allocation identity or lifetime grant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirPrivateByteRangeV38 {
    allocation: usize,
    start: u64,
    length: u64,
}
use CanonicalKirPrivateByteRangeV38 as ByteRange;
impl ByteRange {
    /// Original inventory operation ordinal of the private Alloca.
    pub const fn allocation_operation(&self) -> usize {
        self.allocation
    }
    /// Byte offset, never an index in the legacy V1 scalar-cell domain.
    pub const fn byte_start(&self) -> u64 {
        self.start
    }
    /// Number of bytes, including zero-sized formation ranges.
    pub const fn byte_length(&self) -> u64 {
        self.length
    }
    fn end(self) -> R<u64> {
        self.start.checked_add(self.length).ok_or_else(arithmetic)
    }
}

/// Exact private address geometry. It carries no source value equivalence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirPrivateByteAddressV38 {
    range: ByteRange,
    alignment: u32,
    object: Object,
    elements: u64,
}
use CanonicalKirPrivateByteAddressV38 as ByteAddress;
impl ByteAddress {
    /// Exact original allocation and remaining byte extent of this pointer.
    pub const fn range(&self) -> &ByteRange {
        &self.range
    }
    /// Guaranteed base/offset alignment, not a numeric machine address.
    pub const fn alignment(&self) -> u32 {
        self.alignment
    }
    /// Layout in the exact borrowed inventory owner, when this is a storage object.
    pub const fn storage_layout(&self) -> Option<LayoutId> {
        match self.object {
            Object::Storage(layout) => Some(layout),
            Object::Scalar(_) | Object::Vector(_) => None,
        }
    }
}

/// Meaning of one actual canonical operation in the physical byte analysis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalKirPrivateByteOperationKindV38 {
    /// No private byte memory action is performed.
    None,
    /// A fresh execution of a static Alloca resets its initialization state.
    Allocate,
    /// Pointer formation retains its own obligations even if its extent is zero.
    Project,
    /// Reads precisely the typed payload, excluding trailing padding.
    Read,
    /// Initializes precisely the typed payload, preserving trailing padding.
    Write,
    /// Copies bytes and initializedness from a source snapshot, including padding.
    Copy,
    /// The physical memory behavior requires a separate consumer argument.
    Unmodeled,
}
use CanonicalKirPrivateByteOperationKindV38 as OperationKindV38;

/// Immutable outcome for one exact operation. A consumer must handle every
/// listed obligation; absence of an initialization error alone is insufficient.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirPrivateByteOperationV38 {
    kind: OperationKindV38,
    ranges: [Option<ByteRange>; 2],
    obligations: u16,
    initialized: Option<bool>,
}
use CanonicalKirPrivateByteOperationV38 as Fact;
impl Fact {
    const NONE: Self = Self {
        kind: OperationKindV38::None,
        ranges: [None, None],
        obligations: 0,
        initialized: None,
    };
    /// Actual memory operation class, not a reconstructed source statement.
    pub const fn kind(&self) -> OperationKindV38 {
        self.kind
    }
    /// Source then destination for Copy; the accessed range for other operations.
    pub fn range(&self, ordinal: usize) -> Option<&ByteRange> {
        self.ranges.get(ordinal).and_then(Option::as_ref)
    }
    /// Tests one unresolved condition without allocating diagnostic storage.
    pub const fn requires(&self, condition: Obligation) -> bool {
        self.obligations & (1 << condition as u16) != 0
    }
    /// Whether every physical obligation represented by this analysis was proved.
    /// This does not include original source value, lifetime or native authority.
    pub const fn is_proven(&self) -> bool {
        self.obligations == 0
    }
    /// Final fixed-point initializedness for a reachable typed read with exact
    /// geometry. `None` is unknown or not a typed read, never implicit success.
    pub const fn payload_initialized(&self) -> Option<bool> {
        self.initialized
    }
    /// Visits every unresolved obligation exactly once, without allocation.
    /// Consumers account for `CanonicalKirPrivateByteObligationV38::COUNT` flag
    /// checks and must exhaustively discharge or refuse each emitted variant.
    pub fn try_visit_obligations<E>(
        &self,
        mut visitor: impl FnMut(Obligation) -> Result<(), E>,
    ) -> Result<(), E> {
        for condition in Obligation::ALL {
            if self.requires(*condition) {
                visitor(*condition)?;
            }
        }
        Ok(())
    }
    fn require(&mut self, condition: Obligation) {
        self.obligations |= 1 << condition as u16;
    }
}

/// Owner-borrowed physical facts. The nominal type cannot be substituted for
/// the stronger legacy scalar/exact-store or original source lifetime receipts.
pub struct CanonicalKirPrivateByteAnalysisV38<'a, 'g> {
    inventory: &'a Inventory<'g>,
    definitions: Vec<Option<ByteAddress>>,
    operations: Vec<Fact>,
}
use CanonicalKirPrivateByteAnalysisV38 as Analysis;
impl<'a, 'g> Analysis<'a, 'g> {
    /// Exact immutable inventory from which all byte coordinates were derived.
    pub const fn inventory(&self) -> &'a Inventory<'g> {
        self.inventory
    }
    /// Checks nominal inventory ownership, not merely equal serialized contents.
    pub fn is_for(&self, inventory: &Inventory<'_>) -> bool {
        std::ptr::eq(self.inventory, inventory)
    }
    /// Allocation-free exact definition lookup. Consumers account for queries.
    pub fn address(&self, definition: usize) -> Option<&ByteAddress> {
        self.definitions.get(definition).and_then(Option::as_ref)
    }
    /// Allocation-free actual operation lookup, including unresolved obligations.
    pub fn operation(&self, operation: usize) -> Option<&Fact> {
        self.operations.get(operation)
    }
    /// Physical initialization never grants source, native, artifact or launch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
    fn retained_storage(&self) -> R<usize> {
        size_of::<Self>()
            .checked_add(capacity_bytes(&self.definitions)?)
            .and_then(|bytes| bytes.checked_add(capacity_bytes(&self.operations).ok()?))
            .ok_or_else(arithmetic)
    }
}

/// Unreserved transfer of the actual retained result header and backing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirPrivateByteStorageV38(usize);
impl CanonicalKirPrivateByteStorageV38 {
    /// Reserve immediately before using the returned analysis, as for V1/V18.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Analyzes exact V18 private byte geometry and initialization. Unknown outcomes
/// remain explicit per-operation obligations, not successful initialized reads.
/// The existing scoped ledger contract drops scratch before refund and returns
/// an unreserved receipt for the retained result. No caller-supplied graph or
/// layout table is accepted, and legacy scalar consumers are unchanged.
pub fn analyze_canonical_kir_private_bytes_v38<'a, 'g>(
    inventory: &'a Inventory<'g>,
    limits: CanonicalKirPrivateByteLimitsV38,
    budget: &mut Budget<'_>,
) -> R<(Analysis<'a, 'g>, CanonicalKirPrivateByteStorageV38)> {
    scoped(budget, |budget| {
        charge(budget, 32)?;
        budget.reserve_storage(headers()?)?;
        let mut census = geometry::derive(inventory, budget)?;
        let partitions = intervals::derive(&mut census, limits.max_boundaries, budget)?;
        cfg::analyze(
            inventory,
            &census.events,
            &partitions,
            &mut census.facts,
            budget,
        )?;
        let analysis = Analysis {
            inventory,
            definitions: census.addresses,
            operations: census.facts,
        };
        let storage = CanonicalKirPrivateByteStorageV38(analysis.retained_storage()?);
        Ok((analysis, storage))
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Object {
    Scalar(fe2o3_kernel_ir::ScalarType),
    Vector(fe2o3_kernel_ir::FixedVectorTypeV12),
    Storage(LayoutId),
}
#[derive(Clone, Copy)]
struct Allocation {
    operation: usize,
    extent: u64,
}
#[derive(Clone, Copy)]
enum Event {
    None,
    Reset(usize),
    Read(ByteRange),
    Write(ByteRange),
    Copy {
        source: ByteRange,
        destination: ByteRange,
    },
    UnknownWrite,
}
struct Census {
    allocations: Vec<Allocation>,
    addresses: Vec<Option<ByteAddress>>,
    facts: Vec<Fact>,
    events: Vec<Event>,
}

fn filled<T: Copy>(count: usize, value: T, budget: &mut Budget<'_>) -> R<Vec<T>> {
    let mut rows = scratch(count, budget)?;
    charge(budget, count)?;
    rows.resize(count, value);
    Ok(rows)
}

fn headers() -> R<usize> {
    let terms = [
        size_of::<Analysis<'_, '_>>(),
        size_of::<R<Analysis<'_, '_>>>(),
        size_of::<Census>(),
        size_of::<R<Census>>(),
        size_of::<CanonicalKirPrivateByteStorageV38>(),
        size_of::<R<(Analysis<'_, '_>, CanonicalKirPrivateByteStorageV38)>>(),
        size_of::<(&Inventory<'_>, CanonicalKirPrivateByteLimitsV38)>(),
        h::<&mut Budget<'_>>()?,
        h::<&mut Cleanup<'_, '_>>()?,
        h::<
            AssertUnwindSafe<(
                &mut Cleanup<'_, '_>,
                (&Inventory<'_>, CanonicalKirPrivateByteLimitsV38),
            )>,
        >()?,
        h::<Object>()?,
        h::<ByteAddress>()?,
        h::<ByteRange>()?,
        h::<Fact>()?,
        h::<Event>()?,
        h::<Allocation>()?,
        geometry::headers()?,
        intervals::headers()?,
        cfg::headers()?,
    ];
    terms.into_iter().try_fold(0usize, |total, term| {
        total.checked_add(term).ok_or_else(arithmetic)
    })
}

fn h<T>() -> R<usize> {
    size_of::<T>()
        .checked_add(size_of::<R<T>>())
        .ok_or_else(arithmetic)
}

fn header_sum(terms: &[usize]) -> R<usize> {
    terms.iter().try_fold(0usize, |sum, term| {
        sum.checked_add(*term).ok_or_else(arithmetic)
    })
}

fn header_copies<T>(count: usize) -> R<usize> {
    h::<T>()?.checked_mul(count).ok_or_else(arithmetic)
}

#[cfg(test)]
#[path = "canonical_kir_private_byte_memory_v38_tests.rs"]
mod tests;
