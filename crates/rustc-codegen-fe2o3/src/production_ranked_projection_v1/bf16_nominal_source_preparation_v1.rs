//! Genuine-source preparation for the private observational dense capability pass.
//! No ranked owner, generic scalar-empty decision, or normal admission is made.
use super::bf16_nominal_dense_v1::NominalCapabilityInputsV1;
use super::bf16_nominal_preparation_resources_v1::{PreparationResourcesV1, resource};
use super::bf16_nominal_source_algorithms_v1::{
    assertion_definition_inventory_with_resources_v1, constant_locals_with_resources_v1,
    local_allocation_contracts_with_resources_v1, local_provenance_with_resources_v1,
};
use super::*;
use fe2o3_kernel_analysis::CanonicalKirInventoryV1;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_lower_mir_kernel::{
    Bf16NominalCallQueryErrorV1 as QueryError, ProductionPreRankedKirOwnerV1,
};
use fe2o3_mir_model::{SemanticEnumPayloadMeterV1, SemanticEnumPayloadMeteredErrorV1};
use std::mem::size_of;
use std::panic::{AssertUnwindSafe, catch_unwind};

type Result<T> = std::result::Result<T, QueryError>;

fn query_error(error: ProductionRankedProjectionErrorV1) -> QueryError {
    match error {
        ProductionRankedProjectionErrorV1::CanonicalAssertions(
            CanonicalAssertionErrorV1::Resource(error),
        ) => QueryError::Resource(error),
        _ => QueryError::Unavailable("actual source preparation refused"),
    }
}
struct ModelMeter<'r, 'b, 'w>(&'r mut PreparationResourcesV1<'b, 'w>);
impl SemanticEnumPayloadMeterV1 for ModelMeter<'_, '_, '_> {
    type Error = ProductionRankedProjectionErrorV1;
    fn charge_work(&mut self, amount: usize) -> std::result::Result<(), Self::Error> {
        self.0.work(amount)
    }
    fn reserve_storage(&mut self, amount: usize) -> std::result::Result<(), Self::Error> {
        self.0.reserve_storage(amount)
    }
}
fn model_error(
    error: SemanticEnumPayloadMeteredErrorV1<ProductionRankedProjectionErrorV1>,
) -> ProductionRankedProjectionErrorV1 {
    match error {
        SemanticEnumPayloadMeteredErrorV1::Meter(error) => error,
        SemanticEnumPayloadMeteredErrorV1::Allocation => resource(Resource::Allocation),
        SemanticEnumPayloadMeteredErrorV1::Arithmetic => resource(Resource::Arithmetic),
        SemanticEnumPayloadMeteredErrorV1::Analysis(error) => {
            ProductionRankedProjectionErrorV1::Unsupported(error.detail())
        }
    }
}
struct PreparedSourceV1 {
    dominance: SemanticEnumPayloadDominanceV1,
    allocations: Vec<Option<AllocationContractV1>>,
    constants: Vec<Option<u64>>,
}
struct RetainedPreparationTablesV1 {
    scalar: AssertionDefinitionInventoryV1,
    provenance: LocalProvenanceV1,
}

#[path = "bf16_nominal_rich_source_preparation_v1.rs"]
mod rich_source_preparation_v1;
#[allow(unused_imports)]
pub(super) use rich_source_preparation_v1::{
    RichNominalSourceTablesV1, with_nominal_rich_source_preparation_v1,
};
#[cfg(test)]
pub(super) use rich_source_preparation_v1::{
    measure_preparation_core_for_test_v1, observe_rich_source_comparison_for_test_v1,
    rich_frame_for_test_v1, with_rich_tables_for_test_v1,
};

#[path = "bf16_nominal_root_source_preparation_v1.rs"]
mod root_source_preparation_v1;
#[allow(unused_imports)]
pub(super) use root_source_preparation_v1::{
    NominalRootCfgSourceV1, NominalRootSourceTablesV1, with_nominal_root_cfg_preparation_v1,
    with_nominal_root_source_preparation_v1,
};
#[cfg(test)]
#[allow(unused_imports)]
pub(super) use root_source_preparation_v1::{
    observe_root_cfg_preparation_for_test_v1, observe_root_source_preparation_for_test_v1,
    root_cfg_preparation_controls_for_test_v1, root_source_preparation_controls_for_test_v1,
};

