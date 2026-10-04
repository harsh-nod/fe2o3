use super::*;

#[test]
fn actual_u32_multi_entry_source_keeps_guarded_certificate_route_closed() {
    actual_u32_entry_source_keeps_guarded_certificate_route_closed(false);
}

#[test]
fn actual_u32_distant_initializer_keeps_guarded_certificate_route_closed() {
    actual_u32_entry_source_keeps_guarded_certificate_route_closed(true);
}

fn actual_u32_entry_source_keeps_guarded_certificate_route_closed(distant: bool) {
    let (ssa, launch) = fixture::source_with(30, target(false), |function| {
        let edge = |role, target| {
            SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
        };
        let goto = |target| SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, target));
        let switch = |operand, left, right| SemanticTerminatorKindV1::SwitchInt {
            discriminant: operand,
            targets: SemanticSwitchTargetsV1::new(
                vec![SemanticSwitchTargetV1::new(
                    0,
                    edge(SemanticEdgeRoleV1::SwitchValue, left),
                )],
                edge(SemanticEdgeRoleV1::SwitchOtherwise, right),
            )
            .unwrap(),
        };
        let block = |tag, statements, kind| {
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([tag; 32]),
                function.source(),
                statements,
                SemanticTerminatorV1::new(function.source(), kind),
            )
            .unwrap()
        };
        let mut blocks = vec![
            block(
                211,
                vec![assign(2, U32, SemanticRvalueKindV1::Use(constant(0)))],
                switch(value(1, U32), 1, 2),
            ),
            block(212, vec![], goto(3)),
            block(213, vec![], goto(3)),
            block(
                214,
                vec![assign(
                    3,
                    BOOL,
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::LessThan,
                        left: value(2, U32),
                        right: constant(3),
                    },
                )],
                switch(value(3, BOOL), 5, 4),
            ),
            block(
                215,
                vec![assign(
                    2,
                    U32,
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::Add,
                        left: value(2, U32),
                        right: constant(1),
                    },
                )],
                goto(3),
            ),
            block(216, vec![], SemanticTerminatorKindV1::Return),
        ];
        if distant {
            blocks[1] = block(212, vec![], goto(6));
            blocks[2] = block(213, vec![], goto(6));
            blocks.push(block(217, vec![], goto(3)));
        }
        vec![
            SemanticFunctionDeclV1::new(
                function.identity(),
                function.role(),
                function.item_definition_identity(),
                function.monomorphization_identity(),
                function.generic_type_arguments_identity(),
                function.const_generic_arguments_identity(),
                function.source(),
                function.abi().clone(),
                function.locals().to_vec(),
                function.entry(),
                blocks,
            )
            .unwrap()
            .with_kernel_entry(function.kernel_entry().unwrap().clone()),
        ]
    });
    assert_eq!(
        ssa.source_semantic().functions()[0].blocks().len(),
        6 + usize::from(distant)
    );
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let sibling = [0x57_u8; FLOOR];
    budget.reserve_storage(FLOOR).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let capture =
        Capture::try_materialize_with_budget_v1(ssa, launch, Default::default(), &mut budget)
            .unwrap();
    let retained = capture.retained_analysis_storage_v1();
    budget.reserve_storage(retained).unwrap();
    let result = GuardedRankedSourceV1::try_project_v1(
        capture,
        &inputs(1),
        &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
        &mut budget,
    );
    assert!(matches!(
        result,
        Err(ProductionRankedProjectionErrorV1::Incomplete(
            "guarded U32 progress requires its historical single-entry source certificate"
        ))
    ));
    assert_eq!(budget.storage(), FLOOR);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert!(sibling.iter().all(|byte| *byte == 0x57));
    assert_eq!(budget.failed_storage(), None);
}
