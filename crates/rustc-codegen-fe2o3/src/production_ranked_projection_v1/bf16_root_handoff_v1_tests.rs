use super::*;

/// Called only by the retained real-rustc BF16 fixture, not a synthetic owner.
pub(crate) fn test_guarded_bf16_root_owner_handoff(
    owner: &ProductionSemanticSsaOwnerV1,
    root: SemanticFunctionIdV1,
    contexts: &crate::collector::AuthenticatedProductionKernelContextsV1,
    launch: &LaunchContract,
) {
    let entry = contexts.checked_ranked_entry(owner, root, launch, MAX_PROJECTED_CAPABILITY_DATAFLOW_WORK_V1).unwrap();
    let make_source = || contexts.prepare_ranked_bf16_events(owner, root, launch,
        entry.as_ref().map(|(entry, _)| entry), MAX_PROJECTED_CAPABILITY_DATAFLOW_WORK_V1)
        .unwrap().expect("the original complete BF16 source session");
    let mut source = make_source();
    let body = owner.execution_view_for_root(root).unwrap().body();
    let clone = body.clone();
    assert!(matches!(source.require_ranked_root(owner, root, &clone),
        Err(ProductionRankedProjectionErrorV1::StructuralValidation(
            fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1::CorrespondenceMismatch))));
    assert!(matches!(source.with_ranked_root_inputs(&clone, owner.source_semantic().types(),
        |_, _, _, _| panic!("foreign copied body reached root builders")),
        Err(ProductionRankedProjectionErrorV1::StructuralValidation(
            fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1::CorrespondenceMismatch))));
    let copied_types = owner.source_semantic().types().to_vec();
    assert!(matches!(source.with_ranked_root_inputs(body, &copied_types,
        |_, _, _, _| panic!("foreign copied type table reached root builders")),
        Err(ProductionRankedProjectionErrorV1::StructuralValidation(
            fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1::CorrespondenceMismatch))));
    let mut visits = 0;
    let error = source.with_ranked_root_inputs(body, owner.source_semantic().types(), |_, _, _, _| {
        visits += 1;
        Err(ProductionRankedProjectionErrorV1::Incomplete("original root consumer failure"))
    });
    assert_eq!(visits, 1);
    assert!(matches!(error, Err(ProductionRankedProjectionErrorV1::Incomplete("original root consumer failure"))));
    assert!(matches!(source.finish_ranked_root(),
        Err(ProductionRankedProjectionErrorV1::StructuralValidation(
            fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1::Unsupported {
                detail: "BF16 source replay did not consume the exact checked lane roster", ..
            }))));

    let mut source = make_source();
    let scoped = source.matrix_uses_while_reads_pending().unwrap();
    let semantic = owner.source_semantic();
    let effects = derive_defined_callable_empty_effect_summaries_v1(
        semantic.types(), semantic.functions(), semantic.callables(),
    ).unwrap();
    let selection = semantic.select_kernel_body_for_root_v1(root).unwrap();
    let references = crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::new(Vec::new());
    let result = project_and_verify_ranked_root_with_scoped_matrix_v1(
        owner, &effects, selection, "original_bf16_owner_handoff", launch, &references,
        entry.as_ref(), Some(&scoped), Some(source),
    );
    let error = result.err().expect("pending BF16 reads cannot construct a successful root");
    assert!(matches!(error, ProductionRankedProjectionErrorV1::Incomplete(
        "BF16 ranked guarded reads remain unconsumed after original root input binding"
    )), "original root handoff stopped at: {error:?}");
}
