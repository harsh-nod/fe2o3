//! Local physical occurrences over one exact inventory and checked catalog.
//! No source correspondence, memory-safety, lifecycle or transitive-call authority.

use std::{error::Error as StdError, fmt, mem::size_of, ops::Range};

use fe2o3_kernel_ir::{
    AddressSpace, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    InertCanonicalKernelIrContractCatalogV1 as Catalog, KirLocalMemoryEffectRefV1 as Effect,
    OperationKind, Type, VerificationContractOperationV12,
};

use crate::{
    CanonicalKirInventoryV1 as Inventory, CanonicalKirKernelRefV1, CanonicalKirMustAliasErrorV1,
    CanonicalKirMustAliasLimitsV1, CanonicalKirMustAliasV1, CanonicalKirOperationRefV1,
    CanonicalKirUseRefV1, CheckedKernelIrContractCatalogV1, KernelIrContractCatalogBindingErrorV1,
};

#[path = "canonical_kir_physical_occurrences_v1/operands.rs"]
mod operands;
use operands::{allocation_index, binding_index, pointer_address, pointer_role};

/// Unresolved WG pointer provenance, not evidence of disjointness or no access.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalKirPhysicalAddressIssueV1 {
    UnknownOrigin,
    UnsupportedProducer,
    UnsupportedOffsetBase,
}

/// An allocation-relative address observation, not a bounds or equality proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalKirPhysicalAddressV1 {
    Allocation(usize),
    /// One actual GEP; the offset is a dense original use, never evaluated here.
    ElementOffset {
        allocation: usize,
        operation: usize,
        offset_use: usize,
    },
    /// Other address spaces, including Generic, are outside this WG origin model.
    OutsideWorkgroupModel(AddressSpace),
    Unresolved(CanonicalKirPhysicalAddressIssueV1),
}

/// An explicitly unsupported use for allocation/alias escape closure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalKirPhysicalEscapeV1 {
    Return,
    Call,
    StoredValue,
    OpaqueAssembly,
    UnsupportedUse,
}

/// Exact operand role. Forwarding is agreed only for one plain storage origin.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalKirPhysicalPointerRoleV1 {
    AccessAddress,
    ContractStorage,
    ElementBase {
        result: usize,
    },
    Forwarding {
        target_definition: usize,
        edge_argument: Option<usize>,
        same_allocation: bool,
    },
    Escape(CanonicalKirPhysicalEscapeV1),
}

/// Every top-level pointer-typed use, in exact inventory use order.
#[derive(Debug)]
pub struct CanonicalKirPhysicalPointerUseV1 {
    ordinal: usize,
    use_index: usize,
    address: CanonicalKirPhysicalAddressV1,
    role: CanonicalKirPhysicalPointerRoleV1,
}
impl CanonicalKirPhysicalPointerUseV1 {
    pub const fn use_index(&self) -> usize {
        self.use_index
    }
    pub const fn address(&self) -> CanonicalKirPhysicalAddressV1 {
        self.address
    }
    pub const fn role(&self) -> CanonicalKirPhysicalPointerRoleV1 {
        self.role
    }
}

/// What the borrowed operation can supply to a later footprint extractor.
/// These tags do not compute byte widths, bounds, predicates or memory ordering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalKirPhysicalFootprintV1 {
    TypedAccess,
    VectorAccess,
    RepeatedElements,
    AtomicAccess,
    Unmodeled,
}

/// Every local effect plus contract/call/opaque assembly occurrences.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalKirPhysicalOccurrenceKindV1 {
    /// None means an allocator outside the unchanged WorkgroupMemory model.
    Allocation {
        effect: usize,
        allocation: Option<usize>,
        binding: Option<usize>,
    },
    Access {
        effect: usize,
        pointer_use: Option<usize>,
        footprint: CanonicalKirPhysicalFootprintV1,
    },
    Synchronization {
        effect: usize,
    },
    Contract {
        storage_use: usize,
        epoch_use: usize,
        allocation: usize,
        binding: usize,
    },
    Call {
        call: usize,
    },
    OpaqueAssembly,
}

