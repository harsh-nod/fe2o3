mod semantic_call_read_tests {
    use super::*;
    use fe2o3_pliron::{
        OperationHandleError, ProductionNumericalContractV2, ProductionPlironSessionV1,
        ProductionSemanticReadModeV2,
    };

    fn id(index: u32) -> ProductionRankedValueIdV1 {
        ProductionRankedValueIdV1::new(index)
    }
    fn value(index: u32) -> ProductionRankedValueV1 {
        ProductionRankedValueV1::Local(id(index))
    }
    fn place(index: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(index), vec![], ty).unwrap()
    }
    fn call(
        callee: u32,
        args: Vec<SemanticOperandV1>,
        result: u32,
        ty: SemanticTypeIdV1,
        target: u32,
    ) -> SemanticTerminatorKindV1 {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(callee),
                args,
                Some(SemanticCallDestinationV1::new(
                    place(result, ty),
                    cfg_edge(SemanticEdgeRoleV1::CallReturn, target),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    }
    fn write_callable(kind: SemanticWriteOnlyDisjointWriteKindV1) -> SemanticCallableDeclV1 {
        compiler_intrinsic_callable(
            SemanticCompilerIntrinsicOperationV1::WriteOnlyDisjointSliceWrite {
                disjoint_slice: SCALAR_TYPE,
                witness: SCALAR_TYPE,
                element: F32_TYPE,
                raw_index: SCALAR_TYPE,
                index_space: SemanticDisjointIndexSpaceV1::Index1d,
                kind,
            },
        )
    }

    // A private adapter fixture, not semantic admission: callable ABI validation,
    // receiver provenance and capability checks run earlier in the production path.
    struct Fixture {
        types: Vec<SemanticTypeDeclV1>,
        function: SemanticFunctionDeclV1,
        callables: Vec<SemanticCallableDeclV1>,
        blocks: Vec<ProductionRankedBlockV1>,
        sources: Vec<ProjectedAccessSourceV1>,
    }
    impl Fixture {
        fn new() -> Self {
            let function = projection_function_with_locals(
                vec![
                    block(
                        130,
                        vec![],
                        SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 3)),
                    ),
                    block(
                        131,
                        vec![typed_assignment(
                            6,
                            F32_TYPE,
                            SemanticRvalueKindV1::Binary {
                                operation: SemanticBinaryOpV1::Add,
                                left: typed_operand(4, F32_TYPE),
                                right: typed_operand(5, F32_TYPE),
                            },
                        )],
                        call(
                            1,
                            vec![
                                typed_operand(3, SCALAR_TYPE),
                                typed_operand(0, SCALAR_TYPE),
                                typed_operand(6, F32_TYPE),
                            ],
                            7,
                            BOOL_TYPE,
                            4,
                        ),
                    ),
                    block(
                        132,
                        vec![],
                        call(
                            0,
                            vec![typed_operand(2, SCALAR_TYPE), typed_operand(0, SCALAR_TYPE)],
                            5,
                            F32_TYPE,
                            1,
                        ),
                    ),
                    block(
                        133,
                        vec![],
                        call(
                            0,
                            vec![typed_operand(1, SCALAR_TYPE), typed_operand(0, SCALAR_TYPE)],
                            4,
                            F32_TYPE,
                            2,
                        ),
                    ),
                    block(134, vec![], SemanticTerminatorKindV1::Return),
                ],
                vec![
                    local(135, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                    local(136, SCALAR_TYPE, SemanticLocalRoleV1::Argument(0)),
                    local(137, SCALAR_TYPE, SemanticLocalRoleV1::Argument(1)),
                    local(138, SCALAR_TYPE, SemanticLocalRoleV1::Argument(2)),
                    local(139, F32_TYPE, SemanticLocalRoleV1::Temporary),
                    local(140, F32_TYPE, SemanticLocalRoleV1::Temporary),
                    local(141, F32_TYPE, SemanticLocalRoleV1::Temporary),
                    local(142, BOOL_TYPE, SemanticLocalRoleV1::Temporary),
                ],
            );
            let mut entry = (0..3)
                .map(|index| ProductionRankedOperationV1::View {
                    result: id(index),
                    element_width: 32,
                    writable: index == 2,
                    shape: vec![64],
                    dynamic_extents: vec![],
                    allocation_origin: u64::from(index + 1),
                    noalias_class: u64::from(index + 1),
                })
                .collect::<Vec<_>>();
            entry.push(ProductionRankedOperationV1::IndexConstant {
                result: id(3),
                value: 0,
            });
            let read = |view, target| {
                ProductionRankedBlockV1::new(
                    vec![ProductionRankedOperationV1::Access {
                        kind: AccessKindAttr::Read,
                        view: value(view),
                        indices: vec![value(3)],
                    }],
                    ProductionRankedTerminatorV1::Branch { target },
                )
            };
            let blocks = vec![
                ProductionRankedBlockV1::new(
                    entry,
                    ProductionRankedTerminatorV1::Branch { target: 3 },
                ),
                ProductionRankedBlockV1::new(
                    vec![ProductionRankedOperationV1::Access {
                        kind: AccessKindAttr::Write,
                        view: value(2),
                        indices: vec![value(3)],
                    }],
                    ProductionRankedTerminatorV1::Branch { target: 4 },
                ),
                read(1, 1),
                read(0, 2),
                ProductionRankedBlockV1::new(vec![], ProductionRankedTerminatorV1::Return),
            ];
            let sources = [
                (3, AccessKindAttr::Read),
                (2, AccessKindAttr::Read),
                (1, AccessKindAttr::Write),
            ]
            .into_iter()
            .map(|(block, access)| ProjectedAccessSourceV1 {
                block,
                operation: 0,
                access,
                memory_space: MemorySpaceAttr::Global,
                source: function.blocks()[block].terminator().source(),
                semantic_site: Some(ProjectedSemanticAccessSiteV1 {
                    block,
                    statement: None,
                }),
            })
            .collect();
            Self {
                types: optional_selector_types(),
                function,
                blocks,
                sources,
                callables: vec![
                    compiler_intrinsic_callable(
                        SemanticCompilerIntrinsicOperationV1::MemoryVolatileLoad {
                            element: F32_TYPE,
                        },
                    ),
                    write_callable(SemanticWriteOnlyDisjointWriteKindV1::Thread { disjoint: true }),
                ],
            }
        }
        fn resolver(
            &self,
        ) -> Result<GpuSemanticExpressionResolverV2<'_>, ProductionRankedProjectionErrorV1>
        {
            GpuSemanticExpressionResolverV2::with_ranked_reads(
                &self.types,
                &self.callables,
                &self.function,
                &self.blocks,
                &self.sources,
            )
        }
        fn replace_block(
            &mut self,
            index: usize,
            statements: Vec<SemanticStatementV1>,
            term: SemanticTerminatorKindV1,
        ) {
            let mut blocks = self.function.blocks().to_vec();
            blocks[index] = block(150 + index as u8, statements, term);
            self.function =
                projection_function_with_locals(blocks, self.function.locals().to_vec());
        }
        fn expression(&self) -> Result<ProductionSemanticExpressionV2, &'static str> {
            self.resolver()
                .unwrap()
                .resolve_source_write_v2(ProjectedSemanticAccessSiteV1 {
                    block: 1,
                    statement: None,
                })
        }
    }

    #[test]
    fn volatile_call_results_retain_original_events_through_typed_write_and_owner() {
        let fixture = Fixture::new();
        let writes = projected_reference_gpu_writes_v2(
            &fixture.types,
            &fixture.callables,
            &fixture.function,
            &fixture.blocks,
            &fixture.sources,
        )
        .unwrap();
        assert_eq!(writes.len(), 1);
        assert_eq!(
            (
                writes[0].block,
                writes[0].operation,
                writes[0].allocation_origin
            ),
            (1, 0, 3)
        );
        let expression = writes[0].value.clone().unwrap();
        let ProductionSemanticExpressionV2::Binary {
            operation,
            scalar,
            lhs,
            rhs,
            ..
        } = &expression
        else {
            panic!("typed write must retain the actual addition");
        };
        assert_eq!(*operation, ProductionSemanticBinaryOpV2::Add);
        assert_eq!(*scalar, ProductionSemanticScalarTypeV2::Float { bits: 32 });
        for (index, leaf) in [lhs, rhs].into_iter().enumerate() {
            assert_eq!(
                leaf.as_ref(),
                &ProductionSemanticExpressionV2::Load(ProductionSemanticLoadV2 {
                    block: 3 - index as u32,
                    operation: 0,
                    scalar: *scalar,
                    read_mode: ProductionSemanticReadModeV2::UnorderedVolatile,
                    allocation_origin: index as u64 + 1,
                    view: value(index as u32),
                    indices: vec![value(3)].into_boxed_slice(),
                })
            );
        }
        for bypass in [false, true] {
            let mut blocks = fixture.blocks.clone();
            blocks[1] = ProductionRankedBlockV1::new(
                vec![
                    ProductionRankedOperationV1::SemanticExpression {
                        result: id(4),
                        numerical_contract: ProductionNumericalContractV2::exact_for_expression(
                            &expression,
                        ),
                        expression: expression.clone(),
                    },
                    ProductionRankedOperationV1::ValueAccess {
                        kind: AccessKindAttr::Write,
                        view: value(2),
                        indices: vec![value(3)],
                        value: value(4),
                    },
                ],
                ProductionRankedTerminatorV1::Branch { target: 4 },
            );
            if bypass {
                blocks[0] = ProductionRankedBlockV1::new(
                    blocks[0].operations().to_vec(),
                    ProductionRankedTerminatorV1::IndexLessThan {
                        lhs: value(3),
                        rhs: ProductionRankedValueV1::Argument(0),
                        true_block: 3,
                        false_block: 1,
                    },
                );
            }
            let recipe =
                ProductionRankedKernelV1::new("volatile_call_result", usize::from(bypass), blocks)
                    .unwrap();
            let construction =
                ProductionConstructionV1::ranked_kernel("volatile_call_module", recipe).unwrap();
            let mut session = ProductionPlironSessionV1::new(
                ProductionSessionLimitsV1::default(),
                [
                    dialect_kernel::dialect_registration().unwrap(),
                    dialect_gpu::dialect_registration().unwrap(),
                ],
            )
            .unwrap();
            let registered = session.register_construction(construction).unwrap();
            let result = session.construct_registered(registered);
            if bypass {
                assert!(matches!(
                    result,
                    Err(ProductionSessionErrorV1::Operation(
                        OperationHandleError::OperationVerificationRejected
                    ))
                ));
            } else {
                assert!(result.is_ok(), "{result:?}");
            }
        }
    }

    #[test]
    fn volatile_call_results_require_unique_unescaped_definitions() {
        for kind in [
            SemanticRvalueKindV1::Use(typed_operand(5, F32_TYPE)),
            SemanticRvalueKindV1::AddressOf {
                mutability: SemanticMutabilityV1::Mutable,
                place: place(4, F32_TYPE),
            },
        ] {
            let mut fixture = Fixture::new();
            let destination = if matches!(kind, SemanticRvalueKindV1::Use(_)) {
                4
            } else {
                6
            };
            let term = fixture.function.blocks()[1].terminator().kind().clone();
            fixture.replace_block(1, vec![typed_assignment(destination, F32_TYPE, kind)], term);
            assert!(matches!(
                fixture.resolver(),
                Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "volatile call result is not uniquely defined and unescaped"
                ))
            ));
        }
        let mut fixture = Fixture::new();
        let mut locals = fixture.function.locals().to_vec();
        locals[4] = local(139, F32_TYPE, SemanticLocalRoleV1::Argument(3));
        fixture.function =
            projection_function_with_locals(fixture.function.blocks().to_vec(), locals);
        assert!(fixture.resolver().is_err());
    }

    #[test]
    fn volatile_call_results_require_exact_read_correspondence() {
        let mut fixture = Fixture::new();
        fixture.sources.remove(0);
        assert_eq!(
            fixture.expression(),
            Err("GPU semantic scalar local has no unique definition")
        );
        let mut fixture = Fixture::new();
        fixture.sources.push(fixture.sources[0].clone());
        assert!(fixture.resolver().is_err());
        let mut fixture = Fixture::new();
        fixture.callables[0] =
            compiler_intrinsic_callable(SemanticCompilerIntrinsicOperationV1::FabsF32);
        assert_eq!(
            fixture.expression(),
            Err("GPU semantic scalar local has no unique definition")
        );
        let mut fixture = Fixture::new();
        fixture.sources[0].operation = 100;
        assert!(fixture.resolver().is_err());
        let mut fixture = Fixture::new();
        fixture.blocks[3] = ProductionRankedBlockV1::new(
            vec![ProductionRankedOperationV1::Access {
                kind: AccessKindAttr::Write,
                view: value(0),
                indices: vec![value(3)],
            }],
            ProductionRankedTerminatorV1::Branch { target: 2 },
        );
        assert!(fixture.resolver().is_err());
    }

    #[test]
    fn source_alias_mutations_cannot_be_erased_by_substitution() {
        for mutation in [
            SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                place(6, F32_TYPE),
                typed_operand(4, F32_TYPE),
                SemanticVolatilityV1::NonVolatile,
                None,
            )),
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(0, SCALAR_TYPE),
                SemanticRvalueV1::new(
                    SCALAR_TYPE,
                    SemanticRvalueKindV1::AddressOf {
                        mutability: SemanticMutabilityV1::Mutable,
                        place: place(6, F32_TYPE),
                    },
                ),
            )),
        ] {
            let mut fixture = Fixture::new();
            let mut statements = fixture.function.blocks()[1].statements().to_vec();
            statements.push(statement(mutation));
            let term = fixture.function.blocks()[1].terminator().kind().clone();
            fixture.replace_block(1, statements, term);
            assert!(matches!(
                fixture.expression(),
                Err("GPU semantic scalar local has no unique definition"
                    | "GPU semantic scalar local address escaped")
            ));
        }
        let mut fixture = Fixture::new();
        let mut blocks = fixture.function.blocks().to_vec();
        blocks[4] = block(
            160,
            vec![],
            call(2, vec![typed_operand(6, F32_TYPE)], 6, F32_TYPE, 5),
        );
        blocks.push(block(161, vec![], SemanticTerminatorKindV1::Return));
        fixture.function =
            projection_function_with_locals(blocks, fixture.function.locals().to_vec());
        fixture.callables.push(compiler_intrinsic_callable(
            SemanticCompilerIntrinsicOperationV1::FabsF32,
        ));
        assert_eq!(
            fixture.expression(),
            Err("GPU semantic scalar local has no unique definition")
        );
    }

    #[test]
    fn source_alias_uses_require_their_own_definition_and_return_order() {
        let mut fixture = Fixture::new();
        let term = fixture.function.blocks()[1].terminator().kind().clone();
        fixture.replace_block(1, vec![], term);
        fixture.replace_block(
            0,
            vec![typed_assignment(
                6,
                F32_TYPE,
                SemanticRvalueKindV1::Use(typed_operand(4, F32_TYPE)),
            )],
            SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 3)),
        );
        assert_eq!(
            fixture.expression(),
            Err("GPU volatile call return does not dominate its source use")
        );

        let mut fixture = Fixture::new();
        let mut locals = fixture.function.locals().to_vec();
        locals.push(local(162, F32_TYPE, SemanticLocalRoleV1::Temporary));
        fixture.function =
            projection_function_with_locals(fixture.function.blocks().to_vec(), locals);
        let term = fixture.function.blocks()[1].terminator().kind().clone();
        fixture.replace_block(
            1,
            vec![
                typed_assignment(
                    6,
                    F32_TYPE,
                    SemanticRvalueKindV1::Use(typed_operand(8, F32_TYPE)),
                ),
                typed_assignment(
                    8,
                    F32_TYPE,
                    SemanticRvalueKindV1::Use(typed_operand(4, F32_TYPE)),
                ),
            ],
            term,
        );
        assert_eq!(
            fixture.expression(),
            Err("GPU scalar assignment does not dominate its source use")
        );

        let mut fixture = Fixture::new();
        fixture.replace_block(
            0,
            vec![],
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: typed_operand(7, BOOL_TYPE),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        0,
                        cfg_edge(SemanticEdgeRoleV1::SwitchValue, 3),
                    )],
                    cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 1),
                )
                .unwrap(),
            },
        );
        assert_eq!(
            fixture.expression(),
            Err("GPU volatile call return does not dominate its source use")
        );
    }

    #[test]
    fn source_value_substitution_rejects_cleanup_and_loop_instances() {
        let mut fixture = Fixture::new();
        let SemanticTerminatorKindV1::Call(original) =
            fixture.function.blocks()[3].terminator().kind()
        else {
            unreachable!()
        };
        let term = SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                original.callee(),
                original.arguments().to_vec(),
                original.destination().cloned(),
                SemanticUnwindActionV1::Cleanup(cfg_edge(SemanticEdgeRoleV1::CallUnwind, 1)),
            )
            .unwrap(),
        );
        fixture.replace_block(3, vec![], term);
        assert!(matches!(
            fixture.resolver(),
            Err(ProductionRankedProjectionErrorV1::Incomplete(
                "source value availability does not support cleanup edges"
            ))
        ));
        let mut fixture = Fixture::new();
        fixture.replace_block(
            4,
            vec![],
            SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 3)),
        );
        assert_eq!(
            fixture.expression(),
            Err("GPU scalar substitution requires acyclic source definitions")
        );
    }

    #[test]
    fn scalar_arguments_must_be_unmodified_and_type_exact() {
        for mutate in [false, true] {
            let types = optional_selector_types();
            let function = projection_function_with_locals(
                vec![block(
                    163,
                    if mutate {
                        vec![typed_assignment(
                            1,
                            F32_TYPE,
                            SemanticRvalueKindV1::Use(typed_operand(1, F32_TYPE)),
                        )]
                    } else {
                        vec![]
                    },
                    SemanticTerminatorKindV1::Return,
                )],
                vec![
                    local(164, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                    local(165, F32_TYPE, SemanticLocalRoleV1::Argument(0)),
                ],
            );
            let site = ScalarAssignmentSiteV1 {
                block: 0,
                statement: function.blocks()[0].statements().len(),
            };
            let operand = typed_operand(1, F32_TYPE);
            let mut resolver = GpuSemanticExpressionResolverV2::new(&types, &function).unwrap();
            let result = resolver.resolve_operand_v2(&operand, 0, site);
            if mutate {
                assert_eq!(result, Err("GPU semantic scalar argument is overwritten"));
            } else {
                assert!(matches!(
                    result,
                    Ok(ProductionSemanticExpressionV2::Symbol { .. })
                ));
            }
            let mistyped = typed_operand(1, SCALAR_TYPE);
            assert_eq!(
                resolver.resolve_operand_v2(&mistyped, 0, site),
                Err("GPU semantic scalar local type changed")
            );
        }
    }

    #[test]
    fn source_value_queries_share_a_fail_closed_work_budget() {
        let fixture = Fixture::new();
        let mut resolver = fixture.resolver().unwrap();
        assert_eq!(
            resolver.source_proof.work,
            check_source_value_analysis_size_v2(&fixture.function).unwrap()
        );
        assert!(resolver.source_proof.work > 0);
        let site = ProjectedSemanticAccessSiteV1 {
            block: 1,
            statement: None,
        };
        assert!(resolver.resolve_source_write_v2(site).is_ok());
        let work = resolver.source_proof.work;
        assert!(work > 0);
        assert!(resolver.resolve_source_write_v2(site).is_ok());
        assert!(resolver.source_proof.work > work);
        resolver.source_proof.work = MAX_PROJECTED_LOOP_GRAPH_WORK_V1;
        assert_eq!(
            resolver.resolve_source_write_v2(site),
            Err("GPU semantic source dependence exceeds its analysis budget")
        );
        assert_eq!(
            resolver.resolve_source_write_v2(site),
            Err("GPU semantic source dependence exceeds its analysis budget")
        );
    }

    #[test]
    fn source_value_preflight_bounds_raw_edges_and_construction() {
        for edges in [MAX_RANKED_BOUNDS_EDGES, MAX_RANKED_BOUNDS_EDGES + 1] {
            let function = projection_function(vec![
                block(
                    166,
                    vec![],
                    SemanticTerminatorKindV1::SwitchInt {
                        discriminant: typed_operand(0, SCALAR_TYPE),
                        targets: SemanticSwitchTargetsV1::new(
                            (0..edges - 1)
                                .map(|index| {
                                    SemanticSwitchTargetV1::new(
                                        index as u128,
                                        cfg_edge(SemanticEdgeRoleV1::SwitchValue, 1),
                                    )
                                })
                                .collect(),
                            cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 1),
                        )
                        .unwrap(),
                    },
                ),
                block(167, vec![], SemanticTerminatorKindV1::Return),
            ]);
            let types = projection_types();
            let result = GpuSemanticExpressionResolverV2::new(&types, &function);
            if edges == MAX_RANKED_BOUNDS_EDGES {
                assert!(result.is_ok());
            } else {
                assert!(matches!(
                    result,
                    Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "source value analysis exceeds its raw edge limit"
                    ))
                ));
            }
        }
        let function = projection_function(vec![block(
            168,
            vec![statement(SemanticStatementKindV1::Nop); MAX_RANKED_BOUNDS_OPERATIONS - 1],
            SemanticTerminatorKindV1::Return,
        )]);
        assert!(matches!(
            check_source_value_analysis_size_v2(&function),
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "uniform induction CFG analysis exceeds its work limit"
            ))
        ));
    }

    #[test]
    fn disconnected_source_cycles_do_not_change_reachable_scalar_values() {
        let mut fixture = Fixture::new();
        let expected = fixture.expression();
        let mut blocks = fixture.function.blocks().to_vec();
        blocks.push(block(
            169,
            vec![],
            SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 5)),
        ));
        fixture.function =
            projection_function_with_locals(blocks, fixture.function.locals().to_vec());
        assert_eq!(fixture.expression(), expected);
    }

    #[test]
    fn typed_write_payloads_use_closed_kind_specific_arity() {
        use SemanticWriteOnlyDisjointWriteKindV1 as Kind;
        for (kind, arity) in [
            (Kind::Thread { disjoint: false }, 3),
            (Kind::Thread { disjoint: true }, 3),
            (Kind::GridExclusive, 4),
            (
                Kind::Block {
                    lanes_per_block: 64,
                    elements_per_lane: 1,
                },
                4,
            ),
            (
                Kind::Tiled2d {
                    lanes_per_tile: 64,
                    tile_rows: 8,
                    tile_columns: 8,
                    elements_per_lane: 1,
                },
                7,
            ),
            (
                Kind::RowStriped2d {
                    lanes_per_row: 64,
                    elements_per_lane: 1,
                },
                7,
            ),
        ] {
            for length in [arity - 1, arity, arity + 1] {
                let mut fixture = Fixture::new();
                let expected = fixture.expression().unwrap();
                fixture.callables[1] = write_callable(kind);
                let mut args = vec![typed_operand(3, SCALAR_TYPE); length - 1];
                args.push(typed_operand(6, F32_TYPE));
                let statements = fixture.function.blocks()[1].statements().to_vec();
                fixture.replace_block(1, statements, call(1, args, 7, BOOL_TYPE, 4));
                if length == arity {
                    assert_eq!(fixture.expression(), Ok(expected));
                } else {
                    assert_eq!(
                        fixture.expression(),
                        Err("GPU typed write call payload arity changed")
                    );
                }
            }
        }
        let mut fixture = Fixture::new();
        fixture.replace_block(
            1,
            vec![],
            call(1, vec![typed_operand(3, SCALAR_TYPE); 3], 7, BOOL_TYPE, 4),
        );
        assert_eq!(
            fixture.expression(),
            Err("GPU typed write call payload type changed")
        );
        let mut fixture = Fixture::new();
        fixture.callables[1] =
            compiler_intrinsic_callable(SemanticCompilerIntrinsicOperationV1::FabsF32);
        assert_eq!(
            fixture.expression(),
            Err("GPU write source call has no authenticated scalar payload contract")
        );
    }
}
