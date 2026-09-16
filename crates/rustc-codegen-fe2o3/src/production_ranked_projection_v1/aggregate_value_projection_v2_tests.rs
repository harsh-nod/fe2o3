#[test]
fn gpu_aggregate_index_range_budget_is_shared_and_fails_closed_v2() {
    let types = aggregate_types_v2();
    let function = aggregate_function_v2(vec![
        aggregate_assignment_v2(1, ARRAY_TYPE, aggregate_array_v2([11, 17, 23, 29])),
        aggregate_assignment_v2(2, SCALAR_TYPE, SemanticRvalueKindV1::Use(constant(1))),
        aggregate_output_v2(aggregate_copy_v2(
            1,
            SCALAR_TYPE,
            vec![
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(2)),
                    SCALAR_TYPE,
                )
                .unwrap(),
            ],
        )),
    ]);
    let mut resolver = GpuSemanticExpressionResolverV2::new(&types, &function).unwrap();
    resolver.definitions.work = MAX_PROJECTED_LOOP_GRAPH_WORK_V1;
    assert_eq!(
        resolver
            .resolve_store_v2(
                function.blocks()[0].statements()[2].kind(),
                ScalarAssignmentSiteV1 {
                    block: 0,
                    statement: 2
                },
            )
            .unwrap_err(),
        "GPU semantic index range analysis is incomplete",
    );
    assert!(resolver.use_site.is_none());
    assert!(resolver.visiting.is_empty());
}

#[test]
fn gpu_aggregate_index_local_is_frozen_at_access_not_construction_v2() {
    let types = aggregate_types_v2();
    let index = SemanticProjectionV1::new(
        SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(2)),
        SCALAR_TYPE,
    )
    .unwrap();
    let function = aggregate_function_v2(vec![
        aggregate_assignment_v2(1, ARRAY_TYPE, aggregate_array_v2([11, 17, 23, 29])),
        aggregate_assignment_v2(
            5,
            ARRAY_TYPE,
            SemanticRvalueKindV1::Use(aggregate_copy_v2(1, ARRAY_TYPE, vec![])),
        ),
        aggregate_assignment_v2(2, SCALAR_TYPE, SemanticRvalueKindV1::Use(constant(1))),
        aggregate_output_v2(aggregate_copy_v2(5, SCALAR_TYPE, vec![index])),
    ]);
    assert_eq!(
        aggregate_last_value_v2(&types, &function),
        Ok(aggregate_expected_v2(17))
    );
}

#[test]
fn gpu_aggregate_index_requires_one_in_bounds_unsigned_value_v2() {
    let types = aggregate_types_v2();
    let index = SemanticProjectionV1::new(
        SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(2)),
        SCALAR_TYPE,
    )
    .unwrap();
    for value in [4, u128::from(u32::MAX)] {
        let function = aggregate_function_v2(vec![
            aggregate_assignment_v2(1, ARRAY_TYPE, aggregate_array_v2([11, 17, 23, 29])),
            aggregate_assignment_v2(2, SCALAR_TYPE, SemanticRvalueKindV1::Use(constant(value))),
            aggregate_output_v2(aggregate_copy_v2(1, SCALAR_TYPE, vec![index])),
        ]);
        assert_eq!(
            aggregate_last_value_v2(&types, &function).unwrap_err(),
            "GPU semantic index has no exact in-bounds value"
        );
    }
    let mut locals = aggregate_function_v2(vec![]).locals().to_vec();
    locals[2] = local(237, SCALAR_TYPE, SemanticLocalRoleV1::Argument(1));
    let function = projection_function_with_locals(
        vec![block(
            240,
            vec![
                aggregate_assignment_v2(1, ARRAY_TYPE, aggregate_array_v2([11, 17, 23, 29])),
                aggregate_output_v2(aggregate_copy_v2(1, SCALAR_TYPE, vec![index])),
            ],
            SemanticTerminatorKindV1::Return,
        )],
        locals,
    );
    assert_eq!(
        aggregate_last_value_v2(&types, &function).unwrap_err(),
        "GPU semantic index has no exact in-bounds value"
    );
}

