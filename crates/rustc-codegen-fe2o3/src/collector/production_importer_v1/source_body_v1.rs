//! Shared original-body construction for import and exact source replay.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::SemanticFunctionDeclV1;

#[path = "source_body_v1/replay.rs"]
mod replay;
pub(super) use replay::{Correspondence, Replay};

pub(super) fn construct<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    function_id: SemanticFunctionIdV1,
    abi: SemanticFunctionAbiV1,
    type_bindings: &[ProductionSemanticTypeBindingV1<'tcx>],
    body_owner: &mut ProductionSemanticBodyRequestOwnerV1<'tcx>,
) -> Result<SemanticFunctionDeclV1, ProductionSemanticImportErrorV1> {
    let function = plan
        .function_producers()
        .get(function_id.index() as usize)
        .ok_or_else(|| body_owner_table_mismatch_v1("original body function"))?;
    let body = plan
        .body_producers()
        .get(function_id.index() as usize)
        .filter(|body| body.function == function_id)
        .ok_or_else(|| body_owner_table_mismatch_v1("original body producer"))?;
    let local_bindings = body
        .locals
        .iter()
        .enumerate()
        .map(|(semantic, local)| {
            Ok(ProductionSemanticLocalBindingV1::new(
                local.rustc_local,
                fe2o3_mir_model::semantic_mir_v1::SemanticLocalIdV1::from_index(
                    u32::try_from(semantic)
                        .map_err(|_| ProductionSemanticImportErrorV1::RootIdentityMismatch)?,
                ),
                local.identity,
                local.source.provenance,
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let block_bindings = body
        .blocks
        .iter()
        .enumerate()
        .map(|(semantic, block)| {
            Ok(ProductionSemanticBlockBindingV1::new(
                block.rustc_block,
                fe2o3_mir_model::semantic_mir_v1::SemanticBlockIdV1::from_index(
                    u32::try_from(semantic)
                        .map_err(|_| ProductionSemanticImportErrorV1::RootIdentityMismatch)?,
                ),
                block.identity,
                block.source.provenance,
                block
                    .statements
                    .iter()
                    .map(|source| source.provenance)
                    .collect(),
                block.terminator.provenance,
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let direct_calls = plan
        .direct_call_producers()
        .iter()
        .filter(|call| call.caller == function_id)
        .map(|call| {
            let callee = plan
                .function_producers()
                .get(call.callee.index() as usize)
                .ok_or(ProductionSemanticImportErrorV1::RootIdentityMismatch)?;
            Ok(ProductionSemanticDirectCallBindingV1::new(
                call.caller,
                call.block,
                callee.instance,
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let terminal_expansions = plan
        .terminal_expansion_producers()
        .iter()
        .filter(|recipe| recipe.caller == function_id)
        .map(|recipe| {
            Ok(ProductionSemanticTerminalExpansionRecipeV1::new(
                recipe.caller,
                recipe.block,
                recipe.instance,
                recipe.expansion,
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let normalized_intrinsics = plan
        .normalized_intrinsic_producers()
        .iter()
        .filter(|recipe| recipe.caller == function_id)
        .map(|recipe| {
            ProductionSemanticNormalizedRustcIntrinsicRecipeV1::new(
                recipe.caller,
                recipe.block,
                recipe.instance,
                recipe.element_type,
                recipe.operation,
            )
        })
        .collect::<Vec<_>>();
    let function = construct_production_semantic_body_v1(
        ProductionSemanticBodyInputV1 {
            tcx,
            instance: function.instance,
            body: plan
                .function_mir(function_id)
                .ok_or_else(|| body_owner_table_mismatch_v1("proof-bound function MIR ordering"))?,
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
            abi,
            type_bindings,
            local_bindings: &local_bindings,
            block_bindings: &block_bindings,
            entry: body.entry,
            direct_calls: &direct_calls,
            terminal_expansions: &terminal_expansions,
            normalized_intrinsics: &normalized_intrinsics,
        },
        body_owner,
    )
    .map_err(|error| ProductionSemanticImportErrorV1::BodyConstruction(Box::new(error)))?;

    Ok(function)
}
