//! Exact source attachment for the existing V23 KernelMatrixDerive contract.
//! The leaf Current and zero-sized wrapper alone do not issue authority.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticKernelMatrixDeriveTypesV1, SemanticKernelMatrixDeriveV1,
};

mod source;

pub(in crate::collector::production_importer_v1) fn attach<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    types: &[SemanticTypeDeclV1],
    functions: &mut [SemanticFunctionDeclV1],
    callables: &[SemanticCallableDeclV1],
    contexts: &AuthenticatedProductionKernelContextsV1,
) -> Result<(), ProductionSemanticImportErrorV1> {
    let candidates = plan
        .function_producers()
        .iter()
        .enumerate()
        .filter_map(|(index, producer)| {
            source::is_getter(tcx, producer.instance)
                .then_some(SemanticFunctionIdV1::from_index(index as u32))
        })
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return Ok(());
    }
    let roster = roster::Roster::new(tcx, plan, types, functions, callables)?;
    let edges = plan
        .direct_call_producers()
        .iter()
        .map(|call| (call.caller, call.callee))
        .collect::<Vec<_>>();
    let mut pending = Vec::new();
    for function in candidates {
        let root = authenticate_capability_memory_root_v1(
            contexts,
            &BTreeSet::from([function]),
            &edges,
            false,
        )?;
        roster.root(root, contexts)?;
        let facts = source::getter(tcx, roster.defined(function)?)?;
        if !rust_kernel_brand_matches_root_v1(tcx, facts.brand, root) {
            return Err(rejected(
                "defined Matrix getter receiver differs from authenticated root",
            ));
        }
        let bridge = roster.defined_instance(facts.bridge)?;
        let current = roster.matrix_current_instance(facts.current)?;
        roster.matrix_getter_edges(function, bridge, current, facts.current)?;
        let ids = roster.types(facts.types)?;
        require_capability_memory_terminal_abi_v1(
            tcx,
            functions[function.index() as usize].abi(),
            types,
            &[facts.types[0]],
            facts.types[2],
            &[SemanticSourceArgumentOwnershipV1::SharedBorrow],
        )?;
        require_capability_memory_terminal_abi_v1(
            tcx,
            functions[bridge.index() as usize].abi(),
            types,
            &[],
            facts.types[2],
            &[],
        )?;
        let binding = callables[current.index() as usize]
            .binding()
            .ok_or_else(|| rejected("defined Matrix Current callable binding"))?;
        require_capability_memory_terminal_abi_v1(
            tcx,
            binding.abi(),
            types,
            &[],
            facts.types[3],
            &[],
        )?;
        let record = SemanticKernelMatrixDeriveV1::for_defined_function(
            function,
            functions,
            callables,
            types,
            SemanticKernelMatrixDeriveTypesV1::new(ids),
            capability_memory_provenance_v1(root, contexts)?,
            rustc_type_identity_v1(tcx, facts.brand.ty),
        )
        .map_err(|error| {
            diagnostics::rejected(
                tcx,
                plan,
                functions,
                root,
                function,
                Some(bridge),
                "matrix-getter-record-construction",
                error,
            )
        })?;
        if record.bridge().function() != bridge
            || record.current_callable() != current
            || record.bridge().source_identity()
                != canonical_function_identities_v1(tcx, facts.bridge).function()
            || record.current_source_identity()
                != canonical_function_identities_v1(tcx, facts.current).function()
        {
            return Err(rejected(
                "defined Matrix canonical dependency identity transport",
            ));
        }
        let attached = functions[function.index() as usize]
            .clone()
            .with_defined_capability_contract(
                SemanticDefinedCapabilityContractV1::KernelMatrixDerive(record),
            )
            .map_err(|error| {
                diagnostics::rejected(
                    tcx,
                    plan,
                    functions,
                    root,
                    function,
                    Some(bridge),
                    "matrix-getter-record-attachment",
                    error,
                )
            })?;
        pending.push((function, attached));
    }
    for (function, attached) in pending {
        functions[function.index() as usize] = attached;
    }
    Ok(())
}

pub(in crate::collector::production_importer_v1) fn validate_carriage<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    contexts: &AuthenticatedProductionKernelContextsV1,
    mir: &AdmittedInertSemanticMirV1,
) -> Result<(), ProductionSemanticImportErrorV1> {
    // Check both directions: no forged attachment and no omitted real getter.
    for (index, function) in mir.functions().iter().enumerate() {
        if matches!(
            function.defined_capability_contract(),
            Some(SemanticDefinedCapabilityContractV1::KernelMatrixDerive(_))
        ) && !plan
            .function_producers()
            .get(index)
            .is_some_and(|producer| source::is_getter(tcx, producer.instance))
        {
            return Err(rejected(
                "defined Matrix attachment lacks its exact reviewed getter",
            ));
        }
    }
    let mut functions = mir.functions().to_vec();
    attach(
        tcx,
        plan,
        mir.types(),
        &mut functions,
        mir.callables(),
        contexts,
    )?;
    if functions != mir.functions() {
        return Err(rejected(
            "defined Matrix source attachment erased or substituted",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
