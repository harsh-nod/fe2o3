    #[test]
    fn reference_effect_partition_rejects_orphan_substitution_and_duplicates() {
        for bindings in [["alpha", "orphan"], ["alpha", "beta"]] {
            assert!(matches!(
                partition_reference_effect_binding_indices_v1(&["alpha", "zeta"], &bindings,),
                Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "a reference-effect binding outside the exact typed root roster"
                ))
            ));
        }
        assert!(matches!(
            partition_reference_effect_binding_indices_v1(&["alpha", "zeta"], &["alpha", "alpha"],),
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "duplicate reference-effect bindings for one ranked root"
            ))
        ));
        assert!(matches!(
            partition_reference_effect_binding_indices_v1(&["alpha", "alpha"], &["alpha"],),
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "duplicate typed logical roots in the ranked roster"
            ))
        ));
    }

    fn projection_types() -> Vec<SemanticTypeDeclV1> {
        vec![
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256(bytes(1)),
                SemanticLayoutIdentityV1::from_sha256(bytes(1)),
                SemanticTypeLayoutV1::new(Some(4), 4).unwrap(),
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 32,
                }),
            ),
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256(bytes(2)),
                SemanticLayoutIdentityV1::from_sha256(bytes(2)),
                SemanticTypeLayoutV1::new(Some(16), 4).unwrap(),
                SemanticTypeShapeV1::Array {
                    element: SCALAR_TYPE,
                    length: 4,
                },
            ),
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256(bytes(3)),
                SemanticLayoutIdentityV1::from_sha256(bytes(3)),
                SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
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
            ),
        ]
    }

    fn projection_types_with_enum() -> Vec<SemanticTypeDeclV1> {
        let mut types = projection_types();
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(4)),
            SemanticLayoutIdentityV1::from_sha256(bytes(4)),
            SemanticTypeLayoutV1::new(Some(4), 4).unwrap(),
            SemanticTypeShapeV1::enum_type(
                SCALAR_TYPE,
                vec![
                    SemanticEnumVariantV1::new(0, SemanticAggregateTypeV1::new(vec![]).unwrap()),
                    SemanticEnumVariantV1::new(1, SemanticAggregateTypeV1::new(vec![]).unwrap()),
                ],
            )
            .unwrap(),
        ));
        types
    }

    fn assertion_proof_types() -> Vec<SemanticTypeDeclV1> {
        let mut types = projection_types_with_enum();
        for (tag, bytes, scalar) in [
            (40, 1, SemanticScalarTypeV1::Bool),
            (
                41,
                8,
                SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 64,
                },
            ),
            (
                42,
                1,
                SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 8,
                },
            ),
            (
                43,
                4,
                SemanticScalarTypeV1::Integer {
                    signed: true,
                    bits: 32,
                },
            ),
        ] {
            types.push(SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256(self::bytes(tag)),
                SemanticLayoutIdentityV1::from_sha256(self::bytes(tag)),
                SemanticTypeLayoutV1::new(Some(bytes), bytes).unwrap(),
                SemanticTypeShapeV1::Scalar(scalar),
            ));
        }
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(44)),
            SemanticLayoutIdentityV1::from_sha256(bytes(44)),
            SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new(
                    U64_TYPE,
                    SemanticMutabilityV1::Mutable,
                    5,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        ));
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(45)),
            SemanticLayoutIdentityV1::from_sha256(bytes(45)),
            SemanticTypeLayoutV1::aggregate(
                Some(16),
                8,
                SemanticAggregateLayoutV1::new(
                    vec![0, 8],
                    vec![SemanticPaddingV1::new(9, 7).unwrap()],
                )
                .unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(
                SemanticAggregateTypeV1::new(vec![U64_TYPE, BOOL_TYPE]).unwrap(),
            ),
        ));
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(46)),
            SemanticLayoutIdentityV1::from_sha256(bytes(46)),
            SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new(
                    CHECKED_U64_TYPE,
                    SemanticMutabilityV1::Mutable,
                    5,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        ));
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(47)),
            SemanticLayoutIdentityV1::from_sha256(bytes(47)),
            SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 64 }),
        ));
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(48)),
            SemanticLayoutIdentityV1::from_sha256(bytes(48)),
            SemanticTypeLayoutV1::new(Some(16), 16).unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 128,
            }),
        ));
        types
    }

    fn optional_selector_types() -> Vec<SemanticTypeDeclV1> {
        let mut types = assertion_proof_types();
        for (tag, bytes, scalar) in [
            (
                49,
                2,
                SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 16,
                },
            ),
            (50, 4, SemanticScalarTypeV1::Char),
            (51, 4, SemanticScalarTypeV1::Float { bits: 32 }),
        ] {
            types.push(SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256(self::bytes(tag)),
                SemanticLayoutIdentityV1::from_sha256(self::bytes(tag)),
                SemanticTypeLayoutV1::new(Some(bytes), bytes).unwrap(),
                SemanticTypeShapeV1::Scalar(scalar),
            ));
        }
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(52)),
            SemanticLayoutIdentityV1::from_sha256(bytes(52)),
            SemanticTypeLayoutV1::new(Some(4), 4).unwrap(),
            SemanticTypeShapeV1::ValidityScalar(
                SemanticValidityScalarTypeV1::new(
                    SemanticScalarTypeV1::Integer {
                        signed: false,
                        bits: 32,
                    },
                    vec![SemanticScalarValidityRangeV1::new(
                        0,
                        u128::from(u32::MAX - 1),
                    )],
                )
                .unwrap(),
            ),
        ));
        types
    }

    fn enum_definition(local: SemanticLocalIdV1, variant: u32) -> SemanticStatementV1 {
        statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            SemanticPlaceV1::new(local, vec![], ENUM_TYPE).unwrap(),
            SemanticRvalueV1::new(
                ENUM_TYPE,
                SemanticRvalueKindV1::Aggregate(
                    SemanticAggregateRvalueV1::new(
                        SemanticAggregateKindV1::EnumVariant(variant),
                        vec![],
                    )
                    .unwrap(),
                ),
            ),
        )))
    }

    fn enum_discriminant(
        carrier: SemanticLocalIdV1,
        destination: SemanticLocalIdV1,
    ) -> SemanticStatementV1 {
        statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            SemanticPlaceV1::new(destination, vec![], SCALAR_TYPE).unwrap(),
            SemanticRvalueV1::new(
                SCALAR_TYPE,
                SemanticRvalueKindV1::Discriminant(
                    SemanticPlaceV1::new(carrier, vec![], ENUM_TYPE).unwrap(),
                ),
            ),
        )))
    }

    fn local(tag: u8, ty: SemanticTypeIdV1, role: SemanticLocalRoleV1) -> SemanticLocalDeclV1 {
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256(bytes(tag)),
            ty,
            role,
            SemanticSourceProvenanceV1::unavailable(),
        )
    }

    fn block(
        tag: u8,
        statements: Vec<SemanticStatementV1>,
        terminator: SemanticTerminatorKindV1,
    ) -> SemanticBasicBlockV1 {
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256(bytes(tag)),
            SemanticSourceProvenanceV1::unavailable(),
            statements,
            SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), terminator),
        )
        .unwrap()
    }

    fn cfg_edge(role: SemanticEdgeRoleV1, target: u32) -> SemanticControlFlowEdgeV1 {
        SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
    }
    fn projection_function(blocks: Vec<SemanticBasicBlockV1>) -> SemanticFunctionDeclV1 {
        projection_function_with_locals(
            blocks,
            vec![
                local(20, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(21, ARRAY_TYPE, SemanticLocalRoleV1::Temporary),
                local(22, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                local(23, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
            ],
        )
    }

    fn projection_function_with_locals(
        blocks: Vec<SemanticBasicBlockV1>,
        locals: Vec<SemanticLocalDeclV1>,
    ) -> SemanticFunctionDeclV1 {
        let abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256(bytes(10)),
            SemanticLayoutIdentityV1::from_sha256(bytes(10)),
            SemanticCanonAbiV1::GpuKernel,
            SemanticExternAbiV1::GpuKernel,
            false,
            false,
            0,
            vec![],
            SemanticAbiValueV1::new(SCALAR_TYPE, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap();
        SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256(bytes(11)),
            SemanticFunctionRoleV1::KernelRoot,
            SemanticItemDefinitionIdentityV1::from_sha256(bytes(12)),
            SemanticMonomorphizationIdentityV1::from_sha256(bytes(13)),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(14)),
            SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(15)),
            SemanticSourceProvenanceV1::unavailable(),
            abi,
            locals,
            SemanticBlockIdV1::from_index(0),
            blocks,
        )
        .unwrap()
    }

    fn projection_function_with_owned_argument(
        blocks: Vec<SemanticBasicBlockV1>,
        locals: Vec<SemanticLocalDeclV1>,
        ownership: SemanticSourceArgumentOwnershipV1,
    ) -> SemanticFunctionDeclV1 {
        let abi = SemanticFunctionAbiV1::new(
            SemanticAbiIdentityV1::from_sha256(bytes(110)),
            SemanticLayoutIdentityV1::from_sha256(bytes(110)),
            SemanticCanonAbiV1::GpuKernel,
            false,
            false,
            vec![SemanticAbiValueV1::new(
                POINTER_TYPE,
                SemanticAbiPassModeV1::Direct(SemanticAbiValueAttributesV1::plain()),
            )],
            SemanticAbiValueV1::new(SCALAR_TYPE, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap()
        .with_source_argument_ownership(vec![ownership])
        .unwrap();
        SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256(bytes(111)),
            SemanticFunctionRoleV1::KernelRoot,
            SemanticItemDefinitionIdentityV1::from_sha256(bytes(112)),
            SemanticMonomorphizationIdentityV1::from_sha256(bytes(113)),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(114)),
            SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(115)),
            SemanticSourceProvenanceV1::unavailable(),
            abi,
            locals,
            SemanticBlockIdV1::from_index(0),
            blocks,
        )
        .unwrap()
    }

    fn barrier_call(target: Option<u32>) -> SemanticTerminatorKindV1 {
        let destination = target.map(|target| {
            SemanticCallDestinationV1::new(
                scalar_place(),
                SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::CallReturn,
                    SemanticBlockIdV1::from_index(target),
                ),
            )
        });
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(0),
                vec![],
                destination,
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    }

    fn statement(kind: SemanticStatementKindV1) -> SemanticStatementV1 {
        SemanticStatementV1::new(SemanticSourceProvenanceV1::unavailable(), kind)
    }

    fn scalar_place() -> SemanticPlaceV1 {
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(2), vec![], SCALAR_TYPE).unwrap()
    }

    fn ranked_place(offset: u64) -> SemanticPlaceV1 {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(1),
            vec![
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::ConstantIndex {
                        offset,
                        minimum_length: 4,
                        from_end: false,
                    },
                    SCALAR_TYPE,
                )
                .unwrap(),
            ],
            SCALAR_TYPE,
        )
        .unwrap()
    }

    fn dereferenced_place() -> SemanticPlaceV1 {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(3),
            vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, SCALAR_TYPE)
                    .unwrap(),
            ],
            SCALAR_TYPE,
        )
        .unwrap()
    }

    fn constant(value: u128) -> SemanticOperandV1 {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            SCALAR_TYPE,
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, 4).unwrap()),
        ))
    }

    fn bounds_check_function(
        operation: SemanticBinaryOpV1,
        expected: bool,
        swap_message_operands: bool,
        alternate_predecessor: bool,
    ) -> SemanticFunctionDeclV1 {
        let condition_local = SemanticLocalIdV1::from_index(2);
        let index_local = SemanticLocalIdV1::from_index(4);
        let length_local = SemanticLocalIdV1::from_index(5);
        let place = |local| SemanticPlaceV1::new(local, vec![], SCALAR_TYPE).unwrap();
        let operand = |local| SemanticOperandV1::Copy(place(local));
        let index_definition =
            statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(index_local),
                SemanticRvalueV1::new(SCALAR_TYPE, SemanticRvalueKindV1::Use(constant(0))),
            )));
        let slice =
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![], ARRAY_TYPE).unwrap();
        let length_definition =
            statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(length_local),
                SemanticRvalueV1::new(SCALAR_TYPE, SemanticRvalueKindV1::Length(slice)),
            )));
        let comparison = statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(condition_local),
            SemanticRvalueV1::new(
                SCALAR_TYPE,
                SemanticRvalueKindV1::Binary {
                    operation,
                    left: operand(index_local),
                    right: operand(length_local),
                },
            ),
        )));
        let (index, length) = if swap_message_operands {
            (operand(length_local), operand(index_local))
        } else {
            (operand(index_local), operand(length_local))
        };
        let success_block = if alternate_predecessor { 2 } else { 1 };
        let mut blocks = vec![block(
            80,
            vec![index_definition, length_definition, comparison],
            SemanticTerminatorKindV1::Assert {
                condition: operand(condition_local),
                expected,
                message: SemanticAssertMessageV1::BoundsCheck { length, index },
                target: cfg_edge(SemanticEdgeRoleV1::AssertSuccess, success_block),
                unwind: SemanticUnwindActionV1::Unreachable,
            },
        )];
        if alternate_predecessor {
            blocks.push(block(
                81,
                vec![],
                SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 2)),
            ));
        }
        blocks.push(block(82, vec![], SemanticTerminatorKindV1::Return));
        projection_function_with_locals(
            blocks,
            vec![
                local(82, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(83, ARRAY_TYPE, SemanticLocalRoleV1::Temporary),
                local(84, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                local(85, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
                local(86, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                local(87, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
            ],
        )
    }

    fn repeated_bounds_check_function(
        same_slice: bool,
        redefine_slice: bool,
    ) -> SemanticFunctionDeclV1 {
        let primary_slice = SemanticLocalIdV1::from_index(1);
        let second_slice = if same_slice {
            primary_slice
        } else {
            SemanticLocalIdV1::from_index(9)
        };
        let source_slice = SemanticLocalIdV1::from_index(8);
        let scalar_place = |local| SemanticPlaceV1::new(local, vec![], SCALAR_TYPE).unwrap();
        let slice_place = |local| SemanticPlaceV1::new(local, vec![], ARRAY_TYPE).unwrap();
        let operand = |local| SemanticOperandV1::Copy(scalar_place(local));
        let assign_slice = |destination| {
            statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                slice_place(destination),
                SemanticRvalueV1::new(
                    ARRAY_TYPE,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(slice_place(source_slice))),
                ),
            )))
        };
        let guard_statements = |slice, index, length, condition, value| {
            vec![
                statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    scalar_place(index),
                    SemanticRvalueV1::new(SCALAR_TYPE, SemanticRvalueKindV1::Use(constant(value))),
                ))),
                statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    scalar_place(length),
                    SemanticRvalueV1::new(
                        SCALAR_TYPE,
                        SemanticRvalueKindV1::Length(slice_place(slice)),
                    ),
                ))),
                statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    scalar_place(condition),
                    SemanticRvalueV1::new(
                        SCALAR_TYPE,
                        SemanticRvalueKindV1::Binary {
                            operation: SemanticBinaryOpV1::LessThan,
                            left: operand(index),
                            right: operand(length),
                        },
                    ),
                ))),
            ]
        };
        let mut first = vec![assign_slice(primary_slice)];
        if !same_slice {
            first.push(assign_slice(second_slice));
        }
        first.extend(guard_statements(
            primary_slice,
            SemanticLocalIdV1::from_index(4),
            SemanticLocalIdV1::from_index(5),
            SemanticLocalIdV1::from_index(2),
            0,
        ));
        let mut second = Vec::new();
        if redefine_slice {
            second.push(assign_slice(primary_slice));
        }
        second.extend(guard_statements(
            second_slice,
            SemanticLocalIdV1::from_index(7),
            SemanticLocalIdV1::from_index(6),
            SemanticLocalIdV1::from_index(3),
            1,
        ));
        projection_function_with_locals(
            vec![
                block(
                    120,
                    first,
                    SemanticTerminatorKindV1::Assert {
                        condition: operand(SemanticLocalIdV1::from_index(2)),
                        expected: true,
                        message: SemanticAssertMessageV1::BoundsCheck {
                            length: operand(SemanticLocalIdV1::from_index(5)),
                            index: operand(SemanticLocalIdV1::from_index(4)),
                        },
                        target: cfg_edge(SemanticEdgeRoleV1::AssertSuccess, 1),
                        unwind: SemanticUnwindActionV1::Unreachable,
                    },
                ),
                block(
                    121,
                    second,
                    SemanticTerminatorKindV1::Assert {
                        condition: operand(SemanticLocalIdV1::from_index(3)),
                        expected: true,
                        message: SemanticAssertMessageV1::BoundsCheck {
                            length: operand(SemanticLocalIdV1::from_index(6)),
                            index: operand(SemanticLocalIdV1::from_index(7)),
                        },
                        target: cfg_edge(SemanticEdgeRoleV1::AssertSuccess, 2),
                        unwind: SemanticUnwindActionV1::Unreachable,
                    },
                ),
                block(122, vec![], SemanticTerminatorKindV1::Return),
            ],
            vec![
                local(123, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(124, ARRAY_TYPE, SemanticLocalRoleV1::Temporary),
                local(125, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                local(126, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                local(127, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                local(128, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                local(129, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                local(130, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                local(131, ARRAY_TYPE, SemanticLocalRoleV1::Temporary),
                local(132, ARRAY_TYPE, SemanticLocalRoleV1::Temporary),
            ],
        )
    }

    fn project_test_bounds_checks(
        function: &SemanticFunctionDeclV1,
        first_argument: usize,
    ) -> Result<ProjectedBoundsChecksV1, ProductionRankedProjectionErrorV1> {
        let mut operations = Vec::new();
        let mut next_value = 0;
        project_rust_bounds_checks(
            function,
            first_argument,
            &[],
            &mut operations,
            &mut next_value,
        )
    }

    fn literal_bounds_check_function(index: u128, length: u128) -> SemanticFunctionDeclV1 {
        projection_function_with_locals(
            vec![
                block(
                    80,
                    vec![],
                    SemanticTerminatorKindV1::Assert {
                        condition: constant(u128::from(index < length)),
                        expected: true,
                        message: SemanticAssertMessageV1::BoundsCheck {
                            length: constant(length),
                            index: constant(index),
                        },
                        target: cfg_edge(SemanticEdgeRoleV1::AssertSuccess, 1),
                        unwind: SemanticUnwindActionV1::Unreachable,
                    },
                ),
                block(81, vec![], SemanticTerminatorKindV1::Return),
            ],
            vec![local(82, SCALAR_TYPE, SemanticLocalRoleV1::Return)],
        )
    }

    struct BranchBoundsCheckOptionsV1 {
        operation: SemanticBinaryOpV1,
        switch_values: Vec<u128>,
        swap_comparison_operands: bool,
        length_from_slice: bool,
        alternate_predecessor: bool,
        duplicate_condition: bool,
        duplicate_index: bool,
        duplicate_length: bool,
    }

    impl Default for BranchBoundsCheckOptionsV1 {
        fn default() -> Self {
            Self {
                operation: SemanticBinaryOpV1::LessThan,
                switch_values: vec![0],
                swap_comparison_operands: false,
                length_from_slice: true,
                alternate_predecessor: false,
                duplicate_condition: false,
                duplicate_index: false,
                duplicate_length: false,
            }
        }
    }

    fn branch_bounds_check_function(options: BranchBoundsCheckOptionsV1) -> SemanticFunctionDeclV1 {
        let condition_local = SemanticLocalIdV1::from_index(2);
        let index_local = SemanticLocalIdV1::from_index(4);
        let length_local = SemanticLocalIdV1::from_index(5);
        let place = |local| SemanticPlaceV1::new(local, vec![], SCALAR_TYPE).unwrap();
        let operand = |local| SemanticOperandV1::Copy(place(local));
        let index_definition =
            statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(index_local),
                SemanticRvalueV1::new(SCALAR_TYPE, SemanticRvalueKindV1::Use(constant(0))),
            )));
        let slice =
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![], ARRAY_TYPE).unwrap();
        let length_value = if options.length_from_slice {
            SemanticRvalueKindV1::Length(slice)
        } else {
            SemanticRvalueKindV1::Use(constant(4))
        };
        let length_definition =
            statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(length_local),
                SemanticRvalueV1::new(SCALAR_TYPE, length_value),
            )));
        let (left, right) = if options.swap_comparison_operands {
            (operand(length_local), operand(index_local))
        } else {
            (operand(index_local), operand(length_local))
        };
        let comparison = statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(condition_local),
            SemanticRvalueV1::new(
                SCALAR_TYPE,
                SemanticRvalueKindV1::Binary {
                    operation: options.operation,
                    left,
                    right,
                },
            ),
        )));
        let mut statements = vec![
            index_definition.clone(),
            length_definition.clone(),
            comparison.clone(),
        ];
        if options.duplicate_index {
            statements.push(index_definition);
        }
        if options.duplicate_length {
            statements.push(length_definition);
        }
        if options.duplicate_condition {
            statements.push(comparison);
        }
        let targets = options
            .switch_values
            .iter()
            .copied()
            .map(|value| {
                SemanticSwitchTargetV1::new(
                    value,
                    cfg_edge(
                        SemanticEdgeRoleV1::SwitchValue,
                        if value == 1 { 1 } else { 2 },
                    ),
                )
            })
            .collect();
        let otherwise_target = if options.switch_values.as_slice() == [0] {
            1
        } else {
            2
        };
        let mut blocks = vec![
            block(
                88,
                statements,
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: operand(condition_local),
                    targets: SemanticSwitchTargetsV1::new(
                        targets,
                        cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, otherwise_target),
                    )
                    .unwrap(),
                },
            ),
            block(89, vec![], SemanticTerminatorKindV1::Return),
        ];
        blocks.push(if options.alternate_predecessor {
            block(
                90,
                vec![],
                SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
            )
        } else {
            block(90, vec![], SemanticTerminatorKindV1::Return)
        });
        projection_function_with_locals(
            blocks,
            vec![
                local(88, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(89, ARRAY_TYPE, SemanticLocalRoleV1::Temporary),
                local(90, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                local(91, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
                local(92, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                local(93, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
            ],
        )
    }

    fn option_dominance_chain(
        producer_count: usize,
    ) -> (SemanticFunctionDeclV1, Vec<SemanticOptionProducerV1>) {
        assert!(producer_count > 0 && producer_count <= 64);
        let mut locals = Vec::with_capacity(1 + 2 * producer_count);
        locals.push(local(0, SCALAR_TYPE, SemanticLocalRoleV1::Return));
        let mut producers = Vec::with_capacity(producer_count);
        let mut blocks = Vec::with_capacity(3 * producer_count + 1);
        let final_some = 2 * producer_count;
        for index in 0..producer_count {
            let option_local = SemanticLocalIdV1::from_index((1 + 2 * index) as u32);
            let discriminator_local = SemanticLocalIdV1::from_index((2 + 2 * index) as u32);
            locals.push(local(
                (1 + 2 * index) as u8,
                POINTER_TYPE,
                SemanticLocalRoleV1::Temporary,
            ));
            locals.push(local(
                (2 + 2 * index) as u8,
                SCALAR_TYPE,
                SemanticLocalRoleV1::Temporary,
            ));
            let producer_block = 2 * index;
            let switch_block = producer_block + 1;
            let some_target = if index + 1 == producer_count {
                final_some
            } else {
                producer_block + 2
            };
            let none_target = final_some + 1 + index;
            let option_place = SemanticPlaceV1::new(option_local, vec![], POINTER_TYPE).unwrap();
            let discriminator_place =
                SemanticPlaceV1::new(discriminator_local, vec![], SCALAR_TYPE).unwrap();
            let call = SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(0),
                vec![],
                Some(SemanticCallDestinationV1::new(
                    option_place.clone(),
                    cfg_edge(SemanticEdgeRoleV1::CallReturn, switch_block as u32),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap();
            blocks.push(block(
                producer_block as u8,
                vec![],
                SemanticTerminatorKindV1::Call(call),
            ));
            blocks.push(block(
                switch_block as u8,
                vec![statement(SemanticStatementKindV1::Assign(
                    SemanticAssignmentV1::new(
                        discriminator_place.clone(),
                        SemanticRvalueV1::new(
                            SCALAR_TYPE,
                            SemanticRvalueKindV1::Discriminant(option_place),
                        ),
                    ),
                ))],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(discriminator_place),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            cfg_edge(SemanticEdgeRoleV1::SwitchValue, none_target as u32),
                        )],
                        cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, some_target as u32),
                    )
                    .unwrap(),
                },
            ));
            producers.push(SemanticOptionProducerV1::new(
                option_local,
                SemanticBlockIdV1::from_index(switch_block as u32),
            ));
        }
        blocks.push(block(
            final_some as u8,
            vec![],
            SemanticTerminatorKindV1::Return,
        ));
        for index in 0..producer_count {
            blocks.push(block(
                (final_some + 1 + index) as u8,
                vec![],
                SemanticTerminatorKindV1::Return,
            ));
        }
        (projection_function_with_locals(blocks, locals), producers)
    }

    fn atomic_access() -> SemanticAtomicAccessV1 {
        SemanticAtomicAccessV1::new(
            SemanticAtomicOrderingV1::Relaxed,
            SemanticAtomicScopeV1::Agent,
        )
    }

    type AuditOutput = (
        Vec<ProductionRankedOperationV1>,
        Vec<ProjectedAccessSourceV1>,
        String,
    );

    fn audit_function(
        function: &SemanticFunctionDeclV1,
    ) -> Result<AuditOutput, ProductionRankedProjectionErrorV1> {
        let local_contracts = synthetic_local_contracts(function);
        audit_function_with_local_contracts(function, &local_contracts)
    }

    fn audit_function_with_local_contracts(
        function: &SemanticFunctionDeclV1,
        local_contracts: &ProjectionLocalContractsV1,
    ) -> Result<AuditOutput, ProductionRankedProjectionErrorV1> {
        let types = assertion_proof_types();
        let constants = constant_locals(function)?;
        let mut operations = Vec::new();
        let mut sources = Vec::new();
        let mut projected_views = vec![None; function.locals().len()];
        let mut guarded_sites = Vec::new();
        let mut next_value = 0;
        let mut ranked_ir = String::new();
        let callable_effects = DefinedCallableEmptyEffectSummariesV1 {
            decisions: Box::new([]),
        };
        for (block_index, basic_block) in function.blocks().iter().enumerate() {
            for semantic_statement in basic_block.statements() {
                project_statement_accesses(
                    &types,
                    function,
                    block_index,
                    &[],
                    semantic_statement,
                    &constants,
                    local_contracts,
                    &[],
                    &mut guarded_sites,
                    &mut projected_views,
                    &mut operations,
                    &mut sources,
                    &mut next_value,
                    &mut ranked_ir,
                )?;
            }
            project_terminator_accesses(
                &[],
                &callable_effects,
                &types,
                function,
                block_index,
                &[],
                basic_block.terminator().kind(),
                basic_block.terminator().source(),
                &constants,
                local_contracts,
                &[],
                &mut guarded_sites,
                &mut projected_views,
                &mut operations,
                &mut sources,
                &mut next_value,
                &mut ranked_ir,
            )?;
        }
        Ok((operations, sources, ranked_ir))
    }

    fn synthetic_local_contracts(function: &SemanticFunctionDeclV1) -> ProjectionLocalContractsV1 {
        ProjectionLocalContractsV1 {
            checked_references: CheckedReferencesV1 {
                origins: vec![None; function.locals().len()],
                option_dominance: SemanticOptionDominanceV1::analyze(function, &[]).unwrap(),
                enum_payload_dominance: SemanticEnumPayloadDominanceV1::analyze(
                    function,
                    &projection_types(),
                )
                .unwrap(),
            },
            allocations: (0..function.locals().len())
                .map(|local| {
                    let identity = local as u64 + 1;
                    Some(AllocationContractV1 {
                        allocation_origin: identity,
                        noalias_class: identity,
                        writable: true,
                        singleton_object: false,
                    })
                })
                .collect(),
            allocation_provenance: (0..function.locals().len())
                .map(|_| Some(LocalAllocationProvenanceV1::Argument(0)))
                .collect(),
        }
    }

    fn audit_statements(
        statements: Vec<SemanticStatementV1>,
    ) -> Result<AuditOutput, ProductionRankedProjectionErrorV1> {
        audit_function(&projection_function(vec![block(
            30,
            statements,
            SemanticTerminatorKindV1::Return,
        )]))
    }

    fn access_kinds(operations: &[ProductionRankedOperationV1]) -> Vec<AccessKindAttr> {
        operations
            .iter()
            .filter_map(|operation| match operation {
                ProductionRankedOperationV1::Access { kind, .. }
                | ProductionRankedOperationV1::ValueAccess { kind, .. }
                | ProductionRankedOperationV1::AtomicAccess { kind, .. }
                | ProductionRankedOperationV1::AtomicValueAccess { kind, .. }
                | ProductionRankedOperationV1::AllocationEffect { kind, .. } => Some(*kind),
                ProductionRankedOperationV1::View { .. }
                | ProductionRankedOperationV1::ExecutionLayout { .. }
                | ProductionRankedOperationV1::ViewInSpace { .. }
                | ProductionRankedOperationV1::PipelineCreate { .. }
                | ProductionRankedOperationV1::PipelineEvent { .. }
                | ProductionRankedOperationV1::IndexConstant { .. }
                | ProductionRankedOperationV1::IndexUnsignedCast { .. }
                | ProductionRankedOperationV1::IndexUnknown { .. }
                | ProductionRankedOperationV1::InvocationIndex { .. }
                | ProductionRankedOperationV1::DeterministicJoin { .. }
                | ProductionRankedOperationV1::IndexBinary { .. }
                | ProductionRankedOperationV1::CheckedTiledIndex2D { .. }
                | ProductionRankedOperationV1::CheckedRowStripedIndex2D { .. }
                | ProductionRankedOperationV1::PredicatedCheckedTiledIndex2D { .. }
                | ProductionRankedOperationV1::PredicatedCheckedRowStripedIndex2D { .. }
                | ProductionRankedOperationV1::PredicatedAccess { .. }
                | ProductionRankedOperationV1::Dimension { .. }
                | ProductionRankedOperationV1::OwnershipContract { .. }
                | ProductionRankedOperationV1::Barrier { .. }
                | ProductionRankedOperationV1::Fence { .. }
                | ProductionRankedOperationV1::TensorLayout { .. }
                | ProductionRankedOperationV1::TensorResultComponent { .. }
                | ProductionRankedOperationV1::SemanticSymbol { .. }
                | ProductionRankedOperationV1::SemanticConstant { .. }
                | ProductionRankedOperationV1::SemanticBinary { .. }
                | ProductionRankedOperationV1::SemanticExpression { .. }
                | ProductionRankedOperationV1::CollectiveSemantics { .. }
                | ProductionRankedOperationV1::RequireEquivalent { .. }
                | ProductionRankedOperationV1::RequireAuthenticatedReferenceEquivalent { .. }
                | ProductionRankedOperationV1::RequestAuthenticatedReferenceEquivalent { .. }
                | ProductionRankedOperationV1::RequireEffectRefinement { .. }
                | ProductionRankedOperationV1::RequestEffectRefinement { .. }
                | ProductionRankedOperationV1::RequireNumericalRefinement { .. }
                | ProductionRankedOperationV1::RequestNumericalRefinement { .. }
                | ProductionRankedOperationV1::RequireTensorRefinement { .. }
                | ProductionRankedOperationV1::RequestTensorRefinement { .. } => None,
            })
            .collect()
    }

    fn single_guarded_cfg(
        entry: Vec<ProductionRankedOperationV1>,
        access: GuardedRankedAccessV1,
    ) -> (
        Vec<ProductionRankedBlockV1>,
        Vec<ProjectedAccessSourceV1>,
        String,
    ) {
        let function =
            projection_function(vec![block(29, vec![], SemanticTerminatorKindV1::Return)]);
        let (blocks, sources, _) = build_ranked_cfg(
            &projection_types(),
            &function,
            &[],
            &vec![None; function.locals().len()],
            &vec![None; function.blocks().len()],
            &[],
            entry,
            vec![ProjectedSemanticBlockV1 {
                items: vec![ProjectedBlockItemV1::Guarded(access)],
            }],
        )
        .unwrap();
        let ranked_ir = format_ranked_cfg("guarded_test", &blocks).unwrap();
        (blocks, sources, ranked_ir)
    }

    fn assert_unsupported(
        result: Result<AuditOutput, ProductionRankedProjectionErrorV1>,
        expected: &'static str,
    ) {
        match result {
            Err(
                ProductionRankedProjectionErrorV1::Incomplete(detail)
                | ProductionRankedProjectionErrorV1::Unsupported(detail),
            ) => {
                assert_eq!(detail, expected)
            }
            Err(other) => panic!("expected unsupported projection, got {other}"),
            Ok(_) => panic!("hostile projection unexpectedly passed"),
        }
    }
