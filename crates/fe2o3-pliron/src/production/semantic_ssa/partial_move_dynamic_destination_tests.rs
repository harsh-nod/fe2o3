use super::*;

fn array_types() -> Vec<SemanticTypeDeclV1> {
    let mut types = test_types(false);
    types[0] = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(test_bytes(40)),
        SemanticLayoutIdentityV1::from_sha256(test_bytes(41)),
        SemanticTypeLayoutV1::new(Some(32), 4).unwrap(),
        SemanticTypeShapeV1::Array {
            element: SemanticTypeIdV1::from_index(1),
            length: 8,
        },
    );
    types
}

fn indexed_place() -> SemanticPlaceV1 {
    let scalar = SemanticTypeIdV1::from_index(1);
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(2)),
                scalar,
            )
            .unwrap(),
        ],
        scalar,
    )
    .unwrap()
}

fn move_element() -> SemanticStatementV1 {
    test_assign(
        2,
        SemanticOperandV1::Move(test_constant_index_place(1, 0, 8)),
    )
}

fn repair_element() -> SemanticStatementV1 {
    test_assign_to(
        test_constant_index_place(1, 0, 8),
        SemanticOperandV1::Copy(test_scalar_place(2)),
    )
}

fn indexed_write(value: u32) -> SemanticStatementV1 {
    test_assign_to(
        indexed_place(),
        SemanticOperandV1::Copy(test_scalar_place(value)),
    )
}

#[test]
fn indexed_destination_preserves_an_available_array_and_following_read() {
    let function = test_function(vec![test_block(
        120,
        vec![
            move_element(),
            repair_element(),
            indexed_write(2),
            test_assign(3, SemanticOperandV1::Copy(indexed_place())),
        ],
        SemanticTerminatorKindV1::Return,
    )]);
    let plan = plan_test_function(&function, &array_types()).unwrap();
    assert_eq!(plan.partial_move_certificate().projected_moves(), 1);
    assert!(plan.partial_move_certificate().work_units() > 0);
}

#[test]
fn indexed_destination_cannot_repair_a_moved_element() {
    let function = test_function(vec![test_block(
        120,
        vec![move_element(), indexed_write(2)],
        SemanticTerminatorKindV1::Return,
    )]);
    assert!(matches!(
        plan_test_function(&function, &array_types()),
        Err(ProductionSemanticSsaErrorV1::PartialMove {
            local: 1,
            statement: Some(1),
            violation: SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
            ..
        })
    ));
}

#[test]
fn indexed_destination_rejects_a_moved_index() {
    let function = test_function(vec![test_block(
        120,
        vec![
            move_element(),
            repair_element(),
            test_assign(3, SemanticOperandV1::Move(test_scalar_place(2))),
            indexed_write(3),
        ],
        SemanticTerminatorKindV1::Return,
    )]);
    let result = plan_test_function(&function, &array_types());
    assert!(
        matches!(
            result,
            Err(ProductionSemanticSsaErrorV1::Planner {
                error: SsaPlannerErrorV1::UndefinedAtUse { block, variable, .. },
                ..
            }) if block == SsaBlockIdV1::new(0) && variable == SsaVariableIdV1::new(2)
        ),
        "unexpected moved-index result: {result:?}"
    );
}

#[test]
fn indexed_destination_rejects_dead_base_storage() {
    let function = test_function(vec![test_block(
        120,
        vec![
            move_element(),
            repair_element(),
            test_storage_dead(1),
            indexed_write(2),
        ],
        SemanticTerminatorKindV1::Return,
    )]);
    assert!(matches!(
        plan_test_function(&function, &array_types()),
        Err(ProductionSemanticSsaErrorV1::PartialMove {
            local: 1,
            statement: Some(3),
            violation: SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
            ..
        })
    ));
}

#[test]
fn indexed_destination_preserves_maybe_moved_state_at_a_join() {
    let function = test_function(vec![
        test_block(
            120,
            vec![test_assign(
                2,
                SemanticOperandV1::Copy(test_constant_index_place(1, 0, 8)),
            )],
            SemanticTerminatorKindV1::FalseEdge {
                real_target: test_edge(SemanticEdgeRoleV1::FalseEdgeReal, 1),
                imaginary_target: test_edge(SemanticEdgeRoleV1::FalseEdgeImaginary, 2),
            },
        ),
        test_block(
            121,
            vec![move_element()],
            SemanticTerminatorKindV1::Goto(test_edge(SemanticEdgeRoleV1::Goto, 3)),
        ),
        test_block(
            122,
            vec![],
            SemanticTerminatorKindV1::Goto(test_edge(SemanticEdgeRoleV1::Goto, 3)),
        ),
        test_block(
            123,
            vec![indexed_write(2)],
            SemanticTerminatorKindV1::Return,
        ),
    ]);
    assert!(matches!(
        plan_test_function(&function, &array_types()),
        Err(ProductionSemanticSsaErrorV1::PartialMove {
            local: 1,
            block: 3,
            statement: Some(0),
            violation: SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
            ..
        })
    ));
}

#[test]
fn indexed_move_operand_remains_unsupported() {
    let function = test_function(vec![test_block(
        120,
        vec![
            test_assign(
                2,
                SemanticOperandV1::Copy(test_constant_index_place(1, 0, 8)),
            ),
            test_assign(3, SemanticOperandV1::Move(indexed_place())),
        ],
        SemanticTerminatorKindV1::Return,
    )]);
    assert!(matches!(
        plan_test_function(&function, &array_types()),
        Err(ProductionSemanticSsaErrorV1::PartialMove {
            local: 1,
            statement: Some(1),
            violation: SemanticPartialMoveViolationV1::UnsupportedProjection,
            ..
        })
    ));
}
