fn mixed_checked_licm_source_v44(
    operation: SemanticCheckedBinaryOpV1,
) -> ProductionSemanticSsaOwnerV1 {
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticAggregateLayoutV1, SemanticAggregateTypeV1, SemanticCheckedBinaryRvalueV1,
        SemanticPaddingV1, SemanticProjectionKindV1, SemanticProjectionV1,
    };
    let base = mixed_licm_source_v28(true);
    let source = base.source_semantic();
    let original = &source.functions()[0];
    let scalar = SemanticTypeIdV1::from_index(1);
    let boolean = original.locals()[13].ty();
    let mut types = source.types().to_vec();
    let pair = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([244; 32]),
        SemanticLayoutIdentityV1::from_sha256([244; 32]),
        SemanticTypeLayoutV1::aggregate(
            Some(8),
            4,
            SemanticAggregateLayoutV1::new(vec![0, 4], vec![SemanticPaddingV1::new(5, 3).unwrap()])
                .unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![scalar, boolean]).unwrap()),
    ));
    let mut locals = original.locals().to_vec();
    let temporary = locals.len() as u32;
    assert_eq!(temporary, 15);
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256([244; 32]),
        pair,
        SemanticLocalRoleV1::Temporary,
        SemanticSourceProvenanceV1::unavailable(),
    ));
    let place =
        |local, ty| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
    let assign = |destination: SemanticPlaceV1, kind| {
        SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                destination.clone(),
                SemanticRvalueV1::new(destination.ty(), kind),
            )),
        )
    };
    let mut blocks = original.blocks().to_vec();
    let body_index = blocks
        .iter()
        .position(|block| block.identity() == SemanticBlockIdentityV1::from_sha256([241; 32]))
        .expect("the original mixed LICM loop-body identity");
    let body = &blocks[body_index];
    let mut statements = vec![
        assign(
            place(temporary, pair),
            SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
                operation,
                SemanticOperandV1::Copy(place(8, scalar)),
                SemanticOperandV1::Constant(SemanticConstantV1::new(
                    scalar,
                    SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(1, 4).unwrap()),
                )),
            )),
        ),
        assign(
            place(9, scalar),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
                SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(temporary),
                    vec![
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), scalar)
                            .unwrap(),
                    ],
                    scalar,
                )
                .unwrap(),
            )),
        ),
    ];
    assert_eq!(body.statements().len(), 5);
    assert!(
        matches!(body.statements()[0].kind(), SemanticStatementKindV1::Assign(assignment)
        if assignment.destination() == &place(9, scalar)
            && matches!(assignment.value().kind(), SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::BitXor,
                left: SemanticOperandV1::Copy(input),
                ..
            } if input == &place(8, scalar)))
    );
    statements.extend_from_slice(&body.statements()[1..]);
    blocks[body_index] = SemanticBasicBlockV1::new(
        body.identity(),
        body.source(),
        statements,
        body.terminator().clone(),
    )
    .unwrap();
    let root = SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        original.abi().clone(),
        locals,
        original.entry(),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(original.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        types,
        source.allocations().to_vec(),
        source.statics().to_vec(),
        source.vtables().to_vec(),
        vec![root],
        source.callables().to_vec(),
        source.roots().to_vec(),
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

#[test]
fn fixed_source_policy11_licm_moves_checked_pairs_and_replays_both_definition_ordinals() {
    use fe2o3_kernel_ir::{
        BinaryOp, CanonicalKirDefinitionCoordinateV1 as Definition, CheckedBinaryOperator,
        OperationKind, Type,
    };
    for (original_operator, actual_operator) in [
        (SemanticCheckedBinaryOpV1::Add, CheckedBinaryOperator::Add),
        (
            SemanticCheckedBinaryOpV1::Subtract,
            CheckedBinaryOperator::Subtract,
        ),
        (
            SemanticCheckedBinaryOpV1::Multiply,
            CheckedBinaryOperator::Multiply,
        ),
    ] {
        let owner = mixed_checked_licm_source_v44(original_operator);
        with_mixed_fixedpoint_prefix_owner_v28(owner, false, |prefix, source, budget| {
            let checked = prefix.output(budget).unwrap();
            assert_eq!(checked.execution().policy_version(), 11);
            assert_eq!(checked.execution().graph_schema(), 18);
            let prefix_identity = checked.owner().identity();
            let execution = checked.execution().canonical_bytes().to_vec();
            let floor = budget.storage();
            let relocated = prefix.prepare_mixed_fixedpoint_licm_v29(budget).unwrap();
            relocated.check_original_source(source, budget).unwrap();
            relocated.replay(budget).unwrap();
            let input = prefix.output(budget).unwrap().owner();
            let tail = relocated.tail(budget).unwrap();
            assert_eq!(tail.input_identity(), input.identity());
            assert_ne!(tail.output().identity(), input.identity());
            assert!(!tail.grants_authority());
            let mut pairs = 0;
            for row in tail.origins() {
                let original = &input.module().functions[row.input.block.function.0 as usize]
                    .body.as_ref().unwrap().blocks[row.input.block.block as usize]
                    .operations[row.input.operation as usize];
                if row.hoist.is_some()
                    && matches!(original.kind, OperationKind::Binary { op: BinaryOp::Checked(op), .. } if op == actual_operator) {
                    assert_eq!(original.results.len(), 2);
                    assert_eq!(original.results[1].ty, Type::BOOL);
                    let actual = &tail.output().module().functions[row.output.block.function.0 as usize]
                        .body.as_ref().unwrap().blocks[row.output.block.block as usize]
                        .operations[row.output.operation as usize];
                    assert_eq!(actual, original);
                    for result in 0..2 {
                        assert!(relocated.definition_projection(budget).unwrap().iter().any(|projection|
                            projection.input == Definition::Result { operation: row.input, result }
                                && projection.output == Definition::Result { operation: row.output, result }));
                    }
                    pairs += 1;
                }
            }
            assert!(pairs > 0, "a live original checked operation must reach the fixed LICM tail");
            let native = relocated.complete_native_v28(budget).unwrap();
            assert!(std::ptr::eq(native.output(budget).unwrap(), tail.output()));
            assert!(native.source_roles_are_complete());
            assert!(native.final_native_completion_is_complete());
            assert!(!native.grants_artifact_or_launch_authority());
            native.discard(budget).unwrap();
            relocated.discard(budget).unwrap();
            assert_eq!(budget.storage(), floor);
            let checked = prefix.output(budget).unwrap();
            assert_eq!(checked.owner().identity(), prefix_identity);
            assert_eq!(checked.execution().canonical_bytes(), execution);
        }).unwrap();
    }
}
