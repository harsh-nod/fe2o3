#[derive(Debug, PartialEq)]
struct IndexTraceOutcomeV1 {
    summary: Option<(ProductionRankedValueV1, u64, Option<u64>, bool)>,
    operations: String,
    arguments: Vec<Option<u32>>,
    next_argument: usize,
    next_value: u32,
    nodes: usize,
    assertion_work: usize,
    states: String,
}

#[allow(clippy::too_many_arguments)]
fn index_trace_query_v1(
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
    function: &SemanticFunctionDeclV1,
    indices: &[Option<ProjectedDisjointIndexV1>],
    operand: &SemanticOperandV1,
    block: usize,
    statement: usize,
    enabled: bool,
) -> (IndexTraceOutcomeV1, String) {
    index_trace_query_mode_v1(
        types, callables, function, indices, operand, block, statement, enabled, false,
    )
}

#[allow(clippy::too_many_arguments)]
fn index_trace_query_mode_v1(
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
    function: &SemanticFunctionDeclV1,
    indices: &[Option<ProjectedDisjointIndexV1>],
    operand: &SemanticOperandV1,
    block: usize,
    statement: usize,
    enabled: bool,
    exclusive: bool,
) -> (IndexTraceOutcomeV1, String) {
    let inventory = assertion_definition_inventory(function).unwrap();
    let constants = constant_locals(function).unwrap();
    let mut proof = SemanticAssertProofsV1::new(types, function).unwrap();
    let mut arguments = vec![None; function.locals().len()];
    let mut next_argument = 0;
    let mut operations = Vec::new();
    let mut next_value = 10_000;
    let mut projector = TotalUnsignedIndexProjectorV1::new(
        types,
        function,
        &constants,
        &inventory.counts,
        &inventory.address_escaped,
        &inventory.assignments,
        &mut proof,
        &mut arguments,
        &mut next_argument,
        &mut operations,
        &mut next_value,
    )
    .unwrap()
    .with_invocation_roots(callables, indices, 1024)
    .unwrap()
    .with_index_rejection_trace_v1(enabled);
    if exclusive {
        projector = projector.with_exclusive_source_arguments_v1().unwrap();
    }
    let value = projector
        .resolve_operand(operand, block, statement)
        .unwrap();
    let mut output = Vec::new();
    projector
        .write_index_rejection_v1(block, &mut output)
        .unwrap();
    assert!(output.len() < 4096);
    let mut states = projector.states.iter().collect::<Vec<_>>();
    states.sort_unstable_by_key(|(local, _)| **local);
    let state_text = format!("{states:?}");
    drop(states);
    let nodes = projector.node_work;
    drop(projector);
    (
        IndexTraceOutcomeV1 {
            summary: value.map(|value| {
                (
                    value.ranked,
                    value.maximum,
                    value.exact,
                    value.invocation_dependent,
                )
            }),
            operations: format!("{operations:?}"),
            arguments,
            next_argument,
            next_value,
            nodes,
            assertion_work: proof.work,
            states: state_text,
        },
        String::from_utf8(output).unwrap(),
    )
}

#[test]
fn index_trace_preserves_authenticated_invocation_result_and_all_query_work() {
    let (types, callables, function) = derived_exclusive_index_fixture(64, 16, 63, true, false);
    let (projection, _) =
        project_capability_index_fixture_with_launch(&types, &callables, &function, Some(1024))
            .unwrap();
    let operand = typed_operand(function.locals().len() as u32 - 1, U64_TYPE);
    let query = |enabled| {
        index_trace_query_v1(
            &types,
            &callables,
            &function,
            &projection.index_values,
            &operand,
            7,
            function.blocks()[7].statements().len(),
            enabled,
        )
    };
    let (off, off_text) = query(false);
    let (on, on_text) = query(true);
    assert!(off.summary.is_some_and(|value| value.3));
    assert_eq!(off, on);
    assert!(off_text.is_empty() && on_text.is_empty());
}