fn prepare(
    callables: &[SemanticCallableDeclV1],
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> std::result::Result<PreparedSourceV1, ProductionRankedProjectionErrorV1> {
    // Legacy C2 drops the same tables at the same algorithm boundary and keeps
    // the exact existing charges/header. The optional retention is private.
    prepare_with_retained(callables, types, function, resources, None)
}

fn prepare_with_retained(
    callables: &[SemanticCallableDeclV1],
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    resources: &mut PreparationResourcesV1<'_, '_>,
    retained: Option<&mut Option<RetainedPreparationTablesV1>>,
) -> std::result::Result<PreparedSourceV1, ProductionRankedProjectionErrorV1> {
    // The existing retired-intrinsic guard makes two finite roster scans.
    resources.work(
        callables
            .len()
            .checked_mul(2)
            .ok_or_else(|| resource(Resource::Arithmetic))?,
    )?;
    reject_retired_production_intrinsics_v1(callables)?;
    let dominance = SemanticEnumPayloadDominanceV1::analyze_with_meter_v1(
        function,
        types,
        &mut ModelMeter(resources),
    )
    .map_err(model_error)?;
    let scalar = assertion_definition_inventory_with_resources_v1(function, resources)?;
    let provenance = local_provenance_with_resources_v1(
        callables,
        types,
        function,
        &scalar.counts,
        &scalar.address_escaped,
        resources,
    )?;
    let allocations = local_allocation_contracts_with_resources_v1(
        types,
        function,
        &provenance.allocation_origins,
        resources,
    )?;
    let constants = constant_locals_with_resources_v1(function, resources)?;
    // Both APIs own the same accepted reservations through their callback.
    // The old C2 path keeps its former drop point and unchanged meter sequence.
    if let Some(retained) = retained {
        *retained = Some(RetainedPreparationTablesV1 { scalar, provenance });
    } else {
        drop(provenance);
        drop(scalar);
    }
    Ok(PreparedSourceV1 {
        dominance,
        allocations,
        constants,
    })
}
fn header<R>() -> Result<usize> {
    size_of::<PreparedSourceV1>()
        .checked_add(size_of::<PreparationResourcesV1<'static, 'static>>())
        .and_then(|n| n.checked_add(size_of::<ModelMeter<'static, 'static, 'static>>()))
        .and_then(|n| n.checked_add(4096))
        .and_then(|n| {
            size_of::<Result<R>>()
                .checked_mul(2)
                .and_then(|r| n.checked_add(r))
        })
        .ok_or(QueryError::Resource(Resource::Arithmetic))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn with_nominal_source_preparation_v1<'w, R: Copy + 'static>(
    owner: &ProductionPreRankedKirOwnerV1,
    inventory: &CanonicalKirInventoryV1<'_>,
    root: SemanticFunctionIdV1,
    caller: SemanticFunctionIdV1,
    block: SemanticBlockIdV1,
    call: &SemanticDirectCallV1,
    budget: &mut Budget<'w>,
    inspect: impl for<'s> FnOnce(NominalCapabilityInputsV1<'s>, &mut Budget<'w>) -> Result<R>,
) -> Result<R> {
    // N1 authenticates full retained owner/occurrence/SAME-inventory floor,
    // exact borrowed call and source relation BEFORE our first allocation.
    owner.with_checked_bf16_nominal_call_v1(
        inventory,
        root,
        caller,
        block,
        call,
        budget,
        |checked, budget| {
            budget.charge_work(32)?; // fixed construction and all cleanup checks
            if budget.failed_work().is_some() || budget.failed_storage().is_some() {
                return Err(Resource::Accounting.into());
            }
            let source_owner = checked.emission().owner();
            if !std::ptr::eq(source_owner, owner) || !checked.belongs_to(inventory) {
                return Err(QueryError::Unavailable(
                    "prepared source owner/inventory differs",
                ));
            }
            let source = source_owner.semantic_ssa().source_semantic();
            let function =
                source
                    .functions()
                    .get(caller.index() as usize)
                    .ok_or(QueryError::Unavailable(
                        "prepared caller absent from actual source",
                    ))?;
            // Closed profile is already sealed by this SAME owner, not chosen
            // by engine inputs. Recheck finite local headers without a new cap.
            if source.functions().len() != 2
                || source.types().len() > 4096
                || source.callables().len() > 4096
                || function.blocks().len() > 32
                || function.locals().len() > 4096
            {
                return Err(QueryError::Unavailable(
                    "prepared source exceeds closed owner profile",
                ));
            }
            let slot = budget as *const Budget<'w>;
            let ledger = budget.work_ledger_identity_v1();
            let entry = budget.storage();
            let frame = header::<R>()?;
            let mut owned = 0usize;
            let outcome = catch_unwind(AssertUnwindSafe(|| {
                let prepared = {
                    let mut resources = PreparationResourcesV1::new(budget, &mut owned);
                    resources.reserve_storage(frame).map_err(query_error)?;
                    prepare(source.callables(), source.types(), function, &mut resources)
                        .map_err(query_error)?
                };
                // The resource adapter's only mutable Budget borrow ended.
                let result = inspect(
                    NominalCapabilityInputsV1::from_borrowed_source_v1(
                        function,
                        &prepared.dominance,
                        &prepared.allocations,
                        &prepared.constants,
                    ),
                    budget,
                );
                drop(prepared);
                result
            }));
            // Partial values and the full prepared owner have dropped even on
            // unwind. Drop the panic payload before servicing owned reservations.
            let result = match outcome {
                Ok(Ok(_))
                    if budget.failed_work().is_some() || budget.failed_storage().is_some() =>
                {
                    Err(Resource::Accounting.into())
                }
                Ok(result) => result,
                Err(payload) => {
                    drop(payload);
                    Err(QueryError::CallbackPanicked)
                }
            };
            let protected = entry.checked_add(owned).ok_or(Resource::Arithmetic)?;
            if budget as *const Budget<'w> != slot
                || budget.work_ledger_identity_v1() != ledger
                || budget.storage() < protected
            {
                return Err(Resource::Accounting.into());
            }
            // Only accepted factory reservations: never a blanket floor delta.
            // Callback-added charges, peak, work and first denial remain.
            budget.release_storage(owned)?;
            result
        },
    )
}
