// Included inside production_ranked_projection_v1::tests.
mod slice_extent_projection_tests_v1 {
    use super::*;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    use fe2o3_mir_model::semantic_mir_v1::SemanticRustTypeKindV1;
    use slice_extent_projection_v1::{Context, Scratch, with_scope};

    type Error = ProductionRankedProjectionErrorV1;
    const SLICE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
    const REFERENCE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
    const USIZE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
    const BOOL: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);

    struct Facts<'a, 'w>(&'a mut Budget<'w>);

    impl ProjectedAssertionFactsV1 for Facts<'_, '_> {
        fn charge_private_array_work(&mut self, amount: usize) -> Result<(), Error> {
            self.0
                .charge_work(amount)
                .map_err(ranked_projection_source_v1::resource)
        }

        fn scalar_private_storage_v1(&self) -> Result<usize, Error> {
            Ok(self.0.storage())
        }

        fn reserve_scalar_private_storage_v1(&mut self, amount: usize) -> Result<(), Error> {
            self.0
                .reserve_storage(amount)
                .map_err(ranked_projection_source_v1::resource)
        }

        fn release_scalar_private_storage_v1(&mut self, amount: usize) -> Result<(), Error> {
            self.0
                .release_storage(amount)
                .map_err(ranked_projection_source_v1::resource)
        }

        fn private_array_initializer_count(
            &mut self,
            _: usize,
            _: usize,
        ) -> Result<Option<u64>, Error> {
            panic!("slice metadata must not request initializer authority")
        }

        fn is_materialized_block(&mut self, _: usize) -> Result<bool, Error> {
            panic!("slice metadata must not request materialization authority")
        }

        fn condition(
            &mut self,
            _: usize,
            _: bool,
            _: SemanticBlockIdV1,
        ) -> Result<canonical_assertion_facts_v1::ProjectedAssertionConditionV1, Error> {
            panic!("slice metadata must not request assertion authority")
        }
    }

    fn types() -> Vec<SemanticTypeDeclV1> {
        let mut types = volatile_load_source_types_v1(
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 32 }),
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
        );
        types.push(
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256(bytes(230)),
                SemanticLayoutIdentityV1::from_sha256(bytes(230)),
                SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 64,
                }),
            )
            .with_rust_type_kind(SemanticRustTypeKindV1::Usize),
        );
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(231)),
            SemanticLayoutIdentityV1::from_sha256(bytes(231)),
            SemanticTypeLayoutV1::new(Some(1), 1).unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool),
        ));
        types
    }

    fn slice_place(receiver: u32, index: Option<u32>) -> SemanticPlaceV1 {
        let mut projections =
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, SLICE).unwrap()];
        if let Some(index) = index {
            projections.push(
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(index)),
                    SCALAR_TYPE,
                )
                .unwrap(),
            );
        }
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(receiver),
            projections,
            if index.is_some() { SCALAR_TYPE } else { SLICE },
        )
        .unwrap()
    }

    fn rebuild(
        original: &SemanticFunctionDeclV1,
        locals: Vec<SemanticLocalDeclV1>,
        blocks: Vec<SemanticBasicBlockV1>,
    ) -> SemanticFunctionDeclV1 {
        SemanticFunctionDeclV1::new(
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
    }

    fn fixture(cast: bool) -> SemanticFunctionDeclV1 {
        let base = projection_function(vec![block(232, vec![], SemanticTerminatorKindV1::Return)]);
        let abi = SemanticFunctionAbiV1::new(
            SemanticAbiIdentityV1::from_sha256(bytes(233)),
            SemanticLayoutIdentityV1::from_sha256(bytes(233)),
            SemanticCanonAbiV1::GpuKernel,
            false,
            false,
            vec![
                SemanticAbiValueV1::new(
                    REFERENCE,
                    SemanticAbiPassModeV1::Pair {
                        first: SemanticAbiValueAttributesV1::plain(),
                        second: SemanticAbiValueAttributesV1::plain(),
                    },
                ),
                SemanticAbiValueV1::new(
                    REFERENCE,
                    SemanticAbiPassModeV1::Pair {
                        first: SemanticAbiValueAttributesV1::plain(),
                        second: SemanticAbiValueAttributesV1::plain(),
                    },
                ),
                SemanticAbiValueV1::new(
                    USIZE,
                    SemanticAbiPassModeV1::Direct(SemanticAbiValueAttributesV1::plain()),
                ),
            ],
            SemanticAbiValueV1::new(SCALAR_TYPE, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap();
        let mut locals = vec![
            local(234, SCALAR_TYPE, SemanticLocalRoleV1::Return),
            local(235, REFERENCE, SemanticLocalRoleV1::Argument(0)),
            local(236, REFERENCE, SemanticLocalRoleV1::Argument(1)),
            local(237, USIZE, SemanticLocalRoleV1::Argument(2)),
            local(238, REFERENCE, SemanticLocalRoleV1::Temporary),
            local(239, REFERENCE, SemanticLocalRoleV1::Temporary),
        ];
        for n in 6..12 {
            locals.push(local(
                234 + n,
                if n < 9 { USIZE } else { BOOL },
                SemanticLocalRoleV1::Temporary,
            ));
        }
        let mut prefix = vec![
            typed_assignment(
                4,
                REFERENCE,
                if cast {
                    SemanticRvalueKindV1::Cast {
                        kind: SemanticCastKindV1::Transmute,
                        operand: typed_operand(1, REFERENCE),
                    }
                } else {
                    SemanticRvalueKindV1::Use(typed_operand(1, REFERENCE))
                },
            ),
            typed_assignment(
                5,
                REFERENCE,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Move(typed_place(4, REFERENCE))),
            ),
            typed_assignment(6, USIZE, SemanticRvalueKindV1::Length(slice_place(4, None))),
            typed_assignment(
                7,
                USIZE,
                SemanticRvalueKindV1::Unary {
                    operation: SemanticUnaryOpV1::PointerMetadata,
                    operand: typed_operand(5, REFERENCE),
                },
            ),
            typed_assignment(8, USIZE, SemanticRvalueKindV1::Length(slice_place(2, None))),
        ];
        let mut blocks = Vec::new();
        for n in 0..3_u32 {
            let mut statements = if n == 0 {
                std::mem::take(&mut prefix)
            } else {
                vec![typed_assignment(
                    0,
                    SCALAR_TYPE,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(slice_place(
                        if n == 1 { 4 } else { 5 },
                        Some(3),
                    ))),
                )]
            };
            statements.push(typed_assignment(
                9 + n,
                BOOL,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::LessThan,
                    left: typed_operand(3, USIZE),
                    right: typed_operand(6 + n, USIZE),
                },
            ));
            blocks.push(block(
                246 + n as u8,
                statements,
                SemanticTerminatorKindV1::Assert {
                    condition: typed_operand(9 + n, BOOL),
                    expected: true,
                    message: SemanticAssertMessageV1::BoundsCheck {
                        length: typed_operand(6 + n, USIZE),
                        index: typed_operand(3, USIZE),
                    },
                    target: cfg_edge(SemanticEdgeRoleV1::AssertSuccess, n + 1),
                    unwind: SemanticUnwindActionV1::Unreachable,
                },
            ));
        }
        blocks.push(block(
            249,
            vec![typed_assignment(
                0,
                SCALAR_TYPE,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(slice_place(2, Some(3)))),
            )],
            SemanticTerminatorKindV1::Return,
        ));
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
            blocks,
        )
        .unwrap()
    }

    fn scratch(types: &[SemanticTypeDeclV1], function: &SemanticFunctionDeclV1) -> Scratch {
        let inventory = assertion_definition_inventory(function).unwrap();
        Scratch {
            origins: local_stable_argument_origins(types, function).unwrap(),
            arguments: vec![None; function.locals().len()],
            definitions: inventory.counts,
            escaped: inventory.address_escaped,
        }
    }

    fn definitions(function: &SemanticFunctionDeclV1) -> Vec<BoundsLocalDefinitionV1<'_>> {
        let mut definitions = vec![BoundsLocalDefinitionV1::default(); function.locals().len()];
        for block in function.blocks() {
            for statement in block.statements() {
                if let SemanticStatementKindV1::Assign(assignment) = statement.kind()
                    && assignment.destination().projections().is_empty()
                {
                    let slot = &mut definitions[assignment.destination().local().index() as usize];
                    slot.count += 1;
                    slot.value = Some(assignment.value());
                }
            }
        }
        definitions
    }

    fn bounds(
        types: &[SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
        scratch: &mut Scratch,
        first: usize,
        operations: &mut Vec<ProductionRankedOperationV1>,
        next_value: &mut u32,
    ) -> ProjectedBoundsChecksV1 {
        project_rust_bounds_checks_with_ordinary_v1(
            types,
            function,
            first,
            &[],
            &[],
            None,
            operations,
            next_value,
            Some(&mut Context {
                scratch,
                facts: &mut ComponentDynamicAssertionFactsV1,
            }),
        )
        .unwrap()
    }

    #[test]
    fn metadata_result_requires_the_exact_pointer_width_for_structural_and_nominal_types() {
        let function = fixture(false);
        for kind in [
            SemanticRustTypeKindV1::Ordinary,
            SemanticRustTypeKindV1::Usize,
        ] {
            for (bits, signed) in [(64, false), (32, false), (64, true)] {
                let mut types = types();
                types[USIZE.index() as usize] = SemanticTypeDeclV1::new(
                    SemanticTypeIdentityV1::from_sha256(bytes(230)),
                    SemanticLayoutIdentityV1::from_sha256(bytes(230)),
                    SemanticTypeLayoutV1::new(Some(u64::from(bits / 8)), u64::from(bits / 8))
                        .unwrap(),
                    SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { bits, signed }),
                )
                .with_rust_type_kind(kind);
                let mut scratch = scratch(&types, &function);
                let mut next_argument = 3;
                let actual = Context {
                    scratch: &mut scratch,
                    facts: &mut ComponentDynamicAssertionFactsV1,
                }
                .extent(
                    &types,
                    &function,
                    SemanticLocalIdV1::from_index(6),
                    &definitions(&function),
                    &mut next_argument,
                )
                .unwrap();
                let valid = bits == 64 && !signed;
                assert_eq!(
                    actual,
                    valid.then_some(ProductionRankedValueV1::Argument(3))
                );
                assert_eq!(next_argument, if valid { 4 } else { 3 });
            }
        }
    }

    #[test]
    fn ordinary_bounds_share_copy_move_metadata_but_distinguish_slice_inputs() {
        let types = types();
        let function = fixture(false);
        let mut scratch = scratch(&types, &function);
        let mut operations = vec![];
        let mut next_value = 0;
        let projected = bounds(
            &types,
            &function,
            &mut scratch,
            3,
            &mut operations,
            &mut next_value,
        );
        assert_eq!(projected.argument_count, 5);
        assert_eq!(projected.checks.len(), 3);
        assert_eq!(
            projected
                .checks
                .iter()
                .map(|c| c.extent)
                .collect::<Vec<_>>(),
            vec![
                ProductionRankedValueV1::Argument(3),
                ProductionRankedValueV1::Argument(3),
                ProductionRankedValueV1::Argument(4),
            ]
        );
        assert!(
            projected
                .checks
                .iter()
                .all(|c| c.index == projected.checks[0].index)
        );
        assert!(matches!(
            operations.as_slice(),
            [ProductionRankedOperationV1::IndexUnknown { .. }]
        ));
        assert_eq!(next_value, 1);
        assert_eq!(&scratch.arguments[..2], &[Some(3), Some(4)]);
    }

    #[test]
    fn ordinary_bounds_reuse_the_existing_volatile_extent_cache() {
        let types = types();
        let function = fixture(false);
        let mut scratch = scratch(&types, &function);
        let mut next_argument = 3;
        let volatile = project_runtime_slice_extent_argument_v1(
            1,
            &scratch.origins,
            &mut scratch.arguments,
            &mut next_argument,
        )
        .unwrap();
        let projected = bounds(
            &types,
            &function,
            &mut scratch,
            next_argument,
            &mut vec![],
            &mut 0,
        );
        assert_eq!(projected.argument_count, 5);
        assert_eq!(projected.checks[0].extent, volatile);
        assert_eq!(projected.checks[1].extent, volatile);
        assert_eq!(
            projected.checks[2].extent,
            ProductionRankedValueV1::Argument(4)
        );
    }

    #[test]
    fn ordinary_bounds_guard_and_view_use_the_grown_argument_count() {
        let types = types();
        let function = fixture(false);
        let mut scratch = scratch(&types, &function);
        let mut operations = vec![];
        let mut next_value = 0;
        let projected = bounds(
            &types,
            &function,
            &mut scratch,
            3,
            &mut operations,
            &mut next_value,
        );
        let mut sites = vec![];
        let mut sources = vec![];
        let mut views = ProjectedViewsV1::new(function.locals().len(), None);
        project_statement_accesses(
            &types,
            &function,
            3,
            &projected.checks,
            &function.blocks()[3].statements()[0],
            &constant_locals(&function).unwrap(),
            &synthetic_local_contracts_with_types(&function, &types),
            &[],
            &mut sites,
            &mut views,
            &mut operations,
            &mut sources,
            &mut next_value,
            &mut String::new(),
        )
        .unwrap();
        assert_eq!(sites.len(), 1);
        let guard = sites.remove(0).access;
        assert_eq!(
            guard.comparisons,
            vec![(
                projected.checks[2].index,
                ProductionRankedValueV1::Argument(4)
            )]
        );
        assert_eq!(guard.output_extent, None);
        assert!(operations.iter().any(|operation| matches!(operation,
            ProductionRankedOperationV1::ViewInSpace { shape, dynamic_extents, .. }
                if shape == &[DYNAMIC_EXTENT] && dynamic_extents == &[ProductionRankedValueV1::Argument(4)]
        )));
        let (blocks, _, _) = single_guarded_cfg(operations, guard);
        let kernel = ProductionRankedKernelV1::new(
            "slice_extent_argument_count",
            projected.argument_count,
            blocks,
        )
        .unwrap();
        let construction =
            ProductionConstructionV1::ranked_kernel("slice_extent_test", kernel).unwrap();
        let lowering = compile_ranked_kernel_for_lowering_v1(
            construction,
            ProductionSessionLimitsV1::default(),
        )
        .unwrap();
        assert!(lowering.bounds_report().is_clean());
    }

    #[test]
    fn ordinary_bounds_reject_projected_length_after_caching_whole_slice_metadata() {
        let types = types();
        let original = fixture(false);
        let mut blocks = original.blocks().to_vec();
        let mut first = blocks[0].statements().to_vec();
        first.remove(3);
        blocks[0] = block(246, first, original.blocks()[0].terminator().kind().clone());

        // Subtype preserves the slice type and is a valid additional MIR projection.
        let projected_slice = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(4),
            vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, SLICE).unwrap(),
                SemanticProjectionV1::new(SemanticProjectionKindV1::Subtype, SLICE).unwrap(),
            ],
            SLICE,
        )
        .unwrap();
        let mut second = blocks[1].statements().to_vec();
        second.insert(
            1,
            typed_assignment(7, USIZE, SemanticRvalueKindV1::Length(projected_slice)),
        );
        blocks[1] = block(
            247,
            second,
            original.blocks()[1].terminator().kind().clone(),
        );
        let function = rebuild(&original, original.locals().to_vec(), blocks);

        let mut probe = scratch(&types, &function);
        let definitions = definitions(&function);
        let mut next_argument = 3;
        let mut context = Context {
            scratch: &mut probe,
            facts: &mut ComponentDynamicAssertionFactsV1,
        };
        assert_eq!(
            context
                .extent(
                    &types,
                    &function,
                    SemanticLocalIdV1::from_index(6),
                    &definitions,
                    &mut next_argument,
                )
                .unwrap(),
            Some(ProductionRankedValueV1::Argument(3)),
        );
        assert_eq!(
            context
                .extent(
                    &types,
                    &function,
                    SemanticLocalIdV1::from_index(7),
                    &definitions,
                    &mut next_argument,
                )
                .unwrap(),
            None,
        );
        assert_eq!(next_argument, 4);

        let mut scratch = scratch(&types, &function);
        let mut operations = vec![];
        let mut next_value = 0;
        let result = project_rust_bounds_checks_with_ordinary_v1(
            &types,
            &function,
            3,
            &[],
            &[],
            None,
            &mut operations,
            &mut next_value,
            Some(&mut Context {
                scratch: &mut scratch,
                facts: &mut ComponentDynamicAssertionFactsV1,
            }),
        );
        assert!(matches!(
            result,
            Err(Error::Incomplete(
                "slice extent changed its source argument relation"
            ))
        ));
        assert_eq!(&scratch.arguments[..2], &[Some(3), None]);
        assert!(matches!(
            operations.as_slice(),
            [ProductionRankedOperationV1::IndexUnknown { result }] if result.get() == 0
        ));
        assert_eq!(next_value, 1);
    }

    #[test]
    fn ordinary_bounds_reject_length_local_rebound_from_unknown_index_to_argument() {
        let types = types();
        let original = fixture(false);
        let mut blocks = original.blocks().to_vec();
        let mut statements = blocks[0].statements().to_vec();
        *statements.last_mut().unwrap() = typed_assignment(
            9,
            BOOL,
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::LessThan,
                left: typed_operand(6, USIZE),
                right: typed_operand(6, USIZE),
            },
        );
        blocks[0] = block(
            246,
            statements,
            SemanticTerminatorKindV1::Assert {
                condition: typed_operand(9, BOOL),
                expected: true,
                message: SemanticAssertMessageV1::BoundsCheck {
                    length: typed_operand(6, USIZE),
                    index: typed_operand(6, USIZE),
                },
                target: cfg_edge(SemanticEdgeRoleV1::AssertSuccess, 1),
                unwind: SemanticUnwindActionV1::Unreachable,
            },
        );
        let function = rebuild(&original, original.locals().to_vec(), blocks);
        let mut scratch = scratch(&types, &function);
        let mut operations = vec![];
        let mut next_value = 0;
        let result = project_rust_bounds_checks_with_ordinary_v1(
            &types,
            &function,
            3,
            &[],
            &[],
            None,
            &mut operations,
            &mut next_value,
            Some(&mut Context {
                scratch: &mut scratch,
                facts: &mut ComponentDynamicAssertionFactsV1,
            }),
        );
        assert!(matches!(
            result,
            Err(Error::Incomplete(
                "slice extent changed an existing ranked local identity"
            ))
        ));
        // The refusal occurs after both identities exist, before emitting a guard for n < n.
        assert!(matches!(
            operations.as_slice(),
            [ProductionRankedOperationV1::IndexUnknown { result }] if result.get() == 0
        ));
        assert_eq!(next_value, 1);
        assert_eq!(&scratch.arguments[..2], &[Some(3), None]);
    }

    #[test]
    fn context_rejects_same_type_cast_even_with_a_stable_argument_origin() {
        let types = types();
        let function = fixture(true);
        let mut scratch = scratch(&types, &function);
        assert_eq!(scratch.origins[4], Some(0));
        let mut next_argument = 3;
        let value = Context {
            scratch: &mut scratch,
            facts: &mut ComponentDynamicAssertionFactsV1,
        }
        .extent(
            &types,
            &function,
            SemanticLocalIdV1::from_index(6),
            &definitions(&function),
            &mut next_argument,
        )
        .unwrap();
        assert_eq!(value, None);
        assert_eq!(next_argument, 3);
        assert!(scratch.arguments.iter().all(Option::is_none));
    }

    #[test]
    fn context_rejects_address_escaped_receiver_and_length() {
        for (escaped, pointee) in [(4, REFERENCE), (6, USIZE)] {
            let mut types = types();
            let pointer = SemanticTypeIdV1::from_index(types.len() as u32);
            types.push(SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256(bytes(250)),
                SemanticLayoutIdentityV1::from_sha256(bytes(250)),
                SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
                SemanticTypeShapeV1::Pointer(
                    SemanticPointerTypeV1::new(
                        pointee,
                        SemanticMutabilityV1::Mutable,
                        0,
                        64,
                        SemanticPointerMetadataV1::None,
                    )
                    .unwrap(),
                ),
            ));
            let original = fixture(false);
            let mut locals = original.locals().to_vec();
            locals.push(local(251, pointer, SemanticLocalRoleV1::Temporary));
            let mut statements = original.blocks()[0].statements().to_vec();
            statements.insert(
                5,
                typed_assignment(
                    12,
                    pointer,
                    SemanticRvalueKindV1::AddressOf {
                        mutability: SemanticMutabilityV1::Mutable,
                        place: typed_place(escaped, pointee),
                    },
                ),
            );
            let mut blocks = original.blocks().to_vec();
            blocks[0] = block(
                246,
                statements,
                original.blocks()[0].terminator().kind().clone(),
            );
            let function = rebuild(&original, locals, blocks);
            let mut scratch = scratch(&types, &function);
            assert!(scratch.escaped[escaped as usize]);
            let mut next_argument = 3;
            let value = Context {
                scratch: &mut scratch,
                facts: &mut ComponentDynamicAssertionFactsV1,
            }
            .extent(
                &types,
                &function,
                SemanticLocalIdV1::from_index(6),
                &definitions(&function),
                &mut next_argument,
            )
            .unwrap();
            assert_eq!(value, None);
            assert_eq!(next_argument, 3);
            assert!(scratch.arguments.iter().all(Option::is_none));
        }
    }

    #[test]
    fn context_argument_limit_refuses_new_metadata_but_allows_cached_metadata() {
        let types = types();
        let function = fixture(false);
        let mut scratch = scratch(&types, &function);
        let mut next_argument = fe2o3_pliron::HARD_MAX_PRODUCTION_RANKED_ARGUMENTS;
        let definitions = definitions(&function);
        assert!(matches!(
            Context {
                scratch: &mut scratch,
                facts: &mut ComponentDynamicAssertionFactsV1
            }
            .extent(
                &types,
                &function,
                SemanticLocalIdV1::from_index(6),
                &definitions,
                &mut next_argument
            ),
            Err(Error::Unsupported(
                "slice metadata exceeds the ranked argument limit"
            ))
        ));
        assert!(scratch.arguments.iter().all(Option::is_none));
        scratch.arguments[0] = Some(3);
        assert_eq!(
            Context {
                scratch: &mut scratch,
                facts: &mut ComponentDynamicAssertionFactsV1
            }
            .extent(
                &types,
                &function,
                SemanticLocalIdV1::from_index(6),
                &definitions,
                &mut next_argument
            )
            .unwrap(),
            Some(ProductionRankedValueV1::Argument(3))
        );
        assert_eq!(
            next_argument,
            fe2o3_pliron::HARD_MAX_PRODUCTION_RANKED_ARGUMENTS
        );
    }

    #[test]
    fn scope_disposal_restores_floor_and_preserves_work_on_success_error_and_unwind() {
        let types = types();
        let function = fixture(false);
        for outcome in 0..3 {
            let mut work = Work::new(10_000);
            work.charge_work(7).unwrap();
            assert!(work.charge_work(10_000).is_err());
            let mut budget = Budget::new(&mut work, 1_000_000);
            budget.reserve_storage(37).unwrap();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                with_scope(function.locals().len(), &mut Facts(&mut budget), |scope| {
                    let mut scratch = scratch(&types, &function);
                    scope.retain(&scratch)?;
                    let definitions = definitions(&function);
                    let extent = Context {
                        scratch: &mut scratch,
                        facts: scope.facts(),
                    }
                    .extent(
                        &types,
                        &function,
                        SemanticLocalIdV1::from_index(6),
                        &definitions,
                        &mut 3,
                    )?;
                    assert_eq!(extent, Some(ProductionRankedValueV1::Argument(3)));
                    match outcome {
                        0 => Ok(()),
                        1 => Err(Error::Incomplete("test callback refusal")),
                        _ => panic!("test callback unwind"),
                    }
                })
            }));
            match outcome {
                0 => assert!(matches!(result, Ok(Ok(())))),
                1 => assert!(matches!(
                    result,
                    Ok(Err(Error::Incomplete("test callback refusal")))
                )),
                _ => assert!(result.is_err()),
            }
            assert_eq!(budget.storage(), 37);
            assert!(budget.peak_storage() > 37);
            assert!(budget.work() > 7);
            drop(budget);
            assert_eq!(work.failed_work(), Some(10_007));
        }
    }

    #[test]
    fn scope_storage_and_work_refusals_restore_the_original_floor() {
        let types = types();
        let function = fixture(false);
        for (work_limit, storage_limit, entered) in [(1_000, 37, false), (19, 1_000_000, true)] {
            let mut work = Work::new(work_limit);
            work.charge_work(7).unwrap();
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(37).unwrap();
            let mut called = false;
            let result = with_scope(function.locals().len(), &mut Facts(&mut budget), |scope| {
                called = true;
                let mut scratch = scratch(&types, &function);
                scope.retain(&scratch)?;
                Context {
                    scratch: &mut scratch,
                    facts: scope.facts(),
                }
                .extent(
                    &types,
                    &function,
                    SemanticLocalIdV1::from_index(6),
                    &definitions(&function),
                    &mut 3,
                )
            });
            assert!(matches!(
                result,
                Err(Error::CanonicalAssertions(
                    CanonicalAssertionErrorV1::Resource(_)
                ))
            ));
            assert_eq!(called, entered);
            assert_eq!(budget.storage(), 37);
            assert_eq!(budget.work(), if entered { 19 } else { 11 });
            drop(budget);
            assert_eq!(work.failed_work(), if entered { Some(35) } else { None });
        }
    }
}