#[test]
fn index_trace_retains_duplicate_definition_rejection_and_exact_use_site() {
    let (types, callables, function) = derived_exclusive_index_fixture(64, 16, 63, true, true);
    let operand = typed_operand(function.locals().len() as u32 - 1, U64_TYPE);
    let indices = vec![None; function.locals().len()];
    let statement = function.blocks()[7].statements().len();
    let query = |enabled| {
        index_trace_query_v1(
            &types, &callables, &function, &indices, &operand, 7, statement, enabled,
        )
    };
    let (off, off_text) = query(false);
    let (on, text) = query(true);
    assert_eq!(off, on);
    assert!(on.summary.is_none());
    assert!(off_text.is_empty());
    assert!(text.contains("reason=non-unique-definition"));
    assert!(text.contains(&format!("use=bb7:s{statement}")));
    assert!(text.contains("definitions=Some(2)"));
    assert_eq!(text.lines().count(), 1);
    assert_incomplete(
        project_capability_index_fixture_with_launch(&types, &callables, &function, Some(1024)),
        "a typed global exclusive store requires an exact invocation-derived index",
    );
}

#[test]
fn index_trace_does_not_relabel_loop_induction_as_entry_argument() {
    let types = projection_types();
    let function = multi_block_induction_function(
        InductionCfgShape::Chain,
        SemanticLocalRoleV1::Argument(0),
        1,
    );
    let (inductions, _, _) = project_test_inductions(&function).unwrap();
    assert_eq!(inductions.len(), 1);
    assert_eq!(inductions[0].source_progress.induction.index(), 1);
    let indices = vec![None; function.locals().len()];
    let query = |enabled| {
        index_trace_query_v1(
            &types,
            &[],
            &function,
            &indices,
            &typed_operand(1, SCALAR_TYPE),
            2,
            0,
            enabled,
        )
    };
    let (off, _) = query(false);
    let (on, text) = query(true);
    assert_eq!(off, on);
    assert!(on.summary.is_none());
    assert!(on.arguments.iter().all(Option::is_none));
    assert!(text.contains("reason=non-unique-definition"));
}

#[test]
fn index_trace_does_not_promote_uniform_source_argument_to_invocation_root() {
    let types = projection_types();
    let function = multi_block_induction_function(
        InductionCfgShape::Chain,
        SemanticLocalRoleV1::Argument(0),
        1,
    );
    assert_eq!(
        function.locals()[3].role(),
        SemanticLocalRoleV1::Argument(0)
    );
    let indices = vec![None; function.locals().len()];
    let query = |enabled| {
        index_trace_query_v1(
            &types,
            &[],
            &function,
            &indices,
            &typed_operand(3, SCALAR_TYPE),
            2,
            0,
            enabled,
        )
    };
    let (off, _) = query(false);
    let (on, text) = query(true);
    assert_eq!(off, on);
    assert!(on.summary.is_none());
    assert!(on.arguments.iter().all(Option::is_none));
    assert!(text.contains("reason=non-invocation-input"));
    assert!(text.contains("role=Some(Argument(0))"));
}

#[test]
fn index_trace_bounds_projection_detail_without_granting_projected_payload() {
    let (types, callables, function) = derived_exclusive_index_fixture(64, 16, 63, true, false);
    // Deliberately malformed input remains rejected before any projected field
    // could be interpreted as a checked arithmetic or enum payload witness.
    let operand = SemanticOperandV1::Copy(
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(0),
            vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::Downcast(1), U64_TYPE).unwrap(),
                SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), U64_TYPE).unwrap(),
                SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), U64_TYPE).unwrap(),
            ],
            U64_TYPE,
        )
        .unwrap(),
    );
    let indices = vec![None; function.locals().len()];
    let query = |enabled| {
        index_trace_query_v1(
            &types,
            &callables,
            &function,
            &indices,
            &operand,
            7,
            function.blocks()[7].statements().len(),
            enabled,
        )
    };
    let (off, _) = query(false);
    let (on, text) = query(true);
    assert_eq!(off, on);
    assert!(on.summary.is_none());
    assert!(text.contains("reason=unsupported-projection"));
    assert!(text.contains("Downcast(1)"));
    assert!(text.contains("projection_count=3 projections_truncated=true"));
}

include!("index_rejection_v1/enum_carrier_tests.rs");
