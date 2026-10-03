use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAggregateLayoutV1, SemanticAggregateTypeV1, SemanticCheckedBinaryOpV1,
    SemanticCheckedBinaryRvalueV1, SemanticPaddingV1,
};

fn checked_owner() -> ProductionSemanticSsaOwnerV1 {
    let base = admitted_single_function_semantic();
    let mut types = base.types().to_vec();
    let u32_scalar = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 32, 4),
        SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
    );
    let bool_scalar = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 8, 1),
        SemanticScalarValidityRangeV1::new(0, 1),
    );
    for (tag, size, scalar, shape) in [
        (
            150,
            4,
            u32_scalar,
            SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            },
        ),
        (152, 1, bool_scalar, SemanticScalarTypeV1::Bool),
    ] {
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(test_bytes(tag)),
            SemanticLayoutIdentityV1::from_sha256(test_bytes(tag + 1)),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(size),
                size,
                SemanticBackendReprV1::scalar(scalar),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(shape),
        ));
    }
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(test_bytes(154)),
        SemanticLayoutIdentityV1::from_sha256(test_bytes(155)),
        SemanticTypeLayoutV1::aggregate(
            Some(8),
            4,
            SemanticAggregateLayoutV1::new(vec![0, 4], vec![SemanticPaddingV1::new(5, 3).unwrap()])
                .unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(
            SemanticAggregateTypeV1::new(vec![
                SemanticTypeIdV1::from_index(1),
                SemanticTypeIdV1::from_index(2),
            ])
            .unwrap(),
        ),
    ));
    let literal = |value: u32| {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            SemanticTypeIdV1::from_index(1),
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value.into(), 4).unwrap()),
        ))
    };
    let nop =
        || SemanticStatementV1::new(base.functions()[0].source(), SemanticStatementKindV1::Nop);
    let checked = SemanticStatementV1::new(
        base.functions()[0].source(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            test_typed_place(2, 3),
            SemanticRvalueV1::new(
                SemanticTypeIdV1::from_index(3),
                SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
                    SemanticCheckedBinaryOpV1::Add,
                    SemanticOperandV1::Copy(test_typed_place(1, 1)),
                    literal(1),
                )),
            ),
        )),
    );
    let root = root_with(
        &base.functions()[0],
        vec![
            test_local(170, 0, SemanticLocalRoleV1::Return),
            test_local(171, 1, SemanticLocalRoleV1::Temporary),
            test_local(172, 3, SemanticLocalRoleV1::Temporary),
            test_local(173, 1, SemanticLocalRoleV1::Temporary),
        ],
        vec![test_block(
            180,
            vec![
                nop(),
                test_assign_to(test_typed_place(1, 1), literal(u32::MAX)),
                nop(),
                checked,
                test_assign_to(
                    test_typed_place(3, 1),
                    SemanticOperandV1::Move(test_typed_place(1, 1)),
                ),
                test_storage_dead(3),
                nop(),
            ],
            SemanticTerminatorKindV1::Return,
        )],
    );
    admit_owner(
        InertSemanticMirRequestV1::new(
            base.target(),
            types,
            vec![],
            vec![],
            vec![],
            vec![root],
            vec![SemanticFunctionIdV1::from_index(0)],
        )
        .unwrap(),
    )
}

