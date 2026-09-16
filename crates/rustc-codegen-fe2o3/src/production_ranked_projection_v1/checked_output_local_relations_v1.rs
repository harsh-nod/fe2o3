#[derive(Debug)]
pub(crate) enum CheckedOutputLocalRelationErrorV1 {
    Resource(fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1),
    Inventory(fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1),
    Catalog(fe2o3_kernel_analysis::KernelIrContractCatalogBindingErrorV1),
    Materialization(fe2o3_lower_mir_kernel::SuppliedNativeMaterializationErrorV1),
    Target(dialect_amdgcn::NativeV12TargetBindingRelationErrorV1),
    Policy3(fe2o3_kernel_opt::CanonicalPolicy3ExecutionReceiptErrorV1),
    Native(dialect_amdgcn::NativeV12TextDescriptorReplayErrorV1),
}

impl fmt::Display for CheckedOutputLocalRelationErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Inventory(error) => error.fmt(f),
            Self::Catalog(error) => error.fmt(f),
            Self::Materialization(error) => error.fmt(f),
            Self::Target(error) => error.fmt(f),
            Self::Policy3(error) => error.fmt(f),
            Self::Native(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for CheckedOutputLocalRelationErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Inventory(error) => Some(error),
            Self::Catalog(error) => Some(error),
            Self::Materialization(error) => Some(error),
            Self::Target(error) => Some(error),
            Self::Policy3(error) => Some(error),
            Self::Native(error) => Some(error),
        }
    }
}

fn local_relation_error_v1(
    error: CheckedOutputLocalRelationErrorV1,
) -> crate::production_pipeline::ProductionPipelineError {
    crate::production_pipeline::ProductionPipelineError::CheckedOutputMemoryTarget(
        crate::production_pipeline::CheckedOutputMemoryTargetErrorV1::LocalRelation(error),
    )
}

fn local_relation_resource_v1(
    error: fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1,
) -> crate::production_pipeline::ProductionPipelineError {
    local_relation_error_v1(CheckedOutputLocalRelationErrorV1::Resource(error))
}

