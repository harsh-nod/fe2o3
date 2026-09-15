use super::super::source_partial_move_diagnostic_v1 as diagnostic;
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticSourceFileIdentityV1, SemanticSourceOriginV1, SemanticSourceProvenanceV1,
};

mod bounds;

fn failure(block: u32, statement: Option<u32>) -> ProductionSemanticSsaErrorV1 {
    ProductionSemanticSsaErrorV1::PartialMove {
        function: SemanticFunctionIdV1::from_index(0),
        block,
        statement,
        local: 1,
        violation: SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
    }
}

fn describe(body: &SemanticFunctionDeclV1, error: &ProductionSemanticSsaErrorV1) -> Option<String> {
    diagnostic::describe(
        SemanticFunctionIdV1::from_index(0),
        body,
        &test_types(false),
        &[],
        error,
    )
}

fn repeat_field() -> SemanticFunctionDeclV1 {
    test_function(vec![
        test_block(
            71,
            vec![test_assign(
                2,
                SemanticOperandV1::Move(test_place(1, Some(0))),
            )],
            SemanticTerminatorKindV1::Goto(test_edge(SemanticEdgeRoleV1::Goto, 1)),
        ),
        test_block(
            72,
            vec![test_assign(
                3,
                SemanticOperandV1::Copy(test_place(1, Some(0))),
            )],
            SemanticTerminatorKindV1::Return,
        ),
    ])
}

#[test]
fn original_partial_move_diagnostic_preserves_real_copy_rejection_and_typed_paths() {
    let body = repeat_field();
    let before = body.clone();
    let error = plan_test_function(&body, &test_types(false)).unwrap_err();
    assert_eq!(error, failure(1, Some(0)));
    let detail = describe(&body, &error).unwrap();
    assert_eq!(body, before);
    assert!(detail.contains("original_function=0 identity=34343434"));
    assert!(detail.contains("EXACT_FAILING_STATEMENT bb1 s0"));
    assert!(
        detail.contains("Move(local1(base_type0,role=Argument(0))->Field(0):type1:result_type1)")
    );
    assert!(
        detail.contains("Copy(local1(base_type0,role=Argument(0))->Field(0):type1:result_type1)")
    );
    assert!(detail.contains("static_predecessor bb0 role=Goto -> bb1"));
    assert!(detail.contains("no active-state, dominance, liveness, or causal-path conclusion"));
    let mut output = Vec::new();
    assert_eq!(
        diagnostic::emit_to(&mut output, Some(&detail), error.clone()),
        error
    );
    assert_eq!(output, detail.as_bytes());
    assert_eq!(
        plan_test_function(&body, &test_types(false)).unwrap_err(),
        error
    );
}

#[test]
fn original_partial_move_diagnostic_does_not_relabel_expansion_or_foreign_function() {
    let body = repeat_field();
    let error = failure(1, Some(0));
    assert!(
        diagnostic::describe(
            SemanticFunctionIdV1::from_index(1),
            &body,
            &test_types(false),
            &[],
            &error
        )
        .is_none()
    );
    let expanded = ProductionSemanticSsaErrorV1::ExpandedExecution {
        root: SemanticFunctionIdV1::from_index(0),
        execution_view_identity: [1; 32],
        source_block: None,
        source_target: None,
        source_statement: None,
        source_terminator: None,
        source_local: None,
        return_transfer_diagnostic: None,
        error: Box::new(error),
    };
    assert!(describe(&body, &expanded).is_none());
    assert!(describe(&body, &ProductionSemanticSsaErrorV1::ReplayMismatch).is_none());
    assert!(describe(&body, &failure(99, Some(0))).is_none());
    assert!(describe(&body, &failure(1, Some(99))).is_none());
}

#[test]
fn original_partial_move_diagnostic_retains_original_expansion_and_callsite_spans() {
    let expansion = SemanticSourceOriginV1::new(
        SemanticSourceFileIdentityV1::from_sha256([0xab; 32]),
        10,
        40,
        7,
        2,
        9,
        8,
    )
    .unwrap();
    let call_site = SemanticSourceOriginV1::new(
        SemanticSourceFileIdentityV1::from_sha256([0xcd; 32]),
        50,
        60,
        22,
        4,
        22,
        14,
    )
    .unwrap();
    let operation = test_assign(2, SemanticOperandV1::Copy(test_place(1, Some(0))));
    let statement = SemanticStatementV1::new(
        SemanticSourceProvenanceV1::new(Some(expansion), Some(call_site)),
        operation.kind().clone(),
    );
    let body = test_function(vec![test_block(
        73,
        vec![statement],
        SemanticTerminatorKindV1::Return,
    )]);
    let text = describe(&body, &failure(0, Some(0))).unwrap();
    assert!(text.contains(&format!(
        "expansion={}:(7, 2)-(9, 8)/bytes(10, 40)",
        "ab".repeat(32)
    )));
    assert!(text.contains(&format!(
        "call_site={}:(22, 4)-(22, 14)/bytes(50, 60)",
        "cd".repeat(32)
    )));
}

