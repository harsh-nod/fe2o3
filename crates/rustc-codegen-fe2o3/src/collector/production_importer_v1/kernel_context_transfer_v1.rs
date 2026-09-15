//! Source-authenticated recovery of the generated wrapper's ignored-ABI move.

mod checked;

use super::*;

pub(super) fn restore_source_transfers_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    functions: &mut [fe2o3_mir_model::semantic_mir_v1::SemanticFunctionDeclV1],
    callables: &[SemanticCallableDeclV1],
    contexts: &AuthenticatedProductionKernelContextsV1,
) -> Result<(), ProductionSemanticImportErrorV1> {
    use fe2o3_mir_model::semantic_mir_v1::SemanticTerminatorKindV1;
    let rejected = ProductionSemanticImportErrorV1::KernelContextBinding;
    for source in &contexts.roots {
        let root_index = source.selected_root.index() as usize;
        let root = functions
            .get(root_index)
            .ok_or_else(|| rejected("context transfer lost its root"))?;
        let helper_index = functions
            .iter()
            .position(|function| function.identity().as_bytes() == &source.logical_helper_identity)
            .ok_or_else(|| rejected("context transfer lost its authenticated logical helper"))?;
        let mut issuance = None;
        for block in root.blocks() {
            let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                continue;
            };
            if let Some(SemanticCallableDeclV1::CompilerIntrinsic {
                binding,
                operation: SemanticCompilerIntrinsicOperationV1::KernelContextIssue { .. },
                ..
            }) = callables.get(call.callee().index() as usize)
            {
                if issuance
                    .replace((block.identity(), binding.identity()))
                    .is_some()
                {
                    return Err(rejected("context transfer has duplicate source issuance"));
                }
            }
        }
        let (issuance_block, issuance_terminal) =
            issuance.ok_or_else(|| rejected("context transfer lost source issuance"))?;
        let producer = plan
            .function_producers()
            .get(root_index)
            .ok_or_else(|| rejected("context transfer lost its rustc body producer"))?;
        let mir_body =
            crate::rustc_semantic_adapter_v1::rustc_mir_body_sha256_v1(tcx, producer.instance);
        if root.identity().as_bytes() != &source.root_function_identity
            || super::super::derive_kernel_context_issuance_identity_v1(
                root.identity().as_bytes(),
                &mir_body,
                issuance_block.as_bytes(),
                issuance_terminal.as_bytes(),
                &source.kernel_marker_identity,
            ) != source.issuance_identity
        {
            return Err(rejected(
                "context transfer source body or issuance binding is stale",
            ));
        }
        let boundary = checked::SourceBoundary {
            root: root.identity(),
            helper: functions[helper_index].identity(),
            issuance_block,
            issuance_terminal,
            physical_arguments: source.physical_argument_count,
            logical_arguments: source.logical_argument_count,
        };
        if let Some(restored) = checked::restore(
            &boundary,
            root,
            SemanticFunctionIdV1::from_index(helper_index as u32),
            &functions[helper_index],
            callables,
        )
        .map_err(rejected)?
        {
            functions[root_index] = restored;
        }
    }
    Ok(())
}