#[test]
fn gpu_aggregate_index_uses_latest_literal_but_rejects_escape_v2() {
    let types = aggregate_types_v2();
    let index = SemanticProjectionV1::new(
        SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(2)),
        SCALAR_TYPE,
    )
    .unwrap();
    let overwritten = aggregate_function_v2(vec![
        aggregate_assignment_v2(1, ARRAY_TYPE, aggregate_array_v2([11, 17, 23, 29])),
        aggregate_assignment_v2(2, SCALAR_TYPE, SemanticRvalueKindV1::Use(constant(1))),
        aggregate_assignment_v2(2, SCALAR_TYPE, SemanticRvalueKindV1::Use(constant(2))),
        aggregate_output_v2(aggregate_copy_v2(1, SCALAR_TYPE, vec![index])),
    ]);
    assert_eq!(
        aggregate_last_value_v2(&types, &overwritten),
        Ok(aggregate_expected_v2(23))
    );
    // Unknown writes and address exposure still cannot freeze an index value.
    for invalidation in [
        aggregate_assignment_v2(
            2,
            SCALAR_TYPE,
            SemanticRvalueKindV1::Use(aggregate_copy_v2(0, SCALAR_TYPE, vec![])),
        ),
        aggregate_assignment_v2(
            3,
            POINTER_TYPE,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: SemanticPlaceV1::new(SemanticLocalIdV1::from_index(2), vec![], SCALAR_TYPE)
                    .unwrap(),
            },
        ),
    ] {
        let function = aggregate_function_v2(vec![
            aggregate_assignment_v2(1, ARRAY_TYPE, aggregate_array_v2([11, 17, 23, 29])),
            aggregate_assignment_v2(2, SCALAR_TYPE, SemanticRvalueKindV1::Use(constant(1))),
            invalidation,
            aggregate_output_v2(aggregate_copy_v2(1, SCALAR_TYPE, vec![index])),
        ]);
        assert!(aggregate_last_value_v2(&types, &function).is_err());
    }
}
fn aggregate_assignment_v2(
    local: u32,
    ty: SemanticTypeIdV1,
    kind: SemanticRvalueKindV1,
) -> SemanticStatementV1 {
    statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap(),
        SemanticRvalueV1::new(ty, kind),
    )))
}

fn aggregate_copy_v2(
    local: u32,
    ty: SemanticTypeIdV1,
    projections: Vec<SemanticProjectionV1>,
) -> SemanticOperandV1 {
    SemanticOperandV1::Copy(
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), projections, ty).unwrap(),
    )
}

fn aggregate_array_v2(values: [u128; 4]) -> SemanticRvalueKindV1 {
    SemanticRvalueKindV1::aggregate(
        SemanticAggregateKindV1::Array,
        values.into_iter().map(constant).collect(),
    )
    .unwrap()
}

fn aggregate_index_v2(offset: u64) -> SemanticProjectionV1 {
    SemanticProjectionV1::new(
        SemanticProjectionKindV1::ConstantIndex {
            offset,
            minimum_length: 4,
            from_end: false,
        },
        SCALAR_TYPE,
    )
    .unwrap()
}

fn aggregate_output_v2(value: SemanticOperandV1) -> SemanticStatementV1 {
    statement(SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
        dereferenced_place(),
        value,
        SemanticVolatilityV1::NonVolatile,
        None,
    )))
}

fn aggregate_function_v2(statements: Vec<SemanticStatementV1>) -> SemanticFunctionDeclV1 {
    projection_function_with_locals(
        vec![block(235, statements, SemanticTerminatorKindV1::Return)],
        vec![
            local(230, SCALAR_TYPE, SemanticLocalRoleV1::Return),
            local(231, ARRAY_TYPE, SemanticLocalRoleV1::Temporary),
            local(232, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
            local(233, POINTER_TYPE, SemanticLocalRoleV1::Argument(0)),
            local(
                234,
                SemanticTypeIdV1::from_index(3),
                SemanticLocalRoleV1::Temporary,
            ),
            local(235, ARRAY_TYPE, SemanticLocalRoleV1::Temporary),
            local(
                236,
                SemanticTypeIdV1::from_index(4),
                SemanticLocalRoleV1::Temporary,
            ),
        ],
    )
}

fn aggregate_types_v2() -> Vec<SemanticTypeDeclV1> {
    let mut types = projection_types();
    let tuple = SemanticTypeIdV1::from_index(3);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(230)),
        SemanticLayoutIdentityV1::from_sha256(bytes(231)),
        SemanticTypeLayoutV1::aggregate(
            Some(20),
            4,
            SemanticAggregateLayoutV1::new(vec![0, 16], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(
            SemanticAggregateTypeV1::new(vec![ARRAY_TYPE, SCALAR_TYPE]).unwrap(),
        ),
    ));
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(232)),
        SemanticLayoutIdentityV1::from_sha256(bytes(233)),
        SemanticTypeLayoutV1::new(Some(40), 4).unwrap(),
        SemanticTypeShapeV1::Array {
            element: tuple,
            length: 2,
        },
    ));
    types
}

