#[derive(Clone, Copy)]
struct ConstantSliceBoundsOptionsV1 {
    compared_index: u128,
    message_index: u128,
    literal_type: SemanticTypeIdV1,
    literal_bytes: u8,
    operation: SemanticBinaryOpV1,
    message_length: u32,
    length_source: u32,
    bypass: bool,
    mutate_slice: bool,
    duplicate_length: bool,
    duplicate_condition: bool,
    projected_length: bool,
}

impl Default for ConstantSliceBoundsOptionsV1 {
    fn default() -> Self {
        Self {
            compared_index: 24,
            message_index: 24,
            literal_type: U64_TYPE,
            literal_bytes: 8,
            operation: SemanticBinaryOpV1::LessThan,
            message_length: 4,
            length_source: 1,
            bypass: false,
            mutate_slice: false,
            duplicate_length: false,
            duplicate_condition: false,
            projected_length: false,
        }
    }
}

fn constant_slice_types_v1() -> Vec<SemanticTypeDeclV1> {
    let mut types = assertion_proof_types();
    types[ARRAY_TYPE.index() as usize] = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(211)),
        SemanticLayoutIdentityV1::from_sha256(bytes(211)),
        SemanticTypeLayoutV1::new(None, 4).unwrap(),
        SemanticTypeShapeV1::Slice {
            element: SCALAR_TYPE,
        },
    );
    types[POINTER_TYPE.index() as usize] = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(212)),
        SemanticLayoutIdentityV1::from_sha256(bytes(212)),
        SemanticTypeLayoutV1::new(Some(16), 8).unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                ARRAY_TYPE,
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Immutable,
                1,
                64,
                SemanticPointerMetadataV1::SliceLength,
            )
            .unwrap(),
        ),
    );
    types
}

fn constant_slice_bounds_function_v1(
    options: ConstantSliceBoundsOptionsV1,
) -> SemanticFunctionDeclV1 {
    let operand = |index, ty| {
        SemanticOperandV1::Copy(
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(index), vec![], ty).unwrap(),
        )
    };
    let assign = |index, ty, kind| {
        statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(index), vec![], ty).unwrap(),
            SemanticRvalueV1::new(ty, kind),
        )))
    };
    let literal = |bits| typed_constant(options.literal_type, bits, options.literal_bytes);
    let length = if options.projected_length {
        SemanticRvalueKindV1::Length(
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(options.length_source),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ARRAY_TYPE)
                        .unwrap(),
                    SemanticProjectionV1::new(
                        SemanticProjectionKindV1::Subslice {
                            from: 0,
                            to: 1,
                            from_end: true,
                        },
                        ARRAY_TYPE,
                    )
                    .unwrap(),
                ],
                ARRAY_TYPE,
            )
            .unwrap(),
        )
    } else {
        SemanticRvalueKindV1::Unary {
            operation: SemanticUnaryOpV1::PointerMetadata,
            operand: operand(options.length_source, POINTER_TYPE),
        }
    };
    let length_assignment = assign(4, U64_TYPE, length);
    let comparison = assign(
        2,
        BOOL_TYPE,
        SemanticRvalueKindV1::Binary {
            operation: options.operation,
            left: literal(options.compared_index),
            right: operand(4, U64_TYPE),
        },
    );
    let mut statements = vec![
        assign(
            3,
            POINTER_TYPE,
            SemanticRvalueKindV1::Use(operand(1, POINTER_TYPE)),
        ),
        length_assignment.clone(),
        assign(
            5,
            U64_TYPE,
            SemanticRvalueKindV1::Unary {
                operation: SemanticUnaryOpV1::PointerMetadata,
                operand: operand(3, POINTER_TYPE),
            },
        ),
        comparison.clone(),
    ];
    if options.duplicate_length {
        statements.push(length_assignment);
    }
    if options.duplicate_condition {
        statements.push(comparison);
    }
    if options.mutate_slice {
        statements.push(assign(
            1,
            POINTER_TYPE,
            SemanticRvalueKindV1::Use(operand(3, POINTER_TYPE)),
        ));
    }
    let mut blocks = vec![
        block(
            213,
            statements,
            SemanticTerminatorKindV1::Assert {
                condition: operand(2, BOOL_TYPE),
                expected: true,
                message: SemanticAssertMessageV1::BoundsCheck {
                    index: literal(options.message_index),
                    length: operand(options.message_length, U64_TYPE),
                },
                target: cfg_edge(SemanticEdgeRoleV1::AssertSuccess, 1),
                unwind: SemanticUnwindActionV1::Unreachable,
            },
        ),
        block(214, vec![], SemanticTerminatorKindV1::Return),
    ];
    if options.bypass {
        blocks.push(block(
            215,
            vec![],
            SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
        ));
    }
    projection_function_with_owned_argument(
        blocks,
        vec![
            local(216, SCALAR_TYPE, SemanticLocalRoleV1::Return),
            local(217, POINTER_TYPE, SemanticLocalRoleV1::Argument(0)),
            local(218, BOOL_TYPE, SemanticLocalRoleV1::Temporary),
            local(219, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
            local(220, U64_TYPE, SemanticLocalRoleV1::Temporary),
            local(221, U64_TYPE, SemanticLocalRoleV1::Temporary),
        ],
        SemanticSourceArgumentOwnershipV1::SharedBorrow,
    )
}

