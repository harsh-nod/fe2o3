mod exclusive_enum_join_tests_v1 {
    use super::*;

    include!("exclusive_enum_join_tests/edge_rejections77.rs");

    fn enumeration(types: &mut Vec<SemanticTypeDeclV1>) -> SemanticTypeIdV1 {
        let ty = SemanticTypeIdV1::from_index(types.len() as u32);
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(248)),
            SemanticLayoutIdentityV1::from_sha256(bytes(248)),
            SemanticTypeLayoutV1::new(Some(16), 8).unwrap(),
            SemanticTypeShapeV1::enum_type(
                U64_TYPE,
                vec![
                    SemanticEnumVariantV1::new(
                        0,
                        SemanticAggregateTypeV1::new(vec![U64_TYPE]).unwrap(),
                    ),
                    SemanticEnumVariantV1::new(
                        1,
                        SemanticAggregateTypeV1::new(vec![U64_TYPE]).unwrap(),
                    ),
                ],
            )
            .unwrap(),
        ));
        ty
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
                    vec![payload],
                )
                .unwrap(),
            ),
        )
    }

    fn payload(local: u32, ty: SemanticTypeIdV1, variant: u32, moved: bool) -> SemanticOperandV1 {
        let place = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(local),
            vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::Downcast(variant), ty).unwrap(),
                SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), U64_TYPE).unwrap(),
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

    fn go(target: u32) -> SemanticTerminatorKindV1 {
        SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, target))
    }

    fn branch(operand: SemanticOperandV1, yes: u32, no: u32) -> SemanticTerminatorKindV1 {
        SemanticTerminatorKindV1::SwitchInt {
            discriminant: operand,
            targets: SemanticSwitchTargetsV1::new(
                vec![SemanticSwitchTargetV1::new(
                    0,
                    cfg_edge(SemanticEdgeRoleV1::SwitchValue, yes),
                )],
                cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, no),
            )
            .unwrap(),
        }
    }

    fn fixture(
        moved: bool,
    ) -> (
        Vec<SemanticTypeDeclV1>,
        SemanticFunctionDeclV1,
        SemanticTypeIdV1,
    ) {
        let mut types = assertion_proof_types();
        let ty = enumeration(&mut types);
        let source = if moved {
            SemanticOperandV1::Move(typed_place(1, ty))
        } else {
            typed_operand(1, ty)
        };
        let function = projection_function_with_locals(
            vec![
                block(210, vec![], branch(typed_constant(U64_TYPE, 0, 8), 4, 5)),
                block(
                    211,
                    vec![typed_assignment(2, ty, SemanticRvalueKindV1::Use(source))],
                    go(2),
                ),
                block(
                    212,
                    vec![typed_assignment(
                        3,
                        U64_TYPE,
                        SemanticRvalueKindV1::Discriminant(typed_place(2, ty)),
                    )],
                    branch(typed_operand(3, U64_TYPE), 3, 6),
                ),
                block(
                    213,
                    vec![typed_assignment(
                        0,
                        U64_TYPE,
                        SemanticRvalueKindV1::Use(payload(2, ty, 0, moved)),
                    )],
                    SemanticTerminatorKindV1::Return,
                ),
                block(
                    214,
                    vec![construct(1, ty, 0, typed_constant(U64_TYPE, 9, 8))],
                    go(1),
                ),
                block(
                    215,
                    vec![construct(1, ty, 1, typed_constant(U64_TYPE, 17, 8))],
                    go(1),
                ),
                block(
                    216,
                    vec![typed_assignment(
                        0,
                        U64_TYPE,
                        SemanticRvalueKindV1::Use(typed_constant(U64_TYPE, 0, 8)),
                    )],
                    SemanticTerminatorKindV1::Return,
                ),
            ],
            vec![
                local(210, U64_TYPE, SemanticLocalRoleV1::Return),
                local(211, ty, SemanticLocalRoleV1::Temporary),
                local(212, ty, SemanticLocalRoleV1::Temporary),
                local(213, U64_TYPE, SemanticLocalRoleV1::Temporary),
            ],
        );
        (types, function, ty)
    }

    fn replace(
        function: &SemanticFunctionDeclV1,
        blocks: Vec<SemanticBasicBlockV1>,
    ) -> SemanticFunctionDeclV1 {
        typed_global_fixture_with_body_v1(function, function.locals().to_vec(), blocks)
    }

    fn query(
        types: &[SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
        conditions: Option<&global_enum_transport_v1::Conditions<'_>>,
        operand: &SemanticOperandV1,
        block: usize,
        statement: usize,
        prefix: usize,
    ) -> Result<(Option<TotalUnsignedIndexValueV1>, usize), ProductionRankedProjectionErrorV1> {
        let inventory = assertion_definition_inventory(function)?;
        let constants = constant_locals(function)?;
        let mut proof = SemanticAssertProofsV1::new(types, function)?;
        let workspace = proof.graph.workspace_identity();
        proof.charge(prefix)?;
        let mut slots = vec![None; function.locals().len()];
        let mut argument = 0;
        let mut operations = Vec::new();
        let mut next_value = 0;
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
            &mut next_value,
        )?
        .with_invocation_roots(&[], &indices, 1024)?
        .with_exclusive_source_arguments_v1()?
        .with_exclusive_enum_conditions_v1(conditions)?;
        let result = projector.resolve_operand(operand, block, statement);
        assert!(
            projector
                .states
                .values()
                .all(|state| !matches!(state, TotalUnsignedIndexStateV1::Visiting))
        );
        assert_eq!(
            projector.assertion_proofs.graph.workspace_identity(),
            workspace
        );
        Ok((result?, projector.assertion_proofs.work))
    }

    fn resolve(
        types: &[SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
        ty: SemanticTypeIdV1,
    ) -> Option<TotalUnsignedIndexValueV1> {
        let conditions = global_enum_transport_v1::Conditions::new(types, function).unwrap();
        query(
            types,
            function,
            Some(&conditions),
            &payload(2, ty, 0, false),
            3,
            0,
            0,
        )
        .unwrap()
        .0
    }

    #[test]
    fn exclusive_enum_join_two_variants_and_original_copy_move_are_exact() {
        for moved in [false, true] {
            let (types, function, ty) = fixture(moved);
            let inventory = assertion_definition_inventory(&function).unwrap();
            assert_eq!(inventory.counts[1], 2);
            assert!(inventory.assignments[1].is_none());
            assert_eq!(inventory.assignments[2].unwrap().block, 1);
            let conditions = global_enum_transport_v1::Conditions::new(&types, &function).unwrap();
            let value = query(
                &types,
                &function,
                Some(&conditions),
                &payload(2, ty, 0, moved),
                3,
                0,
                0,
            )
            .unwrap()
            .0
            .unwrap();
            assert_eq!(value.exact, Some(9));
            assert!(!value.invocation_dependent);
            assert!(
                query(
                    &types,
                    &function,
                    Some(&conditions),
                    &payload(2, ty, 0, !moved),
                    3,
                    0,
                    0
                )
                .unwrap()
                .0
                .is_none()
            );
        }
    }

    #[test]
    fn exclusive_enum_join_requires_original_guard_site_and_same_body() {
        let (types, function, ty) = fixture(false);
        let conditions = global_enum_transport_v1::Conditions::new(&types, &function).unwrap();
        assert!(
            query(&types, &function, None, &payload(2, ty, 0, false), 3, 0, 0)
                .unwrap()
                .0
                .is_none()
        );
        for (variant, block, statement) in [(1, 3, 0), (0, 2, 0), (0, 3, 1)] {
            assert!(
                query(
                    &types,
                    &function,
                    Some(&conditions),
                    &payload(2, ty, variant, false),
                    block,
                    statement,
                    0
                )
                .unwrap()
                .0
                .is_none()
            );
        }
        let foreign = function.clone();
        let foreign_conditions =
            global_enum_transport_v1::Conditions::new(&types, &foreign).unwrap();
        assert!(matches!(
            query(
                &types,
                &function,
                Some(&foreign_conditions),
                &payload(2, ty, 0, false),
                3,
                0,
                0
            ),
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "exclusive enum conditions do not belong to this source query"
            ))
        ));
    }

    #[test]
    fn exclusive_enum_join_rejects_same_variant_even_equal_payload_and_unknown_incoming() {
        for change in 0..4 {
            let (types, function, ty) = fixture(false);
            let mut blocks = function.blocks().to_vec();
            let statement = match change {
                0 => construct(1, ty, 0, typed_constant(U64_TYPE, 9, 8)),
                1 => construct(1, ty, 0, typed_constant(U64_TYPE, 99, 8)),
                2 => typed_assignment(1, ty, SemanticRvalueKindV1::Use(typed_operand(1, ty))),
                _ => typed_assignment(1, ty, SemanticRvalueKindV1::Use(typed_operand(2, ty))),
            };
            blocks[5] = block(215, vec![statement], go(1));
            assert!(
                resolve(&types, &replace(&function, blocks), ty).is_none(),
                "change {change}"
            );
        }
    }

    #[test]
    fn exclusive_enum_join_requires_complete_constructor_and_variant_edge_coverage() {
        for change in 0..4 {
            let (types, function, ty) = fixture(false);
            let mut blocks = function.blocks().to_vec();
            match change {
                0 => {
                    blocks[0] = block(
                        210,
                        vec![],
                        SemanticTerminatorKindV1::SwitchInt {
                            discriminant: typed_constant(U64_TYPE, 0, 8),
                            targets: SemanticSwitchTargetsV1::new(
                                vec![
                                    SemanticSwitchTargetV1::new(
                                        0,
                                        cfg_edge(SemanticEdgeRoleV1::SwitchValue, 4),
                                    ),
                                    SemanticSwitchTargetV1::new(
                                        1,
                                        cfg_edge(SemanticEdgeRoleV1::SwitchValue, 5),
                                    ),
                                ],
                                cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 1),
                            )
                            .unwrap(),
                        },
                    )
                }
                1 => blocks[0] = block(210, vec![], branch(typed_constant(U64_TYPE, 0, 8), 4, 3)),
                2 => {
                    blocks[2] = block(
                        212,
                        blocks[2].statements().to_vec(),
                        branch(typed_constant(U64_TYPE, 0, 8), 3, 6),
                    )
                }
                _ => blocks[5] = block(215, blocks[5].statements().to_vec(), go(3)),
            }
            let changed = replace(&function, blocks);
            if change == 0 {
                let proof = SemanticAssertProofsV1::new(&types, &changed).unwrap();
                assert!(proof.graph.is_entry_reachable(4));
                assert!(proof.graph.is_entry_reachable(5));
                assert_eq!(proof.definition_counts[1], 2);
                let conditions =
                    global_enum_transport_v1::Conditions::new(&types, &changed).unwrap();
                assert!(conditions.allows(
                    &types,
                    &changed,
                    SemanticLocalIdV1::from_index(2),
                    0,
                    3
                ));
            }
            assert!(resolve(&types, &changed, ty).is_none(), "change {change}");
        }
    }

    #[test]
    fn exclusive_enum_join_never_omits_an_unwind_cleanup_incoming() {
        let (types, function, ty) = fixture(false);
        let mut blocks = function.blocks().to_vec();
        let branch = blocks[0].terminator().kind().clone();
        blocks[0] = block(
            210,
            vec![],
            SemanticTerminatorKindV1::Assert {
                condition: typed_constant(BOOL_TYPE, 1, 1),
                expected: true,
                message: SemanticAssertMessageV1::BoundsCheck {
                    length: typed_constant(U64_TYPE, 1, 8),
                    index: typed_constant(U64_TYPE, 0, 8),
                },
                target: cfg_edge(SemanticEdgeRoleV1::AssertSuccess, 7),
                unwind: SemanticUnwindActionV1::Cleanup(cfg_edge(
                    SemanticEdgeRoleV1::AssertUnwind,
                    1,
                )),
            },
        );
        blocks.push(block(217, vec![], branch));
        let changed = replace(&function, blocks);
        let mut proof = SemanticAssertProofsV1::new(&types, &changed).unwrap();
        assert_eq!(proof.graph.successors(0).unwrap(), &[7]);
        assert!(
            proof
                .graph
                .query(
                    CsrWorkV1::new(&mut proof.work, MAX_PROJECTED_LOOP_GRAPH_WORK_V1),
                    |mut query| query.enum_definitions_cover_use(
                        [
                            ScalarAssignmentSiteV1 {
                                block: 4,
                                statement: 0
                            },
                            ScalarAssignmentSiteV1 {
                                block: 5,
                                statement: 0
                            },
                        ],
                        ScalarAssignmentSiteV1 {
                            block: 1,
                            statement: 0
                        }
                    )
                )
                .unwrap(),
            "normal-edge coverage alone omits the cleanup incoming"
        );
        let conditions = global_enum_transport_v1::Conditions::new(&types, &changed).unwrap();
        assert!(conditions.allows(&types, &changed, SemanticLocalIdV1::from_index(2), 0, 3));
        assert!(resolve(&types, &changed, ty).is_none());
    }

    #[test]
    fn exclusive_enum_join_keeps_all_definitions_including_dead_mutations() {
        for change in 0..4 {
            let (types, function, ty) = fixture(false);
            let mut blocks = function.blocks().to_vec();
            let kind = match change {
                0 => SemanticStatementKindV1::SetDiscriminant {
                    place: typed_place(1, ty),
                    variant_index: 0,
                },
                1 => SemanticStatementKindV1::Deinitialize(typed_place(1, ty)),
                2 => SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    raw_operand_place(&payload(1, ty, 0, false))
                        .unwrap()
                        .clone(),
                    SemanticRvalueV1::new(
                        U64_TYPE,
                        SemanticRvalueKindV1::Use(typed_constant(U64_TYPE, 99, 8)),
                    ),
                )),
                _ => construct(1, ty, 0, typed_constant(U64_TYPE, 9, 8))
                    .kind()
                    .clone(),
            };
            blocks.push(block(
                217,
                vec![SemanticStatementV1::new(
                    SemanticSourceProvenanceV1::unavailable(),
                    kind,
                )],
                SemanticTerminatorKindV1::Return,
            ));
            let changed = replace(&function, blocks);
            assert_eq!(
                assertion_definition_inventory(&changed).unwrap().counts[1],
                3
            );
            assert!(resolve(&types, &changed, ty).is_none());
        }
    }

    #[test]
    fn exclusive_enum_join_rejects_escaped_owner_and_bad_sibling_type() {
        let (mut types, function, ty) = fixture(false);
        let pointer = SemanticTypeIdV1::from_index(types.len() as u32);
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(249)),
            SemanticLayoutIdentityV1::from_sha256(bytes(249)),
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
        let mut locals = function.locals().to_vec();
        locals.push(local(214, pointer, SemanticLocalRoleV1::Temporary));
        let mut blocks = function.blocks().to_vec();
        let mut statements = blocks[4].statements().to_vec();
        statements.push(typed_assignment(
            4,
            pointer,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Mutable,
                place: typed_place(1, ty),
            },
        ));
        blocks[4] = block(214, statements, go(1));
        let changed = typed_global_fixture_with_body_v1(&function, locals, blocks);
        assert!(resolve(&types, &changed, ty).is_none());
        let mut blocks = function.blocks().to_vec();
        blocks[5] = block(
            215,
            vec![construct(1, ty, 1, typed_constant(BOOL_TYPE, 1, 1))],
            go(1),
        );
        assert!(resolve(&types, &replace(&function, blocks), ty).is_none());
    }

    #[test]
    fn exclusive_enum_join_loop_requires_constructor_and_capture_on_every_iteration() {
        let (types, function, ty) = fixture(false);
        let mut blocks = function.blocks().to_vec();
        blocks[3] = block(
            213,
            blocks[3].statements().to_vec(),
            branch(typed_constant(U64_TYPE, 0, 8), 0, 6),
        );
        let changed = replace(&function, blocks);
        assert_eq!(resolve(&types, &changed, ty).unwrap().exact, Some(9));
        for target in [1, 2, 3] {
            let mut blocks = function.blocks().to_vec();
            blocks[3] = block(213, blocks[3].statements().to_vec(), go(target));
            assert!(
                resolve(&types, &replace(&function, blocks), ty).is_none(),
                "stale target {target}"
            );
        }
    }

    #[test]
    fn exclusive_enum_join_rejects_pre_use_cycle_and_constructor_capture_reordering() {
        let (types, function, ty) = fixture(false);
        let mut blocks = function.blocks().to_vec();
        blocks[1] = block(
            211,
            blocks[1].statements().to_vec(),
            branch(typed_constant(U64_TYPE, 0, 8), 1, 2),
        );
        assert!(resolve(&types, &replace(&function, blocks), ty).is_none());
        let mut blocks = function.blocks().to_vec();
        blocks[4] = block(
            214,
            blocks[4].statements().to_vec(),
            branch(typed_constant(U64_TYPE, 0, 8), 1, 2),
        );
        assert!(resolve(&types, &replace(&function, blocks), ty).is_none());
    }

    #[test]
    fn exclusive_enum_join_rejects_a_guard_of_the_pre_assignment_argument_version() {
        let (types, function, ty) = fixture(false);
        let mut locals = function.locals().to_vec();
        locals[2] = local(212, ty, SemanticLocalRoleV1::Argument(0));
        let mut blocks = function.blocks().to_vec();
        blocks[0] = block(
            210,
            vec![typed_assignment(
                3,
                U64_TYPE,
                SemanticRvalueKindV1::Discriminant(typed_place(2, ty)),
            )],
            branch(typed_operand(3, U64_TYPE), 7, 6),
        );
        blocks[1] = block(211, blocks[1].statements().to_vec(), go(3));
        blocks[2] = block(212, vec![], SemanticTerminatorKindV1::Return);
        blocks.push(block(
            217,
            vec![],
            branch(typed_constant(U64_TYPE, 0, 8), 4, 5),
        ));
        let abi = SemanticFunctionAbiV1::new(
            function.abi().identity(),
            function.abi().layout_identity(),
            SemanticCanonAbiV1::GpuKernel,
            false,
            false,
            vec![SemanticAbiValueV1::new(
                ty,
                SemanticAbiPassModeV1::Direct(SemanticAbiValueAttributesV1::plain()),
            )],
            SemanticAbiValueV1::new(U64_TYPE, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap()
        .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue])
        .unwrap();
        let changed = SemanticFunctionDeclV1::new(
            function.identity(),
            function.role(),
            function.item_definition_identity(),
            function.monomorphization_identity(),
            function.generic_type_arguments_identity(),
            function.const_generic_arguments_identity(),
            function.source(),
            abi,
            locals,
            function.entry(),
            blocks,
        )
        .unwrap();
        let conditions = global_enum_transport_v1::Conditions::new(&types, &changed).unwrap();
        assert!(
            conditions.allows(&types, &changed, SemanticLocalIdV1::from_index(2), 0, 3),
            "block dominance alone does not prove enum version identity"
        );
        assert!(resolve(&types, &changed, ty).is_none());
    }

    #[test]
    fn exclusive_enum_join_uses_the_unchanged_shared_work_ceiling() {
        let (types, function, ty) = fixture(false);
        let conditions = global_enum_transport_v1::Conditions::new(&types, &function).unwrap();
        let (_, spent) = query(
            &types,
            &function,
            Some(&conditions),
            &payload(2, ty, 0, false),
            3,
            0,
            0,
        )
        .unwrap();
        let prefix = MAX_PROJECTED_LOOP_GRAPH_WORK_V1.checked_sub(spent).unwrap();
        let (value, exact) = query(
            &types,
            &function,
            Some(&conditions),
            &payload(2, ty, 0, false),
            3,
            0,
            prefix,
        )
        .unwrap();
        assert_eq!(value.unwrap().exact, Some(9));
        assert_eq!(exact, MAX_PROJECTED_LOOP_GRAPH_WORK_V1);
        assert!(matches!(
            query(
                &types,
                &function,
                Some(&conditions),
                &payload(2, ty, 0, false),
                3,
                0,
                prefix + 1
            ),
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "uniform induction CFG analysis exceeds its work limit"
            ))
        ));
    }

    fn oracle_reaches(rows: &[Vec<usize>], start: usize, target: usize, cuts: &[usize]) -> bool {
        let mut seen = vec![false; rows.len()];
        let mut pending = vec![start];
        while let Some(block) = pending.pop() {
            if seen[block] {
                continue;
            }
            seen[block] = true;
            if block == target {
                return true;
            }
            if !cuts.contains(&block) {
                pending.extend_from_slice(&rows[block]);
            }
        }
        false
    }

    #[test]
    fn exclusive_enum_join_csr_cut_matches_complete_small_graph_oracle() {
        let edges = [
            (0, 1),
            (0, 2),
            (0, 3),
            (1, 2),
            (1, 3),
            (2, 1),
            (2, 3),
            (3, 0),
        ];
        for mask in 0..1usize << edges.len() {
            let mut rows = vec![Vec::new(); 4];
            for (bit, &(from, to)) in edges.iter().enumerate() {
                if mask & (1 << bit) != 0 {
                    rows[from].push(to);
                }
            }
            let blocks = rows
                .iter()
                .enumerate()
                .map(|(index, row)| {
                    let next = if row.is_empty() {
                        SemanticTerminatorKindV1::Return
                    } else {
                        SemanticTerminatorKindV1::SwitchInt {
                            discriminant: typed_constant(U64_TYPE, 0, 8),
                            targets: SemanticSwitchTargetsV1::new(
                                row[..row.len() - 1]
                                    .iter()
                                    .enumerate()
                                    .map(|(i, to)| {
                                        SemanticSwitchTargetV1::new(
                                            i as u128,
                                            cfg_edge(SemanticEdgeRoleV1::SwitchValue, *to as u32),
                                        )
                                    })
                                    .collect(),
                                cfg_edge(
                                    SemanticEdgeRoleV1::SwitchOtherwise,
                                    row[row.len() - 1] as u32,
                                ),
                            )
                            .unwrap(),
                        }
                    };
                    block(
                        220 + index as u8,
                        vec![typed_assignment(
                            0,
                            U64_TYPE,
                            SemanticRvalueKindV1::Use(typed_constant(U64_TYPE, 0, 8)),
                        )],
                        next,
                    )
                })
                .collect();
            let function = projection_function_with_locals(
                blocks,
                vec![local(220, U64_TYPE, SemanticLocalRoleV1::Return)],
            );
            let mut work = 0;
            let mut graph = LosslessCsrV1::build(
                &function,
                CsrWorkV1::new(&mut work, MAX_PROJECTED_LOOP_GRAPH_WORK_V1),
            )
            .unwrap();
            let identity = graph.workspace_identity();
            let actual = graph
                .query(
                    CsrWorkV1::new(&mut work, MAX_PROJECTED_LOOP_GRAPH_WORK_V1),
                    |mut query| {
                        query.enum_definitions_cover_use(
                            [
                                ScalarAssignmentSiteV1 {
                                    block: 1,
                                    statement: 0,
                                },
                                ScalarAssignmentSiteV1 {
                                    block: 2,
                                    statement: 0,
                                },
                            ],
                            ScalarAssignmentSiteV1 {
                                block: 3,
                                statement: 0,
                            },
                        )
                    },
                )
                .unwrap();
            let expected = [1, 2, 3]
                .iter()
                .all(|&target| oracle_reaches(&rows, 0, target, &[]))
                && !oracle_reaches(&rows, 0, 3, &[1, 2]);
            assert_eq!(actual, expected, "mask {mask}");
            assert_eq!(graph.workspace_identity(), identity);
        }
    }

    #[test]
    fn exclusive_enum_join_csr_cursor_cleanup_survives_short_budget() {
        let (types, function, _) = fixture(false);
        let mut proof = SemanticAssertProofsV1::new(&types, &function).unwrap();
        let identity = proof.graph.workspace_identity();
        let start = proof.work;
        assert!(
            proof
                .graph
                .query(
                    CsrWorkV1::new(&mut proof.work, MAX_PROJECTED_LOOP_GRAPH_WORK_V1),
                    |mut query| query.enum_constructor_is_fresh(4, 5, 3)
                )
                .unwrap()
        );
        let debit = proof.work - start;
        let start = proof.work;
        let result = proof.graph.query(
            CsrWorkV1::new(&mut proof.work, start + debit - 1),
            |mut query| query.enum_constructor_is_fresh(4, 5, 3),
        );
        assert!(matches!(
            result,
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "CSR analysis exceeds its inherited work limit"
            ))
        ));
        assert!(
            proof
                .graph
                .query(
                    CsrWorkV1::new(&mut proof.work, MAX_PROJECTED_LOOP_GRAPH_WORK_V1),
                    |mut query| query.reaches(4, 3)
                )
                .unwrap()
        );
        assert_eq!(proof.graph.workspace_identity(), identity);
    }

    fn consumer(
        invocation: bool,
        stride: u64,
        same_variant: bool,
    ) -> (
        Vec<SemanticTypeDeclV1>,
        Vec<SemanticCallableDeclV1>,
        SemanticFunctionDeclV1,
    ) {
        let (mut types, callables, base) =
            derived_exclusive_index_fixture(64, stride, 63, invocation, false);
        let ty = enumeration(&mut types);
        let mut locals = base.locals().to_vec();
        let carrier = locals.len() as u32;
        let alias = carrier + 1;
        let discriminator = carrier + 2;
        let output = carrier + 3;
        for (index, ty) in [ty, ty, U64_TYPE, U64_TYPE].into_iter().enumerate() {
            locals.push(local(240 + index as u8, ty, SemanticLocalRoleV1::Temporary));
        }
        let mut blocks = base.blocks().to_vec();
        let SemanticTerminatorKindV1::Call(load) = blocks[6].terminator().kind() else {
            panic!()
        };
        blocks[6] = block(
            196,
            blocks[6].statements().to_vec(),
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    load.callee(),
                    load.arguments().to_vec(),
                    Some(SemanticCallDestinationV1::new(
                        load.destination().unwrap().place().clone(),
                        cfg_edge(SemanticEdgeRoleV1::CallReturn, 9),
                    )),
                    load.unwind(),
                )
                .unwrap(),
            ),
        );
        let computations = blocks[7].statements().to_vec();
        let SemanticTerminatorKindV1::Call(store) = blocks[7].terminator().kind() else {
            panic!()
        };
        let mut arguments = store.arguments().to_vec();
        arguments[1] = typed_operand(output, U64_TYPE);
        blocks[7] = block(
            197,
            vec![typed_assignment(
                output,
                U64_TYPE,
                SemanticRvalueKindV1::Use(payload(alias, ty, 0, true)),
            )],
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    store.callee(),
                    arguments,
                    store.destination().cloned(),
                    store.unwind(),
                )
                .unwrap(),
            ),
        );
        blocks.push(block(
            239,
            computations,
            branch(typed_constant(U64_TYPE, 0, 8), 10, 11),
        ));
        blocks.push(block(
            240,
            vec![construct(
                carrier,
                ty,
                0,
                typed_operand(carrier - 1, U64_TYPE),
            )],
            go(12),
        ));
        blocks.push(block(
            241,
            vec![construct(
                carrier,
                ty,
                u32::from(!same_variant),
                typed_constant(U64_TYPE, 0, 8),
            )],
            go(12),
        ));
        blocks.push(block(
            242,
            vec![
                typed_assignment(
                    alias,
                    ty,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(typed_place(carrier, ty))),
                ),
                typed_assignment(
                    discriminator,
                    U64_TYPE,
                    SemanticRvalueKindV1::Discriminant(typed_place(alias, ty)),
                ),
            ],
            branch(typed_operand(discriminator, U64_TYPE), 13, 8),
        ));
        blocks.push(block(243, vec![], go(7)));
        (
            types,
            callables,
            typed_global_fixture_with_body_v1(&base, locals, blocks),
        )
    }

    #[test]
    fn exclusive_enum_join_full_consumer_retains_all_invocation_indices() {
        let (types, callables, function) = consumer(true, 16, false);
        let (projection, operations) =
            project_capability_index_fixture_with_launch(&types, &callables, &function, Some(1024))
                .unwrap();
        let write = projection.direct_write_effects[7].as_ref().unwrap();
        assert_eq!(write.indices.len(), 1);
        assert_eq!(write.comparisons.len(), 1);
        assert_eq!(write.comparisons[0].0, write.indices[0]);
        let extents = operations
            .iter()
            .find_map(|operation| match operation {
                ProductionRankedOperationV1::ViewInSpace {
                    result,
                    writable: true,
                    dynamic_extents,
                    ..
                } if *result == write.view => Some(dynamic_extents.as_slice()),
                _ => None,
            })
            .expect("the original output view and its exact allocation extent");
        assert_eq!(extents, &[write.comparisons[0].1]);
        for raw in 0..1024 {
            assert_eq!(
                evaluate_projected_index(&operations, write.indices[0], raw),
                (raw / 64) * 16 + raw % 64
            );
        }
    }

    #[test]
    fn exclusive_enum_join_full_consumer_keeps_overflow_independence_and_same_variant_rejections() {
        for (invocation, stride, same_variant) in [
            (false, 16, false),
            (true, u64::MAX, false),
            (true, 16, true),
        ] {
            let (types, callables, function) = consumer(invocation, stride, same_variant);
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