fn aggregate_projected_writes_v2(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    statements: &[usize],
) -> Vec<crate::production_reference_effect_join_v2::RankedGpuWriteV2> {
    let view = ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0));
    let index = ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(1));
    let mut operations = vec![
        ProductionRankedOperationV1::ViewInSpace {
            result: ProductionRankedValueIdV1::new(0),
            element_width: 32,
            writable: true,
            shape: vec![1],
            dynamic_extents: vec![],
            memory_space: MemorySpaceAttr::Global,
            allocation_origin: 1,
            noalias_class: 1,
        },
        ProductionRankedOperationV1::IndexConstant {
            result: ProductionRankedValueIdV1::new(1),
            value: 0,
        },
    ];
    let mut sources = Vec::new();
    for &statement in statements {
        let operation = operations.len();
        operations.push(ProductionRankedOperationV1::Access {
            kind: AccessKindAttr::Write,
            view,
            indices: vec![index],
        });
        sources.push(ProjectedAccessSourceV1 {
            block: 0,
            operation,
            access: AccessKindAttr::Write,
            memory_space: MemorySpaceAttr::Global,
            source: SemanticSourceProvenanceV1::unavailable(),
            semantic_site: Some(ProjectedSemanticAccessSiteV1 {
                block: 0,
                statement: Some(statement),
            }),
        });
    }
    projected_reference_gpu_writes_v2(
        types,
        function,
        &[ProductionRankedBlockV1::new(
            operations,
            ProductionRankedTerminatorV1::Return,
        )],
        &sources,
    )
    .unwrap()
}

fn aggregate_expected_v2(bits: u64) -> ProductionSemanticExpressionV2 {
    ProductionSemanticExpressionV2::Constant {
        scalar: ProductionSemanticScalarTypeV2::Integer {
            signed: false,
            bits: 32,
        },
        bits,
    }
}

fn aggregate_last_value_v2(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
) -> Result<ProductionSemanticExpressionV2, &'static str> {
    aggregate_projected_writes_v2(
        types,
        function,
        &[function.blocks()[0].statements().len() - 1],
    )
    .remove(0)
    .value
}

#[test]
fn gpu_nested_aggregate_components_retain_scalar_value_v2() {
    let types = aggregate_types_v2();
    let tuple = SemanticTypeIdV1::from_index(3);
    let tuple_array = SemanticTypeIdV1::from_index(4);
    let field = SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), ARRAY_TYPE).unwrap();
    let function = aggregate_function_v2(vec![
        aggregate_assignment_v2(1, ARRAY_TYPE, aggregate_array_v2([11, 17, 23, 29])),
        aggregate_assignment_v2(
            4,
            tuple,
            SemanticRvalueKindV1::aggregate(
                SemanticAggregateKindV1::Tuple,
                vec![aggregate_copy_v2(1, ARRAY_TYPE, vec![]), constant(31)],
            )
            .unwrap(),
        ),
        aggregate_assignment_v2(
            6,
            tuple_array,
            SemanticRvalueKindV1::aggregate(
                SemanticAggregateKindV1::Array,
                vec![
                    aggregate_copy_v2(4, tuple, vec![]),
                    aggregate_copy_v2(4, tuple, vec![]),
                ],
            )
            .unwrap(),
        ),
        aggregate_output_v2(aggregate_copy_v2(
            4,
            SCALAR_TYPE,
            vec![field, aggregate_index_v2(2)],
        )),
        aggregate_output_v2(aggregate_copy_v2(
            6,
            SCALAR_TYPE,
            vec![
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::ConstantIndex {
                        offset: 1,
                        minimum_length: 2,
                        from_end: false,
                    },
                    tuple,
                )
                .unwrap(),
                field,
                aggregate_index_v2(1),
            ],
        )),
    ]);
    let writes = aggregate_projected_writes_v2(&types, &function, &[3, 4]);
    assert_eq!(writes[0].value, Ok(aggregate_expected_v2(23)));
    assert_eq!(writes[1].value, Ok(aggregate_expected_v2(17)));
}

