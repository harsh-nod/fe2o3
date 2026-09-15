//! Replay original defined bodies, including the checked constructor's error paths.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{SemanticBlockIdV1, SemanticLocalIdV1};

pub(super) fn descendants(
    plan: &ProductionSemanticPreflightPlanV1<'_>,
    function_count: usize,
    mut selected: BTreeSet<SemanticFunctionIdV1>,
) -> Result<BTreeSet<SemanticFunctionIdV1>, ProductionSemanticImportErrorV1> {
    let mut adjacency = vec![Vec::new(); function_count];
    for call in plan.direct_call_producers() {
        if call.callee.index() as usize >= function_count {
            return Err(rejected("global matrix replay callee outside roster"));
        }
        adjacency
            .get_mut(call.caller.index() as usize)
            .ok_or_else(|| rejected("global matrix replay caller outside roster"))?
            .push(call.callee);
    }
    let mut pending = selected.iter().copied().collect::<Vec<_>>();
    while let Some(function) = pending.pop() {
        for callee in adjacency
            .get(function.index() as usize)
            .ok_or_else(|| rejected("global matrix replay function outside roster"))?
        {
            if selected.insert(*callee) {
                pending.push(*callee);
            }
        }
    }
    Ok(selected)
}

pub(in crate::collector::production_importer_v1::numerical_policy_v1::defined_body_v1::matrix::source) fn validate<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    types: &[SemanticTypeDeclV1],
    functions: &[SemanticFunctionDeclV1],
    selected: BTreeSet<SemanticFunctionIdV1>,
) -> Result<(), ProductionSemanticImportErrorV1> {
    let selected = descendants(plan, functions.len(), selected)?;
    // One shared default-budget owner charges every selected body exactly once.
    // The graph and type bindings are bounded by the already checked live plan.
    let mut owner = build_body_request_owner_v1(
        plan,
        types.len(),
        u32::try_from(functions.len())
            .map_err(|_| ProductionSemanticImportErrorV1::RootIdentityMismatch)?,
    )?;
    let type_bindings = plan
        .type_producers()
        .iter()
        .enumerate()
        .map(|(index, producer)| {
            ProductionSemanticTypeBindingV1::new(
                producer.ty,
                SemanticTypeIdV1::from_index(index as u32),
            )
        })
        .collect::<Vec<_>>();
    for function_id in selected {
        let index = function_id.index() as usize;
        let function = &plan.function_producers()[index];
        let body = &plan.body_producers()[index];
        let actual = &functions[index];
        if body.function != function_id {
            return Err(rejected("global matrix replay original body owner"));
        }
        let local_bindings = body
            .locals
            .iter()
            .enumerate()
            .map(|(index, local)| {
                ProductionSemanticLocalBindingV1::new(
                    local.rustc_local,
                    SemanticLocalIdV1::from_index(index as u32),
                    local.identity,
                    local.source.provenance,
                )
            })
            .collect::<Vec<_>>();
        let block_bindings = body
            .blocks
            .iter()
            .enumerate()
            .map(|(index, block)| {
                ProductionSemanticBlockBindingV1::new(
                    block.rustc_block,
                    SemanticBlockIdV1::from_index(index as u32),
                    block.identity,
                    block.source.provenance,
                    block
                        .statements
                        .iter()
                        .map(|source| source.provenance)
                        .collect(),
                    block.terminator.provenance,
                )
            })
            .collect::<Vec<_>>();
        let direct_calls = plan
            .direct_call_producers()
            .iter()
            .filter(|call| call.caller == function_id)
            .map(|call| {
                ProductionSemanticDirectCallBindingV1::new(
                    call.caller,
                    call.block,
                    plan.function_producers()[call.callee.index() as usize].instance,
                )
            })
            .collect::<Vec<_>>();
        let terminal_expansions = plan
            .terminal_expansion_producers()
            .iter()
            .filter(|call| call.caller == function_id)
            .map(|call| {
                ProductionSemanticTerminalExpansionRecipeV1::new(
                    call.caller,
                    call.block,
                    call.instance,
                    call.expansion,
                )
            })
            .collect::<Vec<_>>();
        let normalized_intrinsics = plan
            .normalized_intrinsic_producers()
            .iter()
            .filter(|call| call.caller == function_id)
            .map(|call| {
                ProductionSemanticNormalizedRustcIntrinsicRecipeV1::new(
                    call.caller,
                    call.block,
                    call.instance,
                    call.element_type,
                    call.operation,
                )
            })
            .collect::<Vec<_>>();
        let expected = construct_production_semantic_body_v1(
            ProductionSemanticBodyInputV1 {
                tcx,
                instance: function.instance,
                body: plan
                    .function_mir(function_id)
                    .ok_or_else(|| rejected("global matrix replay missing original MIR"))?,
                function: function_id,
                identities: ProductionSemanticFunctionIdentitiesV1::new(
                    function.identities.function(),
                    function.identities.item_definition(),
                    function.identities.monomorphization(),
                    function.identities.generic_type_arguments(),
                    function.identities.const_generic_arguments(),
                ),
                role: semantic_function_role_v1(function.role),
                export: semantic_function_export_v1(function)?,
                source: body.source.provenance,
                abi: actual.abi().clone(),
                type_bindings: &type_bindings,
                local_bindings: &local_bindings,
                block_bindings: &block_bindings,
                entry: body.entry,
                direct_calls: &direct_calls,
                terminal_expansions: &terminal_expansions,
                normalized_intrinsics: &normalized_intrinsics,
            },
            &mut owner,
        )
        .map_err(|error| ProductionSemanticImportErrorV1::BodyConstruction(Box::new(error)))?;
        if expected != *actual {
            return Err(rejected(
                "global matrix original constructor/helper body replay",
            ));
        }
    }
    Ok(())
}