/// A physical occurrence on this report's exact borrowed inventory.
#[derive(Debug)]
pub struct CanonicalKirPhysicalOccurrenceV1 {
    ordinal: usize,
    operation: usize,
    kind: CanonicalKirPhysicalOccurrenceKindV1,
}
impl CanonicalKirPhysicalOccurrenceV1 {
    pub const fn operation_index(&self) -> usize {
        self.operation
    }
    pub const fn kind(&self) -> CanonicalKirPhysicalOccurrenceKindV1 {
        self.kind
    }
}

#[derive(Debug)]
struct FunctionRanges {
    physical: Range<usize>,
    pointers: Range<usize>,
}

/// Allocation-free slices for one actual function; calls are not expanded.
#[derive(Debug)]
pub struct CanonicalKirPhysicalFunctionViewV1<'a> {
    function: u32,
    physical: &'a [CanonicalKirPhysicalOccurrenceV1],
    pointers: &'a [CanonicalKirPhysicalPointerUseV1],
}
impl<'a> CanonicalKirPhysicalFunctionViewV1<'a> {
    pub const fn function_ordinal(&self) -> u32 {
        self.function
    }
    pub const fn occurrences(&self) -> &'a [CanonicalKirPhysicalOccurrenceV1] {
        self.physical
    }
    pub const fn pointer_uses(&self) -> &'a [CanonicalKirPhysicalPointerUseV1] {
        self.pointers
    }
}

/// The actual module kernel and its entry, not a source or ranked-root join.
#[derive(Debug)]
pub struct CanonicalKirPhysicalKernelEntryViewV1<'a, 'g> {
    kernel: &'a CanonicalKirKernelRefV1<'g>,
    entry: CanonicalKirPhysicalFunctionViewV1<'a>,
}
impl<'a, 'g> CanonicalKirPhysicalKernelEntryViewV1<'a, 'g> {
    pub const fn kernel(&self) -> &'a CanonicalKirKernelRefV1<'g> {
        self.kernel
    }
    pub const fn entry(&self) -> &CanonicalKirPhysicalFunctionViewV1<'a> {
        &self.entry
    }
}

/// One module-wide analysis borrowing existing graph/catalog ownership.
/// All syntactic occurrences survive, including unreachable blocks and helpers.
/// Unknown addresses, escapes, calls and opaque assembly remain explicit. The
/// temporary existing must-alias solver is run at most once, not once per kernel.
///
/// A report cannot outlive its borrowed inventory and catalog:
///
/// ```compile_fail
/// use fe2o3_kernel_analysis::{
///     CanonicalKirInventoryV1, CanonicalKirPhysicalOccurrencesV1,
///     CheckedKernelIrContractCatalogV1,
/// };
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
///
/// fn escape<'a, 'g>(
///     inventory: &'a CanonicalKirInventoryV1<'g>,
///     catalog: &CheckedKernelIrContractCatalogV1<'a, 'g>,
///     budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
/// ) -> CanonicalKirPhysicalOccurrencesV1<'static, 'static> {
///     CanonicalKirPhysicalOccurrencesV1::derive(inventory, catalog, budget).unwrap().0
/// }
/// ```
#[derive(Debug)]
pub struct CanonicalKirPhysicalOccurrencesV1<'a, 'g> {
    inventory: &'a Inventory<'g>,
    catalog: &'a Catalog,
    functions: Vec<FunctionRanges>,
    physical: Vec<CanonicalKirPhysicalOccurrenceV1>,
    pointers: Vec<CanonicalKirPhysicalPointerUseV1>,
    retained: usize,
}