fn constant_slice_checks_v1(
    function: &SemanticFunctionDeclV1,
) -> Result<
    (ProjectedBoundsChecksV1, Vec<ProductionRankedOperationV1>),
    ProductionRankedProjectionErrorV1,
> {
    let mut operations = Vec::new();
    let checks = project_rust_bounds_checks_with_ordinary_v1(
        &constant_slice_types_v1(),
        function,
        0,
        &[],
        &[],
        None,
        &mut operations,
        &mut 0,
    )?;
    Ok((checks, operations))
}

fn project_constant_slice_read_v1(
    function: &SemanticFunctionDeclV1,
    checks: &[ProjectedBoundsCheckV1],
    offset: u64,
    minimum_length: u64,
    from_end: bool,
) -> Result<
    (Vec<ProductionRankedOperationV1>, Vec<GuardedAccessSiteV1>),
    ProductionRankedProjectionErrorV1,
> {
    let place = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![
            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ARRAY_TYPE).unwrap(),
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::ConstantIndex {
                    offset,
                    minimum_length,
                    from_end,
                },
                SCALAR_TYPE,
            )
            .unwrap(),
        ],
        SCALAR_TYPE,
    )
    .unwrap();
    let mut operations = Vec::new();
    let mut sites = Vec::new();
    project_place_access(
        &constant_slice_types_v1(),
        function,
        1,
        checks,
        &place,
        AccessKindAttr::Read,
        PlaceAccessRequirementV1::IfMemory,
        SemanticSourceProvenanceV1::unavailable(),
        &constant_locals(function)?,
        &synthetic_local_contracts(function),
        &[],
        &mut sites,
        &mut vec![None; function.locals().len()],
        &mut operations,
        &mut Vec::new(),
        &mut 100,
        &mut String::new(),
    )?;
    Ok((operations, sites))
}

#[test]
fn constant_slice_literal_keeps_exact_constant_and_dynamic_extent() {
    let function = constant_slice_bounds_function_v1(Default::default());
    let (checks, operations) = constant_slice_checks_v1(&function).unwrap();
    assert!(matches!(operations.as_slice(), [
        ProductionRankedOperationV1::IndexConstant { result: index, value: 24 },
        ProductionRankedOperationV1::IndexUnknown { result: extent },
    ] if index.get() == 0 && extent.get() == 1));
    assert_eq!(checks.checks.len(), 1);
    let check = checks.checks[0];
    assert_eq!(
        check.index_identity,
        ProjectedBoundsIndexIdentityV1::Literal(24)
    );
    for minimum_length in [25, u64::MAX] {
        let (operations, sites) =
            project_constant_slice_read_v1(&function, &checks.checks, 24, minimum_length, false)
                .unwrap();
        assert!(matches!(&operations[0],
            ProductionRankedOperationV1::ViewInSpace { shape, dynamic_extents, .. }
            if shape == &[DYNAMIC_EXTENT] && dynamic_extents == &[check.extent]
        ));
        assert_eq!(sites.len(), 1);
        assert_eq!(sites[0].access.indices, [check.index]);
        assert_eq!(sites[0].access.comparisons, [(check.index, check.extent)]);
    }
}

