use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAggregateKindV1, SemanticAssignmentV1, SemanticConstantV1, SemanticConstantValueV1,
    SemanticScalarValueV1,
};

// These bounded AST fixtures test observation, not source admission or dataflow.
fn bounded_text(body: &SemanticFunctionDeclV1, error: ProductionSemanticSsaErrorV1) -> String {
    let text = describe(body, &error).unwrap();
    assert!(text.len() <= diagnostic::MAX_BYTES);
    assert!(!text.contains("[source-partial-move diagnostic truncated]"));
    let mut output = Vec::new();
    assert_eq!(
        diagnostic::emit_to(&mut output, Some(&text), error.clone()),
        error
    );
    assert_eq!(output, text.as_bytes());
    text
}

fn indexed_block(index: u32, statements: Vec<SemanticStatementV1>) -> SemanticBasicBlockV1 {
    let source = SemanticSourceProvenanceV1::unavailable();
    let mut identity = [0; 32];
    identity[0] = 0xe1;
    identity[1..5].copy_from_slice(&index.to_be_bytes());
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256(identity),
        source,
        statements,
        SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
    )
    .unwrap()
}

fn scalar_zero() -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        SemanticTypeIdV1::from_index(1),
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(0, 4).unwrap()),
    ))
}

#[test]
fn original_partial_move_diagnostic_block_bound_keeps_outside_failure() {
    for count in [256u32, 257] {
        let failing = count - 1;
        let blocks = (0..count)
            .map(|index| {
                indexed_block(
                    index,
                    if index == failing {
                        vec![test_assign(
                            3,
                            SemanticOperandV1::Copy(test_place(1, Some(0))),
                        )]
                    } else {
                        vec![]
                    },
                )
            })
            .collect();
        let body = test_function(blocks);
        let text = bounded_text(&body, failure(failing, Some(0)));
        assert!(text.contains(&format!("EXACT_FAILING_STATEMENT bb{failing} s0")));
        assert!(text.contains(&format!("selected_source_bb{failing} identity=")));
        if count == 256 {
            assert!(text.contains("static_local_scan complete=true statements=1 candidates=1"));
            assert!(text.contains("predecessor_scan complete=true edges=0"));
            assert!(text.contains("static_local_candidate bb255s0"));
        } else {
            assert!(text.contains("static_local_scan complete=false statements=0 candidates=0"));
            assert!(text.contains("predecessor_scan complete=false edges=0"));
            assert!(!text.contains("static_local_candidate bb256s0"));
        }
    }
}

#[test]
fn original_partial_move_diagnostic_edge_bound_excludes_unscanned_predecessor() {
    for values in [2047u32, 2048] {
        let targets = SemanticSwitchTargetsV1::new(
            (0..values)
                .map(|value| {
                    SemanticSwitchTargetV1::new(
                        u128::from(value),
                        test_edge(SemanticEdgeRoleV1::SwitchValue, 2),
                    )
                })
                .collect(),
            test_edge(SemanticEdgeRoleV1::SwitchOtherwise, 1),
        )
        .unwrap();
        let body = test_function(vec![
            test_block(
                101,
                vec![],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: scalar_zero(),
                    targets,
                },
            ),
            test_block(
                102,
                vec![test_assign(
                    3,
                    SemanticOperandV1::Copy(test_place(1, Some(0))),
                )],
                SemanticTerminatorKindV1::Return,
            ),
            test_block(103, vec![], SemanticTerminatorKindV1::Return),
        ]);
        let text = bounded_text(&body, failure(1, Some(0)));
        assert!(text.contains("EXACT_FAILING_STATEMENT bb1 s0"));
        assert!(text.contains("static_local_scan complete=true statements=1 candidates=1"));
        if values == 2047 {
            assert!(text.contains("predecessor_scan complete=true edges=2048"));
            assert!(text.contains("static_predecessor bb0 role=SwitchOtherwise -> bb1"));
        } else {
            assert!(text.contains("predecessor_scan complete=false edges=2048"));
            assert!(!text.contains("static_predecessor bb0"));
        }
    }
}

#[test]
fn original_partial_move_diagnostic_operand_bound_covers_rvalues_and_calls() {
    for count in [32, 33] {
        let mut operands = vec![scalar_zero(); count - 1];
        operands.push(SemanticOperandV1::Move(test_place(1, Some(0))));
        let source = SemanticSourceProvenanceV1::unavailable();
        let assignment = SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                test_scalar_place(2),
                SemanticRvalueV1::new(
                    SemanticTypeIdV1::from_index(1),
                    SemanticRvalueKindV1::aggregate(
                        SemanticAggregateKindV1::Tuple,
                        operands.clone(),
                    )
                    .unwrap(),
                ),
            )),
        );
        let rvalue = test_function(vec![test_block(
            104,
            vec![assignment],
            SemanticTerminatorKindV1::Return,
        )]);
        let call = test_function(vec![test_block(
            105,
            vec![],
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    SemanticCallableIdV1::from_index(0),
                    operands,
                    None,
                    SemanticUnwindActionV1::Unreachable,
                )
                .unwrap(),
            ),
        )]);
        for (body, statement, marker) in
            [(&rvalue, Some(0), "operands"), (&call, None, "arguments")]
        {
            let text = bounded_text(body, failure(0, statement));
            let exact = text
                .lines()
                .find(|line| line.starts_with("EXACT_FAILING_"))
                .unwrap();
            assert_eq!(exact.matches("Constant(").count(), (count - 1).min(32));
            if count == 32 {
                assert!(text.contains("static_local_scan complete=true"));
                assert!(text.contains("candidates=1"));
                assert!(exact.contains("Move(local1"));
                assert!(!exact.contains(&format!("[{marker} truncated]")));
            } else {
                assert!(text.contains("static_local_scan complete=false"));
                assert!(text.contains("candidates=0"));
                assert!(!exact.contains("Move(local1"));
                assert!(exact.contains(&format!("[{marker} truncated]")));
            }
        }
    }
}

#[test]
fn original_partial_move_diagnostic_projection_bound_excludes_late_index_use() {
    let ty = SemanticTypeIdV1::from_index(1);
    for count in [32, 33] {
        let mut projections = vec![
            SemanticProjectionV1::new(SemanticProjectionKindV1::Subtype, ty)
                .unwrap();
            count - 1
        ];
        projections.push(
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(1)),
                ty,
            )
            .unwrap(),
        );
        let place =
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(2), projections, ty).unwrap();
        let body = test_function(vec![test_block(
            106,
            vec![test_assign(3, SemanticOperandV1::Copy(place))],
            SemanticTerminatorKindV1::Return,
        )]);
        let text = bounded_text(&body, failure(0, Some(0)));
        let exact = text
            .lines()
            .find(|line| line.starts_with("EXACT_FAILING_"))
            .unwrap();
        assert_eq!(exact.matches("->Subtype:").count(), (count - 1).min(32));
        if count == 32 {
            assert!(text.contains("static_local_scan complete=true statements=1 candidates=1"));
            assert!(exact.contains("->Index("));
            assert!(!exact.contains("[projections truncated"));
        } else {
            assert!(text.contains("static_local_scan complete=false statements=1 candidates=0"));
            assert!(!exact.contains("->Index("));
            assert!(exact.contains("[projections truncated,total=33"));
        }
    }
}