#[test]
fn gpu_aggregate_alias_uses_definition_site_v2() {
    let types = aggregate_types_v2();
    let function = aggregate_function_v2(vec![
        aggregate_assignment_v2(1, ARRAY_TYPE, aggregate_array_v2([11, 17, 23, 29])),
        aggregate_assignment_v2(
            5,
            ARRAY_TYPE,
            SemanticRvalueKindV1::Use(aggregate_copy_v2(1, ARRAY_TYPE, vec![])),
        ),
        aggregate_assignment_v2(1, ARRAY_TYPE, aggregate_array_v2([91, 97, 103, 109])),
        aggregate_output_v2(aggregate_copy_v2(
            5,
            SCALAR_TYPE,
            vec![aggregate_index_v2(1)],
        )),
        aggregate_output_v2(aggregate_copy_v2(
            1,
            SCALAR_TYPE,
            vec![aggregate_index_v2(1)],
        )),
    ]);
    let writes = aggregate_projected_writes_v2(&types, &function, &[3, 4]);
    assert_eq!(writes[0].value, Ok(aggregate_expected_v2(17)));
    assert_eq!(writes[1].value, Ok(aggregate_expected_v2(97)));
}

#[test]
fn gpu_aggregate_construction_captures_scalar_history_v2() {
    let types = aggregate_types_v2();
    let function = aggregate_function_v2(vec![
        aggregate_assignment_v2(2, SCALAR_TYPE, SemanticRvalueKindV1::Use(constant(11))),
        aggregate_assignment_v2(
            1,
            ARRAY_TYPE,
            SemanticRvalueKindV1::aggregate(
                SemanticAggregateKindV1::Array,
                vec![
                    aggregate_copy_v2(2, SCALAR_TYPE, vec![]),
                    constant(17),
                    constant(23),
                    constant(29),
                ],
            )
            .unwrap(),
        ),
        aggregate_assignment_v2(2, SCALAR_TYPE, SemanticRvalueKindV1::Use(constant(91))),
        aggregate_output_v2(aggregate_copy_v2(
            1,
            SCALAR_TYPE,
            vec![aggregate_index_v2(0)],
        )),
    ]);
    assert_eq!(
        aggregate_last_value_v2(&types, &function),
        Ok(aggregate_expected_v2(11))
    );
}

#[test]
fn gpu_aggregate_front_and_end_indices_are_distinct_v2() {
    let types = aggregate_types_v2();
    for (offset, from_end, expected) in
        [(0, false, 11), (1, false, 17), (1, true, 29), (3, true, 17)]
    {
        let projection = SemanticProjectionV1::new(
            SemanticProjectionKindV1::ConstantIndex {
                offset,
                minimum_length: 4,
                from_end,
            },
            SCALAR_TYPE,
        )
        .unwrap();
        let function = aggregate_function_v2(vec![
            aggregate_assignment_v2(1, ARRAY_TYPE, aggregate_array_v2([11, 17, 23, 29])),
            aggregate_output_v2(aggregate_copy_v2(1, SCALAR_TYPE, vec![projection])),
        ]);
        assert_eq!(
            aggregate_last_value_v2(&types, &function),
            Ok(aggregate_expected_v2(expected))
        );
    }
}

