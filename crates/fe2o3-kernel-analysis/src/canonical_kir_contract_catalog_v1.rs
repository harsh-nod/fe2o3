//! Exact definition binding for native import catalogs; not source authentication.

use std::{error::Error, fmt, mem::size_of};

use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirDefinitionCoordinateV1 as Definition,
    CanonicalKirFunctionCoordinateV1 as FunctionCoordinate,
    InertCanonicalKernelIrContractCatalogV1 as Catalog,
    KernelIrPipelineStorageBindingV1 as Binding, OperationKind, ScalarType, Type, ValueId,
    VerificationContractOperationV12, WorkgroupMemoryExtent,
};

use crate::{CanonicalKirInventoryErrorV1, CanonicalKirInventoryV1 as Inventory};

/// Exact immutable catalog and graph borrowed after structural binding checks.
/// This does not authenticate source claims or prove pipeline lifecycle safety.
#[derive(Debug)]
pub struct CheckedKernelIrContractCatalogV1<'a, 'g> {
    inventory: &'a Inventory<'g>,
    catalog: &'a Catalog,
    markers: usize,
}

impl<'a, 'g> CheckedKernelIrContractCatalogV1<'a, 'g> {
    /// Borrows the exact owner-bound graph inventory checked here.
    pub const fn inventory(&self) -> &'a Inventory<'g> {
        self.inventory
    }
    /// Borrows the exact immutable catalog checked here.
    pub const fn catalog(&self) -> &'a Catalog {
        self.catalog
    }
    /// Returns the number of actual markers whose key/storage/epoch were checked.
    pub const fn marker_count(&self) -> usize {
        self.markers
    }
    /// Graph binding grants no semantic-source, proof, or launch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Inline retained checked-view payload, excluding caller-owned graph and catalog.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KernelIrContractCatalogBindingStorageV1(usize);
impl KernelIrContractCatalogBindingStorageV1 {
    /// Reserve while the checked view lives before another ledger allocation.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Rejected catalog definition or marker binding, with bounded diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KernelIrContractCatalogBindingErrorV1 {
    /// Metered inventory lookup failed.
    Inventory(CanonicalKirInventoryErrorV1),
    /// Explicit logical work or storage admission failed.
    Resource(Resource),
    /// A closed definition or marker binding rule failed.
    Invalid(&'static str),
}
impl From<Resource> for KernelIrContractCatalogBindingErrorV1 {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl From<CanonicalKirInventoryErrorV1> for KernelIrContractCatalogBindingErrorV1 {
    fn from(error: CanonicalKirInventoryErrorV1) -> Self {
        Self::Inventory(error)
    }
}
impl fmt::Display for KernelIrContractCatalogBindingErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Inventory(error) => error.fmt(formatter),
            Self::Resource(error) => error.fmt(formatter),
            Self::Invalid(rule) => write!(
                formatter,
                "native KIR catalog graph binding rejected: {rule}"
            ),
        }
    }
}
impl Error for KernelIrContractCatalogBindingErrorV1 {}
type BindingError = KernelIrContractCatalogBindingErrorV1;

/// Resolves every import allocation and every actual graph marker against the
/// immutable catalog, including exact pointer type, geometry, source packed
/// layout, epoch type, and function-local value identity.
///
/// Work is O(A log V + O + M(log A + log V)), where A is bindings, V definitions,
/// O operations and M <= O pipeline markers;
/// no raw sparse ID determines an allocation. The existing inventory is borrowed,
/// not rebuilt. Scratch is empty; only a checked-view inline receipt transfers.
/// This check does not validate source metadata or prove event-order correctness.
pub fn check_kernel_ir_contract_catalog_v1<'a, 'g>(
    inventory: &'a Inventory<'g>,
    catalog: &'a Catalog,
    budget: &mut Budget<'_>,
) -> Result<
    (
        CheckedKernelIrContractCatalogV1<'a, 'g>,
        KernelIrContractCatalogBindingStorageV1,
    ),
    BindingError,
