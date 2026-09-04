    #[test]
    fn mutated_option_discriminants_do_not_mint_switch_predicates() {
        let predicate = GuardPredicateV1 {
            comparisons: vec![(
                ProductionRankedValueV1::Argument(1),
                ProductionRankedValueV1::Argument(2),
            )],
        };
        for mutation in [None, Some(false), Some(true)] {
            let mut statements = vec![
                enum_definition(SemanticLocalIdV1::from_index(1), 0),
                enum_discriminant(
                    SemanticLocalIdV1::from_index(1),
                    SemanticLocalIdV1::from_index(2),
                ),
            ];
            if let Some(address_escaped) = mutation {
                if address_escaped {
                    statements.push(typed_assignment(
                        3,
                        POINTER_TYPE,
                        SemanticRvalueKindV1::Borrow {
                            kind: SemanticBorrowKindV1::Mutable,
                            place: typed_place(2, SCALAR_TYPE),
                        },
                    ));
                } else {
                    statements.push(typed_assignment(
                        2,
                        SCALAR_TYPE,
                        SemanticRvalueKindV1::Use(constant(1)),
                    ));
                }
            }
            let function = projection_function_with_locals(
                vec![block(141, statements, SemanticTerminatorKindV1::Return)],
                vec![
                    local(141, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                    local(142, ENUM_TYPE, SemanticLocalRoleV1::Temporary),
                    local(143, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                    local(144, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
                ],
            );
            let mut option_predicates = vec![None; function.locals().len()];
            option_predicates[1] = Some(predicate.clone());
            let projected = switch_predicates(
                &function,
                &option_predicates,
                &vec![None; function.locals().len()],
            )
            .unwrap();
            if mutation.is_some() {
                assert!(projected[2].is_none());
            } else {
                assert_eq!(projected[2], Some(predicate.clone()));
            }
        }
    }

    #[test]
    fn local_provenance_preserves_only_value_and_pointer_identity_operations() {
        let pointer_place = |local| {
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], POINTER_TYPE)
                .unwrap()
        };
        let assign = |destination, ty, value| {
            statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                SemanticPlaceV1::new(SemanticLocalIdV1::from_index(destination), vec![], ty)
                    .unwrap(),
                SemanticRvalueV1::new(ty, value),
            )))
        };
        let loaded_scalar = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(3),
            vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, SCALAR_TYPE)
                    .unwrap(),
            ],
            SCALAR_TYPE,
        )
        .unwrap();
        let function = projection_function_with_locals(
            vec![block(
                96,
                vec![
                    assign(
                        2,
                        POINTER_TYPE,
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(pointer_place(1))),
                    ),
                    assign(
                        3,
                        POINTER_TYPE,
                        SemanticRvalueKindV1::Cast {
                            kind: SemanticCastKindV1::Pointer,
                            operand: SemanticOperandV1::Copy(pointer_place(2)),
                        },
                    ),
                    assign(
                        4,
                        SCALAR_TYPE,
                        SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                            loaded_scalar,
                            SemanticVolatilityV1::NonVolatile,
                            None,
                        )),
                    ),
                    assign(
                        5,
                        SCALAR_TYPE,
                        SemanticRvalueKindV1::Cast {
                            kind: SemanticCastKindV1::PointerExposeProvenance,
                            operand: SemanticOperandV1::Copy(pointer_place(3)),
                        },
                    ),
                ],
                SemanticTerminatorKindV1::Return,
            )],
            vec![
                local(96, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(97, POINTER_TYPE, SemanticLocalRoleV1::Argument(0)),
                local(98, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
                local(99, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
                local(100, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                local(101, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
            ],
        );

        let provenance = local_provenance_v1(&projection_types(), &function).unwrap();
        assert_eq!(
            provenance.stable_argument_origins,
            vec![None, Some(0), Some(0), Some(0), None, Some(0)]
        );
        assert_eq!(
            provenance.allocation_origins,
            vec![None, Some(0), Some(0), Some(0), None, None]
        );
    }

    #[test]
    fn local_allocation_provenance_requires_an_exact_reborrow() {
        let assign_borrow = |destination, place| {
            statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(destination),
                    vec![],
                    POINTER_TYPE,
                )
                .unwrap(),
                SemanticRvalueV1::new(
                    POINTER_TYPE,
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Shared,
                        place,
                    },
                ),
            )))
        };
        let dereference =
            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, SCALAR_TYPE).unwrap();
        let pointee_place = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(1),
            vec![dereference],
            SCALAR_TYPE,
        )
        .unwrap();
        let pointer_slot =
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![], POINTER_TYPE).unwrap();
        let function = projection_function_with_locals(
            vec![block(
                102,
                vec![
                    assign_borrow(2, pointee_place),
                    assign_borrow(3, pointer_slot),
                ],
                SemanticTerminatorKindV1::Return,
            )],
            vec![
                local(102, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(103, POINTER_TYPE, SemanticLocalRoleV1::Argument(0)),
                local(104, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
                local(105, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
            ],
        );

        let provenance = local_provenance_v1(&projection_types(), &function).unwrap();
        assert_eq!(
            provenance.allocation_origins,
            vec![None, Some(0), Some(0), None]
        );
        assert_eq!(
            provenance.stable_argument_origins,
            vec![None, Some(0), None, None]
        );
    }

    fn private_local_reborrow_function() -> SemanticFunctionDeclV1 {
        let borrow = |destination, place| {
            statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(destination),
                    vec![],
                    POINTER_TYPE,
                )
                .unwrap(),
                SemanticRvalueV1::new(
                    POINTER_TYPE,
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Shared,
                        place,
                    },
                ),
            )))
        };
        let private = SemanticLocalIdV1::from_index(1);
        let first_reference = SemanticLocalIdV1::from_index(2);
        let dereference =
            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, SCALAR_TYPE).unwrap();
        projection_function_with_locals(
            vec![block(
                126,
                vec![
                    borrow(
                        2,
                        SemanticPlaceV1::new(private, vec![], SCALAR_TYPE).unwrap(),
                    ),
                    borrow(
                        3,
                        SemanticPlaceV1::new(first_reference, vec![dereference], SCALAR_TYPE)
                            .unwrap(),
                    ),
                ],
                SemanticTerminatorKindV1::Return,
            )],
            vec![
                local(126, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(127, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                local(128, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
                local(129, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
            ],
        )
    }

    #[test]
    fn exact_private_local_reborrow_is_authenticated_zero_effect_address_formation() {
        let function = private_local_reborrow_function();
        let provenance = local_provenance_v1(&projection_types(), &function).unwrap();
        let private = Some(LocalAllocationProvenanceV1::Private(
            SemanticLocalIdV1::from_index(1),
        ));
        assert_eq!(
            provenance.allocation_provenance,
            vec![None, None, private, private]
        );

        let mut contracts = synthetic_local_contracts(&function);
        contracts.allocations = vec![None; function.locals().len()];
        contracts.allocation_provenance = provenance.allocation_provenance;
        let (operations, sources, ranked_ir) =
            audit_function_with_local_contracts(&function, &contracts).unwrap();
        assert!(operations.is_empty());
        assert!(sources.is_empty());
        assert!(ranked_ir.is_empty());
    }

    #[test]
    fn unknown_projected_borrow_still_lacks_allocation_provenance() {
        let pointer = SemanticLocalIdV1::from_index(1);
        let dereference =
            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, SCALAR_TYPE).unwrap();
        let address = statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(2), vec![], POINTER_TYPE).unwrap(),
            SemanticRvalueV1::new(
                POINTER_TYPE,
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: SemanticPlaceV1::new(pointer, vec![dereference], SCALAR_TYPE).unwrap(),
                },
            ),
        )));
        let function = projection_function_with_locals(
            vec![block(130, vec![address], SemanticTerminatorKindV1::Return)],
            vec![
                local(130, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(131, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
                local(132, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
            ],
        );
        let provenance = local_provenance_v1(&projection_types(), &function).unwrap();
        assert_eq!(provenance.allocation_provenance, vec![None, None, None]);

        let mut contracts = synthetic_local_contracts(&function);
        contracts.allocations = vec![None; function.locals().len()];
        contracts.allocation_provenance = provenance.allocation_provenance;
        assert!(matches!(
            audit_function_with_local_contracts(&function, &contracts),
            Err(
                ProductionRankedProjectionErrorV1::MissingAllocationProvenance {
                    local: 1,
                    projections: 1,
                    ty: 2,
                }
            )
        ));
    }

    #[test]
    fn private_local_provenance_is_not_an_allocation_contract() {
        let function = private_local_reborrow_function();
        let provenance = local_provenance_v1(&projection_types(), &function).unwrap();
        assert_eq!(provenance.allocation_origins, vec![None, None, None, None]);
        assert!(
            local_allocation_contracts(
                &projection_types(),
                &function,
                &provenance.allocation_origins,
            )
            .unwrap()
            .iter()
            .all(Option::is_none)
        );
    }

    fn pointer_offset(destination: u32, source: u32) -> SemanticStatementV1 {
        let pointer_place = |local| {
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], POINTER_TYPE)
                .unwrap()
        };
        statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            pointer_place(destination),
            SemanticRvalueV1::new(
                POINTER_TYPE,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::Offset,
                    left: SemanticOperandV1::Copy(pointer_place(source)),
                    right: constant(1),
                },
            ),
        )))
    }

    fn projected_pointer_borrow(destination: u32, source: u32) -> SemanticStatementV1 {
        let dereference =
            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, SCALAR_TYPE).unwrap();
        statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(destination),
                vec![],
                POINTER_TYPE,
            )
            .unwrap(),
            SemanticRvalueV1::new(
                POINTER_TYPE,
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: SemanticPlaceV1::new(
                        SemanticLocalIdV1::from_index(source),
                        vec![dereference],
                        SCALAR_TYPE,
                    )
                    .unwrap(),
                },
            ),
        )))
    }

    #[test]
    fn pointer_offset_retains_only_an_external_allocation_contract() {
        let function = projection_function_with_owned_argument(
            vec![block(
                133,
                vec![pointer_offset(2, 1), projected_pointer_borrow(3, 2)],
                SemanticTerminatorKindV1::Return,
            )],
            vec![
                local(133, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(134, POINTER_TYPE, SemanticLocalRoleV1::Argument(0)),
                local(135, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
                local(136, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
            ],
            SemanticSourceArgumentOwnershipV1::RawPointer,
        );
        let types = projection_types();
        let provenance = local_provenance_v1(&types, &function).unwrap();
        assert_eq!(
            provenance.allocation_origins,
            vec![None, Some(0), Some(0), Some(0)]
        );
        assert_eq!(
            provenance.allocation_provenance,
            vec![
                None,
                Some(LocalAllocationProvenanceV1::Argument(0)),
                None,
                None,
            ]
        );

        let mut contracts = synthetic_local_contracts(&function);
        let external_contract = contracts.allocations[1].unwrap();
        contracts.allocations = vec![None; function.locals().len()];
        contracts.allocations[2] = Some(external_contract);
        contracts.allocation_provenance = provenance.allocation_provenance;
        let dereferenced_offset = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(2),
            vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, SCALAR_TYPE)
                    .unwrap(),
            ],
            SCALAR_TYPE,
        )
        .unwrap();
        project_address_formation(&types, &function, &dereferenced_offset, &contracts).unwrap();
    }

    #[test]
    fn private_pointer_offset_cannot_mint_address_formation_authority() {
        let private = SemanticLocalIdV1::from_index(1);
        let direct_borrow = statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(2), vec![], POINTER_TYPE).unwrap(),
            SemanticRvalueV1::new(
                POINTER_TYPE,
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: SemanticPlaceV1::new(private, vec![], SCALAR_TYPE).unwrap(),
                },
            ),
        )));
        let function = projection_function_with_locals(
            vec![block(
                137,
                vec![
                    direct_borrow,
                    pointer_offset(3, 2),
                    projected_pointer_borrow(4, 3),
                ],
                SemanticTerminatorKindV1::Return,
            )],
            vec![
                local(137, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(138, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                local(139, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
                local(140, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
                local(141, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
            ],
        );
        let provenance = local_provenance_v1(&projection_types(), &function).unwrap();
        assert_eq!(
            provenance.allocation_provenance,
            vec![
                None,
                None,
                Some(LocalAllocationProvenanceV1::Private(private)),
                None,
                None,
            ]
        );
        assert_eq!(
            provenance.allocation_origins,
            vec![None, None, None, None, None]
        );

        let mut contracts = synthetic_local_contracts(&function);
        contracts.allocations = vec![None; function.locals().len()];
        contracts.allocation_provenance = provenance.allocation_provenance;
        assert!(matches!(
            audit_function_with_local_contracts(&function, &contracts),
            Err(
                ProductionRankedProjectionErrorV1::MissingAllocationProvenance {
                    local: 3,
                    projections: 1,
                    ty: 2,
                }
            )
        ));
    }

    #[test]
    fn unknown_or_multiply_defined_pointer_offsets_remain_unauthenticated() {
        let unknown = projection_function_with_locals(
            vec![block(
                142,
                vec![pointer_offset(2, 1), projected_pointer_borrow(3, 2)],
                SemanticTerminatorKindV1::Return,
            )],
            vec![
                local(142, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(143, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
                local(144, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
                local(145, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
            ],
        );
        let unknown_provenance = local_provenance_v1(&projection_types(), &unknown).unwrap();
        assert_eq!(
            unknown_provenance.allocation_origins,
            vec![None, None, None, None]
        );
        assert_eq!(
            unknown_provenance.allocation_provenance,
            vec![None, None, None, None]
        );

        let multiply_defined = projection_function_with_owned_argument(
            vec![block(
                146,
                vec![pointer_offset(2, 1), pointer_offset(2, 1)],
                SemanticTerminatorKindV1::Return,
            )],
            vec![
                local(146, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(147, POINTER_TYPE, SemanticLocalRoleV1::Argument(0)),
                local(148, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
            ],
            SemanticSourceArgumentOwnershipV1::RawPointer,
        );
        let multiply_defined_provenance =
            local_provenance_v1(&projection_types(), &multiply_defined).unwrap();
        assert_eq!(
            multiply_defined_provenance.allocation_origins,
            vec![None, Some(0), None]
        );
        assert_eq!(
            multiply_defined_provenance.allocation_provenance,
            vec![None, Some(LocalAllocationProvenanceV1::Argument(0)), None,]
        );
    }

    #[test]
    fn scalar_pair_field_zero_preserves_authenticated_first_pointer_provenance() {
        let pointer_primitive = SemanticBackendPrimitiveV1::pointer(1, 8, 8);
        let pointer_scalar = SemanticBackendScalarV1::initialized(
            pointer_primitive,
            SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
        );
        let length_scalar = SemanticBackendScalarV1::initialized(
            SemanticBackendPrimitiveV1::integer(false, 64, 8),
            SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
        );
        let mut types = projection_types();
        types[POINTER_TYPE.index() as usize] = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(118)),
            SemanticLayoutIdentityV1::from_sha256(bytes(118)),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(8),
                8,
                SemanticBackendReprV1::scalar(pointer_scalar),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new(
                    SCALAR_TYPE,
                    SemanticMutabilityV1::Mutable,
                    1,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        );
        let scalar_pair = SemanticTypeIdV1::from_index(types.len() as u32);
        types.push(
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256(bytes(119)),
                SemanticLayoutIdentityV1::from_sha256(bytes(119)),
                SemanticTypeLayoutV1::with_exact_rustc_layout(
                    16,
                    8,
                    SemanticFieldsShapeV1::arbitrary(vec![0, 8], vec![0, 1]).unwrap(),
                    SemanticRustcVariantsV1::Single { index: 0 },
                    SemanticBackendReprV1::scalar_pair(pointer_scalar, length_scalar),
                    None,
                    false,
                    None,
                    8,
                    0,
                    SemanticTypeLayoutDetailsV1::Aggregate(
                        SemanticAggregateLayoutV1::new(vec![0, 8], vec![]).unwrap(),
                    ),
                )
                .unwrap(),
                SemanticTypeShapeV1::Aggregate(
                    SemanticAggregateTypeV1::new(vec![POINTER_TYPE, SCALAR_TYPE]).unwrap(),
                ),
            )
            .with_rustc_abi_properties(
                SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                    Some(
                        SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap(),
                    ),
                    None,
                ),
            ),
        );
        let function = projection_function_with_locals(
            vec![block(119, vec![], SemanticTerminatorKindV1::Return)],
            vec![
                local(119, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(120, scalar_pair, SemanticLocalRoleV1::Argument(0)),
                local(121, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
            ],
        );
        let field = |index| {
            SemanticOperandV1::Copy(
                SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(1),
                    vec![
                        SemanticProjectionV1::new(
                            SemanticProjectionKindV1::Field(index),
                            POINTER_TYPE,
                        )
                        .unwrap(),
                    ],
                    POINTER_TYPE,
                )
                .unwrap(),
            )
        };

        assert_eq!(
            allocation_operand_local_v1(&types, &function, &field(0)),
            Some(SemanticLocalIdV1::from_index(1)),
        );
        assert_eq!(
            allocation_operand_local_v1(&types, &function, &field(1)),
            None
        );

        let pointer_place = |local| {
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], POINTER_TYPE)
                .unwrap()
        };
        let field_zero_copy =
            statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                pointer_place(2),
                SemanticRvalueV1::new(POINTER_TYPE, SemanticRvalueKindV1::Use(field(0))),
            )));
        let exact_chain = projection_function_with_locals(
            vec![block(
                149,
                vec![
                    field_zero_copy,
                    pointer_offset(3, 2),
                    projected_pointer_borrow(4, 3),
                ],
                SemanticTerminatorKindV1::Return,
            )],
            vec![
                local(149, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(150, scalar_pair, SemanticLocalRoleV1::Argument(0)),
                local(151, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
                local(152, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
                local(153, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
            ],
        );
        let provenance = local_provenance_v1(&types, &exact_chain).unwrap();
        assert_eq!(
            provenance.allocation_origins,
            vec![None, Some(0), Some(0), Some(0), Some(0)]
        );
    }

    #[test]
    fn direct_borrow_preserves_only_an_authenticated_exclusive_owner() {
        let direct_place =
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![], POINTER_TYPE).unwrap();
        let direct_borrow = |place| {
            statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                SemanticPlaceV1::new(SemanticLocalIdV1::from_index(2), vec![], POINTER_TYPE)
                    .unwrap(),
                SemanticRvalueV1::new(
                    POINTER_TYPE,
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Mutable,
                        place,
                    },
                ),
            )))
        };
        let build = |ownership| {
            projection_function_with_owned_argument(
                vec![block(
                    116,
                    vec![direct_borrow(direct_place.clone())],
                    SemanticTerminatorKindV1::Return,
                )],
                vec![
                    local(116, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                    local(117, POINTER_TYPE, SemanticLocalRoleV1::Argument(0)),
                    local(118, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
                ],
                ownership,
            )
        };

        assert_eq!(
            local_provenance_v1(
                &projection_types(),
                &build(SemanticSourceArgumentOwnershipV1::ExclusiveOwner),
            )
            .unwrap()
            .allocation_origins,
            vec![None, Some(0), Some(0)]
        );
        for ownership in [
            SemanticSourceArgumentOwnershipV1::RawPointer,
            SemanticSourceArgumentOwnershipV1::SharedBorrow,
            SemanticSourceArgumentOwnershipV1::UniqueBorrow,
            SemanticSourceArgumentOwnershipV1::ByValue,
            SemanticSourceArgumentOwnershipV1::Unspecified,
        ] {
            assert_eq!(
                local_provenance_v1(&projection_types(), &build(ownership))
                    .unwrap()
                    .allocation_origins,
                vec![None, Some(0), None]
            );
        }
    }

    #[test]
    fn local_provenance_merge_conflicts_fail_closed() {
        let mut origins = vec![Some(0), Some(1), None];
        let edges = vec![vec![2], vec![2], vec![]];
        assert!(matches!(
            propagate_exact_local_origins_v1(&mut origins, &edges, "conflicting test origins",),
            Err(ProductionRankedProjectionErrorV1::Incomplete(
                "conflicting test origins"
            ))
        ));

        let mut allocation_provenance = vec![
            Some(LocalAllocationProvenanceV1::Argument(0)),
            Some(LocalAllocationProvenanceV1::Private(
                SemanticLocalIdV1::from_index(1),
            )),
            None,
        ];
        assert!(matches!(
            propagate_exact_local_origins_v1(
                &mut allocation_provenance,
                &edges,
                "conflicting allocation provenance",
            ),
            Err(ProductionRankedProjectionErrorV1::Incomplete(
                "conflicting allocation provenance"
            ))
        ));
    }

    #[test]
    fn repeated_comparison_definitions_must_be_identical() {
        let first = GuardPredicateV1 {
            comparisons: vec![(
                ProductionRankedValueV1::Argument(1),
                ProductionRankedValueV1::Argument(2),
            )],
        };
        let conflicting = GuardPredicateV1 {
            comparisons: vec![(
                ProductionRankedValueV1::Argument(2),
                ProductionRankedValueV1::Argument(1),
            )],
        };
        let mut slot = None;
        retain_identical_direct_switch_predicate_v1(&mut slot, first.clone()).unwrap();
        retain_identical_direct_switch_predicate_v1(&mut slot, first.clone()).unwrap();
        assert_eq!(slot, Some(first));
        assert!(matches!(
            retain_identical_direct_switch_predicate_v1(&mut slot, conflicting),
            Err(ProductionRankedProjectionErrorV1::Incomplete(
                "one comparison local has conflicting source definitions"
            ))
        ));
    }

    fn explicit_binary_switch_with_fallback(
        variants: [u128; 2],
        fallback_statements: Vec<SemanticStatementV1>,
        fallback_terminator: SemanticTerminatorKindV1,
    ) -> SemanticFunctionDeclV1 {
        explicit_binary_switch_with_targets(
            variants,
            [1, 2],
            fallback_statements,
            fallback_terminator,
        )
    }

    fn explicit_binary_switch_with_targets(
        variants: [u128; 2],
        variant_targets: [u32; 2],
        fallback_statements: Vec<SemanticStatementV1>,
        fallback_terminator: SemanticTerminatorKindV1,
    ) -> SemanticFunctionDeclV1 {
        projection_function_with_locals(
            vec![
                block(
                    98,
                    vec![],
                    SemanticTerminatorKindV1::SwitchInt {
                        discriminant: tensor_operand(1),
                        targets: SemanticSwitchTargetsV1::new(
                            vec![
                                SemanticSwitchTargetV1::new(
                                    variants[0],
                                    cfg_edge(SemanticEdgeRoleV1::SwitchValue, variant_targets[0]),
                                ),
                                SemanticSwitchTargetV1::new(
                                    variants[1],
                                    cfg_edge(SemanticEdgeRoleV1::SwitchValue, variant_targets[1]),
                                ),
                            ],
                            cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 3),
                        )
                        .unwrap(),
                    },
                ),
                block(99, vec![], SemanticTerminatorKindV1::Return),
                block(100, vec![], SemanticTerminatorKindV1::Return),
                block(101, fallback_statements, fallback_terminator),
            ],
            vec![
                local(98, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(99, SCALAR_TYPE, SemanticLocalRoleV1::Argument(0)),
            ],
        )
    }

    fn explicit_multi_switch(successor_count: usize) -> SemanticFunctionDeclV1 {
        assert!(successor_count >= 2);
        let explicit = (0..successor_count - 1)
            .map(|index| {
                SemanticSwitchTargetV1::new(
                    index as u128,
                    cfg_edge(SemanticEdgeRoleV1::SwitchValue, index as u32 + 1),
                )
            })
            .collect();
        let mut blocks = Vec::with_capacity(successor_count + 1);
        blocks.push(block(
            205,
            vec![],
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: tensor_operand(1),
                targets: SemanticSwitchTargetsV1::new(
                    explicit,
                    cfg_edge(
                        SemanticEdgeRoleV1::SwitchOtherwise,
                        successor_count as u32,
                    ),
                )
                .unwrap(),
            },
        ));
        blocks.extend(
            (0..successor_count)
                .map(|_| block(206, vec![], SemanticTerminatorKindV1::Return)),
        );
        projection_function_with_locals(
            blocks,
            vec![
                local(205, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(206, SCALAR_TYPE, SemanticLocalRoleV1::Argument(0)),
            ],
        )
    }

    fn parallel_switch(
        explicit_count: usize,
        explicit_target: u32,
        otherwise_target: u32,
        otherwise_terminator: SemanticTerminatorKindV1,
    ) -> SemanticFunctionDeclV1 {
        assert!(explicit_count > 0);
        let explicit = (0..explicit_count)
            .map(|value| {
                SemanticSwitchTargetV1::new(
                    value as u128,
                    cfg_edge(SemanticEdgeRoleV1::SwitchValue, explicit_target),
                )
            })
            .collect();
        projection_function_with_locals(
            vec![
                block(
                    207,
                    vec![],
                    SemanticTerminatorKindV1::SwitchInt {
                        discriminant: tensor_operand(1),
                        targets: SemanticSwitchTargetsV1::new(
                            explicit,
                            cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, otherwise_target),
                        )
                        .unwrap(),
                    },
                ),
                block(208, vec![], SemanticTerminatorKindV1::Return),
                block(209, vec![], otherwise_terminator),
            ],
            vec![
                local(207, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(208, SCALAR_TYPE, SemanticLocalRoleV1::Argument(0)),
            ],
        )
    }

    #[test]
    fn unresolved_multiway_switch_preserves_distinct_successors_in_source_order() {
        let function = explicit_multi_switch(4);
        assert_eq!(
            projected_cfg_terminator(
                &function,
                0,
                &[],
                false,
                &[],
                &[const { None }; 2],
                &[],
            )
            .unwrap(),
            ProjectedCfgTerminatorV1::AnalysisMultiSplit {
                blocks: vec![1, 2, 3, 4],
            }
        );
    }

    #[test]
    fn unresolved_switch_coalesces_parallel_targets_without_losing_order() {
        let function = explicit_binary_switch_with_targets(
            [0, 1],
            [1, 1],
            vec![],
            SemanticTerminatorKindV1::Return,
        );
        assert_eq!(
            projected_cfg_terminator(
                &function,
                0,
                &[],
                false,
                &[],
                &[const { None }; 2],
                &[],
            )
            .unwrap(),
            ProjectedCfgTerminatorV1::AnalysisSplit {
                first_block: 1,
                second_block: 3,
            }
        );

        let function = explicit_binary_switch_with_targets(
            [0, 1],
            [3, 3],
            vec![],
            SemanticTerminatorKindV1::Return,
        );
        assert_eq!(
            projected_cfg_terminator(
                &function,
                0,
                &[],
                false,
                &[],
                &[const { None }; 2],
                &[],
            )
            .unwrap(),
            ProjectedCfgTerminatorV1::Branch(3)
        );

        let function = parallel_switch(1, 1, 1, SemanticTerminatorKindV1::Return);
        assert_eq!(
            projected_cfg_terminator(
                &function,
                0,
                &[],
                false,
                &[],
                &[const { None }; 2],
                &[],
            )
            .unwrap(),
            ProjectedCfgTerminatorV1::Branch(1)
        );

        let function = explicit_binary_switch_with_targets(
            [0, 1],
            [1, 1],
            vec![],
            SemanticTerminatorKindV1::Unreachable,
        );
        assert_eq!(
            projected_cfg_terminator(
                &function,
                0,
                &[],
                false,
                &[],
                &[const { None }; 2],
                &[],
            )
            .unwrap(),
            ProjectedCfgTerminatorV1::Branch(1)
        );
    }

    #[test]
    fn raw_parallel_switch_edges_are_bounded_separately_from_unique_successors() {
        let collapsed = parallel_switch(
            MAX_RANKED_BOUNDS_BLOCKS + 1,
            1,
            2,
            SemanticTerminatorKindV1::Return,
        );
        assert_eq!(
            projected_cfg_terminator(
                &collapsed,
                0,
                &[],
                false,
                &[],
                &[const { None }; 2],
                &[],
            )
            .unwrap(),
            ProjectedCfgTerminatorV1::AnalysisSplit {
                first_block: 1,
                second_block: 2,
            }
        );

        let excessive = parallel_switch(
            MAX_RANKED_BOUNDS_EDGES,
            1,
            2,
            SemanticTerminatorKindV1::Return,
        );
        assert!(matches!(
            projected_cfg_terminator(
                &excessive,
                0,
                &[],
                false,
                &[],
                &[const { None }; 2],
                &[],
            ),
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "analysis switch edge count exceeds the ranked edge limit"
            ))
        ));
    }

    #[test]
    fn explicit_zero_one_switch_elides_only_an_empty_unreachable_fallback() {
        let function = explicit_binary_switch_with_fallback(
            [0, 1],
            vec![],
            SemanticTerminatorKindV1::Unreachable,
        );
        assert_eq!(
            projected_cfg_terminator(&function, 0, &[], false, &[], &[const { None }; 2], &[])
                .unwrap(),
            ProjectedCfgTerminatorV1::AnalysisSplit {
                first_block: 1,
                second_block: 2,
            }
        );
    }

    fn boolean_domain_switch(values: &[u128]) -> SemanticFunctionDeclV1 {
        let discriminant = SemanticOperandV1::Copy(
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![], BOOL_TYPE).unwrap(),
        );
        let explicit = values
            .iter()
            .enumerate()
            .map(|(index, value)| {
                SemanticSwitchTargetV1::new(
                    *value,
                    cfg_edge(SemanticEdgeRoleV1::SwitchValue, index as u32 + 1),
                )
            })
            .collect();
        projection_function_with_locals(
            vec![
                block(
                    201,
                    vec![],
                    SemanticTerminatorKindV1::SwitchInt {
                        discriminant,
                        targets: SemanticSwitchTargetsV1::new(
                            explicit,
                            cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 3),
                        )
                        .unwrap(),
                    },
                ),
                block(202, vec![], SemanticTerminatorKindV1::Return),
                block(203, vec![], SemanticTerminatorKindV1::Return),
                block(204, vec![], SemanticTerminatorKindV1::Unreachable),
            ],
            vec![
                local(201, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(202, BOOL_TYPE, SemanticLocalRoleV1::Argument(0)),
            ],
        )
    }

    #[test]
    fn only_exact_boolean_domains_exhaust_an_empty_switch_fallback() {
        for (values, expected) in [(&[0_u128, 1][..], true), (&[0_u128, 2][..], false)] {
            let function = boolean_domain_switch(values);
            let SemanticTerminatorKindV1::SwitchInt {
                discriminant,
                targets,
            } = function.blocks()[0].terminator().kind()
            else {
                panic!("boolean fixture lost its switch");
            };
            assert_eq!(
                authenticated_switch_targets_exhaust_domain_v1(
                    &assertion_proof_types(),
                    &function,
                    discriminant,
                    targets,
                    &local_definition_counts(&function),
                ),
                expected,
            );
        }

        let scalar = explicit_binary_switch_with_fallback(
            [0, 1],
            vec![],
            SemanticTerminatorKindV1::Unreachable,
        );
        let SemanticTerminatorKindV1::SwitchInt {
            discriminant,
            targets,
        } = scalar.blocks()[0].terminator().kind()
        else {
            panic!("scalar fixture lost its switch");
        };
        assert!(!authenticated_switch_targets_exhaust_domain_v1(
            &projection_types(),
            &scalar,
            discriminant,
            targets,
            &local_definition_counts(&scalar),
        ));
    }

    fn non_bounds_assert_function(condition: SemanticOperandV1) -> SemanticFunctionDeclV1 {
        projection_function_with_locals(
            vec![
                block(
                    138,
                    vec![],
                    SemanticTerminatorKindV1::Assert {
                        condition,
                        expected: true,
                        message: SemanticAssertMessageV1::DivisionByZero(tensor_operand(2)),
                        target: cfg_edge(SemanticEdgeRoleV1::AssertSuccess, 1),
                        unwind: SemanticUnwindActionV1::Unreachable,
                    },
                ),
                block(139, vec![], SemanticTerminatorKindV1::Return),
            ],
            vec![
                local(138, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(139, SCALAR_TYPE, SemanticLocalRoleV1::Argument(0)),
                local(140, SCALAR_TYPE, SemanticLocalRoleV1::Argument(1)),
            ],
        )
    }
