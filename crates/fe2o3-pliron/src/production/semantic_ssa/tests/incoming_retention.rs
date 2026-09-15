use super::*;

fn go(target: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Goto(test_edge(SemanticEdgeRoleV1::Goto, target))
}

#[test]
fn single_edge_inputs_preserve_exact_source_plan_and_replay() {
    let function = test_function(vec![
        test_block(
            130,
            vec![test_assign(
                2,
                SemanticOperandV1::Move(test_place(1, Some(0))),
            )],
            go(1),
        ),
        test_block(131, vec![], go(2)),
        test_block(
            132,
            vec![test_assign_to(
                test_place(1, Some(0)),
                SemanticOperandV1::Copy(test_scalar_place(2)),
            )],
            go(3),
        ),
        test_block(
            133,
            vec![test_assign(
                3,
                SemanticOperandV1::Move(test_place(1, Some(1))),
            )],
            go(4),
        ),
        test_block(
            134,
            vec![test_assign_to(
                test_place(1, Some(1)),
                SemanticOperandV1::Copy(test_scalar_place(3)),
            )],
            go(5),
        ),
        test_block(
            135,
            vec![test_assign(
                2,
                SemanticOperandV1::Copy(test_place(1, Some(0))),
            )],
            SemanticTerminatorKindV1::Return,
        ),
    ]);
    let before = function.clone();
    let types = test_types(false);
    let (input, _, _) =
        semantic_function_ssa_input_v1(&function, Some(&types), &[], &BTreeSet::new());
    let expected = plan_ssa_with_limits_v1(&input, SsaPlannerLimitsV1::default()).unwrap();
    let first = plan_test_function(&function, &types).unwrap();
    let replay = plan_test_function(&function, &types).unwrap();
    assert_eq!(first.plan(), &expected);
    assert_eq!(first, replay);
    assert_eq!(function, before);
    assert_eq!(first.partial_move_certificate().projected_moves(), 2);
}

#[test]
fn single_edge_inputs_cannot_hide_a_moved_field() {
    let function = test_function(vec![
        test_block(
            136,
            vec![test_assign(
                2,
                SemanticOperandV1::Move(test_place(1, Some(0))),
            )],
            go(1),
        ),
        test_block(137, vec![], go(2)),
        test_block(
            138,
            vec![test_assign(
                3,
                SemanticOperandV1::Copy(test_place(1, Some(0))),
            )],
            SemanticTerminatorKindV1::Return,
        ),
    ]);
    assert!(matches!(
        plan_test_function(&function, &test_types(false)),
        Err(ProductionSemanticSsaErrorV1::PartialMove {
            block: 2,
            statement: Some(0),
            violation: SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
            ..
        })
    ));
}

#[test]
fn single_edge_loop_preserves_exact_reinitialization() {
    let function = test_function(vec![
        test_block(139, vec![], go(1)),
        test_block(
            140,
            vec![test_assign(
                2,
                SemanticOperandV1::Move(test_place(1, Some(0))),
            )],
            go(2),
        ),
        test_block(
            141,
            vec![test_assign_to(
                test_place(1, Some(0)),
                SemanticOperandV1::Copy(test_scalar_place(2)),
            )],
            go(1),
        ),
    ]);
    let first = plan_test_function(&function, &test_types(false)).unwrap();
    assert_eq!(first.partial_move_certificate().projected_moves(), 1);
    assert_eq!(
        first,
        plan_test_function(&function, &test_types(false)).unwrap()
    );
}

#[test]
fn single_edge_loop_cannot_hide_a_later_iteration_read() {
    let function = test_function(vec![
        test_block(142, vec![], go(1)),
        test_block(
            143,
            vec![test_assign(
                3,
                SemanticOperandV1::Copy(test_place(1, Some(0))),
            )],
            go(2),
        ),
        test_block(
            144,
            vec![test_assign(
                2,
                SemanticOperandV1::Move(test_place(1, Some(0))),
            )],
            go(3),
        ),
        test_block(145, vec![], go(1)),
    ]);
    assert!(matches!(
        plan_test_function(&function, &test_types(false)),
        Err(ProductionSemanticSsaErrorV1::PartialMove {
            block: 1,
            statement: Some(0),
            violation: SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
            ..
        })
    ));
}