#[test]
fn gpu_aggregate_malformed_literals_and_projections_reject_v2() {
    let types = aggregate_types_v2();
    for value in [
        SemanticRvalueKindV1::aggregate(SemanticAggregateKindV1::Array, vec![constant(11)])
            .unwrap(),
        SemanticRvalueKindV1::aggregate(
            SemanticAggregateKindV1::Tuple,
            vec![constant(11), constant(17), constant(23), constant(29)],
        )
        .unwrap(),
        SemanticRvalueKindV1::aggregate(
            SemanticAggregateKindV1::Array,
            vec![
                constant(11),
                constant(17),
                constant(23),
                aggregate_copy_v2(3, POINTER_TYPE, vec![]),
            ],
        )
        .unwrap(),
    ] {
        let function = aggregate_function_v2(vec![
            aggregate_assignment_v2(1, ARRAY_TYPE, value),
            aggregate_output_v2(aggregate_copy_v2(
                1,
                SCALAR_TYPE,
                vec![aggregate_index_v2(0)],
            )),
        ]);
        assert!(aggregate_last_value_v2(&types, &function).is_err());
    }
    for (kind, result_type) in [
        (
            SemanticProjectionKindV1::ConstantIndex {
                offset: 0,
                minimum_length: 5,
                from_end: false,
            },
            SCALAR_TYPE,
        ),
        (
            SemanticProjectionKindV1::ConstantIndex {
                offset: 0,
                minimum_length: 4,
                from_end: true,
            },
            SCALAR_TYPE,
        ),
        (
            SemanticProjectionKindV1::ConstantIndex {
                offset: 5,
                minimum_length: 6,
                from_end: true,
            },
            SCALAR_TYPE,
        ),
        (
            SemanticProjectionKindV1::ConstantIndex {
                offset: 0,
                minimum_length: 4,
                from_end: false,
            },
            POINTER_TYPE,
        ),
        (
            SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(2)),
            SCALAR_TYPE,
        ),
        (SemanticProjectionKindV1::Field(0), SCALAR_TYPE),
        (SemanticProjectionKindV1::Dereference, SCALAR_TYPE),
        (SemanticProjectionKindV1::OpaqueCast, SCALAR_TYPE),
    ] {
        let function = aggregate_function_v2(vec![
            aggregate_assignment_v2(1, ARRAY_TYPE, aggregate_array_v2([11, 17, 23, 29])),
            aggregate_output_v2(aggregate_copy_v2(
                1,
                result_type,
                vec![SemanticProjectionV1::new(kind, result_type).unwrap()],
            )),
        ]);
        assert!(
            aggregate_last_value_v2(&types, &function).is_err(),
            "{kind:?}"
        );
    }
}

#[test]
fn gpu_aggregate_projected_writes_deinitialization_and_escapes_reject_v2() {
    let types = aggregate_types_v2();
    let component = ranked_place(1);
    let whole = SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![], ARRAY_TYPE).unwrap();
    let mut invalidations = vec![
        statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            component.clone(),
            SemanticRvalueV1::new(SCALAR_TYPE, SemanticRvalueKindV1::Use(constant(99))),
        ))),
        statement(SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
            component.clone(),
            constant(99),
            SemanticVolatilityV1::NonVolatile,
            None,
        ))),
        statement(SemanticStatementKindV1::Deinitialize(whole)),
        aggregate_assignment_v2(
            3,
            POINTER_TYPE,
            SemanticRvalueKindV1::AddressOf {
                mutability: SemanticMutabilityV1::Mutable,
                place: component.clone(),
            },
        ),
    ];
    for kind in [
        SemanticBorrowKindV1::Mutable,
        SemanticBorrowKindV1::Shared,
        SemanticBorrowKindV1::Fake,
    ] {
        invalidations.push(aggregate_assignment_v2(
            3,
            POINTER_TYPE,
            SemanticRvalueKindV1::Borrow {
                kind,
                place: component.clone(),
            },
        ));
    }
    for invalidation in invalidations {
        let function = aggregate_function_v2(vec![
            aggregate_assignment_v2(1, ARRAY_TYPE, aggregate_array_v2([11, 17, 23, 29])),
            invalidation,
            aggregate_output_v2(aggregate_copy_v2(
                1,
                SCALAR_TYPE,
                vec![aggregate_index_v2(1)],
            )),
        ]);
        assert!(aggregate_last_value_v2(&types, &function).is_err());
    }
}

#[test]
fn gpu_aggregate_future_definition_and_uninitialized_alias_reject_v2() {
    let types = aggregate_types_v2();
    let function = aggregate_function_v2(vec![
        aggregate_assignment_v2(
            5,
            ARRAY_TYPE,
            SemanticRvalueKindV1::Use(aggregate_copy_v2(1, ARRAY_TYPE, vec![])),
        ),
        aggregate_assignment_v2(1, ARRAY_TYPE, aggregate_array_v2([11, 17, 23, 29])),
        aggregate_output_v2(aggregate_copy_v2(
            5,
            SCALAR_TYPE,
            vec![aggregate_index_v2(1)],
        )),
    ]);
    assert!(aggregate_last_value_v2(&types, &function).is_err());
}

