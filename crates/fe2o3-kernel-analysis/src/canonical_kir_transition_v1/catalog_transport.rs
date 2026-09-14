//! Checked placement transport for a native catalog, not source authentication.

use std::{error::Error as StdError, fmt, mem::size_of};

use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirBlockCoordinateV1 as Block, CanonicalKirDefinitionCoordinateV1 as Definition,
    CanonicalKirDefinitionDescendantKindV1 as DescendantKind,
    CanonicalKirFunctionCoordinateV1 as Function, CanonicalKirOperationCoordinateV1 as Operation,
    CanonicalKirOperationOriginV1 as Origin, InertCanonicalKernelIrContractCatalogV1 as Catalog,
    KernelIrContractCatalogErrorV1 as CatalogError, KernelIrPipelineStorageBindingV1 as Binding,
    OperationKind, VerificationContractOperationV12,
    VerifiedCanonicalKernelIrIdentityV12 as Identity,
};

use crate::{
    CheckedKernelIrContractCatalogV1, KernelIrContractCatalogBindingErrorV1 as BindingError,
    check_kernel_ir_contract_catalog_v1,
};

use super::{
    CanonicalKirTransitionErrorV1 as TransitionError, CheckedCanonicalKirTransitionV1, index,
};

/// Actual allocation placement before and after the checked transition.
/// A missing output is a checked omission, not a substituted allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KernelIrPipelineAllocationTransportV1 {
    pub input: Binding,
    pub output: Option<Binding>,
}

/// Actual marker occurrence before and after the checked transition.
/// An omitted ordered marker was admitted by the checker's control rules;
/// this record does not assert its source precondition or lifecycle safety.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KernelIrPipelineMarkerTransportV1 {
    pub input: Operation,
    pub output: Option<Operation>,
}

/// A fresh output catalog and inert placement audit for the exact checked graphs.
/// The original catalog remains an input audit, never an output attachment.
#[derive(Debug)]
pub struct TransportedKernelIrContractCatalogV1 {
    input: Identity,
    output: Identity,
    catalog: Catalog,
    allocations: Vec<KernelIrPipelineAllocationTransportV1>,
    markers: Vec<KernelIrPipelineMarkerTransportV1>,
}

