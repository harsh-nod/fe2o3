//! Canonical MIR V20 transport of authenticated source facts, not Workgroup SSA proof.

use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticFunctionDeclV1, SemanticWorkgroupEpochProjectionTypesV1,
    SemanticWorkgroupEpochProjectionV1,
};

pub(super) fn subgroup_operation_v1(
    source: WorkgroupReferenceSourceV1,
    abi: &SemanticFunctionAbiV1,
    source_identity: SemanticFunctionIdentityV1,
) -> Result<SemanticCompilerIntrinsicOperationV1, ProductionSemanticImportErrorV1> {
    if source.source_identity() != source_identity
        || abi.source_input_types() != [source.reference()]
        || abi.source_output_type() != source.output()
        || source.reference() == source.workgroup()
        || source.receiver_argument() != 0
        || !exact_source_abi_v1(abi, &[SemanticSourceArgumentOwnershipV1::SharedBorrow])
    {
        return Err(rejected(
            "borrowed subgroup complete source contract carriage",
        ));
    }
    let contract = SemanticExecutionCapabilityContractV1::new(
        SemanticExecutionCapabilityOperationV1::SubgroupDeriveBorrowed {
            workgroup_reference: source.reference(),
            workgroup: source.workgroup(),
            subgroup: source.output(),
            width: 64,
        },
        SemanticExecutionCapabilitySignatureV1::new(
            abi.source_input_types(),
            abi.source_output_type(),
        )
        .map_err(ProductionSemanticImportErrorV1::SemanticSchema)?,
        source.provenance(),
        source.brand(),
        source.epoch(),
        None,
        source_identity,
    )
    .map_err(ProductionSemanticImportErrorV1::SemanticSchema)?;
    Ok(SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract })
}

pub(in crate::collector::production_importer_v1) fn epoch_source_for_function_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    function: SemanticFunctionIdV1,
    abi: &SemanticFunctionAbiV1,
    types: &[SemanticTypeDeclV1],
    contexts: &AuthenticatedProductionKernelContextsV1,
) -> Result<Option<WorkgroupEpochProjectionSourceV1>, ProductionSemanticImportErrorV1> {
    let producer = plan
        .function_producers()
        .get(function.index() as usize)
        .ok_or_else(|| rejected("workgroup epoch function owner"))?;
    if !trusted_device_items::is_exact_reviewed_provider_definition_v1(
        tcx,
        producer.instance.def_id(),
        EPOCH_METHOD,
    ) {
        return Ok(None);
    }
    let edges = plan
        .direct_call_producers()
        .iter()
        .map(|call| (call.caller, call.callee))
        .collect::<Vec<_>>();
    let root = authenticate_capability_memory_root_v1(
        contexts,
        &BTreeSet::from([function]),
        &edges,
        false,
    )?;
    let source =
        workgroup_epoch_projection_source_v1(tcx, producer.instance, abi, types, root, contexts)?;
    if source.receiver().source_identity() != producer.identities.function()
        || !plan
            .function_mir(function)
            .is_some_and(|body| std::ptr::eq(body, tcx.instance_mir(producer.instance.def)))
    {
        return Err(rejected("workgroup epoch retained source body identity"));
    }
    Ok(Some(source))
}

pub(in crate::collector::production_importer_v1) fn attach_epoch_projection_v1(
    function: SemanticFunctionIdV1,
    body: SemanticFunctionDeclV1,
    source: WorkgroupEpochProjectionSourceV1,
) -> Result<SemanticFunctionDeclV1, ProductionSemanticImportErrorV1> {
    let receiver = source.receiver();
    if receiver.source_identity() != body.identity()
        || body.abi().source_input_types() != [receiver.reference()]
        || body.abi().source_output_type() != receiver.output()
        || receiver.receiver_argument() != 0
        || source.source_field() != 2
    {
        return Err(rejected(
            "workgroup epoch complete source contract carriage",
        ));
    }
    let record = SemanticWorkgroupEpochProjectionV1::for_defined_function(
        function,
        &body,
        SemanticWorkgroupEpochProjectionTypesV1::new([
            receiver.reference(),
            receiver.workgroup(),
            receiver.output(),
            source.epoch_type(),
        ]),
        receiver.provenance(),
        receiver.brand(),
        receiver.epoch(),
    )
    .map_err(ProductionSemanticImportErrorV1::SemanticSchema)?;
    body.with_workgroup_epoch_projection(record)
        .map_err(ProductionSemanticImportErrorV1::SemanticSchema)
}

#[cfg(test)]
mod import_tests;