fn with_local_relation_scope_v1(
    budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    body: impl FnOnce(
        &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<(), crate::production_pipeline::ProductionPipelineError>,
) -> Result<(), crate::production_pipeline::ProductionPipelineError> {
    use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
    // Two scope actions, three receipt acceptances, one early release, and
    // two cleanup actions. Delegated byte/engine work is paid independently.
    budget.charge_work(8).map_err(local_relation_resource_v1)?;
    let floor = budget.storage();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| body(budget)));
    let released = budget
        .storage()
        .checked_sub(floor)
        .ok_or_else(|| local_relation_resource_v1(Resource::Accounting))?;
    budget
        .release_storage(released)
        .map_err(local_relation_resource_v1)?;
    match outcome {
        Ok(result) => result,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

// Only the genuine pre-B/O source gate can produce custody for this adapter.
// Access stays module-private; no original-R1 getter or reconstructed owner.
#[allow(clippy::too_many_arguments)]
pub(crate) fn with_source_checked_output_local_relations_v1(
    custody: &SourceRankedCustodyV1<'_>,
    bound: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    checked: &fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1,
    input_catalog: &fe2o3_kernel_ir::InertCanonicalKernelIrContractCatalogV1,
    catalog: &fe2o3_kernel_ir::InertCanonicalKernelIrContractCatalogV1,
    formals: &[fe2o3_lower_mir_kernel::ProductionScopedCompleteFormalMemoryV1<'_, '_, '_, '_>],
    typed_roots: &[crate::compiler_descriptor::TypedDescriptorRootV1],
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    target: fe2o3_compiler_ffi::DeviceTargetV1,
    source_envelope: Option<&fe2o3_compiler_ffi::CompilerFfiEnvelopeV1>,
    budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    next: impl FnOnce(
        &fe2o3_kernel_opt::CheckedCanonicalPolicy3ExecutionReceiptV1<'_, '_>,
        &dialect_amdgcn::ReplayedNativeV12TextDescriptorRelationV1<'_, '_, '_, '_>,
        &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<(), crate::production_pipeline::ProductionPipelineError>,
) -> Result<(), crate::production_pipeline::ProductionPipelineError> {
    with_native_input_relations_v1(
        custody.materialized,
        bound,
        input_catalog,
        profile,
        budget,
        |_, _, budget| {
            with_checked_output_local_relations_v1(
                custody.original,
                bound,
                checked,
                catalog,
                formals,
                typed_roots,
                profile,
                target,
                source_envelope,
                budget,
                next,
            )
        },
    )
}

fn with_native_input_relation_scope_v1(
    budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    body: impl FnOnce(
        &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<(), crate::production_pipeline::ProductionPipelineError>,
) -> Result<(), crate::production_pipeline::ProductionPipelineError> {
    use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
    // One scope entry, six immediate receipt acceptances and two cleanup actions.
    budget.charge_work(9).map_err(local_relation_resource_v1)?;
    let floor = budget.storage();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| body(budget)));
    // The callback frame, including non-Drop catalog views, has ended here.
    let released = budget
        .storage()
        .checked_sub(floor)
        .ok_or_else(|| local_relation_resource_v1(Resource::Accounting))?;
    budget
        .release_storage(released)
        .map_err(local_relation_resource_v1)?;
    match outcome {
        Ok(result) => result,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

// Inert caller-borrow consistency only. The production caller separately keeps
// original source/Some/reference custody; this prefix cannot manufacture it.
// Each inventory is built once for this module. J's earlier inventory/coordinate
// work is not reused, and existing emitter/replay engine domains stay separate.
fn with_native_input_relations_v1(
    materialized: &fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1,
    bound: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    input_catalog: &fe2o3_kernel_ir::InertCanonicalKernelIrContractCatalogV1,
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    next: impl FnOnce(
        &fe2o3_lower_mir_kernel::ReplayedSuppliedNativeMaterializationV1<'_, '_, '_, '_>,
        &dialect_amdgcn::CheckedNativeV12TargetBindingRelationV1<'_, '_, '_, '_>,
        &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<(), crate::production_pipeline::ProductionPipelineError>,
) -> Result<(), crate::production_pipeline::ProductionPipelineError> {
    use CheckedOutputLocalRelationErrorV1 as Error;
    use fe2o3_kernel_analysis::{CanonicalKirInventoryV1, check_kernel_ir_contract_catalog_v1};
    with_native_input_relation_scope_v1(budget, |budget| {
        let neutral = materialized.executable();
        let (neutral_inventory, neutral_inventory_storage) =
            CanonicalKirInventoryV1::derive(neutral, budget)
                .map_err(|error| local_relation_error_v1(Error::Inventory(error)))?;
        budget
            .reserve_storage(neutral_inventory_storage.retained_storage())
            .map_err(local_relation_resource_v1)?;
        let (neutral_catalog, neutral_catalog_storage) =
            check_kernel_ir_contract_catalog_v1(&neutral_inventory, input_catalog, budget)
                .map_err(|error| local_relation_error_v1(Error::Catalog(error)))?;
        budget
            .reserve_storage(neutral_catalog_storage.retained_storage())
            .map_err(local_relation_resource_v1)?;
        let (materialization, materialization_storage) =
            fe2o3_lower_mir_kernel::check_supplied_native_materialization_consistency_v1(
                materialized.semantic_ssa(),
                materialized.source_launch(),
                neutral,
                &neutral_catalog,
                fe2o3_lower_mir_kernel::ProductionSemanticKirLimitsV1::default(),
                budget,
            )
            .map_err(|error| local_relation_error_v1(Error::Materialization(error)))?;
        budget
            .reserve_storage(materialization_storage.retained_storage())
            .map_err(local_relation_resource_v1)?;
        let (bound_inventory, bound_inventory_storage) =
            CanonicalKirInventoryV1::derive(bound, budget)
                .map_err(|error| local_relation_error_v1(Error::Inventory(error)))?;
        budget
            .reserve_storage(bound_inventory_storage.retained_storage())
            .map_err(local_relation_resource_v1)?;
        let (bound_catalog, bound_catalog_storage) =
            check_kernel_ir_contract_catalog_v1(&bound_inventory, input_catalog, budget)
                .map_err(|error| local_relation_error_v1(Error::Catalog(error)))?;
        budget
            .reserve_storage(bound_catalog_storage.retained_storage())
            .map_err(local_relation_resource_v1)?;
        let (target, target_storage) = dialect_amdgcn::check_native_v12_target_binding_relation_v1(
            neutral,
            &neutral_catalog,
            bound,
            &bound_catalog,
            profile,
            budget,
        )
        .map_err(|error| local_relation_error_v1(Error::Target(error)))?;
        budget
            .reserve_storage(target_storage.retained_storage())
            .map_err(local_relation_resource_v1)?;
        next(&materialization, &target, budget)
    })
}

// Shared inert core; direct component use does not establish the source gate.
// Callers preserve the original budget/history and every live input reservation.
// Unit output prevents returned custody, not arbitrary caller side-effect copies.
// Text/descriptor construction retains its existing engine resource domains.
#[allow(clippy::too_many_arguments)]
fn with_checked_output_local_relations_v1(
    source: &fe2o3_lower_mir_kernel::ProductionBorrowedRankedCorrespondenceV1<'_>,
    bound: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    checked: &fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1,
    catalog: &fe2o3_kernel_ir::InertCanonicalKernelIrContractCatalogV1,
    formals: &[fe2o3_lower_mir_kernel::ProductionScopedCompleteFormalMemoryV1<'_, '_, '_, '_>],
    typed_roots: &[crate::compiler_descriptor::TypedDescriptorRootV1],
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    target: fe2o3_compiler_ffi::DeviceTargetV1,
    source_envelope: Option<&fe2o3_compiler_ffi::CompilerFfiEnvelopeV1>,
    budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    next: impl FnOnce(
        &fe2o3_kernel_opt::CheckedCanonicalPolicy3ExecutionReceiptV1<'_, '_>,
        &dialect_amdgcn::ReplayedNativeV12TextDescriptorRelationV1<'_, '_, '_, '_>,
        &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<(), crate::production_pipeline::ProductionPipelineError>,
) -> Result<(), crate::production_pipeline::ProductionPipelineError> {
    use CheckedOutputLocalRelationErrorV1 as Error;
    with_local_relation_scope_v1(budget, |budget| {
        let encoded = fe2o3_kernel_opt::encode_checked_canonical_policy3_execution_receipt_v1(
            bound, checked, budget,
        )
        .map_err(|error| local_relation_error_v1(Error::Policy3(error)))?;
        let encoded_storage = encoded.storage().retained_storage();
        budget
            .reserve_storage(encoded_storage)
            .map_err(local_relation_resource_v1)?;
        let relation = fe2o3_kernel_opt::decode_and_check_canonical_policy3_execution_receipt_v1(
            bound,
            checked,
            encoded.canonical_bytes(),
            budget,
        )
        .map_err(|error| local_relation_error_v1(Error::Policy3(error)))?;
        budget
            .reserve_storage(relation.storage().retained_storage())
            .map_err(local_relation_resource_v1)?;
        drop(encoded);
        budget
            .release_storage(encoded_storage)
            .map_err(local_relation_resource_v1)?;
        crate::production_pipeline::with_checked_output_descriptor_text_v1(
            source,
            checked,
            formals,
            typed_roots,
            profile,
            target,
            source_envelope,
            budget,
            |text, descriptor, budget| {
                let native = dialect_amdgcn::check_native_v12_text_descriptor_relation_v1(
                    checked.owner(),
                    catalog,
                    checked.owner().canonical().canonical_bytes(),
                    profile,
                    descriptor.table(),
                    text.llvm_ir(),
                    budget,
                )
                .map_err(|error| local_relation_error_v1(Error::Native(error)))?;
                budget
                    .reserve_storage(native.storage().retained_storage())
                    .map_err(local_relation_resource_v1)?;
                // Native drops before the descriptor scope's floor cleanup.
                // The decoded T relation remains paid through its postflight.
                next(&relation, &native, budget)
            },
        )
    })
}
