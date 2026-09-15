//! Original-source attachment for retained PolicyMatrix constructors.
//! No terminal, context replacement, SSA issuer, or numerical proof is made here.

use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticDefinedMatrixIdentityV1, SemanticPolicyGfx950NarrowTypesV1,
    SemanticPolicyGfx950NarrowV1, SemanticPolicyMatrixBindTypesV1, SemanticPolicyMatrixBindV1,
};

pub(in crate::collector::production_importer_v1) mod abi;
mod body;
mod source;

pub(in crate::collector::production_importer_v1) use source::global_bf16_constructors::{
    ConstructorRosterV1, ConstructorRowV1, capture as capture_bf16_constructors,
};

// Private shared source predicates; these return no canonical or SSA authority.
pub(in crate::collector::production_importer_v1) fn transpose_view_layout_v1<'tcx>(
    tcx: TyCtxt<'tcx>, view: Ty<'tcx>, global_reference: Ty<'tcx>, format: Ty<'tcx>, role: Ty<'tcx>, brand: Ty<'tcx>,
) -> bool {
    source::global_views::layout::view(tcx, view, global_reference, format, role, brand)
}

pub(in crate::collector::production_importer_v1) fn transpose_fragment_layout_v1<'tcx>(
    tcx: TyCtxt<'tcx>, fragment: Ty<'tcx>, format: Ty<'tcx>, role: Ty<'tcx>, brand: Ty<'tcx>,
) -> Option<Ty<'tcx>> {
    source::global_views::layout::fragment(tcx, fragment, format, role, brand)
}


pub(in crate::collector::production_importer_v1) fn validate_source<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Result<(), ProductionSemanticImportErrorV1> {
    source::global_views::validate_source(tcx, instance)?;
    match source::kind(tcx, instance) {
        Some(source::Kind::Bind) => {
            source::bind(tcx, instance)?;
        }
        Some(source::Kind::Narrow) => {
            source::narrow(tcx, instance)?;
        }
        None => {}
    }
    Ok(())
}

fn identity<'tcx>(
    tcx: TyCtxt<'tcx>,
    facts: &source::Identity<'tcx>,
    root: &AuthenticatedProductionKernelContextRootV1,
    contexts: &AuthenticatedProductionKernelContextsV1,
) -> Result<SemanticDefinedMatrixIdentityV1, ProductionSemanticImportErrorV1> {
    if !rust_kernel_brand_matches_root_v1(tcx, facts.root, root) {
        return Err(rejected(
            "defined matrix receiver differs from authenticated root",
        ));
    }
    let identity = SemanticDefinedMatrixIdentityV1::new(
        capability_memory_provenance_v1(root, contexts)?,
        rustc_type_identity_v1(tcx, facts.policy),
        rustc_type_identity_v1(tcx, facts.root.ty),
        rustc_type_identity_v1(tcx, facts.matrix_brand),
        rustc_type_identity_v1(tcx, facts.epoch),
    )
    .map_err(ProductionSemanticImportErrorV1::SemanticSchema)?;
    if facts.execution_brand == facts.root.ty {
        Ok(identity)
    } else {
        identity
            .with_execution_brand(rustc_type_identity_v1(tcx, facts.execution_brand))
            .map_err(ProductionSemanticImportErrorV1::SemanticSchema)
    }
}

fn edges(
    plan: &ProductionSemanticPreflightPlanV1<'_>,
    function: SemanticFunctionIdV1,
    projection: Option<SemanticFunctionIdV1>,
) -> Result<(), ProductionSemanticImportErrorV1> {
    let calls = plan
        .direct_call_producers()
        .iter()
        .filter(|call| call.caller == function)
        .collect::<Vec<_>>();
    let exact = match projection {
        Some(projection) => {
            matches!(calls.as_slice(), [call] if call.block == 0 && call.callee == projection)
        }
        None => calls.is_empty(),
    };
    if !exact
        || plan
            .terminal_expansion_producers()
            .iter()
            .any(|call| call.caller == function)
        || plan
            .normalized_intrinsic_producers()
            .iter()
            .any(|call| call.caller == function)
        || projection.is_some_and(|projection| {
            plan.direct_call_producers()
                .iter()
                .any(|call| call.caller == projection)
                || plan
                    .terminal_expansion_producers()
                    .iter()
                    .any(|call| call.caller == projection)
                || plan
                    .normalized_intrinsic_producers()
                    .iter()
                    .any(|call| call.caller == projection)
        })
    {
        return Err(rejected(
            "defined matrix original constructor/projection call edges",
        ));
    }
    Ok(())
}