/// Logical report header and exact requested row payloads; graph/catalog are
/// caller-owned. Reserve before later controlled allocation, release after drop.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirPhysicalOccurrenceStorageV1(usize);
impl CanonicalKirPhysicalOccurrenceStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalKirPhysicalOccurrenceErrorV1 {
    Resource(Resource),
    Alias(CanonicalKirMustAliasErrorV1),
    Catalog(KernelIrContractCatalogBindingErrorV1),
    InventoryMismatch,
    InconsistentInventory,
    InvalidCoordinate,
    ForeignOccurrence,
    ForeignPointerUse,
}
type Error = CanonicalKirPhysicalOccurrenceErrorV1;
type Result<T> = std::result::Result<T, Error>;
impl From<Resource> for Error {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl From<CanonicalKirMustAliasErrorV1> for Error {
    fn from(value: CanonicalKirMustAliasErrorV1) -> Self {
        match value {
            CanonicalKirMustAliasErrorV1::Resource(error) => Self::Resource(error),
            error => Self::Alias(error),
        }
    }
}
impl From<KernelIrContractCatalogBindingErrorV1> for Error {
    fn from(value: KernelIrContractCatalogBindingErrorV1) -> Self {
        match value {
            KernelIrContractCatalogBindingErrorV1::Resource(error) => Self::Resource(error),
            error => Self::Catalog(error),
        }
    }
}
impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(formatter),
            Self::Alias(error) => error.fmt(formatter),
            Self::Catalog(error) => error.fmt(formatter),
            Self::InventoryMismatch => {
                formatter.write_str("physical occurrence inventory mismatch")
            }
            Self::InconsistentInventory => {
                formatter.write_str("inconsistent physical occurrence inventory")
            }
            Self::InvalidCoordinate => {
                formatter.write_str("invalid physical occurrence coordinate")
            }
            Self::ForeignOccurrence => formatter.write_str("foreign physical occurrence row"),
            Self::ForeignPointerUse => formatter.write_str("foreign physical pointer-use row"),
        }
    }
}
impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Alias(error) => Some(error),
            Self::Catalog(error) => Some(error),
            _ => None,
        }
    }
}

impl<'a, 'g> CanonicalKirPhysicalOccurrencesV1<'a, 'g> {
    /// One precharged structural census and fill, plus the existing bounded
    /// must-alias solve and numeric catalog lookups. No names/types are copied.
    /// Storage is the header plus three exact row vectors. Solver scratch is
    /// dropped before transferring the report. Every returned path restores the
    /// caller's storage floor; accepted work/peak/first failures are preserved.
    /// This Result-floor contract is not standalone panic recovery or RSS metering.
    pub fn derive(
        inventory: &'a Inventory<'g>,
        checked_catalog: &CheckedKernelIrContractCatalogV1<'a, 'g>,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, CanonicalKirPhysicalOccurrenceStorageV1)> {
        let floor = budget.storage();
        let result = Self::build(inventory, checked_catalog, budget);
        let release = budget
            .storage()
            .checked_sub(floor)
            .ok_or(Resource::Accounting)?;
        budget.release_storage(release)?;
        result.map(|report| {
            let retained = report.retained;
            (report, CanonicalKirPhysicalOccurrenceStorageV1(retained))
        })
    }