impl TransportedKernelIrContractCatalogV1 {
    pub const fn input_identity(&self) -> Identity {
        self.input
    }
    pub const fn output_identity(&self) -> Identity {
        self.output
    }
    pub const fn catalog(&self) -> &Catalog {
        &self.catalog
    }
    pub fn allocations(&self) -> &[KernelIrPipelineAllocationTransportV1] {
        &self.allocations
    }
    pub fn markers(&self) -> &[KernelIrPipelineMarkerTransportV1] {
        &self.markers
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Reserve this transferred logical payload while the result is retained.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KernelIrContractCatalogTransportStorageV1(usize);
impl KernelIrContractCatalogTransportStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KernelIrContractCatalogTransportErrorV1 {
    Resource(Resource),
    Transition(TransitionError),
    Catalog(CatalogError),
    Binding(BindingError),
    /// The catalog view must borrow the exact input inventory, not a lookalike.
    InputInventory,
    /// A transport invariant failed despite the prior checked transition.
    Invalid(&'static str),
}
type Error = KernelIrContractCatalogTransportErrorV1;
type Result<T> = std::result::Result<T, Error>;
impl From<Resource> for Error {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl From<TransitionError> for Error {
    fn from(value: TransitionError) -> Self {
        Self::Transition(value)
    }
}
impl From<CatalogError> for Error {
    fn from(value: CatalogError) -> Self {
        Self::Catalog(value)
    }
}
impl From<BindingError> for Error {
    fn from(value: BindingError) -> Self {
        Self::Binding(value)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(formatter),
            Self::Transition(error) => error.fmt(formatter),
            Self::Catalog(error) => error.fmt(formatter),
            Self::Binding(error) => error.fmt(formatter),
            Self::InputInventory => {
                formatter.write_str("catalog transport input inventory mismatch")
            }
            Self::Invalid(rule) => {
                write!(formatter, "catalog placement transport rejected: {rule}")
            }
        }
    }
}
impl StdError for Error {}

/// Transports only checked allocation and marker placement onto the actual
/// output graph. Contract definitions, keys and semantic-source identity remain
/// unchanged. All output markers must pass the existing catalog graph checker.
///
/// Only Retained allocation results create bindings; substituted values, block
/// parameters and constant synthesis never do. The output graph-binding checker
/// resolves marker operands through its owner-bound must-alias analysis; this
/// permits agreed loop/branch-carried pointers without creating alias bindings.
///
/// Work is linear in operations and examined descendants, plus O(A log A) for
/// sorting A surviving bindings and the existing codec/graph-checker costs.
/// Scratch is one dense input-operation inverse and A binding slots. No raw SSA
/// identifier sizes an allocation. Every allocation and variable traversal is
/// charged before execution. All returned exits restore the incoming floor;
/// success transfers the owned output catalog and origin rows, not graph owners.
pub fn transport_kernel_ir_contract_catalog_v1(
    transition: &CheckedCanonicalKirTransitionV1<'_, '_, '_, '_>,
    input_catalog: &CheckedKernelIrContractCatalogV1<'_, '_>,
    budget: &mut Budget<'_>,
) -> Result<(
    TransportedKernelIrContractCatalogV1,
    KernelIrContractCatalogTransportStorageV1,
)> {
    let floor = budget.storage();
    let result = build(transition, input_catalog, budget);
    let retained = budget
        .storage()
        .checked_sub(floor)
        .ok_or(Resource::Accounting)?;
    budget.release_storage(retained)?;
    result.map(|result| (result, KernelIrContractCatalogTransportStorageV1(retained)))
}

fn build(
    transition: &CheckedCanonicalKirTransitionV1<'_, '_, '_, '_>,
    input_catalog: &CheckedKernelIrContractCatalogV1<'_, '_>,
    budget: &mut Budget<'_>,
) -> Result<TransportedKernelIrContractCatalogV1> {
    budget.charge_work(1)?;
    if !std::ptr::eq(transition.input(), input_catalog.inventory()) {
        return Err(Error::InputInventory);
    }
    let input = transition.input();
    let output = transition.output();
    let source_catalog = input_catalog.catalog();
    let rows = transition.rows();
    let count = source_catalog.bindings().len();
    budget.reserve_storage(
        size_of::<TransportedKernelIrContractCatalogV1>()
            .checked_sub(size_of::<Catalog>())
            .ok_or(Resource::Accounting)?,
    )?;
    let mut allocations = reserve::<KernelIrPipelineAllocationTransportV1>(count, budget)?;
    let mut markers =
        reserve::<KernelIrPipelineMarkerTransportV1>(input_catalog.marker_count(), budget)?;
    let scratch = size_of::<Vec<usize>>()
        .checked_add(size_of::<Vec<Binding>>())
        .and_then(|n| n.checked_add(input.operations().len().checked_mul(size_of::<usize>())?))
        .and_then(|n| n.checked_add(count.checked_mul(size_of::<Binding>())?))
        .ok_or(Resource::Accounting)?;
    budget.reserve_storage(size_of::<Vec<usize>>() + size_of::<Vec<Binding>>())?;
    let mut retained_operations = reserve::<usize>(input.operations().len(), budget)?;
    budget.charge_work(input.operations().len())?;
    retained_operations.resize(input.operations().len(), usize::MAX);
    let mut bindings = reserve::<Binding>(count, budget)?;
    for row in rows.operations {
        budget.charge_work(1)?;
        if let Origin::Retained(original) = row.origin {
            let old = index::operation(input, original, budget)?;
            let new = index::operation(output, row.output, budget)?;
            if retained_operations[old] != usize::MAX {
                return Err(Error::Invalid("duplicate retained operation"));
            }
            retained_operations[old] = new;
        }
    }
    for binding in source_catalog.bindings() {
        budget.charge_work(1)?;
        let original = Operation {
            block: Block {
                function: Function(binding.function),
                block: binding.block,
            },
            operation: binding.operation,
        };
        let old = index::operation(input, original, budget)?;
        let original_definition = Definition::Result {
            operation: original,
            result: 0,
        };
        let definition = index::definition(input, original_definition, budget)?;
        let descendants = index::range(
            rows.definitions[definition].outputs,
            rows.definition_outputs.len(),
            budget,
        )?;
        let new = retained_operations[old];
        let transported = if new == usize::MAX {
            if !descendants.is_empty() {
                return Err(Error::Invalid("omitted allocation still has descendants"));
            }
            None
        } else {
            let operation = &output.operations()[new];
            let output_definition = Definition::Result {
                operation: operation.coordinate,
                result: 0,
            };
            let mut retained_result = false;
            for descendant in &rows.definition_outputs[descendants] {
                budget.charge_work(1)?;
                if descendant.output == output_definition
                    && descendant.kind == DescendantKind::Retained
                {
                    retained_result = true;
                }
            }
            if !retained_result
                || operation.operation.results.len() != 1
                || !matches!(operation.operation.kind, OperationKind::WorkgroupMemory(_))
            {
                return Err(Error::Invalid("allocation retained result"));
            }
            let definition = index::definition(output, output_definition, budget)?;
            let value = output.definitions()[definition]
                .value
                .ok_or(Error::Invalid("allocation output value"))?;
            let binding = Binding {
                function: operation.coordinate.block.function.0,
                storage: value.0,
                key: binding.key,
                block: operation.coordinate.block.block,
                operation: operation.coordinate.operation,
            };
            append(&mut bindings, count, binding, budget)?;
            Some(binding)
        };
        append(
            &mut allocations,
            count,
            KernelIrPipelineAllocationTransportV1 {
                input: *binding,
                output: transported,
            },
            budget,
        )?;
    }
    for (ordinal, operation) in input.operations().iter().enumerate() {
        budget.charge_work(1)?;
        if !matches!(
            operation.operation.kind,
            OperationKind::VerificationContract(
                VerificationContractOperationV12::WorkgroupPipelineEvent { .. }
            )
        ) {
            continue;
        }
        let new = retained_operations[ordinal];
        let output = if new == usize::MAX {
            None
        } else {
            Some(output.operations()[new].coordinate)
        };
        append(
            &mut markers,
            input_catalog.marker_count(),
            KernelIrPipelineMarkerTransportV1 {
                input: operation.coordinate,
                output,
            },
            budget,
        )?;
    }
    sort_bindings(&mut bindings, budget)?;
    let (catalog, storage) = Catalog::from_rows_with_budget(
        *source_catalog.semantic_source(),
        source_catalog.definitions(),
        &bindings,
        budget,
    )?;
    budget.reserve_storage(storage.retained_storage())?;
    let storage = {
        let (checked, storage) = check_kernel_ir_contract_catalog_v1(output, &catalog, budget)?;
        budget.reserve_storage(storage.retained_storage())?;
        let mut surviving_markers = 0_usize;
        for row in &markers {
            budget.charge_work(1)?;
            if row.output.is_some() {
                surviving_markers = surviving_markers
                    .checked_add(1)
                    .ok_or(Resource::Accounting)?;
            }
        }
        if checked.marker_count() != surviving_markers {
            return Err(Error::Invalid("output marker coverage"));
        }
        storage
    };
    budget.release_storage(storage.retained_storage())?;
    drop(bindings);
    drop(retained_operations);
    budget.release_storage(scratch)?;
    Ok(TransportedKernelIrContractCatalogV1 {
        input: input.identity(),
        output: output.identity(),
        catalog,
        allocations,
        markers,
    })
}

fn reserve<T>(count: usize, budget: &mut Budget<'_>) -> Result<Vec<T>> {
    budget.reserve_storage(
        count
            .checked_mul(size_of::<T>())
            .ok_or(Resource::Accounting)?,
    )?;
    budget.charge_work(1)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(count)
        .map_err(|_| Resource::Allocation)?;
    Ok(rows)
}

fn append<T>(rows: &mut Vec<T>, admitted: usize, row: T, budget: &mut Budget<'_>) -> Result<()> {
    budget.charge_work(1)?;
    if rows.len() >= admitted || rows.len() == rows.capacity() {
        return Err(Error::Invalid("transport row census"));
    }
    rows.push(row);
    Ok(())
}

// Fallible in-place heapsort charges each comparison and swap before execution.
fn sort_bindings(rows: &mut [Binding], budget: &mut Budget<'_>) -> Result<()> {
    fn sift(rows: &mut [Binding], mut root: usize, budget: &mut Budget<'_>) -> Result<()> {
        while root < rows.len() / 2 {
            budget.charge_work(1)?;
            let mut child = root
                .checked_mul(2)
                .and_then(|n| n.checked_add(1))
                .ok_or(Resource::Accounting)?;
            if child + 1 < rows.len() {
                budget.charge_work(1)?;
                if (rows[child].function, rows[child].storage)
                    < (rows[child + 1].function, rows[child + 1].storage)
                {
                    child += 1;
                }
            }
            budget.charge_work(1)?;
            if (rows[root].function, rows[root].storage)
                >= (rows[child].function, rows[child].storage)
            {
                break;
            }
            budget.charge_work(1)?;
            rows.swap(root, child);
            root = child;
        }
        Ok(())
    }
    for root in (0..rows.len() / 2).rev() {
        sift(rows, root, budget)?;
    }
    for end in (1..rows.len()).rev() {
        budget.charge_work(1)?;
        rows.swap(0, end);
        sift(&mut rows[..end], 0, budget)?;
    }
    Ok(())
}