#[test]
fn gpu_aggregate_write_binding_uses_exact_semantic_statement_v2() {
    let types = aggregate_types_v2();
    let function = aggregate_function_v2(vec![
        aggregate_assignment_v2(1, ARRAY_TYPE, aggregate_array_v2([11, 17, 23, 29])),
        aggregate_output_v2(aggregate_copy_v2(
            1,
            SCALAR_TYPE,
            vec![aggregate_index_v2(2)],
        )),
        aggregate_output_v2(constant(83)),
    ]);
    for (statements, expected) in [([1, 2], [23, 83]), ([2, 1], [83, 23])] {
        let writes = aggregate_projected_writes_v2(&types, &function, &statements);
        for (ordinal, write) in writes.iter().enumerate() {
            assert_eq!(
                (write.block, write.operation, write.allocation_origin),
                (0, ordinal + 2, 1)
            );
            assert_eq!(
                write.view,
                ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0))
            );
            assert_eq!(
                write.indices,
                vec![ProductionRankedValueV1::Local(
                    ProductionRankedValueIdV1::new(1)
                )]
            );
            assert_eq!(write.value, Ok(aggregate_expected_v2(expected[ordinal])));
        }
    }
    assert!(
        aggregate_projected_writes_v2(&types, &function, &[usize::MAX])[0]
            .value
            .is_err()
    );
    let statement = function.blocks()[0].statements()[1].kind();
    let mut resolver = GpuSemanticExpressionResolverV2::new(&types, &function).unwrap();
    assert!(
        resolver
            .resolve_store_v2(
                statement,
                ScalarAssignmentSiteV1 {
                    block: 0,
                    statement: 2
                }
            )
            .is_err()
    );
}

#[test]
fn gpu_aggregate_detached_local_and_work_exhaustion_reject_v2() {
    let types = aggregate_types_v2();
    let function = aggregate_function_v2(vec![
        aggregate_assignment_v2(1, ARRAY_TYPE, aggregate_array_v2([11, 17, 23, 29])),
        aggregate_output_v2(aggregate_copy_v2(
            1,
            SCALAR_TYPE,
            vec![aggregate_index_v2(1)],
        )),
    ]);
    let operand = aggregate_copy_v2(1, SCALAR_TYPE, vec![aggregate_index_v2(1)]);
    let mut resolver = GpuSemanticExpressionResolverV2::new(&types, &function).unwrap();
    assert_eq!(
        resolver.resolve_operand_v2(&operand, 0).unwrap_err(),
        "GPU semantic local has no exact use site"
    );
    resolver.work = fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2;
    assert_eq!(
        resolver
            .resolve_store_v2(
                function.blocks()[0].statements()[1].kind(),
                ScalarAssignmentSiteV1 {
                    block: 0,
                    statement: 1
                },
            )
            .unwrap_err(),
        "GPU semantic expression exceeds its bounded node budget"
    );
    assert!(resolver.use_site.is_none());
}

#[test]
fn gpu_aggregate_predecessor_definitions_reject_v2() {
    let types = aggregate_types_v2();
    for define_other_branch in [false, true] {
        let mut locals = aggregate_function_v2(vec![]).locals().to_vec();
        locals[2] = local(237, SCALAR_TYPE, SemanticLocalRoleV1::Argument(1));
        let function = projection_function_with_locals(
            vec![
                block(
                    240,
                    vec![],
                    SemanticTerminatorKindV1::SwitchInt {
                        discriminant: aggregate_copy_v2(2, SCALAR_TYPE, vec![]),
                        targets: SemanticSwitchTargetsV1::new(
                            vec![SemanticSwitchTargetV1::new(
                                0,
                                cfg_edge(SemanticEdgeRoleV1::SwitchValue, 1),
                            )],
                            cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                        )
                        .unwrap(),
                    },
                ),
                block(
                    241,
                    vec![aggregate_assignment_v2(
                        1,
                        ARRAY_TYPE,
                        aggregate_array_v2([11, 17, 23, 29]),
                    )],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 3)),
                ),
                block(
                    242,
                    if define_other_branch {
                        vec![aggregate_assignment_v2(
                            1,
                            ARRAY_TYPE,
                            aggregate_array_v2([91, 97, 103, 109]),
                        )]
                    } else {
                        vec![]
                    },
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 3)),
                ),
                block(
                    243,
                    vec![aggregate_output_v2(aggregate_copy_v2(
                        1,
                        SCALAR_TYPE,
                        vec![aggregate_index_v2(1)],
                    ))],
                    SemanticTerminatorKindV1::Return,
                ),
            ],
            locals,
        );
        let mut resolver = GpuSemanticExpressionResolverV2::new(&types, &function).unwrap();
        assert_eq!(
            resolver
                .resolve_store_v2(
                    function.blocks()[3].statements()[0].kind(),
                    ScalarAssignmentSiteV1 {
                        block: 3,
                        statement: 0
                    },
                )
                .unwrap_err(),
            "GPU semantic local has no exact reaching assignment"
        );
        assert!(resolver.use_site.is_none());
    }
}