/// Called after complete canonical functions/callables and before admission.
/// Replaying this function compares source facts, not consumer-inferred types.
pub(in crate::collector::production_importer_v1) fn attach<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    types: &[SemanticTypeDeclV1],
    functions: &mut [SemanticFunctionDeclV1],
    callables: &[SemanticCallableDeclV1],
    contexts: &AuthenticatedProductionKernelContextsV1,
) -> Result<(), ProductionSemanticImportErrorV1> {
    source::global_views::validate_canonical(tcx, plan, types, functions, callables, contexts)?;
    abi::validate_carriage(tcx, plan, types, callables)?;
    let candidates = plan
        .function_producers()
        .iter()
        .enumerate()
        .filter_map(|(index, function)| {
            source::kind(tcx, function.instance)
                .map(|kind| (SemanticFunctionIdV1::from_index(index as u32), kind))
        })
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return Ok(());
    }
    let roster = roster::Roster::new(tcx, plan, types, functions, callables)?;
    let call_edges = plan
        .direct_call_producers()
        .iter()
        .map(|call| (call.caller, call.callee))
        .collect::<Vec<_>>();
    let mut pending = Vec::new();
    for (function, kind) in candidates {
        let root = authenticate_capability_memory_root_v1(
            contexts,
            &BTreeSet::from([function]),
            &call_edges,
            false,
        )?;
        roster.root(root, contexts)?;
        let instance = roster.defined(function)?;
        let contract = match kind {
            source::Kind::Bind => {
                let facts = source::bind(tcx, instance)?;
                let ids = roster.types(facts.types)?;
                edges(plan, function, None)?;
                require_capability_memory_terminal_abi_v1(
                    tcx,
                    functions[function.index() as usize].abi(),
                    types,
                    &[facts.types[0], facts.types[2]],
                    facts.types[4],
                    &[SemanticSourceArgumentOwnershipV1::SharedBorrow; 2],
                )?;
                let record = SemanticPolicyMatrixBindV1::for_defined_function(
                    function,
                    functions,
                    callables,
                    types,
                    SemanticPolicyMatrixBindTypesV1::new(ids),
                    identity(tcx, &facts.identity, root, contexts)?,
                )
                .map_err(|error| {
                    diagnostics::rejected(
                        tcx,
                        plan,
                        functions,
                        root,
                        function,
                        None,
                        "matrix-bind-record-construction",
                        error,
                    )
                })?;
                SemanticDefinedCapabilityContractV1::PolicyMatrixBind(record)
            }
            source::Kind::Narrow => {
                let facts = source::narrow(tcx, instance)?;
                let ids = roster.types(facts.types)?;
                let projection = roster.defined_instance(facts.projection)?;
                edges(plan, function, Some(projection))?;
                require_capability_memory_terminal_abi_v1(
                    tcx,
                    functions[function.index() as usize].abi(),
                    types,
                    &[facts.types[5]],
                    facts.types[6],
                    &[SemanticSourceArgumentOwnershipV1::SharedBorrow],
                )?;
                require_capability_memory_terminal_abi_v1(
                    tcx,
                    functions[projection.index() as usize].abi(),
                    types,
                    &[facts.types[5]],
                    facts.types[0],
                    &[SemanticSourceArgumentOwnershipV1::SharedBorrow],
                )?;
                let record = SemanticPolicyGfx950NarrowV1::for_defined_function(
                    function,
                    functions,
                    callables,
                    types,
                    SemanticPolicyGfx950NarrowTypesV1::new(ids),
                    identity(tcx, &facts.identity, root, contexts)?,
                )
                .map_err(|error| {
                    diagnostics::rejected(
                        tcx,
                        plan,
                        functions,
                        root,
                        function,
                        Some(projection),
                        "matrix-narrow-record-construction",
                        error,
                    )
                })?;
                if record.projection().function() != projection {
                    return Err(rejected(
                        "defined matrix original projection identity transport",
                    ));
                }
                SemanticDefinedCapabilityContractV1::PolicyGfx950Narrow(record)
            }
        };
        let attached = functions[function.index() as usize]
            .clone()
            .with_defined_capability_contract(contract)
            .map_err(|error| {
                diagnostics::rejected(
                    tcx,
                    plan,
                    functions,
                    root,
                    function,
                    None,
                    "matrix-defined-record-attachment",
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
    abi::validate_carriage(tcx, plan, mir.types(), mir.callables())?;
    source::global_views::validate_canonical(
        tcx,
        plan,
        mir.types(),
        mir.functions(),
        mir.callables(),
        contexts,
    )?;
    for (index, function) in mir.functions().iter().enumerate() {
        let expected_kind = match function.defined_capability_contract() {
            Some(SemanticDefinedCapabilityContractV1::PolicyMatrixBind(_)) => source::Kind::Bind,
            Some(SemanticDefinedCapabilityContractV1::PolicyGfx950Narrow(_)) => {
                source::Kind::Narrow
            }
            _ => continue,
        };
        if !plan
            .function_producers()
            .get(index)
            .is_some_and(|producer| source::kind(tcx, producer.instance) == Some(expected_kind))
        {
            return Err(rejected(
                "defined matrix attachment lacks its reviewed original source",
            ));
        }
    }
    if !plan
        .function_producers()
        .iter()
        .any(|producer| source::kind(tcx, producer.instance).is_some())
    {
        return Ok(());
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
            "defined matrix canonical source attachment was erased",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod body_tests;

#[cfg(test)]
mod tests;
