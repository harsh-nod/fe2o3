use super::*;

fn holder_owner(complete: bool) -> ProductionSemanticSsaOwnerV1 {
    use fe2o3_mir_model::semantic_mir_v1::{SemanticAggregateLayoutV1, SemanticAggregateTypeV1};
    let base = admitted_single_function_semantic();
    let mut types = scalar_types(&base);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(test_bytes(162)),
        SemanticLayoutIdentityV1::from_sha256(test_bytes(163)),
        SemanticTypeLayoutV1::aggregate(
            Some(16),
            8,
            SemanticAggregateLayoutV1::new(vec![0, 8], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(
            SemanticAggregateTypeV1::new(vec![SemanticTypeIdV1::from_index(1); 2]).unwrap(),
        ),
    ));
    let field = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::Field(0),
                SemanticTypeIdV1::from_index(1),
            )
            .unwrap(),
        ],
        SemanticTypeIdV1::from_index(1),
    )
    .unwrap();
    let mut statements = Vec::new();
    if complete {
        statements.push(SemanticStatementV1::new(
            base.functions()[0].source(),
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                test_typed_place(1, 2),
                SemanticRvalueV1::new(
                    SemanticTypeIdV1::from_index(2),
                    SemanticRvalueKindV1::aggregate(
                        SemanticAggregateKindV1::Tuple,
                        vec![number(10), number(11)],
                    )
                    .unwrap(),
                ),
            )),
        ));
    }
    statements.push(test_assign_to(field.clone(), number(12)));
    statements.push(test_assign_to(
        test_typed_place(2, 1),
        SemanticOperandV1::Copy(field),
    ));
    let root = root_with(
        &base.functions()[0],
        vec![
            test_local(170, 0, SemanticLocalRoleV1::Return),
            test_local(171, 2, SemanticLocalRoleV1::Temporary),
            test_local(172, 1, SemanticLocalRoleV1::Temporary),
        ],
        vec![test_block(
            180,
            statements,
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
fn holder_actual_source_capture_replays_final_retained_and_promoted_classification() {
    for complete in [false, true] {
        let owner = holder_owner(complete);
        let identity = owner.identity();
        assert_eq!(
            owner.plans[0]
                .plan
                .promoted_variables()
                .contains(&variable(1)),
            complete
        );
        with_capture(owner, |owner| {
            assert_eq!(owner.identity(), identity);
            let view = owner.occurrences_v1().unwrap();
            let function = view.function(SemanticFunctionIdV1::from_index(0)).unwrap();
            let events = function.events();
            let holder: Vec<_> = events
                .iter()
                .filter(|event| event.event().variable() == variable(1))
                .collect();
            assert_eq!(holder.len(), if complete { 4 } else { 3 });
            assert!(holder.iter().all(|event| event.is_promoted() == complete));
            assert!(
                holder
                    .iter()
                    .all(|event| event.resolved().is_some() == complete)
            );
            let update_statement = if complete { 1 } else { 0 };
            let destination: Vec<_> = holder
                .iter()
                .filter(|event| {
                    event.site() == statement(0, update_statement)
                        && event.operand() == Operand::Destination
                })
                .collect();
            assert_eq!(destination.len(), 2);
            assert_eq!(destination[0].role(), EventRole::BaseUse);
            assert_eq!(destination[1].role(), EventRole::DestinationDefine);
        });
    }
}

#[test]
fn holder_actual_source_capture_rejects_changed_promotion_identity_without_publishing() {
    let mut owner = holder_owner(false);
    let replacement = holder_owner(true);
    owner.plans = replacement.plans;
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    budget.reserve_storage(37).unwrap();
    assert!(matches!(
        owner.try_capture_occurrences_with_budget_v1(&mut budget),
        Err(CaptureError::Replay(
            ProductionSemanticSsaErrorV1::ReplayMismatch
        ))
    ));
    assert!(owner.occurrences_v1().is_none());
    assert_eq!(budget.storage(), 37);
}
