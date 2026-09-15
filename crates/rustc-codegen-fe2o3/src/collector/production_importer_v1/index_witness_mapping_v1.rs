//! Exact source index contracts. This classifier does not issue a capability.

use super::*;

#[cfg(test)]
mod compiler_tests;

#[derive(Clone, Copy, Debug)]
pub(super) enum RustIndexMappingV1<'tcx> {
    Invocation(SemanticDisjointIndexSpaceV1),
    WorkgroupMemory(RustWorkgroupMemoryBrandV1<'tcx>),
}

#[derive(Clone, Copy, Debug)]
pub(super) struct RustIndexWitnessContractV1<'tcx> {
    pub(super) space: Ty<'tcx>,
    pub(super) brand: Ty<'tcx>,
    pub(super) mapping: RustIndexMappingV1<'tcx>,
}

pub(super) fn index_witness_contract_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    witness: Ty<'tcx>,
    item: TrustedDeviceItem,
) -> Option<RustIndexWitnessContractV1<'tcx>> {
    if !matches!(
        item,
        TrustedDeviceItem::ThreadIndex | TrustedDeviceItem::DisjointIndex
    ) {
        return None;
    }
    let (space, brand) = rust_branded_index_v1(tcx, witness, item)?;
    let mapping = if rust_is_exact_trusted_marker_v1(
        tcx,
        space,
        TrustedDeviceItem::WorkgroupMemoryIndexSpace1D,
    ) {
        RustIndexMappingV1::WorkgroupMemory(rust_workgroup_memory_brand_v1(tcx, brand)?)
    } else {
        RustIndexMappingV1::Invocation(rust_disjoint_index_space_v1(tcx, space)?)
    };
    Some(RustIndexWitnessContractV1 {
        space,
        brand,
        mapping,
    })
}

pub(super) fn thread_into_disjoint_contract_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    input: Ty<'tcx>,
    output: Ty<'tcx>,
) -> Option<RustIndexWitnessContractV1<'tcx>> {
    let input = index_witness_contract_v1(tcx, input, TrustedDeviceItem::ThreadIndex)?;
    let output = index_witness_contract_v1(tcx, output, TrustedDeviceItem::DisjointIndex)?;
    (input.space == output.space && input.brand == output.brand).then_some(input)
}

pub(super) fn conversion_requires_root_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Result<bool, ProductionSemanticImportErrorV1> {
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    );
    let [input] = signature.inputs() else {
        return Err(body_owner_table_mismatch_v1(
            "terminal disjoint conversion arity",
        ));
    };
    let contract = thread_into_disjoint_contract_v1(tcx, *input, signature.output())
        .ok_or_else(|| body_owner_table_mismatch_v1("terminal disjoint mapping"))?;
    Ok(matches!(
        contract.mapping,
        RustIndexMappingV1::WorkgroupMemory(_)
    ))
}

pub(super) fn disjoint_slice_get_mut_contract_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    slice_reference: Ty<'tcx>,
    index: Ty<'tcx>,
) -> Option<(Ty<'tcx>, SemanticDisjointIndexSpaceV1)> {
    let (element, slice_space) = rust_reference_pointee_v1(slice_reference)
        .and_then(|ty| rust_disjoint_slice_v1(tcx, ty))?;
    let index_space = rust_index_witness_space_v1(tcx, index, TrustedDeviceItem::ThreadIndex)?;
    (slice_space == index_space).then_some((element, slice_space))
}

pub(super) fn validate_conversion_carriage_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    contexts: &AuthenticatedProductionKernelContextsV1,
    mir: &AdmittedInertSemanticMirV1,
    terminal_index: usize,
) -> Result<(), ProductionSemanticImportErrorV1> {
    let terminal = plan
        .terminal_producers()
        .get(terminal_index)
        .ok_or_else(|| body_owner_table_mismatch_v1("scoped conversion source owner"))?;
    if !conversion_requires_root_v1(tcx, terminal.instance)? {
        return Ok(());
    }
    let ordinal = u32::try_from(terminal_index)
        .map_err(|_| ProductionSemanticImportErrorV1::RootIdentityMismatch)?;
    let root =
        capability_memory_root_for_terminal_v1(tcx, plan, contexts, ordinal, terminal.expansion)?
            .ok_or_else(|| body_owner_table_mismatch_v1("scoped conversion root custody"))?;
    let callable_index = plan
        .function_producers()
        .len()
        .checked_add(terminal_index)
        .ok_or(ProductionSemanticImportErrorV1::RootIdentityMismatch)?;
    let Some(SemanticCallableDeclV1::CompilerIntrinsic {
        binding, operation, ..
    }) = mir.callables().get(callable_index)
    else {
        return Err(body_owner_table_mismatch_v1(
            "scoped conversion canonical callable",
        ));
    };
    if mir.wire_version() < SemanticMirWireVersionV1::V22
        || binding.identity() != terminal.identities.function()
        || binding.abi().identity() != terminal.abi.identity
        || !matches!(operation, SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract }
            if matches!(contract.operation(), SemanticExecutionCapabilityOperationV1::WorkgroupMemoryIndexIntoDisjoint { .. }))
        || *operation
            != terminal_operation_v1(
                tcx,
                terminal.instance,
                terminal.expansion,
                binding.abi(),
                mir.types(),
                Some(root),
                terminal.identities.function(),
                contexts,
            )?
    {
        return Err(body_owner_table_mismatch_v1(
            "scoped conversion complete source contract carriage",
        ));
    }
    Ok(())
}
