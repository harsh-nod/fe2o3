//! Original-source attribution after complete type/function/callable construction.
//! These attachments describe defined bodies; they do not issue SSA authority.

use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticDefinedCapabilityContractV1, SemanticFunctionDeclV1, SemanticKernelMathDeriveTypesV1,
    SemanticKernelMathDeriveV1, SemanticPolicyMathBindTypesV1, SemanticPolicyMathBindV1,
};

mod body;
mod diagnostics;
pub(in crate::collector::production_importer_v1) mod matrix;
pub(in crate::collector::production_importer_v1) mod matrix_issuer;
mod roster;
pub(in crate::collector::production_importer_v1) use roster::Roster as DefinedSourceRosterV1;
mod source;

/// Called once after all defined bodies and non-body callables exist, before
/// admission. Failure leaves every existing attachment and body unchanged.
pub(in crate::collector::production_importer_v1) fn attach_math_defined_contracts_v1<'tcx>(
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
            source::kind(tcx, producer.instance)
                .map(|kind| (SemanticFunctionIdV1::from_index(index as u32), kind))
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
    for (function, kind) in candidates {
        let root = authenticate_capability_memory_root_v1(
            contexts,
            &BTreeSet::from([function]),
            &edges,
            false,
        )?;
        roster.root(root, contexts)?;
        let instance = roster.defined(function)?;
        let contract = match kind {
            source::Kind::Getter => {
                let facts = source::getter(tcx, instance)?;
                if !rust_kernel_brand_matches_root_v1(tcx, facts.brand, root) {
                    return Err(rejected(
                        "defined Math getter receiver differs from authenticated root",
                    ));
                }
                let bridge = roster.defined_instance(facts.bridge)?;
                let current = roster.current_instance(facts.current)?;
                roster.getter_edges(function, bridge, current, facts.current)?;
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
                    .ok_or_else(|| rejected("defined Math Current callable binding"))?;
                if !super::super::math_current_v1::matches_current(
                    tcx,
                    facts.current,
                    binding.abi(),
                    types,
                ) {
                    return Err(rejected(
                        "defined Math bridge requires the exact unbranded Current ABI",
                    ));
                }
                let record = SemanticKernelMathDeriveV1::for_defined_function(
                    function,
                    functions,
                    callables,
                    types,
                    SemanticKernelMathDeriveTypesV1::new(ids),
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
                        "math-getter-record-construction",
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
                        "defined Math canonical dependency identity transport",
                    ));
                }
                SemanticDefinedCapabilityContractV1::KernelMathDerive(record)
            }
            source::Kind::Bind => {
                let facts = source::bind(tcx, instance)?;
                if !rust_kernel_brand_matches_root_v1(tcx, facts.brand, root) {
                    return Err(rejected(
                        "defined Math Bind receivers differ from authenticated root",
                    ));
                }
                let ids = roster.types(facts.types)?;
                require_capability_memory_terminal_abi_v1(
                    tcx,
                    functions[function.index() as usize].abi(),
                    types,
                    &[facts.types[0], facts.types[2]],
                    facts.types[4],
                    &[SemanticSourceArgumentOwnershipV1::SharedBorrow; 2],
                )?;
                SemanticDefinedCapabilityContractV1::PolicyMathBind(
                    SemanticPolicyMathBindV1::for_defined_function(
                        function,
                        functions,
                        callables,
                        types,
                        SemanticPolicyMathBindTypesV1::new(ids),
                        capability_memory_provenance_v1(root, contexts)?,
                        rustc_type_identity_v1(tcx, facts.policy),
                        rustc_type_identity_v1(tcx, facts.brand.ty),
                    )
                    .map_err(|error| {
                        diagnostics::rejected(
                            tcx,
                            plan,
                            functions,
                            root,
                            function,
                            None,
                            "math-bind-record-construction",
                            error,
                        )
                    })?,
                )
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
                    "math-defined-record-attachment",
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

#[cfg(test)]
mod tests;