#[test]
fn gpu_aggregate_dominating_definition_and_unknown_call_result_v2() {
    let types = aggregate_types_v2();
    for replace_with_call in [false, true] {
        let whole =
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![], ARRAY_TYPE).unwrap();
        let terminator = if replace_with_call {
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    SemanticCallableIdV1::from_index(0),
                    vec![],
                    Some(SemanticCallDestinationV1::new(
                        whole,
                        cfg_edge(SemanticEdgeRoleV1::CallReturn, 1),
                    )),
                    SemanticUnwindActionV1::Unreachable,
                )
                .unwrap(),
            )
        } else {
            SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1))
        };
        let function = projection_function_with_locals(
            vec![
                block(
                    240,
                    vec![aggregate_assignment_v2(
                        1,
                        ARRAY_TYPE,
                        aggregate_array_v2([11, 17, 23, 29]),
                    )],
                    terminator,
                ),
                block(
                    241,
                    vec![aggregate_output_v2(aggregate_copy_v2(
                        1,
                        SCALAR_TYPE,
                        vec![aggregate_index_v2(1)],
                    ))],
                    SemanticTerminatorKindV1::Return,
                ),
            ],
            aggregate_function_v2(vec![]).locals().to_vec(),
        );
        let value = GpuSemanticExpressionResolverV2::new(&types, &function)
            .unwrap()
            .resolve_store_v2(
                function.blocks()[1].statements()[0].kind(),
                ScalarAssignmentSiteV1 {
                    block: 1,
                    statement: 0,
                },
            );
        if replace_with_call {
            assert_eq!(
                value.unwrap_err(),
                "GPU semantic local has no exact reaching assignment"
            );
        } else {
            assert_eq!(value, Ok(aggregate_expected_v2(17)));
        }
    }
}

#[test]
fn gpu_aggregate_changed_input_cannot_become_entry_symbol_v2() {
    let types = aggregate_types_v2();
    for mutated in [false, true] {
        for through_aggregate in [false, true] {
            let mut statements = Vec::new();
            if mutated {
                statements.push(aggregate_assignment_v2(
                    2,
                    SCALAR_TYPE,
                    SemanticRvalueKindV1::Use(constant(91)),
                ));
            }
            if through_aggregate {
                statements.push(aggregate_assignment_v2(
                    1,
                    ARRAY_TYPE,
                    SemanticRvalueKindV1::aggregate(
                        SemanticAggregateKindV1::Array,
                        vec![
                            aggregate_copy_v2(2, SCALAR_TYPE, vec![]),
                            constant(17),
                            constant(23),
                            constant(29),
                        ],
                    )
                    .unwrap(),
                ));
            }
            statements.push(aggregate_output_v2(if through_aggregate {
                aggregate_copy_v2(1, SCALAR_TYPE, vec![aggregate_index_v2(0)])
            } else {
                aggregate_copy_v2(2, SCALAR_TYPE, vec![])
            }));
            let mut locals = aggregate_function_v2(vec![]).locals().to_vec();
            locals[2] = local(237, SCALAR_TYPE, SemanticLocalRoleV1::Argument(1));
            let function = projection_function_with_locals(
                vec![block(240, statements, SemanticTerminatorKindV1::Return)],
                locals,
            );
            let value = aggregate_last_value_v2(&types, &function);
            if mutated {
                assert_eq!(
                    value.unwrap_err(),
                    "GPU semantic input argument is not an unchanged entry value"
                );
            } else {
                assert_eq!(
                    value,
                    Ok(ProductionSemanticExpressionV2::Symbol {
                        symbol: crate::reference_effect_v1::kernel_scalar_symbol_v2(1).unwrap(),
                        scalar: ProductionSemanticScalarTypeV2::Integer {
                            signed: false,
                            bits: 32
                        },
                    })
                );
            }
        }
    }
}

