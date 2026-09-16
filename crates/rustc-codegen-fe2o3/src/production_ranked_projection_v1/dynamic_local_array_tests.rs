#[test]
fn dynamic_local_array_and_slice_guards_share_authenticated_component_indices() {
    let (mut types, original) = ordinary_component_payload_fixture();
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(210)),
        SemanticLayoutIdentityV1::from_sha256(bytes(210)),
        SemanticTypeLayoutV1::new(Some(1), 1).unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool),
    ));
    let slice_type = SemanticTypeIdV1::from_index(5);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(211)),
        SemanticLayoutIdentityV1::from_sha256(bytes(211)),
        SemanticTypeLayoutV1::new(None, 4).unwrap(),
        SemanticTypeShapeV1::Slice {
            element: SCALAR_TYPE,
        },
    ));
    let mut blocks = original.blocks().to_vec();
    let fixed_comparison = typed_assignment(
        4,
        BOOL_TYPE,
        SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::LessThan,
            left: typed_operand(3, SCALAR_TYPE),
            right: constant(4),
        },
    );
    blocks[1] = block(
        212,
        vec![blocks[1].statements()[0].clone(), fixed_comparison],
        SemanticTerminatorKindV1::Assert {
            condition: typed_operand(4, BOOL_TYPE),
            expected: true,
            message: SemanticAssertMessageV1::BoundsCheck {
                length: constant(4),
                index: typed_operand(3, SCALAR_TYPE),
            },
            target: cfg_edge(SemanticEdgeRoleV1::AssertSuccess, 4),
            unwind: SemanticUnwindActionV1::Unreachable,
        },
    );
    blocks.push(block(
        213,
        vec![
            typed_assignment(
                5,
                SCALAR_TYPE,
                SemanticRvalueKindV1::Length(
                    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(6), vec![], slice_type)
                        .unwrap(),
                ),
            ),
            typed_assignment(
                7,
                BOOL_TYPE,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::LessThan,
                    left: typed_operand(3, SCALAR_TYPE),
                    right: typed_operand(5, SCALAR_TYPE),
                },
            ),
        ],
        SemanticTerminatorKindV1::Assert {
            condition: typed_operand(7, BOOL_TYPE),
            expected: true,
            message: SemanticAssertMessageV1::BoundsCheck {
                length: typed_operand(5, SCALAR_TYPE),
                index: typed_operand(3, SCALAR_TYPE),
            },
            target: cfg_edge(SemanticEdgeRoleV1::AssertSuccess, 5),
            unwind: SemanticUnwindActionV1::Unreachable,
        },
    ));
    blocks.push(block(214, vec![], SemanticTerminatorKindV1::Return));
    let mut locals = original.locals().to_vec();
    locals.extend([
        local(215, BOOL_TYPE, SemanticLocalRoleV1::Temporary),
        local(216, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
        local(217, slice_type, SemanticLocalRoleV1::Argument(0)),
        local(218, BOOL_TYPE, SemanticLocalRoleV1::Temporary),
    ]);
    let function = projection_function_with_locals(blocks, locals);
    let dominance = SemanticEnumPayloadDominanceV1::analyze(&function, &types).unwrap();
    let mut payloads = vec![None; function.locals().len()];
    let expected = ProductionRankedValueV1::Argument(7);
    payloads[1] = Some(expected);
    let ordinary = bind_component_index_enum_payloads_v1(
        &function,
        &payloads,
        &local_definition_counts(&function),
        &vec![false; function.locals().len()],
        &[PendingEnumPayloadLoadV1 {
            carrier: 1,
            variant: 1,
            destination: 3,
            use_block: 1,
            statement: 0,
        }],
        &dominance,
    )
    .unwrap();
    let mut operations = vec![];
    let checks = project_rust_bounds_checks_with_ordinary_v1(
        &types,
        &function,
        0,
        &[],
        &ordinary,
        Some(&dominance),
        &mut operations,
        &mut 0,
    )
    .unwrap();
    assert_eq!(checks.checks.len(), 2);
    assert!(checks.checks.iter().all(|check| check.index == expected));
    assert_eq!(
        operations
            .iter()
            .filter(|operation| matches!(
                operation,
                ProductionRankedOperationV1::IndexUnknown { .. }
            ))
            .count(),
        1,
        "only the slice extent is unknown; the shared index must retain its identity"
    );
    let index_local = SemanticLocalIdV1::from_index(3);
    let mut values = vec![None; function.locals().len()];
    for _ in 0..2 {
        reconcile_bounds_ordinary_index_v1(
            1,
            index_local,
            &ordinary,
            Some(&dominance),
            &mut values,
        )
        .unwrap();
        assert_eq!(values[3], Some(expected));
    }
    values[3] = Some(ProductionRankedValueV1::Argument(8));
    assert!(
        reconcile_bounds_ordinary_index_v1(
            1,
            index_local,
            &ordinary,
            Some(&dominance),
            &mut values
        )
        .is_err()
    );
    values[3] = None;
    assert!(
        reconcile_bounds_ordinary_index_v1(
            2,
            index_local,
            &ordinary,
            Some(&dominance),
            &mut values
        )
        .is_err()
    );
    assert!(
        reconcile_bounds_ordinary_index_v1(1, index_local, &ordinary, None, &mut values).is_err()
    );
}