#[test]
fn constant_slice_missing_or_wrong_literal_guard_cannot_authorize_read() {
    let function = constant_slice_bounds_function_v1(Default::default());
    let (checks, _) = constant_slice_checks_v1(&function).unwrap();
    assert!(project_constant_slice_read_v1(&function, &[], 24, u64::MAX, false).is_err());
    for offset in [23, 25] {
        assert!(
            project_constant_slice_read_v1(&function, &checks.checks, offset, 64, false).is_err()
        );
    }
    let duplicate = [checks.checks[0], checks.checks[0]];
    assert!(project_constant_slice_read_v1(&function, &duplicate, 24, 25, false).is_err());
}

#[test]
fn constant_slice_from_end_is_rejected_without_runtime_subtraction_proof() {
    let function = constant_slice_bounds_function_v1(Default::default());
    let (checks, _) = constant_slice_checks_v1(&function).unwrap();
    assert!(project_constant_slice_read_v1(&function, &checks.checks, 24, 25, true).is_err());
}

#[test]
fn constant_slice_literal_guard_rejects_forged_comparison_and_length() {
    for options in [
        ConstantSliceBoundsOptionsV1 {
            message_index: 23,
            ..Default::default()
        },
        ConstantSliceBoundsOptionsV1 {
            operation: SemanticBinaryOpV1::LessOrEqual,
            ..Default::default()
        },
        ConstantSliceBoundsOptionsV1 {
            message_length: 5,
            ..Default::default()
        },
        ConstantSliceBoundsOptionsV1 {
            projected_length: true,
            ..Default::default()
        },
    ] {
        assert!(constant_slice_checks_v1(&constant_slice_bounds_function_v1(options)).is_err());
    }
    let function = constant_slice_bounds_function_v1(ConstantSliceBoundsOptionsV1 {
        length_source: 3,
        ..Default::default()
    });
    let (checks, _) = constant_slice_checks_v1(&function).unwrap();
    assert!(project_constant_slice_read_v1(&function, &checks.checks, 24, 25, false).is_err());
}

#[test]
fn constant_slice_literal_guard_rejects_bypass_and_mutation() {
    for options in [
        ConstantSliceBoundsOptionsV1 {
            bypass: true,
            ..Default::default()
        },
        ConstantSliceBoundsOptionsV1 {
            mutate_slice: true,
            ..Default::default()
        },
        ConstantSliceBoundsOptionsV1 {
            duplicate_length: true,
            ..Default::default()
        },
        ConstantSliceBoundsOptionsV1 {
            duplicate_condition: true,
            ..Default::default()
        },
    ] {
        assert!(constant_slice_checks_v1(&constant_slice_bounds_function_v1(options)).is_err());
    }
}

#[test]
fn constant_slice_literal_guard_requires_exact_unsigned_64_bit_scalar() {
    for (literal_type, literal_bytes) in [
        (I32_TYPE, 4),
        (F32_TYPE, 4),
        (SCALAR_TYPE, 4),
        (U128_TYPE, 16),
        (U64_TYPE, 4),
    ] {
        let function = constant_slice_bounds_function_v1(ConstantSliceBoundsOptionsV1 {
            literal_type,
            literal_bytes,
            ..Default::default()
        });
        assert!(constant_slice_checks_v1(&function).is_err());
    }
}