#[test]
fn checked_add_capture_uses_actual_original_plan_values_and_preserves_identity() {
    let mut owner = checked_owner();
    let identity = owner.identity();
    let summary = owner.summary();
    let original_plans = owner.plans.clone();
    let original_source = owner.source_semantic_sha256;
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    assert_eq!(owner.identity(), identity);
    assert_eq!(owner.summary(), summary);
    assert_eq!(owner.plans, original_plans);
    assert_eq!(owner.source_semantic_sha256, original_source);
    let view = owner.occurrences_v1().unwrap();
    let rows = view.function(SemanticFunctionIdV1::from_index(0)).unwrap();
    let (range, events) = rows.statement_events(SsaBlockIdV1::new(0), 3).unwrap();
    assert_eq!(range, 1..3);
    assert_eq!(events.len(), 2);
    assert_eq!(
        (events[0].operand(), events[0].event()),
        (Operand::RvalueOperand(0), SsaEventV1::Use(variable(1)))
    );
    assert_eq!(
        (events[1].operand(), events[1].event()),
        (Operand::Destination, SsaEventV1::Define(variable(2)))
    );
    let resolved = original_plans[0]
        .plan()
        .resolved_events(SsaBlockIdV1::new(0))
        .unwrap();
    for row in events {
        let original = resolved
            .iter()
            .find(|(ordinal, _)| *ordinal == row.ordinal())
            .unwrap();
        assert_eq!(row.resolved(), Some(original.1));
        assert!(row.is_promoted() && row.is_reachable());
    }
    let (_, initialized) = rows.statement_events(SsaBlockIdV1::new(0), 1).unwrap();
    let Some(SsaResolvedEventV1::Define { value: input, .. }) = initialized[0].resolved() else {
        panic!("literal assignment must define the retained lhs");
    };
    assert_eq!(
        events[0].resolved(),
        Some(SsaResolvedEventV1::Use {
            variable: variable(1),
            value: input
        })
    );
    let constant = rows
        .constants()
        .iter()
        .find(|row| row.site() == statement(0, 3))
        .unwrap();
    assert_eq!(constant.operand(), Operand::RvalueOperand(1));
    assert_eq!(constant.next_event(), 2);
    owner.verify_replay().unwrap();
    assert_eq!(budget.storage(), 0);
    assert!(!owner.grants_proof_or_artifact_authority());
}

#[test]
fn statement_query_preserves_empty_literal_move_kill_and_invalid_coordinates() {
    with_capture(checked_owner(), |owner| {
        let view = owner.occurrences_v1().unwrap();
        let rows = view.function(SemanticFunctionIdV1::from_index(0)).unwrap();
        for (statement, expected) in [
            (0, 0..0),
            (1, 0..1),
            (2, 1..1),
            (3, 1..3),
            (4, 3..6),
            (5, 6..7),
            (6, 7..7),
        ] {
            let (range, events) = rows
                .statement_events(SsaBlockIdV1::new(0), statement)
                .unwrap();
            assert_eq!(range, expected);
            assert_eq!(events.len(), range.len());
            assert!(
                events
                    .iter()
                    .all(|row| row.site() == super::statement(0, statement))
            );
        }
        let (_, moved) = rows.statement_events(SsaBlockIdV1::new(0), 4).unwrap();
        assert_eq!(
            moved.iter().map(|row| row.event()).collect::<Vec<_>>(),
            [
                SsaEventV1::Use(variable(1)),
                SsaEventV1::Kill(variable(1)),
                SsaEventV1::Define(variable(3)),
            ]
        );
        assert!(rows.statement_events(SsaBlockIdV1::new(0), 7).is_none());
        assert!(rows.statement_events(SsaBlockIdV1::new(1), 0).is_none());
    });
    with_capture(sparse_owner(), |owner| {
        let view = owner.occurrences_v1().unwrap();
        let rows = view.function(SemanticFunctionIdV1::from_index(0)).unwrap();
        let (range, events) = rows.statement_events(SsaBlockIdV1::new(1), 0).unwrap();
        assert_eq!(range, 0..2);
        assert!(
            events
                .iter()
                .all(|row| !row.is_reachable() && row.resolved().is_none())
        );
        let (_, unpromoted) = rows.statement_events(SsaBlockIdV1::new(0), 2).unwrap();
        assert!(unpromoted[0].is_reachable());
        assert!(!unpromoted[0].is_promoted());
        assert!(unpromoted[0].resolved().is_none());
    });
}
