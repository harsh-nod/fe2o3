mod semantic_read_scheduling_tests {
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

    fn dereference(local: u32) -> SemanticPlaceV1 {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(local),
            vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, F32_TYPE).unwrap(),
            ],
            F32_TYPE,
        )
        .unwrap()
    }

    #[test]
    fn source_rhs_from_later_listed_reads_constructs_through_owner() {
        let mut types = optional_selector_types();
        let pointer = SemanticTypeIdV1::from_index(types.len() as u32);
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(120)),
            SemanticLayoutIdentityV1::from_sha256(bytes(120)),
            SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new(
                    F32_TYPE,
                    SemanticMutabilityV1::Mutable,
                    1,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        ));
        // Source block order is [entry, consumer, producer], but execution is 0 -> 2 -> 1.
        // Temporaries retain their load definitions instead of becoming argument symbols.
        let function = projection_function_with_locals(
            vec![
                block(
                    121,
                    vec![],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 2)),
                ),
                block(
                    122,
                    vec![statement(SemanticStatementKindV1::Assign(
                        SemanticAssignmentV1::new(
                            dereference(3),
                            SemanticRvalueV1::new(
                                F32_TYPE,
                                SemanticRvalueKindV1::Binary {
                                    operation: SemanticBinaryOpV1::Add,
                                    left: typed_operand(4, F32_TYPE),
                                    right: typed_operand(5, F32_TYPE),
                                },
                            ),
                        ),
                    ))],
                    SemanticTerminatorKindV1::Return,
                ),
                block(
                    123,
                    (1..=2)
                        .map(|pointer| {
                            typed_assignment(
                                pointer + 3,
                                F32_TYPE,
                                SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                                    dereference(pointer),
                                    SemanticVolatilityV1::Volatile,
                                    None,
                                )),
                            )
                        })
                        .collect(),
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
                ),
            ],
            vec![
                local(124, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(125, pointer, SemanticLocalRoleV1::Argument(0)),
                local(126, pointer, SemanticLocalRoleV1::Argument(1)),
                local(127, pointer, SemanticLocalRoleV1::Argument(2)),
                local(128, F32_TYPE, SemanticLocalRoleV1::Temporary),
                local(129, F32_TYPE, SemanticLocalRoleV1::Temporary),
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
        let mut blocks = vec![
            ProductionRankedBlockV1::new(
                entry.clone(),
                ProductionRankedTerminatorV1::Branch { target: 2 },
            ),
            ProductionRankedBlockV1::new(vec![], ProductionRankedTerminatorV1::Return),
            ProductionRankedBlockV1::new(
                vec![
                    ProductionRankedOperationV1::IndexUnsignedCast {
                        result: id(5),
                        source: value(3),
                        bit_width: 32,
                    },
                    ProductionRankedOperationV1::Access {
                        kind: AccessKindAttr::Read,
                        view: value(0),
                        indices: vec![value(5)],
                    },
                    ProductionRankedOperationV1::Access {
                        kind: AccessKindAttr::Read,
                        view: value(1),
                        indices: vec![value(5)],
                    },
                ],
                ProductionRankedTerminatorV1::Branch { target: 1 },
            ),
        ];
        let sources = (0..2)
            .map(|statement| ProjectedAccessSourceV1 {
                block: 2,
                operation: statement + 1,
                access: AccessKindAttr::Read,
                memory_space: MemorySpaceAttr::Global,
                source: function.blocks()[2].statements()[statement].source(),
                semantic_site: Some(ProjectedSemanticAccessSiteV1 {
                    block: 2,
                    statement: Some(statement),
                }),
            })
            .collect::<Vec<_>>();
        let resolve = |sources: &[ProjectedAccessSourceV1]| {
            GpuSemanticExpressionResolverV2::with_ranked_reads(
                &types,
                &[],
                &function,
                &blocks,
                sources,
            )
            .unwrap()
            .resolve_source_write_v2(ProjectedSemanticAccessSiteV1 {
                block: 1,
                statement: Some(0),
            })
        };
        assert_eq!(
            resolve(&sources[..1]),
            Err("GPU semantic RHS load has no exact ranked read correspondence")
        );
        let expression = resolve(&sources).unwrap();
        let ProductionSemanticExpressionV2::Binary {
            operation,
            scalar,
            lhs,
            rhs,
            ..
        } = &expression
        else {
            panic!("source addition must retain both loads")
        };
        assert_eq!(*operation, ProductionSemanticBinaryOpV2::Add);
        assert_eq!(*scalar, ProductionSemanticScalarTypeV2::Float { bits: 32 });
        for (index, expression) in [lhs, rhs].into_iter().enumerate() {
            assert_eq!(
                expression.as_ref(),
                &ProductionSemanticExpressionV2::Load(ProductionSemanticLoadV2 {
                    block: 2,
                    operation: (index + 1) as u32,
                    scalar: *scalar,
                    read_mode: ProductionSemanticReadModeV2::UnorderedVolatile,
                    allocation_origin: (index + 1) as u64,
                    view: value(index as u32),
                    indices: vec![value(5)].into_boxed_slice(),
                })
            );
        }
        let numerical_contract = ProductionNumericalContractV2::exact_for_expression(&expression);
        blocks[1] = ProductionRankedBlockV1::new(
            vec![
                ProductionRankedOperationV1::SemanticExpression {
                    result: id(4),
                    expression,
                    numerical_contract,
                },
                ProductionRankedOperationV1::ValueAccess {
                    kind: AccessKindAttr::Write,
                    view: value(2),
                    indices: vec![value(3)],
                    value: value(4),
                },
            ],
            ProductionRankedTerminatorV1::Return,
        );
        for bypass in [false, true] {
            let mut blocks = blocks.clone();
            if bypass {
                blocks[0] = ProductionRankedBlockV1::new(
                    entry.clone(),
                    ProductionRankedTerminatorV1::IndexLessThan {
                        lhs: value(3),
                        rhs: ProductionRankedValueV1::Argument(0),
                        true_block: 2,
                        false_block: 1,
                    },
                );
            }
            let recipe =
                ProductionRankedKernelV1::new("source_read_schedule", usize::from(bypass), blocks)
                    .unwrap();
            let mut session = ProductionPlironSessionV1::new(
                ProductionSessionLimitsV1::default(),
                [
                    dialect_kernel::dialect_registration().unwrap(),
                    dialect_gpu::dialect_registration().unwrap(),
                ],
            )
            .unwrap();
            let construction =
                ProductionConstructionV1::ranked_kernel("source_read_module", recipe).unwrap();
            let registered = session.register_construction(construction).unwrap();
            let constructed = session.construct_registered(registered);
            if bypass {
                assert!(matches!(
                    constructed,
                    Err(ProductionSessionErrorV1::Operation(
                        OperationHandleError::OperationVerificationRejected
                    ))
                ));
            } else {
                assert!(constructed.is_ok(), "{constructed:?}");
            }
        }
    }
}
