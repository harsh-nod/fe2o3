// Component fixtures exercise the provenance census, not intrinsic authentication
// or whole-program admission. Ordinary wrapped-fill integration is separate.
mod exclusive_owner_carrier_tests {
    use super::*;
    use fe2o3_mir_model::semantic_mir_v1::{SemanticMemoryStoreV1, SemanticVolatilityV1};

    fn place(index: u32) -> SemanticPlaceV1 {
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(index), vec![], POINTER_TYPE).unwrap()
    }

    fn operand(index: u32) -> SemanticOperandV1 {
        SemanticOperandV1::Move(place(index))
    }

    fn assign(index: u32, value: SemanticRvalueKindV1) -> SemanticStatementV1 {
        statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(index),
            SemanticRvalueV1::new(POINTER_TYPE, value),
        )))
    }

    fn borrow(destination: u32, source: u32) -> SemanticStatementV1 {
        assign(
            destination,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Mutable,
                place: place(source),
            },
        )
    }

    fn call(
        callee: u32,
        arguments: Vec<SemanticOperandV1>,
        target: u32,
    ) -> SemanticTerminatorKindV1 {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(callee),
                arguments,
                Some(SemanticCallDestinationV1::new(
                    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(5), vec![], SCALAR_TYPE)
                        .unwrap(),
                    cfg_edge(SemanticEdgeRoleV1::CallReturn, target),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    }

    fn callables() -> Vec<SemanticCallableDeclV1> {
        vec![
            compiler_intrinsic_callable(
                SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut {
                    disjoint_slice: POINTER_TYPE,
                    index_witness: SCALAR_TYPE,
                    element: SCALAR_TYPE,
                    raw_index: SCALAR_TYPE,
                },
            ),
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1)),
        ]
    }

    fn function(
        first: Vec<SemanticStatementV1>,
        first_terminator: SemanticTerminatorKindV1,
        second: Vec<SemanticStatementV1>,
        second_arguments: Vec<SemanticOperandV1>,
        ownership: SemanticSourceArgumentOwnershipV1,
    ) -> SemanticFunctionDeclV1 {
        let abi = SemanticFunctionAbiV1::new(
            SemanticAbiIdentityV1::from_sha256(bytes(210)),
            SemanticLayoutIdentityV1::from_sha256(bytes(210)),
            SemanticCanonAbiV1::GpuKernel,
            false,
            false,
            vec![
                SemanticAbiValueV1::new(
                    POINTER_TYPE,
                    SemanticAbiPassModeV1::Direct(SemanticAbiValueAttributesV1::plain())
                );
                2
            ],
            SemanticAbiValueV1::new(SCALAR_TYPE, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap()
        .with_source_argument_ownership(vec![
            ownership,
            SemanticSourceArgumentOwnershipV1::RawPointer,
        ])
        .unwrap();
        SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256(bytes(211)),
            SemanticFunctionRoleV1::KernelRoot,
            SemanticItemDefinitionIdentityV1::from_sha256(bytes(212)),
            SemanticMonomorphizationIdentityV1::from_sha256(bytes(213)),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(214)),
            SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(215)),
            SemanticSourceProvenanceV1::unavailable(),
            abi,
            vec![
                local(210, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(211, POINTER_TYPE, SemanticLocalRoleV1::Argument(0)),
                local(212, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
                local(213, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
                local(214, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
                local(215, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                local(216, POINTER_TYPE, SemanticLocalRoleV1::Argument(1)),
                local(217, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
            ],
            SemanticBlockIdV1::from_index(0),
            vec![
                block(210, first, first_terminator),
                block(211, second, call(0, second_arguments, 2)),
                block(212, vec![], SemanticTerminatorKindV1::Return),
            ],
        )
        .unwrap()
    }

    fn ordinary(first: Vec<SemanticStatementV1>) -> SemanticFunctionDeclV1 {
        function(
            first,
            SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
            vec![borrow(3, 2)],
            vec![operand(3), constant(0)],
            SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
        )
    }

    fn projected(function: &SemanticFunctionDeclV1) -> LocalProvenanceV1 {
        let inventory = assertion_definition_inventory(function).unwrap();
        local_provenance_with_scalar_inventory_v1(
            &callables(),
            &projection_types(),
            function,
            &inventory.counts,
            &inventory.address_escaped,
        )
        .unwrap()
    }

    fn with_parts(
        base: &SemanticFunctionDeclV1,
        abi: SemanticFunctionAbiV1,
        locals: Vec<SemanticLocalDeclV1>,
    ) -> SemanticFunctionDeclV1 {
        SemanticFunctionDeclV1::new(
            base.identity(),
            base.role(),
            base.item_definition_identity(),
            base.monomorphization_identity(),
            base.generic_type_arguments_identity(),
            base.const_generic_arguments_identity(),
            base.source(),
            abi,
            locals,
            base.entry(),
            base.blocks().to_vec(),
        )
        .unwrap()
    }

    #[test]
    fn carrier_copy_move_and_transitive_chain_preserve_exact_exclusive_owner() {
        for value in [SemanticOperandV1::Copy(place(1)), operand(1)] {
            let function = ordinary(vec![assign(2, SemanticRvalueKindV1::Use(value))]);
            let inventory = assertion_definition_inventory(&function).unwrap();
            assert!(inventory.address_escaped[2]);
            let provenance = projected(&function);
            assert_eq!(provenance.allocation_origins[3], Some(0));
            assert_eq!(
                provenance.allocation_provenance[3],
                Some(LocalAllocationProvenanceV1::Argument(0))
            );
            assert_eq!(provenance.stable_argument_origins[2], None);
        }
        let function = ordinary(vec![
            assign(7, SemanticRvalueKindV1::Use(operand(1))),
            assign(2, SemanticRvalueKindV1::Use(operand(7))),
        ]);
        assert_eq!(projected(&function).allocation_origins[3], Some(0));
    }

    #[test]
    fn carrier_requires_exclusive_ownership_and_exact_argument_identity() {
        for ownership in [
            SemanticSourceArgumentOwnershipV1::RawPointer,
            SemanticSourceArgumentOwnershipV1::SharedBorrow,
            SemanticSourceArgumentOwnershipV1::UniqueBorrow,
            SemanticSourceArgumentOwnershipV1::ByValue,
            SemanticSourceArgumentOwnershipV1::Unspecified,
        ] {
            let function = function(
                vec![assign(2, SemanticRvalueKindV1::Use(operand(1)))],
                SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
                vec![borrow(3, 2)],
                vec![operand(3), constant(0)],
                ownership,
            );
            assert_eq!(projected(&function).allocation_origins[3], None);
        }
        let unrelated = ordinary(vec![assign(2, SemanticRvalueKindV1::Use(operand(6)))]);
        assert_eq!(projected(&unrelated).allocation_origins[2], Some(1));
        assert_eq!(projected(&unrelated).allocation_origins[3], None);
        let both_owned = with_parts(
            &unrelated,
            unrelated
                .abi()
                .clone()
                .with_source_argument_ownership(
                    vec![SemanticSourceArgumentOwnershipV1::ExclusiveOwner; 2],
                )
                .unwrap(),
            unrelated.locals().to_vec(),
        );
        assert_eq!(projected(&both_owned).allocation_origins[3], Some(1));
        assert_ne!(projected(&both_owned).allocation_origins[3], Some(0));
    }

    #[test]
    fn carrier_casts_fields_and_pointer_offsets_never_supply_whole_value_authority() {
        let field = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(1),
            vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), POINTER_TYPE)
                    .unwrap(),
            ],
            POINTER_TYPE,
        )
        .unwrap();
        let values = [
            SemanticRvalueKindV1::Cast {
                kind: SemanticCastKindV1::Pointer,
                operand: operand(1),
            },
            SemanticRvalueKindV1::Cast {
                kind: SemanticCastKindV1::Transmute,
                operand: operand(1),
            },
            SemanticRvalueKindV1::Cast {
                kind: SemanticCastKindV1::Integer,
                operand: operand(1),
            },
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(field)),
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::Offset,
                left: operand(1),
                right: constant(0),
            },
        ];
        for value in values {
            assert_eq!(
                projected(&ordinary(vec![assign(2, value)])).allocation_origins[3],
                None
            );
        }
    }

    #[test]
    fn carrier_receiver_escape_through_unknown_call_or_alias_copy_is_rejected() {
        let escaped = function(
            vec![
                assign(2, SemanticRvalueKindV1::Use(operand(1))),
                borrow(4, 2),
            ],
            call(1, vec![operand(4)], 1),
            vec![borrow(3, 2)],
            vec![operand(3), constant(0)],
            SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
        );
        assert_eq!(projected(&escaped).allocation_origins[3], None);
        let alias = function(
            vec![
                assign(2, SemanticRvalueKindV1::Use(operand(1))),
                borrow(4, 2),
                assign(7, SemanticRvalueKindV1::Use(operand(4))),
            ],
            call(0, vec![operand(4), constant(0)], 1),
            vec![borrow(3, 2)],
            vec![operand(3), constant(0)],
            SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
        );
        assert_eq!(projected(&alias).allocation_origins[3], None);
        let address = ordinary(vec![
            assign(2, SemanticRvalueKindV1::Use(operand(1))),
            assign(
                4,
                SemanticRvalueKindV1::AddressOf {
                    mutability: SemanticMutabilityV1::Mutable,
                    place: place(2),
                },
            ),
        ]);
        assert_eq!(projected(&address).allocation_origins[3], None);
    }

    #[test]
    fn carrier_indirect_write_and_direct_reinitialization_are_rejected() {
        let dereference = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(4),
            vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, POINTER_TYPE)
                    .unwrap(),
            ],
            POINTER_TYPE,
        )
        .unwrap();
        let indirect = ordinary(vec![
            assign(2, SemanticRvalueKindV1::Use(operand(1))),
            borrow(4, 2),
            statement(SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                dereference,
                operand(6),
                SemanticVolatilityV1::NonVolatile,
                None,
            ))),
        ]);
        let replaced = ordinary(vec![
            assign(2, SemanticRvalueKindV1::Use(operand(1))),
            assign(2, SemanticRvalueKindV1::Use(operand(6))),
        ]);
        let argument_replaced = ordinary(vec![
            assign(1, SemanticRvalueKindV1::Use(operand(6))),
            assign(2, SemanticRvalueKindV1::Use(operand(1))),
        ]);
        for function in [indirect, replaced, argument_replaced] {
            let inventory = assertion_definition_inventory(&function).unwrap();
            let origins = exclusive_owner_carrier_v1::exclusive_owner_value_origins_v1(
                &callables(),
                &function,
                &inventory.counts,
            )
            .unwrap();
            assert_eq!(origins[2], None);
            match local_provenance_with_scalar_inventory_v1(
                &callables(),
                &projection_types(),
                &function,
                &inventory.counts,
                &inventory.address_escaped,
            ) {
                Ok(provenance) => assert_eq!(provenance.allocation_origins[3], None),
                Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "a local may alias multiple kernel allocation origins",
                )) => {}
                Err(error) => panic!("unexpected carrier refusal: {error:?}"),
            }
        }
    }

    #[test]
    fn carrier_mutation_after_a_copy_invalidates_descendant_provenance() {
        let function = ordinary(vec![
            assign(7, SemanticRvalueKindV1::Use(operand(1))),
            assign(2, SemanticRvalueKindV1::Use(operand(7))),
            assign(
                4,
                SemanticRvalueKindV1::AddressOf {
                    mutability: SemanticMutabilityV1::Mutable,
                    place: place(7),
                },
            ),
        ]);
        assert_eq!(projected(&function).allocation_origins[3], None);
        let cycle = ordinary(vec![
            assign(7, SemanticRvalueKindV1::Use(operand(2))),
            assign(2, SemanticRvalueKindV1::Use(operand(7))),
        ]);
        assert_eq!(projected(&cycle).allocation_origins[3], None);
    }

    #[test]
    fn carrier_intrinsic_receiver_position_and_whole_operand_are_exact() {
        let first = || vec![assign(2, SemanticRvalueKindV1::Use(operand(1)))];
        let reversed = function(
            first(),
            SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
            vec![borrow(3, 2)],
            vec![constant(0), operand(3)],
            SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
        );
        assert_eq!(projected(&reversed).allocation_origins[3], None);
        let duplicate = function(
            first(),
            SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
            vec![borrow(3, 2)],
            vec![operand(3), operand(3)],
            SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
        );
        assert_eq!(projected(&duplicate).allocation_origins[3], None);
        let function = ordinary(first());
        let inventory = assertion_definition_inventory(&function).unwrap();
        let origins = exclusive_owner_carrier_v1::exclusive_owner_value_origins_v1(
            &[],
            &function,
            &inventory.counts,
        )
        .unwrap();
        assert_eq!(origins[2], None);
    }

    #[test]
    fn carrier_census_has_exact_work_boundary_and_rejects_foreign_definition_shape() {
        let function = ordinary(vec![assign(2, SemanticRvalueKindV1::Use(operand(1)))]);
        let inventory = assertion_definition_inventory(&function).unwrap();
        let (expected, work) = exclusive_owner_carrier_v1::origins_with_limit(
            &callables(),
            &function,
            &inventory.counts,
            usize::MAX,
        )
        .unwrap();
        assert_eq!(expected[2], Some(0));
        assert_eq!(
            exclusive_owner_carrier_v1::origins_with_limit(
                &callables(),
                &function,
                &inventory.counts,
                work
            )
            .unwrap(),
            (expected, work)
        );
        assert!(matches!(
            exclusive_owner_carrier_v1::origins_with_limit(
                &callables(),
                &function,
                &inventory.counts,
                work - 1
            ),
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "ExclusiveOwner carrier census exceeds the projection work limit"
            ))
        ));
        assert!(matches!(
            exclusive_owner_carrier_v1::exclusive_owner_value_origins_v1(
                &callables(),
                &function,
                &inventory.counts[..7]
            ),
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "ExclusiveOwner carrier definitions do not match the local table"
            ))
        ));
    }

    #[test]
    fn carrier_no_owner_fast_path_charges_full_abi_and_only_zero_origin_table() {
        let base = ordinary(vec![assign(2, SemanticRvalueKindV1::Use(operand(1)))]);
        let abi = SemanticFunctionAbiV1::new(
            SemanticAbiIdentityV1::from_sha256(bytes(220)),
            SemanticLayoutIdentityV1::from_sha256(bytes(220)),
            SemanticCanonAbiV1::GpuKernel,
            false,
            false,
            vec![
                SemanticAbiValueV1::new(
                    POINTER_TYPE,
                    SemanticAbiPassModeV1::Direct(SemanticAbiValueAttributesV1::plain())
                );
                64
            ],
            SemanticAbiValueV1::new(SCALAR_TYPE, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap()
        .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::RawPointer; 64])
        .unwrap();
        let function = with_parts(&base, abi, base.locals().to_vec());
        let counts = assertion_definition_inventory(&function).unwrap().counts;
        let exact = function.locals().len() + function.abi().source_argument_ownership().len();
        assert_eq!(
            exclusive_owner_carrier_v1::origins_with_limit(&[], &function, &counts, exact).unwrap(),
            (vec![None; 8], exact)
        );
        assert!(matches!(
            exclusive_owner_carrier_v1::origins_with_limit(&[], &function, &counts, exact - 1),
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "ExclusiveOwner carrier census exceeds the projection work limit"
            ))
        ));
    }

    #[test]
    fn carrier_long_copy_graph_is_linear_and_has_exact_one_short_boundary() {
        let mut statements = vec![assign(7, SemanticRvalueKindV1::Use(operand(1)))];
        for index in 8..72 {
            statements.push(assign(index, SemanticRvalueKindV1::Use(operand(index - 1))));
        }
        statements.push(assign(2, SemanticRvalueKindV1::Use(operand(71))));
        let base = ordinary(statements);
        let mut locals = base.locals().to_vec();
        for index in 8..72 {
            locals.push(local(
                index as u8,
                POINTER_TYPE,
                SemanticLocalRoleV1::Temporary,
            ));
        }
        let function = with_parts(&base, base.abi().clone(), locals);
        let counts = assertion_definition_inventory(&function).unwrap().counts;
        let (origins, work) = exclusive_owner_carrier_v1::origins_with_limit(
            &callables(),
            &function,
            &counts,
            usize::MAX,
        )
        .unwrap();
        assert_eq!(origins[2], Some(0));
        assert!(work < 30 * function.locals().len());
        assert_eq!(
            exclusive_owner_carrier_v1::origins_with_limit(&callables(), &function, &counts, work)
                .unwrap()
                .0,
            origins
        );
        assert!(
            exclusive_owner_carrier_v1::origins_with_limit(
                &callables(),
                &function,
                &counts,
                work - 1
            )
            .is_err()
        );
    }
}
