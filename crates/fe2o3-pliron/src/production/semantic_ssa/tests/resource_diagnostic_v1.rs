use super::*;

fn storage_limit(words: usize) -> ProductionSemanticSsaLimitsV1 {
    let limits = SsaPlannerLimitsV1::default();
    ProductionSemanticSsaLimitsV1::new(
        SsaPlannerLimitsV1::try_new(
            limits.max_variables(),
            limits.max_blocks(),
            limits.max_edges(),
            limits.max_events(),
            limits.max_edge_definitions(),
            limits.max_output_items(),
            words,
            limits.max_work_units(),
        )
        .unwrap(),
    )
}

fn check(
    error: ProductionSemanticSsaErrorV1,
    stage: &str,
    auxiliary: usize,
    plan: Option<usize>,
    state: (usize, usize, usize),
    limit: usize,
) {
    let ProductionSemanticSsaErrorV1::ResourceStage {
        stage: actual_stage,
        auxiliary_storage_words,
        plan_storage_words,
        live_storage_words,
        peak_storage_words,
        requested_storage_words,
        error: original,
    } = &error
    else {
        panic!("missing storage stage: {error:?}")
    };
    assert_eq!(*actual_stage, stage);
    assert_eq!(*auxiliary_storage_words, auxiliary);
    assert_eq!(*plan_storage_words, plan);
    assert_eq!(
        (
            *live_storage_words,
            *peak_storage_words,
            *requested_storage_words
        ),
        state
    );
    let required = auxiliary + plan.unwrap_or(0) + state.0 + state.2;
    assert_eq!(
        **original,
        ProductionSemanticSsaErrorV1::PartialMoveResourceLimit {
            function: SemanticFunctionIdV1::from_index(0),
            resource: SsaPlannerResourceV1::StorageWords,
            required,
            limit,
        }
    );
    assert_eq!(
        std::error::Error::source(&error)
            .unwrap()
            .downcast_ref::<ProductionSemanticSsaErrorV1>(),
        Some(original.as_ref())
    );
    let text = error.to_string();
    assert!(text.starts_with(&original.to_string()));
    assert!(text.contains(&format!("storage-stage {stage} A={auxiliary} P=")));
    assert!(text.ends_with(&format!(
        "live={} peak={} requested={}",
        state.0, state.1, state.2
    )));
    assert!(text.len() < 512);
}

#[test]
fn actual_auxiliary_and_combined_gates_retain_exact_original_error() {
    let function = test_function(vec![test_block(
        0,
        vec![],
        SemanticTerminatorKindV1::Return,
    )]);
    let function_id = SemanticFunctionIdV1::from_index(0);
    let run = |limits| plan_semantic_function_ssa_v1(function_id, &function, limits);
    let baseline = run(ProductionSemanticSsaLimitsV1::default()).unwrap();
    let auxiliary = baseline.auxiliary_resources.storage_words;
    let plan = baseline.resources().storage_words();
    let early_auxiliary = auxiliary
        - source_uses_v1::definitions_v1::ValueOriginsV1::resources_for(baseline.plan())
            .unwrap()
            .storage_words;
    check(
        run(storage_limit(early_auxiliary - 1)).unwrap_err(),
        "auxiliary",
        early_auxiliary,
        None,
        (0, 0, 0),
        early_auxiliary - 1,
    );
    check(
        run(storage_limit(auxiliary + plan - 1)).unwrap_err(),
        "combined",
        auxiliary,
        Some(plan),
        (0, 0, 0),
        auxiliary + plan - 1,
    );
    assert_eq!(run(storage_limit(auxiliary + plan)).unwrap(), baseline);
}

#[test]
fn actual_partial_move_workspace_retention_and_dynamic_gates_are_distinct() {
    let function = test_function(vec![test_block(
        70,
        vec![
            test_assign(2, SemanticOperandV1::Move(test_place(1, Some(0)))),
            test_assign(3, SemanticOperandV1::Move(test_place(1, Some(1)))),
        ],
        SemanticTerminatorKindV1::Return,
    )]);
    let types = test_types(false);
    let run = |limits| {
        plan_semantic_function_ssa_with_module_v1(
            SemanticFunctionIdV1::from_index(0),
            &function,
            &types,
            &[],
            limits,
        )
    };
    let baseline = run(ProductionSemanticSsaLimitsV1::default()).unwrap();
    let auxiliary = baseline.auxiliary_resources.storage_words;
    let plan = baseline.resources().storage_words();
    let base = auxiliary + plan;
    let workspace = 4 * function.blocks().len() + 80;
    let retention = function.blocks().len().div_ceil(size_of::<usize>()) + 4;
    check(
        run(storage_limit(base)).unwrap_err(),
        "workspace",
        auxiliary,
        Some(plan),
        (0, 0, workspace),
        base,
    );
    check(
        run(storage_limit(base + workspace)).unwrap_err(),
        "retention",
        auxiliary,
        Some(plan),
        (workspace, workspace, retention),
        base + workspace,
    );
    let error = run(storage_limit(base + workspace + retention)).unwrap_err();
    let ProductionSemanticSsaErrorV1::ResourceStage {
        requested_storage_words,
        ..
    } = &error
    else {
        panic!("missing dynamic stage: {error:?}")
    };
    assert!(*requested_storage_words > 0);
    let requested = *requested_storage_words;
    check(
        error,
        "dynamic-state",
        auxiliary,
        Some(plan),
        (workspace + retention, workspace + retention, requested),
        base + workspace + retention,
    );
    assert_eq!(
        run(storage_limit(base + baseline.partial_moves.state_entries())).unwrap(),
        baseline
    );
}