> {
    for binding in catalog.bindings() {
        budget.charge_work(1)?;
        let definition = catalog
            .definitions()
            .get(binding.key as usize)
            .ok_or(BindingError::Invalid("allocation contract key"))?;
        let function = FunctionCoordinate(binding.function);
        let actual = inventory
            .definition_for_value(function, ValueId(binding.storage), budget)?
            .ok_or(BindingError::Invalid("allocation value"))?;
        let Definition::Result {
            operation,
            result: 0,
        } = actual.coordinate
        else {
            return Err(BindingError::Invalid("storage is not an allocation result"));
        };
        if operation.block.function != function
            || operation.block.block != binding.block
            || operation.operation != binding.operation
        {
            return Err(BindingError::Invalid("allocation occurrence"));
        }
        let function_ref = inventory
            .functions()
            .get(binding.function as usize)
            .ok_or(BindingError::Invalid("allocation function"))?;
        let blocks = inventory
            .blocks()
            .get(function_ref.blocks.clone())
            .ok_or(BindingError::Invalid("allocation function range"))?;
        let block = blocks
            .get(binding.block as usize)
            .ok_or(BindingError::Invalid("allocation block"))?;
        let operations = inventory
            .operations()
            .get(block.operations.clone())
            .ok_or(BindingError::Invalid("allocation block range"))?;
        let operation_ref = operations
            .get(binding.operation as usize)
            .ok_or(BindingError::Invalid("allocation operation"))?;
        if operation_ref.coordinate != operation || operation_ref.operation.results.len() != 1 {
            return Err(BindingError::Invalid("allocation result arity"));
        }
        let packed = packed_scalar(definition.packed_bits)?;
        let Type::Pointer(pointer) = actual.ty else {
            return Err(BindingError::Invalid("allocation pointer"));
        };
        if pointer.address_space != AddressSpace::Workgroup
            || pointer.access != AccessMode::ReadWrite
            || *pointer.pointee != Type::Scalar(packed)
        {
            return Err(BindingError::Invalid("allocation pointer type"));
        }
        let OperationKind::WorkgroupMemory(memory) = &operation_ref.operation.kind else {
            return Err(BindingError::Invalid("allocation producer kind"));
        };
        let extent = definition
            .elements
            .checked_mul(u64::from(definition.buffers))
            .and_then(|n| u32::try_from(n).ok())
            .ok_or(BindingError::Invalid("allocation extent overflow"))?;
        let alignment = definition
            .source_alignment_bytes
            .max(definition.source_size_bytes);
        if memory.element != Type::Scalar(packed)
            || memory.extent != WorkgroupMemoryExtent::Static(extent)
            || u64::from(memory.alignment) != alignment
        {
            return Err(BindingError::Invalid("allocation geometry or layout"));
        }
    }
    let mut markers = 0_usize;
    for operation in inventory.operations() {
        budget.charge_work(1)?;
        let OperationKind::VerificationContract(
            VerificationContractOperationV12::WorkgroupPipelineEvent {
                contract,
                storage,
                epoch,
                ..
            },
        ) = operation.operation.kind
        else {
            continue;
        };
        if !operation.operation.results.is_empty() {
            return Err(BindingError::Invalid("marker has results"));
        }
        let function = operation.coordinate.block.function;
        let binding = find_binding(catalog.bindings(), function.0, storage.0, budget)?
            .ok_or(BindingError::Invalid("marker storage has no binding"))?;
        if binding.key != contract.index() {
            return Err(BindingError::Invalid(
                "marker contract differs from storage",
            ));
        }
        let epoch_definition = inventory
            .definition_for_value(function, epoch, budget)?
            .ok_or(BindingError::Invalid("marker epoch definition"))?;
        if epoch_definition.ty != &Type::Scalar(ScalarType::Index) {
            return Err(BindingError::Invalid("marker epoch type"));
        }
        markers = markers
            .checked_add(1)
            .ok_or(BindingError::Invalid("marker count overflow"))?;
    }
    let retained = size_of::<CheckedKernelIrContractCatalogV1<'_, '_>>();
    budget.reserve_storage(retained)?;
    budget.release_storage(retained)?;
    Ok((
        CheckedKernelIrContractCatalogV1 {
            inventory,
            catalog,
            markers,
        },
        KernelIrContractCatalogBindingStorageV1(retained),
    ))
}

fn packed_scalar(bits: u16) -> Result<ScalarType, BindingError> {
    match bits {
        8 => Ok(ScalarType::U8),
        16 => Ok(ScalarType::U16),
        32 => Ok(ScalarType::U32),
        64 => Ok(ScalarType::U64),
        128 => Ok(ScalarType::U128),
        _ => Err(BindingError::Invalid("packed scalar width")),
    }
}

fn find_binding<'a>(
    bindings: &'a [Binding],
    function: u32,
    storage: u32,
    budget: &mut Budget<'_>,
) -> Result<Option<&'a Binding>, BindingError> {
    let mut low = 0;
    let mut high = bindings.len();
    while low < high {
        budget.charge_work(1)?;
        let middle = low + (high - low) / 2;
        let row = &bindings[middle];
        match (row.function, row.storage).cmp(&(function, storage)) {
            std::cmp::Ordering::Less => low = middle + 1,
            std::cmp::Ordering::Greater => high = middle,
            std::cmp::Ordering::Equal => return Ok(Some(row)),
        }
    }
    Ok(None)
}

#[cfg(test)]
#[path = "canonical_kir_contract_catalog_v1_tests.rs"]
mod tests;
