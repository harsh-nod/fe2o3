mod exclusive_enum_index_tests_v1 {
    use super::*;

    fn add_enum(types: &mut Vec<SemanticTypeDeclV1>) -> SemanticTypeIdV1 {
        let ty = SemanticTypeIdV1::from_index(types.len() as u32);
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(249)),
            SemanticLayoutIdentityV1::from_sha256(bytes(249)),
            SemanticTypeLayoutV1::new(Some(24), 8).unwrap(),
            SemanticTypeShapeV1::enum_type(
                U64_TYPE,
                vec![
                    SemanticEnumVariantV1::new(
                        0,
                        SemanticAggregateTypeV1::new(vec![U64_TYPE, U64_TYPE]).unwrap(),
                    ),
                    SemanticEnumVariantV1::new(
                        1,
                        SemanticAggregateTypeV1::new(vec![U64_TYPE, U64_TYPE]).unwrap(),
                    ),
                ],
            )
            .unwrap(),
        ));
        ty
    }

    fn projected(
        local: u32,
        ty: SemanticTypeIdV1,
        variant: u32,
        field: u32,
        moved: bool,
    ) -> SemanticOperandV1 {
        let place = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(local),
            vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::Downcast(variant), ty).unwrap(),
                SemanticProjectionV1::new(SemanticProjectionKindV1::Field(field), U64_TYPE)
                    .unwrap(),
            ],
            U64_TYPE,
        )
        .unwrap();
        if moved {
            SemanticOperandV1::Move(place)
        } else {
            SemanticOperandV1::Copy(place)
        }
    }

    fn construct(
        local: u32,
        ty: SemanticTypeIdV1,
        variant: u32,
        payload: SemanticOperandV1,
    ) -> SemanticStatementV1 {
        typed_assignment(
            local,
            ty,
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(
                    SemanticAggregateKindV1::EnumVariant(variant),
                    vec![payload, typed_constant(U64_TYPE, 17, 8)],
                )
                .unwrap(),
            ),
        )
    }

    fn fixture(
        moved: bool,
    ) -> (
        Vec<SemanticTypeDeclV1>,
        SemanticFunctionDeclV1,
        SemanticTypeIdV1,
    ) {
        let mut types = assertion_proof_types();
        let ty = add_enum(&mut types);
        let operand = if moved {
            SemanticOperandV1::Move(typed_place(1, ty))
        } else {
            typed_operand(1, ty)
        };
        let function = projection_function_with_locals(
            vec![
                block(
                    230,
                    vec![],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 2)),
                ),
                block(
                    231,
                    vec![
                        typed_assignment(2, ty, SemanticRvalueKindV1::Use(operand)),
                        typed_assignment(
                            3,
                            U64_TYPE,
                            SemanticRvalueKindV1::Use(projected(2, ty, 0, 0, moved)),
                        ),
                        typed_assignment(
                            0,
                            U64_TYPE,
                            SemanticRvalueKindV1::Use(projected(2, ty, 0, 1, moved)),
                        ),
                    ],
                    SemanticTerminatorKindV1::Return,
                ),
                block(
                    232,
                    vec![construct(1, ty, 0, typed_constant(U64_TYPE, 9, 8))],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
                ),
            ],
            vec![
                local(230, U64_TYPE, SemanticLocalRoleV1::Return),
                local(231, ty, SemanticLocalRoleV1::Temporary),
                local(232, ty, SemanticLocalRoleV1::Temporary),
                local(233, U64_TYPE, SemanticLocalRoleV1::Temporary),
            ],
        );
        (types, function, ty)
    }

    fn replace(
        base: &SemanticFunctionDeclV1,
        blocks: Vec<SemanticBasicBlockV1>,
    ) -> SemanticFunctionDeclV1 {
        typed_global_fixture_with_body_v1(base, base.locals().to_vec(), blocks)
    }

    fn query(
        types: &[SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
        operands: &[(SemanticOperandV1, usize, usize)],
        prefix: usize,
        exclusive: bool,
    ) -> Result<(Vec<Option<TotalUnsignedIndexValueV1>>, usize), ProductionRankedProjectionErrorV1>
    {
        let inventory = assertion_definition_inventory(function)?;
        let constants = constant_locals(function)?;
        let mut proof = SemanticAssertProofsV1::new(types, function)?;
        proof.charge(prefix)?;
        let mut slots = vec![None; function.locals().len()];
        let mut argument = 1;
        let mut operations = Vec::new();
        let mut value = 0;
        // No invented invocation roots: constant-only component queries must
        // remain invocation-independent. The full consumer test uses real roots.
        let indices = vec![None; function.locals().len()];
        let mut projector = TotalUnsignedIndexProjectorV1::new(
            types,
            function,
            &constants,
            &inventory.counts,
            &inventory.address_escaped,
            &inventory.assignments,
            &mut proof,
            &mut slots,
            &mut argument,
            &mut operations,
            &mut value,
        )?;
        if exclusive {
            projector = projector
                .with_invocation_roots(&[], &indices, 1024)?
                .with_exclusive_source_arguments_v1()?;
        }
        let mut results = Vec::new();
        for (operand, block, statement) in operands {
            results.push(projector.resolve_operand(operand, *block, *statement)?);
        }
        assert!(projector.states.len() <= MAX_PURE_UNIFORM_INDEX_NODES_V1);
        Ok((results, projector.assertion_proofs.work))
    }

    #[test]
    fn exclusive_enum_index_keeps_late_numbered_constructor_and_each_exact_field() {
        for moved in [false, true] {
            let (types, function, ty) = fixture(moved);
            let requests = [
                (projected(2, ty, 0, 0, moved), 1, 1),
                (projected(2, ty, 0, 1, moved), 1, 2),
                (projected(2, ty, 1, 0, moved), 1, 1),
                (projected(2, ty, 0, 0, moved), 1, 0),
            ];
            let (values, _) = query(&types, &function, &requests, 0, true).unwrap();
            assert_eq!(values[0].unwrap().exact, Some(9));
            assert_eq!(values[1].unwrap().exact, Some(17));
            assert!(!values[0].unwrap().invocation_dependent);
            assert!(values[2].is_none());
            assert!(values[3].is_none());
        }
    }

    #[test]
    fn exclusive_enum_index_does_not_enable_uniform_bound_or_optional_projection() {
        let (types, function, ty) = fixture(false);
        let (values, _) = query(
            &types,
            &function,
            &[(projected(2, ty, 0, 0, false), 1, 1)],
            0,
            false,
        )
        .unwrap();
        assert!(values[0].is_none());
    }

    #[test]
    fn exclusive_enum_index_requires_the_original_operand_and_source_coordinate() {
        let (types, function, ty) = fixture(false);
        for request in [
            (projected(2, ty, 0, 0, true), 1, 1),
            (projected(2, ty, 0, 1, false), 1, 1),
            (projected(2, ty, 0, 0, false), 2, 1),
            (projected(2, ty, 0, 0, false), 1, 99),
        ] {
            assert!(query(&types, &function, &[request], 0, true).unwrap().0[0].is_none());
        }
    }

    #[test]
    fn exclusive_enum_index_rejects_wrong_variant_field_and_projection_types() {
        let (types, function, ty) = fixture(false);
        let wrong_downcast = SemanticOperandV1::Copy(
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(2),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Downcast(0), U64_TYPE)
                        .unwrap(),
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), U64_TYPE)
                        .unwrap(),
                ],
                U64_TYPE,
            )
            .unwrap(),
        );
        let wrong_scalar = SemanticOperandV1::Copy(
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(2),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Downcast(0), ty).unwrap(),
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), SCALAR_TYPE)
                        .unwrap(),
                ],
                SCALAR_TYPE,
            )
            .unwrap(),
        );
        for operand in [
            projected(2, ty, 1, 0, false),
            projected(2, ty, 0, 2, false),
            wrong_downcast,
            wrong_scalar,
        ] {
            // Keep the hostile operand at a real retained source use; this
            // reaches the type/variant check, not merely the occurrence guard.
            let mut blocks = function.blocks().to_vec();
            let mut statements = blocks[1].statements().to_vec();
            statements[1] =
                typed_assignment(3, operand.ty(), SemanticRvalueKindV1::Use(operand.clone()));
            blocks[1] = block(231, statements, SemanticTerminatorKindV1::Return);
            let changed = replace(&function, blocks);
            assert!(
                query(&types, &changed, &[(operand, 1, 1)], 0, true)
                    .unwrap()
                    .0[0]
                    .is_none()
            );
        }
    }

    #[test]
    fn exclusive_enum_index_rejects_malformed_sibling_arity_and_uninhabited_variant() {
        for mutation in 0..4 {
            let (mut types, function, ty) = fixture(false);
            let mut blocks = function.blocks().to_vec();
            let statement = match mutation {
                0 => typed_assignment(
                    1,
                    ty,
                    SemanticRvalueKindV1::Aggregate(
                        SemanticAggregateRvalueV1::new(
                            SemanticAggregateKindV1::Tuple,
                            vec![
                                typed_constant(U64_TYPE, 9, 8),
                                typed_constant(U64_TYPE, 17, 8),
                            ],
                        )
                        .unwrap(),
                    ),
                ),
                1 | 2 => typed_assignment(
                    1,
                    ty,
                    SemanticRvalueKindV1::Aggregate(
                        SemanticAggregateRvalueV1::new(
                            SemanticAggregateKindV1::EnumVariant(0),
                            if mutation == 1 {
                                vec![typed_constant(U64_TYPE, 9, 8)]
                            } else {
                                vec![
                                    typed_constant(U64_TYPE, 9, 8),
                                    typed_constant(SCALAR_TYPE, 17, 4),
                                ]
                            },
                        )
                        .unwrap(),
                    ),
                ),
                _ => {
                    let original = &types[ty.index() as usize];
                    types[ty.index() as usize] = SemanticTypeDeclV1::new(
                        original.identity(),
                        original.layout_identity(),
                        original.layout().clone(),
                        SemanticTypeShapeV1::enum_type(
                            U64_TYPE,
                            vec![
                                SemanticEnumVariantV1::new_with_inhabitedness(
                                    0,
                                    SemanticAggregateTypeV1::new(vec![U64_TYPE, U64_TYPE]).unwrap(),
                                    true,
                                ),
                                SemanticEnumVariantV1::new(
                                    1,
                                    SemanticAggregateTypeV1::new(vec![]).unwrap(),
                                ),
                            ],
                        )
                        .unwrap(),
                    );
                    blocks[2].statements()[0].clone()
                }
            };
            blocks[2] = block(232, vec![statement], blocks[2].terminator().kind().clone());
            let changed = replace(&function, blocks);
            assert!(
                query(
                    &types,
                    &changed,
                    &[(projected(2, ty, 0, 0, false), 1, 1)],
                    0,
                    true
                )
                .unwrap()
                .0[0]
                    .is_none()
            );
        }
    }

    #[test]
    fn exclusive_enum_index_requires_complete_cfg_dominance_not_numeric_order() {
        for bypass in [false, true] {
            let (types, function, ty) = fixture(false);
            let mut blocks = function.blocks().to_vec();
            blocks[0] = block(
                230,
                vec![],
                if bypass {
                    SemanticTerminatorKindV1::SwitchInt {
                        discriminant: typed_constant(U64_TYPE, 0, 8),
                        targets: SemanticSwitchTargetsV1::new(
                            vec![SemanticSwitchTargetV1::new(
                                0,
                                cfg_edge(SemanticEdgeRoleV1::SwitchValue, 2),
                            )],
                            cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 1),
                        )
                        .unwrap(),
                    }
                } else {
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1))
                },
            );
            let changed = replace(&function, blocks);
            assert!(
                query(
                    &types,
                    &changed,
                    &[(projected(2, ty, 0, 0, false), 1, 1)],
                    0,
                    true
                )
                .unwrap()
                .0[0]
                    .is_none()
            );
        }
    }

    #[test]
    fn exclusive_enum_index_rejects_duplicate_partial_write_deinitialization_and_cycles() {
        for mutation in 0..5 {
            let (types, function, ty) = fixture(false);
            let mut blocks = function.blocks().to_vec();
            let mut statements = blocks[2].statements().to_vec();
            let kind = match mutation {
                0 => statements[0].kind().clone(),
                1 => SemanticStatementKindV1::SetDiscriminant {
                    place: typed_place(1, ty),
                    variant_index: 1,
                },
                2 => SemanticStatementKindV1::Deinitialize(typed_place(1, ty)),
                3 => {
                    let source = projected(1, ty, 0, 0, false);
                    let place = raw_operand_place(&source).unwrap().clone();
                    SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                        place,
                        SemanticRvalueV1::new(
                            U64_TYPE,
                            SemanticRvalueKindV1::Use(typed_constant(U64_TYPE, 7, 8)),
                        ),
                    ))
                }
                _ => {
                    statements.clear();
                    typed_assignment(1, ty, SemanticRvalueKindV1::Use(typed_operand(1, ty)))
                        .kind()
                        .clone()
                }
            };
            statements.push(SemanticStatementV1::new(
                SemanticSourceProvenanceV1::unavailable(),
                kind,
            ));
            blocks[2] = block(232, statements, blocks[2].terminator().kind().clone());
            let changed = replace(&function, blocks);
            assert!(
                query(
                    &types,
                    &changed,
                    &[(projected(2, ty, 0, 0, false), 1, 1)],
                    0,
                    true
                )
                .unwrap()
                .0[0]
                    .is_none()
            );
        }
    }

    #[test]
    fn exclusive_enum_index_rejects_escaped_carrier_and_unknown_forwarding_source() {
        for escape in [false, true] {
            let (mut types, function, ty) = fixture(false);
            let mut locals = function.locals().to_vec();
            let mut blocks = function.blocks().to_vec();
            if escape {
                let pointer = SemanticTypeIdV1::from_index(types.len() as u32);
                types.push(SemanticTypeDeclV1::new(
                    SemanticTypeIdentityV1::from_sha256(bytes(250)),
                    SemanticLayoutIdentityV1::from_sha256(bytes(250)),
                    SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
                    SemanticTypeShapeV1::Pointer(
                        SemanticPointerTypeV1::new_with_kind(
                            ty,
                            SemanticPointerKindV1::Reference,
                            SemanticMutabilityV1::Mutable,
                            0,
                            64,
                            SemanticPointerMetadataV1::None,
                        )
                        .unwrap(),
                    ),
                ));
                locals.push(local(234, pointer, SemanticLocalRoleV1::Temporary));
                let mut statements = blocks[2].statements().to_vec();
                statements.push(typed_assignment(
                    4,
                    pointer,
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Mutable,
                        place: typed_place(1, ty),
                    },
                ));
                blocks[2] = block(232, statements, blocks[2].terminator().kind().clone());
            } else {
                locals.push(local(234, ty, SemanticLocalRoleV1::Temporary));
                blocks[2] = block(
                    232,
                    vec![typed_assignment(
                        1,
                        ty,
                        SemanticRvalueKindV1::Use(typed_operand(4, ty)),
                    )],
                    blocks[2].terminator().kind().clone(),
                );
            }
            let changed = typed_global_fixture_with_body_v1(&function, locals, blocks);
            assert!(
                query(
                    &types,
                    &changed,
                    &[(projected(2, ty, 0, 0, false), 1, 1)],
                    0,
                    true
                )
                .unwrap()
                .0[0]
                    .is_none()
            );
        }
    }

    #[test]
    fn exclusive_enum_index_keeps_exact_shared_work_boundary_and_prior_debit() {
        let (types, function, ty) = fixture(false);
        let requests = [(projected(2, ty, 0, 0, false), 1, 1)];
        let (_, work) = query(&types, &function, &requests, 0, true).unwrap();
        let prefix = MAX_PROJECTED_LOOP_GRAPH_WORK_V1 - work;
        assert!(query(&types, &function, &requests, prefix, true).unwrap().0[0].is_some());
        assert!(query(&types, &function, &requests, prefix + 1, true).is_err());
        assert_eq!(
            query(&types, &function, &requests, 17, true).unwrap().1,
            work + 17
        );
    }

    #[test]
    fn exclusive_enum_index_accepts_the_constructed_variant_not_a_fixed_success_number() {
        let (types, function, ty) = fixture(false);
        let mut blocks = function.blocks().to_vec();
        blocks[2] = block(
            232,
            vec![construct(1, ty, 1, typed_constant(U64_TYPE, 23, 8))],
            blocks[2].terminator().kind().clone(),
        );
        blocks[1] = block(
            231,
            vec![
                blocks[1].statements()[0].clone(),
                typed_assignment(
                    3,
                    U64_TYPE,
                    SemanticRvalueKindV1::Use(projected(2, ty, 1, 0, false)),
                ),
                typed_assignment(
                    0,
                    U64_TYPE,
                    SemanticRvalueKindV1::Use(projected(2, ty, 1, 1, false)),
                ),
            ],
            SemanticTerminatorKindV1::Return,
        );
        let changed = replace(&function, blocks);
        let (values, _) = query(
            &types,
            &changed,
            &[
                (projected(2, ty, 1, 0, false), 1, 1),
                (projected(2, ty, 0, 0, false), 1, 1),
            ],
            0,
            true,
        )
        .unwrap();
        assert_eq!(values[0].unwrap().exact, Some(23));
        assert!(values[1].is_none());
    }

    #[test]
    fn exclusive_enum_index_retains_the_existing_node_ceiling_for_forwarding_chains() {
        for count in [8, MAX_PURE_UNIFORM_INDEX_NODES_V1 + 1] {
            let mut types = assertion_proof_types();
            let ty = add_enum(&mut types);
            let mut locals = vec![local(0, U64_TYPE, SemanticLocalRoleV1::Return)];
            let mut statements = Vec::new();
            for index in 1..=count {
                locals.push(local(index as u8, ty, SemanticLocalRoleV1::Temporary));
                statements.push(if index == 1 {
                    construct(1, ty, 0, typed_constant(U64_TYPE, 9, 8))
                } else {
                    typed_assignment(
                        index as u32,
                        ty,
                        SemanticRvalueKindV1::Use(typed_operand(index as u32 - 1, ty)),
                    )
                });
            }
            statements.push(typed_assignment(
                0,
                U64_TYPE,
                SemanticRvalueKindV1::Use(projected(count as u32, ty, 0, 0, false)),
            ));
            let function = projection_function_with_locals(
                vec![block(230, statements, SemanticTerminatorKindV1::Return)],
                locals,
            );
            let result = query(
                &types,
                &function,
                &[(projected(count as u32, ty, 0, 0, false), 0, count)],
                0,
                true,
            );
            if count == 8 {
                assert_eq!(result.unwrap().0[0].unwrap().exact, Some(9));
            } else {
                assert!(matches!(
                    result,
                    Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "pure uniform index expression exceeds its node limit"
                    ))
                ));
            }
        }
    }

    #[test]
    fn exclusive_enum_index_backedge_reexecutes_the_same_dominating_constructor() {
        let (types, function, ty) = fixture(false);
        let mut blocks = function.blocks().to_vec();
        blocks[1] = block(
            231,
            blocks[1].statements().to_vec(),
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: typed_constant(U64_TYPE, 0, 8),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        0,
                        cfg_edge(SemanticEdgeRoleV1::SwitchValue, 2),
                    )],
                    cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 3),
                )
                .unwrap(),
            },
        );
        blocks.push(block(233, vec![], SemanticTerminatorKindV1::Return));
        let changed = replace(&function, blocks);
        let inventory = assertion_definition_inventory(&changed).unwrap();
        assert_eq!(inventory.counts[1], 1);
        assert_eq!(inventory.counts[2], 1);
        let mut proof = SemanticAssertProofsV1::new(&types, &changed).unwrap();
        assert!(proof.graph.predecessors(2).unwrap().contains(&1));
        assert!(proof.block_dominates(2, 1).unwrap());
        let (values, _) = query(
            &types,
            &changed,
            &[
                (projected(2, ty, 0, 0, false), 1, 1),
                (projected(2, ty, 0, 1, false), 1, 2),
            ],
            0,
            true,
        )
        .unwrap();
        assert_eq!(values[0].unwrap().exact, Some(9));
        assert_eq!(values[1].unwrap().exact, Some(17));
    }

    #[test]
    fn exclusive_enum_index_backedge_never_hides_an_initial_constructor_bypass() {
        let (types, function, ty) = fixture(false);
        let mut blocks = function.blocks().to_vec();
        blocks[0] = block(
            230,
            vec![],
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: typed_constant(U64_TYPE, 0, 8),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        0,
                        cfg_edge(SemanticEdgeRoleV1::SwitchValue, 2),
                    )],
                    cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 3),
                )
                .unwrap(),
            },
        );
        blocks[1] = block(
            231,
            blocks[1].statements().to_vec(),
            SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 2)),
        );
        blocks.push(block(
            233,
            vec![],
            SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
        ));
        let changed = replace(&function, blocks);
        let inventory = assertion_definition_inventory(&changed).unwrap();
        assert_eq!(inventory.counts[1], 1);
        assert_eq!(inventory.counts[2], 1);
        let mut proof = SemanticAssertProofsV1::new(&types, &changed).unwrap();
        assert_eq!(proof.graph.predecessors(1).unwrap(), &[2, 3]);
        assert!(proof.graph.predecessors(2).unwrap().contains(&1));
        assert!(!proof.block_dominates(2, 1).unwrap());
        assert!(
            query(
                &types,
                &changed,
                &[(projected(2, ty, 0, 0, false), 1, 1)],
                0,
                true
            )
            .unwrap()
            .0[0]
                .is_none()
        );
    }

    fn consumer_fixture(
        invocation: bool,
        checked: Option<(u64, bool)>,
        variant: u32,
    ) -> (
        Vec<SemanticTypeDeclV1>,
        Vec<SemanticCallableDeclV1>,
        SemanticFunctionDeclV1,
    ) {
        let (mut types, callables, base) =
            derived_exclusive_index_fixture(64, 16, 63, invocation, false);
        let ty = add_enum(&mut types);
        let mut locals = base.locals().to_vec();
        let carrier = locals.len() as u32;
        let alias = carrier + 1;
        let payload = carrier + 2;
        let discriminator = carrier + 3;
        let checked_local = carrier + 4;
        for (index, ty) in [ty, ty, U64_TYPE, U64_TYPE, CHECKED_U64_TYPE]
            .into_iter()
            .enumerate()
        {
            locals.push(local(245 + index as u8, ty, SemanticLocalRoleV1::Temporary));
        }
        let mut blocks = base.blocks().to_vec();
        assert_eq!(blocks.len(), 9);
        let SemanticTerminatorKindV1::Call(load) = blocks[6].terminator().kind() else {
            panic!()
        };
        let destination = load.destination().unwrap();
        blocks[6] = block(
            196,
            blocks[6].statements().to_vec(),
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    load.callee(),
                    load.arguments().to_vec(),
                    Some(SemanticCallDestinationV1::new(
                        destination.place().clone(),
                        cfg_edge(SemanticEdgeRoleV1::CallReturn, 9),
                    )),
                    load.unwind(),
                )
                .unwrap(),
            ),
        );
        let SemanticTerminatorKindV1::Call(store) = blocks[7].terminator().kind() else {
            panic!()
        };
        let mut arguments = store.arguments().to_vec();
        arguments[1] = typed_operand(payload, U64_TYPE);
        let store_kind = SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                store.callee(),
                arguments,
                store.destination().cloned(),
                store.unwind(),
            )
            .unwrap(),
        );
        let mut computations = blocks[7].statements().to_vec();
        blocks[7] = block(
            197,
            vec![typed_assignment(
                payload,
                U64_TYPE,
                SemanticRvalueKindV1::Use(projected(alias, ty, 0, 0, true)),
            )],
            store_kind,
        );
        let base_index = typed_operand(carrier - 1, U64_TYPE);
        let (payload_source, next) = if let Some((constant, wrong)) = checked {
            let right = typed_constant(U64_TYPE, u128::from(constant), 8);
            computations.push(typed_assignment(
                checked_local,
                CHECKED_U64_TYPE,
                SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
                    SemanticCheckedBinaryOpV1::Add,
                    base_index.clone(),
                    right.clone(),
                )),
            ));
            let field = |index, result_type| {
                SemanticOperandV1::Copy(
                    SemanticPlaceV1::new(
                        SemanticLocalIdV1::from_index(checked_local),
                        vec![
                            SemanticProjectionV1::new(
                                SemanticProjectionKindV1::Field(index),
                                result_type,
                            )
                            .unwrap(),
                        ],
                        result_type,
                    )
                    .unwrap(),
                )
            };
            (
                field(0, U64_TYPE),
                SemanticTerminatorKindV1::Assert {
                    condition: field(1, BOOL_TYPE),
                    expected: false,
                    message: SemanticAssertMessageV1::Overflow {
                        operation: if wrong {
                            SemanticBinaryOpV1::Subtract
                        } else {
                            SemanticBinaryOpV1::Add
                        },
                        left: base_index,
                        right,
                    },
                    target: cfg_edge(SemanticEdgeRoleV1::AssertSuccess, 10),
                    unwind: SemanticUnwindActionV1::Unreachable,
                },
            )
        } else {
            (
                base_index,
                SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 10)),
            )
        };
        blocks.push(block(240, computations, next));
        blocks.push(block(
            241,
            vec![
                construct(carrier, ty, variant, payload_source),
                typed_assignment(
                    discriminator,
                    U64_TYPE,
                    SemanticRvalueKindV1::Discriminant(typed_place(carrier, ty)),
                ),
            ],
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: typed_operand(discriminator, U64_TYPE),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        0,
                        cfg_edge(SemanticEdgeRoleV1::SwitchValue, 11),
                    )],
                    cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 8),
                )
                .unwrap(),
            },
        ));
        blocks.push(block(
            242,
            vec![typed_assignment(
                alias,
                ty,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Move(typed_place(carrier, ty))),
            )],
            SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 7)),
        ));
        (
            types,
            callables,
            typed_global_fixture_with_body_v1(&base, locals, blocks),
        )
    }

    #[test]
    fn exclusive_enum_index_full_consumer_retains_invocation_and_exact_checked_arithmetic() {
        for checked in [None, Some((1, false))] {
            let (types, callables, function) = consumer_fixture(true, checked, 0);
            let (projection, operations) = project_capability_index_fixture_with_launch(
                &types,
                &callables,
                &function,
                Some(1024),
            )
            .unwrap();
            let write = projection.direct_write_effects[7].as_ref().unwrap();
            assert_eq!(write.indices.len(), 1);
            assert_eq!(write.comparisons.len(), 1);
            for raw in 0..1024 {
                assert_eq!(
                    evaluate_projected_index(&operations, write.indices[0], raw),
                    (raw / 64) * 16 + raw % 64 + u64::from(checked.is_some())
                );
            }
        }
    }

    #[test]
    fn exclusive_enum_index_full_consumer_keeps_overflow_variant_and_invocation_failures() {
        for (invocation, checked, variant) in [
            (true, Some((u64::MAX, false)), 0),
            (true, Some((1, true)), 0),
            (true, None, 1),
            (false, None, 0),
        ] {
            let (types, callables, function) = consumer_fixture(invocation, checked, variant);
            assert_incomplete(
                project_capability_index_fixture_with_launch(
                    &types,
                    &callables,
                    &function,
                    Some(1024),
                ),
                "a typed global exclusive store requires an exact invocation-derived index",
            );
        }
    }
}
