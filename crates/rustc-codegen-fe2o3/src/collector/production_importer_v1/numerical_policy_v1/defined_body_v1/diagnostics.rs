use super::*;

/// Preserve the schema error while identifying which original defined body
/// failed. The canonical indices are intentionally not raw rustc MIR indices.
#[allow(clippy::too_many_arguments)]
pub(super) fn rejected(
    tcx: TyCtxt<'_>,
    plan: &ProductionSemanticPreflightPlanV1<'_>,
    functions: &[SemanticFunctionDeclV1],
    root: &AuthenticatedProductionKernelContextRootV1,
    function: SemanticFunctionIdV1,
    bridge: Option<SemanticFunctionIdV1>,
    stage: &'static str,
    error: fe2o3_mir_model::semantic_mir_v1::SemanticMirErrorV1,
) -> ProductionSemanticImportErrorV1 {
    let producer = &plan.function_producers()[function.index() as usize];
    let root_producer = &plan.function_producers()[root.selected_root.index() as usize];
    let path = tcx.def_path_str(producer.instance.def_id());
    let mut detail = format!("{error}; {}", body_context(function, functions));
    if let Some(bridge) = bridge {
        detail.push_str(&format!("; bridge {}", body_context(bridge, functions)));
    }
    ProductionSemanticImportErrorV1::CapabilityTerminalRejected {
        root: tcx.def_path_str(root_producer.instance.def_id()),
        span: tcx
            .sess
            .source_map()
            .span_to_diagnostic_string(tcx.def_span(producer.instance.def_id())),
        helper_chain: path,
        stage,
        detail,
    }
}

fn body_context(function: SemanticFunctionIdV1, functions: &[SemanticFunctionDeclV1]) -> String {
    let body = &functions[function.index() as usize];
    let locals = body
        .locals()
        .iter()
        .take(8)
        .enumerate()
        .map(|(index, local)| (index, local.role(), local.ty()))
        .collect::<Vec<_>>();
    let abi = body.abi();
    format!(
        "function={} entry={} blocks={} locals={locals:?} ABI canon={:?} extern={:?} unwind={} fixed={} inputs={:?} ownership={:?} output={:?} return={:?}",
        function.index(),
        body.entry().index(),
        body.blocks().len(),
        abi.canon_abi(),
        abi.extern_abi(),
        abi.can_unwind(),
        abi.fixed_count(),
        abi.source_input_types(),
        abi.source_argument_ownership(),
        abi.source_output_type(),
        abi.return_value().mode(),
    )
}