#[derive(Clone, Copy, Debug, Default)]
struct FixedGuardOptions {
    wrong_comparison: bool,
    wrong_message: bool,
    late_index: bool,
    bypass: bool,
    expected_false: bool,
    mutated_array: bool,
    extent: u64,
}

fn fixed_guard_function(options: FixedGuardOptions) -> SemanticFunctionDeclV1 {
    let place =
        |local, ty| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
    let operand = |local, ty| SemanticOperandV1::Copy(place(local, ty));
    let bound = |n| {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            U64_TYPE,
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(n, 8).unwrap()),
        ))
    };
    let assign = |local, ty, kind| {
        statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(local, ty),
            SemanticRvalueV1::new(ty, kind),
        )))
    };
    let array = || {
        assign(
            1,
            ARRAY_TYPE,
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(
                    SemanticAggregateKindV1::Array,
                    vec![constant(11), constant(22), constant(33), constant(44)],
                )
                .unwrap(),
            ),
        )
    };
    let mut statements = vec![
        array(),
        assign(
            3,
            BOOL_TYPE,
            SemanticRvalueKindV1::Binary {
                operation: if options.wrong_comparison {
                    SemanticBinaryOpV1::GreaterThan
                } else {
                    SemanticBinaryOpV1::LessThan
                },
                left: operand(2, U64_TYPE),
                right: bound(u128::from(options.extent)),
            },
        ),
    ];
    if options.late_index {
        statements.push(assign(
            2,
            U64_TYPE,
            SemanticRvalueKindV1::Use(operand(4, U64_TYPE)),
        ));
    }
    if options.mutated_array {
        statements.push(array());
    }
    let mut blocks = vec![
        block(
            190,
            statements,
            SemanticTerminatorKindV1::Assert {
                condition: operand(3, BOOL_TYPE),
                expected: !options.expected_false,
                message: SemanticAssertMessageV1::BoundsCheck {
                    length: bound(if options.wrong_message {
                        3
                    } else {
                        u128::from(options.extent)
                    }),
                    index: operand(2, U64_TYPE),
                },
                target: cfg_edge(SemanticEdgeRoleV1::AssertSuccess, 1),
                unwind: SemanticUnwindActionV1::Unreachable,
            },
        ),
        block(191, vec![], SemanticTerminatorKindV1::Return),
    ];
    if options.bypass {
        blocks.push(block(
            192,
            vec![],
            SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
        ));
    }
    projection_function_with_locals(
        blocks,
        vec![
            local(193, SCALAR_TYPE, SemanticLocalRoleV1::Return),
            local(194, ARRAY_TYPE, SemanticLocalRoleV1::Temporary),
            local(195, U64_TYPE, SemanticLocalRoleV1::Argument(0)),
            local(196, BOOL_TYPE, SemanticLocalRoleV1::Temporary),
            local(197, U64_TYPE, SemanticLocalRoleV1::Argument(1)),
        ],
    )
}

fn collect_fixed_guards(
    function: &SemanticFunctionDeclV1,
) -> Result<ProjectedBoundsChecksV1, ProductionRankedProjectionErrorV1> {
    project_rust_bounds_checks_with_ordinary_v1(
        &assertion_proof_types(),
        function,
        0,
        &[],
        &[],
        None,
        &mut vec![],
        &mut 0,
    )
}