#[test]
fn gpu_plain_aggregate_field_uses_typed_literal_v2() {
    let tuple = SemanticTypeIdV1::from_index(3);
    let mut types = aggregate_types_v2();
    types[3] = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(230)),
        SemanticLayoutIdentityV1::from_sha256(bytes(231)),
        SemanticTypeLayoutV1::aggregate(
            Some(20),
            4,
            SemanticAggregateLayoutV1::new(vec![0, 16], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(
            SemanticAggregateTypeV1::new(vec![ARRAY_TYPE, SCALAR_TYPE]).unwrap(),
        ),
    );
    let function = aggregate_function_v2(vec![
        aggregate_assignment_v2(1, ARRAY_TYPE, aggregate_array_v2([11, 17, 23, 29])),
        aggregate_assignment_v2(
            4,
            tuple,
            SemanticRvalueKindV1::aggregate(
                SemanticAggregateKindV1::Aggregate,
                vec![aggregate_copy_v2(1, ARRAY_TYPE, vec![]), constant(31)],
            )
            .unwrap(),
        ),
        aggregate_output_v2(aggregate_copy_v2(
            4,
            SCALAR_TYPE,
            vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::Field(1), SCALAR_TYPE).unwrap(),
            ],
        )),
    ]);
    assert_eq!(
        aggregate_last_value_v2(&types, &function),
        Ok(aggregate_expected_v2(31))
    );
}

#[test]
fn gpu_aggregate_load_retains_ranked_read_identity_v2() {
    let types = aggregate_types_v2();
    let function = aggregate_function_v2(vec![
        aggregate_assignment_v2(
            1,
            ARRAY_TYPE,
            SemanticRvalueKindV1::aggregate(
                SemanticAggregateKindV1::Array,
                vec![
                    SemanticOperandV1::Copy(dereferenced_place()),
                    constant(17),
                    constant(23),
                    constant(29),
                ],
            )
            .unwrap(),
        ),
        aggregate_output_v2(aggregate_copy_v2(
            1,
            SCALAR_TYPE,
            vec![aggregate_index_v2(0)],
        )),
    ]);
    let view = ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0));
    let index = ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(1));
    let blocks = [ProductionRankedBlockV1::new(
        vec![
            ProductionRankedOperationV1::ViewInSpace {
                result: ProductionRankedValueIdV1::new(0),
                element_width: 32,
                writable: true,
                shape: vec![1],
                dynamic_extents: vec![],
                memory_space: MemorySpaceAttr::Global,
                allocation_origin: 1,
                noalias_class: 1,
            },
            ProductionRankedOperationV1::IndexConstant {
                result: ProductionRankedValueIdV1::new(1),
                value: 0,
            },
            ProductionRankedOperationV1::Access {
                kind: AccessKindAttr::Read,
                view,
                indices: vec![index],
            },
            ProductionRankedOperationV1::Access {
                kind: AccessKindAttr::Write,
                view,
                indices: vec![index],
            },
        ],
        ProductionRankedTerminatorV1::Return,
    )];
    let sources = [AccessKindAttr::Read, AccessKindAttr::Write]
        .into_iter()
        .enumerate()
        .map(|(ordinal, access)| ProjectedAccessSourceV1 {
            block: 0,
            operation: ordinal + 2,
            access,
            memory_space: MemorySpaceAttr::Global,
            source: SemanticSourceProvenanceV1::unavailable(),
            semantic_site: Some(ProjectedSemanticAccessSiteV1 {
                block: 0,
                statement: Some(ordinal),
            }),
        })
        .collect::<Vec<_>>();
    let writes = projected_reference_gpu_writes_v2(&types, &function, &blocks, &sources).unwrap();
    assert_eq!(
        writes[0].value,
        Ok(ProductionSemanticExpressionV2::Load(
            ProductionSemanticLoadV2 {
                block: 0,
                operation: 2,
                scalar: ProductionSemanticScalarTypeV2::Integer {
                    signed: false,
                    bits: 32
                },
                allocation_origin: 1,
                view,
                indices: vec![index].into_boxed_slice(),
            }
        ))
    );
}