    pub const fn inventory(&self) -> &'a Inventory<'g> {
        self.inventory
    }
    pub const fn catalog(&self) -> &'a Catalog {
        self.catalog
    }
    pub fn occurrences(&self) -> &[CanonicalKirPhysicalOccurrenceV1] {
        &self.physical
    }
    pub fn pointer_uses(&self) -> &[CanonicalKirPhysicalPointerUseV1] {
        &self.pointers
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }

    /// Three prepaid bounded coordinate/range reads; no heap allocation.
    pub fn function(
        &self,
        ordinal: u32,
        budget: &mut Budget<'_>,
    ) -> Result<CanonicalKirPhysicalFunctionViewV1<'_>> {
        budget.charge_work(3)?;
        let range = self
            .functions
            .get(ordinal as usize)
            .ok_or(Error::InvalidCoordinate)?;
        Ok(CanonicalKirPhysicalFunctionViewV1 {
            function: ordinal,
            physical: self
                .physical
                .get(range.physical.clone())
                .ok_or(Error::InconsistentInventory)?,
            pointers: self
                .pointers
                .get(range.pointers.clone())
                .ok_or(Error::InconsistentInventory)?,
        })
    }

    /// Six prepaid bounded kernel/function/range reads; no callee traversal.
    pub fn kernel_entry(
        &self,
        ordinal: u32,
        budget: &mut Budget<'_>,
    ) -> Result<CanonicalKirPhysicalKernelEntryViewV1<'_, 'g>> {
        budget.charge_work(3)?;
        let kernel = self
            .inventory
            .kernels()
            .get(ordinal as usize)
            .ok_or(Error::InvalidCoordinate)?;
        let entry = self.function(kernel.entry.0, budget)?;
        Ok(CanonicalKirPhysicalKernelEntryViewV1 { kernel, entry })
    }

    /// Three units; only an exact borrowed row from this report is accepted.
    pub fn operation_for(
        &self,
        row: &CanonicalKirPhysicalOccurrenceV1,
        budget: &mut Budget<'_>,
    ) -> Result<&'a CanonicalKirOperationRefV1<'g>> {
        budget.charge_work(3)?;
        if !self
            .physical
            .get(row.ordinal)
            .is_some_and(|actual| std::ptr::eq(actual, row))
        {
            return Err(Error::ForeignOccurrence);
        }
        self.inventory
            .operations()
            .get(row.operation)
            .ok_or(Error::InconsistentInventory)
    }

    /// Three units; same-shaped rows from another report are not accepted.
    pub fn use_for(
        &self,
        row: &CanonicalKirPhysicalPointerUseV1,
        budget: &mut Budget<'_>,
    ) -> Result<&'a CanonicalKirUseRefV1> {
        budget.charge_work(3)?;
        if !self
            .pointers
            .get(row.ordinal)
            .is_some_and(|actual| std::ptr::eq(actual, row))
        {
            return Err(Error::ForeignPointerUse);
        }
        self.inventory
            .uses()
            .get(row.use_index)
            .ok_or(Error::InconsistentInventory)
    }

    fn build(
        inventory: &'a Inventory<'g>,
        checked: &CheckedKernelIrContractCatalogV1<'a, 'g>,
        budget: &mut Budget<'_>,
    ) -> Result<Self> {
        budget.charge_work(16)?;
        if !std::ptr::eq(inventory, checked.inventory()) {
            return Err(Error::InventoryMismatch);
        }
        let counts = Counts::census(inventory, budget)?;
        let (aliases, alias_storage) = if counts.workgroup_pointers != 0 {
            let (aliases, storage) = CanonicalKirMustAliasV1::derive(
                inventory,
                CanonicalKirMustAliasLimitsV1::default(),
                budget,
            )?;
            budget.reserve_storage(storage.retained_storage())?;
            (Some(aliases), storage.retained_storage())
        } else {
            (None, 0)
        };
        let function_bytes = bytes::<FunctionRanges>(counts.functions)?;
        let physical_bytes = bytes::<CanonicalKirPhysicalOccurrenceV1>(counts.physical)?;
        let pointer_bytes = bytes::<CanonicalKirPhysicalPointerUseV1>(counts.pointers)?;
        let retained = size_of::<Self>()
            .checked_add(function_bytes)
            .and_then(|n| n.checked_add(physical_bytes))
            .and_then(|n| n.checked_add(pointer_bytes))
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(size_of::<Self>())?;
        let mut report = Self {
            inventory,
            catalog: checked.catalog(),
            functions: reserve(counts.functions, budget)?,
            physical: reserve(counts.physical, budget)?,
            pointers: reserve(counts.pointers, budget)?,
            retained,
        };
        report.fill(&counts, aliases.as_ref(), budget)?;
        budget.charge_work(4)?;
        if report.functions.len() != counts.functions
            || report.physical.len() != counts.physical
            || report.pointers.len() != counts.pointers
            || counts.markers != checked.marker_count()
        {
            return Err(Error::InconsistentInventory);
        }
        drop(aliases);
        budget.release_storage(alias_storage)?;
        Ok(report)
    }

    fn fill(
        &mut self,
        counts: &Counts,
        aliases: Option<&CanonicalKirMustAliasV1<'a, 'g>>,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        let mut call = 0;
        for function in self.inventory.functions() {
            budget.charge_work(1)?;
            let physical = self.physical.len();
            let pointers = self.pointers.len();
            let blocks = self
                .inventory
                .blocks()
                .get(function.blocks.clone())
                .ok_or(Error::InconsistentInventory)?;
            for block in blocks {
                budget.charge_work(1)?;
                for operation in block.operations.clone() {
                    budget.charge_work(1)?;
                    let op = self
                        .inventory
                        .operations()
                        .get(operation)
                        .ok_or(Error::InconsistentInventory)?;
                    let mut addresses = [None; 2];
                    let mut storage_pointer = None;
                    for use_index in op.operands.clone() {
                        if let Some(pointer) =
                            self.push_pointer(use_index, counts, aliases, budget)?
                        {
                            let ordinal = use_index - op.operands.start;
                            if let Some(slot) = operands::address_slot(&op.operation.kind, ordinal)
                            {
                                addresses[slot] = Some(pointer);
                            }
                            if ordinal == 0 {
                                storage_pointer = Some(pointer);
                            }
                        }
                    }
                    for effect in op.effects.clone() {
                        budget.charge_work(4)?;
                        let row = self
                            .inventory
                            .effects()
                            .get(effect)
                            .ok_or(Error::InconsistentInventory)?;
                        let kind = match row.effect {
                            Effect::Allocate(_) => {
                                let allocation = allocation_index(self.inventory, operation)?;
                                let binding = allocation
                                    .map(|allocation| {
                                        binding_index(
                                            self.inventory,
                                            self.catalog,
                                            allocation,
                                            budget,
                                        )
                                    })
                                    .transpose()?
                                    .flatten();
                                CanonicalKirPhysicalOccurrenceKindV1::Allocation {
                                    effect,
                                    allocation,
                                    binding,
                                }
                            }
                            Effect::Synchronize { .. } | Effect::Fence { .. } => {
                                CanonicalKirPhysicalOccurrenceKindV1::Synchronization { effect }
                            }
                            Effect::Read(_)
                            | Effect::Write(_)
                            | Effect::VolatileRead(_)
                            | Effect::VolatileWrite(_)
                            | Effect::Atomic { .. } => {
                                let relative = effect - op.effects.start;
                                CanonicalKirPhysicalOccurrenceKindV1::Access {
                                    effect,
                                    pointer_use: addresses.get(relative).copied().flatten(),
                                    footprint: operands::footprint(&op.operation.kind),
                                }
                            }
                        };
                        self.push_physical(operation, kind, counts)?;
                    }
                    match &op.operation.kind {
                        OperationKind::VerificationContract(
                            VerificationContractOperationV12::WorkgroupPipelineEvent {
                                contract,
                                ..
                            },
                        ) => {
                            budget.charge_work(4)?;
                            let pointer = storage_pointer
                                .and_then(|index| self.pointers.get(index))
                                .ok_or(Error::InconsistentInventory)?;
                            let CanonicalKirPhysicalAddressV1::Allocation(allocation) =
                                pointer.address
                            else {
                                return Err(Error::InconsistentInventory);
                            };
                            let binding =
                                binding_index(self.inventory, self.catalog, allocation, budget)?
                                    .ok_or(Error::InconsistentInventory)?;
                            if self.catalog.bindings()[binding].key != contract.index()
                                || op.operands.len() != 2
                            {
                                return Err(Error::InconsistentInventory);
                            }
                            self.push_physical(
                                operation,
                                CanonicalKirPhysicalOccurrenceKindV1::Contract {
                                    storage_use: op.operands.start,
                                    epoch_use: op.operands.start + 1,
                                    allocation,
                                    binding,
                                },
                                counts,
                            )?;
                        }
                        OperationKind::Call { .. } => {
                            budget.charge_work(4)?;
                            let actual = self
                                .inventory
                                .calls()
                                .get(call)
                                .ok_or(Error::InconsistentInventory)?;
                            if actual.coordinate != op.coordinate
                                || !std::ptr::eq(actual.operation, op.operation)
                            {
                                return Err(Error::InconsistentInventory);
                            }
                            self.push_physical(
                                operation,
                                CanonicalKirPhysicalOccurrenceKindV1::Call { call },
                                counts,
                            )?;
                            call += 1;
                        }
                        OperationKind::InlineAssembly(_) => {
                            budget.charge_work(4)?;
                            self.push_physical(
                                operation,
                                CanonicalKirPhysicalOccurrenceKindV1::OpaqueAssembly,
                                counts,
                            )?;
                        }
                        _ => {}
                    }
                }
                for use_index in block.terminator_uses.clone() {
                    self.push_pointer(use_index, counts, aliases, budget)?;
                }
            }
            append(
                &mut self.functions,
                counts.functions,
                FunctionRanges {
                    physical: physical..self.physical.len(),
                    pointers: pointers..self.pointers.len(),
                },
            )?;
        }
        budget.charge_work(1)?;
        if call != self.inventory.calls().len() {
            return Err(Error::InconsistentInventory);
        }
        Ok(())
    }

    fn push_pointer(
        &mut self,
        use_index: usize,
        counts: &Counts,
        aliases: Option<&CanonicalKirMustAliasV1<'a, 'g>>,
        budget: &mut Budget<'_>,
    ) -> Result<Option<usize>> {
        budget.charge_work(1)?;
        let used = self
            .inventory
            .uses()
            .get(use_index)
            .ok_or(Error::InconsistentInventory)?;
        let ty = self
            .inventory
            .definitions()
            .get(used.definition)
            .ok_or(Error::InconsistentInventory)?
            .ty;
        let Type::Pointer(pointer) = ty else {
            return Ok(None);
        };
        budget.charge_work(8)?;
        let address = pointer_address(
            self.inventory,
            used.definition,
            pointer.address_space,
            aliases,
            budget,
        )?;
        let role = pointer_role(self.inventory, used, address, aliases, budget)?;
        let ordinal = self.pointers.len();
        append(
            &mut self.pointers,
            counts.pointers,
            CanonicalKirPhysicalPointerUseV1 {
                ordinal,
                use_index,
                address,
                role,
            },
        )?;
        Ok(Some(ordinal))
    }

    fn push_physical(
        &mut self,
        operation: usize,
        kind: CanonicalKirPhysicalOccurrenceKindV1,
        counts: &Counts,
    ) -> Result<()> {
        let ordinal = self.physical.len();
        append(
            &mut self.physical,
            counts.physical,
            CanonicalKirPhysicalOccurrenceV1 {
                ordinal,
                operation,
                kind,
            },
        )
    }
}