#[test]
fn dynamic_local_array_ranked_guard_retains_exact_literal_and_kind() {
    let function = fixed_guard_function(FixedGuardOptions {
        extent: 4,
        ..Default::default()
    });
    let mut operations = vec![];
    let mut next_value = 0;
    let checks = project_rust_bounds_checks_with_ordinary_v1(
        &assertion_proof_types(),
        &function,
        0,
        &[],
        &[],
        None,
        &mut operations,
        &mut next_value,
    )
    .unwrap();
    let check = projected_bounds_check(
        &checks.checks,
        1,
        ProjectedBoundsExtentSourceV1::FixedArray(4),
        SemanticLocalIdV1::from_index(2),
    )
    .unwrap();
    assert!(check.must_authorize_access);
    assert!(matches!(
        operations.as_slice(),
        [
            ProductionRankedOperationV1::IndexUnknown { .. },
            ProductionRankedOperationV1::IndexConstant { value: 4, .. },
        ]
    ));
    for source in [
        ProjectedBoundsExtentSourceV1::FixedArray(3),
        ProjectedBoundsExtentSourceV1::Slice(SemanticLocalIdV1::from_index(1)),
    ] {
        assert!(
            projected_bounds_check(&checks.checks, 1, source, SemanticLocalIdV1::from_index(2))
                .is_err()
        );
    }
    let duplicated = vec![check, check];
    assert!(
        projected_bounds_check(
            &duplicated,
            1,
            check.extent_source,
            SemanticLocalIdV1::from_index(2)
        )
        .is_err()
    );
}

#[test]
fn dynamic_local_array_ranked_guards_reject_forged_or_stale_history() {
    let good = FixedGuardOptions {
        extent: 4,
        ..Default::default()
    };
    for options in [
        FixedGuardOptions {
            wrong_comparison: true,
            ..good
        },
        FixedGuardOptions {
            wrong_message: true,
            ..good
        },
        FixedGuardOptions {
            late_index: true,
            ..good
        },
        FixedGuardOptions {
            bypass: true,
            ..good
        },
        FixedGuardOptions {
            expected_false: true,
            ..good
        },
        FixedGuardOptions { extent: 0, ..good },
    ] {
        assert!(
            collect_fixed_guards(&fixed_guard_function(options)).is_err(),
            "{options:?}"
        );
    }
}

#[test]
fn dynamic_local_array_access_is_guarded_private_and_readonly() {
    let types = assertion_proof_types();
    for mutated in [false, true] {
        let function = fixed_guard_function(FixedGuardOptions {
            extent: 4,
            mutated_array: mutated,
            ..Default::default()
        });
        let checks = collect_fixed_guards(&function).unwrap();
        let mut contracts = synthetic_local_contracts_with_types(&function, &types);
        contracts.checked_references.enum_payload_dominance =
            SemanticEnumPayloadDominanceV1::analyze(&function, &types).unwrap();
        for access in [AccessKindAttr::Read, AccessKindAttr::Write] {
            let mut guarded = vec![];
            let mut operations = vec![];
            let mut sources = vec![];
            let place = SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(1),
                vec![
                    SemanticProjectionV1::new(
                        SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(2)),
                        SCALAR_TYPE,
                    )
                    .unwrap(),
                ],
                SCALAR_TYPE,
            )
            .unwrap();
            let result = project_place_access(
                &types,
                &function,
                1,
                &checks.checks,
                &place,
                access,
                PlaceAccessRequirementV1::IfMemory,
                SemanticSourceProvenanceV1::unavailable(),
                &constant_locals(&function).unwrap(),
                &contracts,
                &[],
                &mut guarded,
                &mut vec![None; function.locals().len()],
                &mut operations,
                &mut sources,
                &mut 10,
                &mut String::new(),
            );
            if mutated || access == AccessKindAttr::Write {
                assert!(result.is_err());
            } else {
                result.unwrap();
                assert_eq!(guarded.len(), 1);
                assert_eq!(guarded[0].access.memory_space, MemorySpaceAttr::Private);
                assert_eq!(
                    guarded[0].access.comparisons,
                    vec![(checks.checks[0].index, checks.checks[0].extent)]
                );
                let block = ProjectedSemanticBlockV1 {
                    items: vec![ProjectedBlockItemV1::Guarded(guarded[0].access.clone())],
                };
                assert!(block.has_memory_access());
                assert!(!block.requires_invocation_index());
                assert!(sources.is_empty());
            }
        }
    }
}