#[test]
fn original_partial_move_diagnostic_scans_bounded_prefix_but_keeps_exact_failure() {
    let source = SemanticSourceProvenanceV1::unavailable();
    let mut statements = vec![SemanticStatementV1::new(source, SemanticStatementKindV1::Nop); 2048];
    statements.push(test_assign(
        3,
        SemanticOperandV1::Copy(test_place(1, Some(0))),
    ));
    let body = test_function(vec![test_block(
        74,
        statements,
        SemanticTerminatorKindV1::Return,
    )]);
    let text = describe(&body, &failure(0, Some(2048))).unwrap();
    assert!(text.contains("EXACT_FAILING_STATEMENT bb0 s2048"));
    assert!(text.contains("static_local_scan complete=false statements=2048"));
    assert!(text.len() <= diagnostic::MAX_BYTES);
}

#[test]
fn original_partial_move_diagnostic_preserves_callreturn_and_unwind_edges() {
    let body = test_function(vec![
        test_block(
            75,
            vec![],
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    SemanticCallableIdV1::from_index(0),
                    vec![SemanticOperandV1::Move(test_place(1, Some(0)))],
                    Some(SemanticCallDestinationV1::new(
                        test_scalar_place(2),
                        test_edge(SemanticEdgeRoleV1::CallReturn, 1),
                    )),
                    SemanticUnwindActionV1::Unreachable,
                )
                .unwrap(),
            ),
        ),
        test_block(
            76,
            vec![test_assign(
                3,
                SemanticOperandV1::Copy(test_place(1, Some(0))),
            )],
            SemanticTerminatorKindV1::Return,
        ),
    ]);
    let callable = SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(7));
    let text = diagnostic::describe(
        SemanticFunctionIdV1::from_index(0),
        &body,
        &test_types(false),
        &[callable],
        &failure(1, Some(0)),
    )
    .unwrap();
    assert!(text.contains("static_predecessor bb0 role=CallReturn -> bb1"));
    assert!(text.contains("Call callable0/Defined(function7)"));
    assert!(text.contains("static_local_candidate bb0terminator source="));
    assert!(text.contains("Move(local1"));
    assert!(text.contains("unwind=Unreachable"));
}

#[test]
fn original_partial_move_diagnostic_caps_predecessors_without_claiming_completeness() {
    let mut blocks = (0..10)
        .map(|index| {
            test_block(
                80 + index,
                vec![],
                SemanticTerminatorKindV1::Goto(test_edge(SemanticEdgeRoleV1::Goto, 10)),
            )
        })
        .collect::<Vec<_>>();
    blocks.push(test_block(
        90,
        vec![test_assign(
            3,
            SemanticOperandV1::Copy(test_place(1, Some(0))),
        )],
        SemanticTerminatorKindV1::Return,
    ));
    let body = test_function(blocks);
    let text = describe(&body, &failure(10, Some(0))).unwrap();
    assert!(text.contains("predecessor_scan complete=false"));
    assert!(text.contains("edge roles retained, reachability not inferred"));
    assert!(text.len() <= diagnostic::MAX_BYTES);
}

#[test]
fn original_partial_move_diagnostic_sink_failure_keeps_the_exact_error() {
    struct Broken;
    impl std::io::Write for Broken {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::ErrorKind::BrokenPipe.into())
        }
    }
    let error = failure(0, Some(0));
    assert_eq!(
        diagnostic::emit_to(&mut Broken, Some("bounded observation"), error.clone()),
        error
    );
    let mut output = Vec::new();
    assert_eq!(diagnostic::emit_to(&mut output, None, error.clone()), error);
    assert!(output.is_empty());
}

#[test]
fn original_partial_move_diagnostic_byte_limit_does_not_change_the_validator() {
    let statement = test_assign(3, SemanticOperandV1::Copy(test_place(1, Some(0))));
    let body = test_function(vec![test_block(
        91,
        vec![statement; 2048],
        SemanticTerminatorKindV1::Return,
    )]);
    let text = describe(&body, &failure(0, Some(0))).unwrap();
    assert!(text.len() <= diagnostic::MAX_BYTES);
    assert!(
        text.contains("candidates=24")
            || text.ends_with("[source-partial-move diagnostic truncated]\n")
    );
}
