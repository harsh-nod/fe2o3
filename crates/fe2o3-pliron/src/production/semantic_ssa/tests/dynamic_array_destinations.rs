use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticConstantV1, SemanticConstantValueV1, SemanticScalarValueV1,
};

fn array_types() -> Vec<SemanticTypeDeclV1> {
    let mut types = test_types(false);
    types[0] = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(test_bytes(40)),
        SemanticLayoutIdentityV1::from_sha256(test_bytes(41)),
        SemanticTypeLayoutV1::new(Some(16), 4).unwrap(),
        SemanticTypeShapeV1::Array {
            element: SemanticTypeIdV1::from_index(1),
            length: 4,
        },
    );
    types
}

fn scalar(value: u128) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        SemanticTypeIdV1::from_index(1),
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, 4).unwrap()),
    ))
}

fn dynamic_element() -> SemanticPlaceV1 {
    let ty = SemanticTypeIdV1::from_index(1);
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(2)),
                ty,
            )
            .unwrap(),
        ],
        ty,
    )
    .unwrap()
}

fn moved_element() -> SemanticStatementV1 {
    test_assign(
        3,
        SemanticOperandV1::Move(test_constant_index_place(1, 0, 4)),
    )
}

#[test]
fn dynamic_owned_array_write_preserves_later_move_tracking() {
    for read_moved_element in [false, true] {
        let mut statements = vec![
            test_assign(2, scalar(1)),
            test_assign_to(dynamic_element(), scalar(7)),
            moved_element(),
        ];
        if read_moved_element {
            statements.push(test_assign(
                3,
                SemanticOperandV1::Copy(test_constant_index_place(1, 0, 4)),
            ));
        }
        let function = test_function(vec![test_block(
            90,
            statements,
            SemanticTerminatorKindV1::Return,
        )]);
        let result = plan_test_function(&function, &array_types());
        if read_moved_element {
            assert!(matches!(
                result,
                Err(ProductionSemanticSsaErrorV1::PartialMove {
                    local: 1,
                    statement: Some(3),
                    violation: SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
                    ..
                })
            ));
        } else {
            assert_eq!(
                result.unwrap().partial_move_certificate().projected_moves(),
                1
            );
        }
    }
}

#[test]
fn dynamic_owned_array_write_cannot_reinitialize_a_moved_array_or_element() {
    for whole in [false, true] {
        let prior_move = if whole {
            test_assign(3, SemanticOperandV1::Move(test_place(1, None)))
        } else {
            moved_element()
        };
        let function = test_function(vec![test_block(
            91,
            vec![
                test_assign(2, scalar(0)),
                prior_move,
                test_assign_to(dynamic_element(), scalar(7)),
                moved_element(),
            ],
            SemanticTerminatorKindV1::Return,
        )]);
        assert!(matches!(
            plan_test_function(&function, &array_types()),
            Err(ProductionSemanticSsaErrorV1::PartialMove {
                local: 1,
                statement: Some(2),
                violation: SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
                ..
            })
        ));
    }
}

#[test]
fn dynamic_owned_array_write_requires_a_live_index_and_exact_array_type() {
    let function = test_function(vec![test_block(
        92,
        vec![
            test_assign(2, scalar(0)),
            test_assign(3, SemanticOperandV1::Move(test_scalar_place(2))),
            test_assign_to(dynamic_element(), scalar(7)),
            moved_element(),
        ],
        SemanticTerminatorKindV1::Return,
    )]);
    assert!(plan_test_function(&function, &array_types()).is_err());

    let function = test_function(vec![test_block(
        93,
        vec![
            test_assign(2, scalar(0)),
            test_assign_to(dynamic_element(), scalar(7)),
            moved_element(),
        ],
        SemanticTerminatorKindV1::Return,
    )]);
    assert!(matches!(
        plan_test_function(&function, &test_types(false)),
        Err(ProductionSemanticSsaErrorV1::PartialMove {
            local: 1,
            statement: Some(1),
            violation: SemanticPartialMoveViolationV1::UnsupportedProjection,
            ..
        })
    ));
    assert!(
        plan_semantic_function_ssa_v1(
            SemanticFunctionIdV1::from_index(0),
            &function,
            ProductionSemanticSsaLimitsV1::default()
        )
        .is_err()
    );
    let mut types = array_types();
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(test_bytes(44)),
        SemanticLayoutIdentityV1::from_sha256(test_bytes(45)),
        SemanticTypeLayoutV1::new(Some(4), 4).unwrap(),
        SemanticTypeShapeV1::Opaque,
    ));
    types[0] = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(test_bytes(40)),
        SemanticLayoutIdentityV1::from_sha256(test_bytes(41)),
        SemanticTypeLayoutV1::new(Some(16), 4).unwrap(),
        SemanticTypeShapeV1::Array {
            element: SemanticTypeIdV1::from_index(2),
            length: 4,
        },
    );
    assert!(plan_test_function(&function, &types).is_err());
}

#[test]
fn dynamic_owned_array_move_remains_rejected() {
    let function = test_function(vec![test_block(
        94,
        vec![
            test_assign(2, scalar(0)),
            test_assign(3, SemanticOperandV1::Move(dynamic_element())),
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