struct Counts {
    functions: usize,
    physical: usize,
    pointers: usize,
    workgroup_pointers: usize,
    markers: usize,
}
impl Counts {
    fn census(inventory: &Inventory<'_>, budget: &mut Budget<'_>) -> Result<Self> {
        let mut counts = Self {
            functions: inventory.functions().len(),
            physical: inventory.effects().len(),
            pointers: 0,
            workgroup_pointers: 0,
            markers: 0,
        };
        for operation in inventory.operations() {
            budget.charge_work(1)?;
            match operation.operation.kind {
                OperationKind::VerificationContract(_) => {
                    counts.markers = add(counts.markers, 1)?;
                    counts.physical = add(counts.physical, 1)?;
                }
                OperationKind::Call { .. } | OperationKind::InlineAssembly(_) => {
                    counts.physical = add(counts.physical, 1)?;
                }
                _ => {}
            }
        }
        for used in inventory.uses() {
            budget.charge_work(1)?;
            let ty = inventory
                .definitions()
                .get(used.definition)
                .ok_or(Error::InconsistentInventory)?
                .ty;
            if let Type::Pointer(pointer) = ty {
                counts.pointers = add(counts.pointers, 1)?;
                if pointer.address_space == AddressSpace::Workgroup {
                    counts.workgroup_pointers = add(counts.workgroup_pointers, 1)?;
                }
            }
        }
        Ok(counts)
    }
}

fn add(left: usize, right: usize) -> Result<usize> {
    left.checked_add(right).ok_or(Resource::Arithmetic.into())
}
fn bytes<T>(count: usize) -> Result<usize> {
    count
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic.into())
}
fn reserve<T>(count: usize, budget: &mut Budget<'_>) -> Result<Vec<T>> {
    if count == 0 {
        return Ok(Vec::new());
    }
    budget.charge_work(1)?;
    budget.reserve_storage(bytes::<T>(count)?)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| Resource::Allocation)?;
    if result.capacity() != count {
        return Err(Resource::Allocation.into());
    }
    Ok(result)
}
fn append<T>(rows: &mut Vec<T>, count: usize, row: T) -> Result<()> {
    if rows.len() >= count {
        return Err(Error::InconsistentInventory);
    }
    rows.push(row);
    Ok(())
}

#[cfg(test)]
#[path = "canonical_kir_physical_occurrences_v1/tests.rs"]
mod tests;
