use super::*;

fn changed_owner(
    change: impl FnOnce(Vec<SemanticBasicBlockV1>) -> Vec<SemanticBasicBlockV1>,
) -> ProductionPreRankedKirOwnerV1 {
    array_owner_with_cfg(ArrayCase::ValueRead { local_index: true }, false, change).unwrap()
}
fn changed_ssa(
    change: impl FnOnce(
        &mut Vec<SemanticTypeDeclV1>,
        &mut Vec<SemanticLocalDeclV1>,
        &mut Vec<SemanticBasicBlockV1>,
    ),
) -> Result<ProductionSemanticSsaOwnerV1, fe2o3_mir_model::semantic_mir_v1::SemanticMirErrorV1> {
    let seed = array_owner(ArrayCase::ValueRead { local_index: true });
    let semantic = seed.semantic_ssa().source_semantic();
    let original = &semantic.functions()[0];
    let mut types = semantic.types().to_vec();
    let mut locals = original.locals().to_vec();
    let mut blocks = original.blocks().to_vec();
    change(&mut types, &mut locals, &mut blocks);
    let function = SemanticFunctionDeclV1::new(
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
    .with_kernel_entry(original.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types,
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![ARRAY_ROOT],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())?;
    let source =
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    Ok(
        ProductionSemanticSsaOwnerV1::try_new(source, ProductionSemanticSsaLimitsV1::default())
            .unwrap(),
    )
}
fn original(
    owner: &ProductionSemanticSsaOwnerV1,
    block: u32,
    statement: u32,
    role: Role,
    array: u32,
    index: u32,
    limit: usize,
) -> (
    Result<Option<u64>, fe2o3_pliron::ProductionSemanticSsaOccurrenceErrorV1>,
    usize,
) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let result = owner.original_unsigned_literal_index_v1(
        ARRAY_ROOT,
        Site::Statement {
            block: SsaBlockIdV1::new(block),
            statement,
        },
        role,
        SemanticLocalIdV1::from_index(array),
        SemanticLocalIdV1::from_index(index),
        &mut budget,
    );
    assert_eq!((budget.storage(), budget.peak_storage()), (FLOOR, FLOOR));
    (result, budget.work())
}
fn direct(owner: &ProductionSemanticSsaOwnerV1, block: u32, statement: u32) -> Option<u64> {
    original(owner, block, statement, Role::RvalueOperand(0), 1, 2, WORK)
        .0
        .unwrap()
}
fn replace_block(blocks: &mut Vec<SemanticBasicBlockV1>, statements: Vec<SemanticStatementV1>) {
    *blocks = vec![block(211, statements, SemanticTerminatorKindV1::Return)];
}
fn literal(value: u128) -> SemanticStatementV1 {
    assignment(
        2,
        ARRAY_SCALAR,
        SemanticRvalueKindV1::Use(constant(ARRAY_SCALAR, value, 4)),
    )
}
fn go(target: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, target))
}

#[test]
fn original_literal_zero_and_two_are_supported_absence_not_retained_memory() {
    for value in [0, 2] {
        let owner = changed_owner(|blocks| {
            let mut rows = blocks[0].statements().to_vec();
            rows[0] = literal(value);
            vec![block(211, rows, SemanticTerminatorKindV1::Return)]
        });
        assert_eq!(direct(owner.semantic_ssa(), 0, 2), Some(value as u64));
        assert_eq!(query(&owner, 0, 2, Role::RvalueOperand(0)), Ok(false));
        assert!(!owner.correspondence.private_arrays.active);
        assert!(owner.correspondence.private_arrays.slots.is_empty());
        assert!(owner.correspondence.private_arrays.effects.is_empty());
        let moved = Box::new(owner);
        assert_eq!(query(&moved, 0, 2, Role::RvalueOperand(0)), Ok(false));
    }
}

#[test]
fn original_literal_cross_block_definition_matches_exact_use() {
    let owner = changed_owner(|blocks| {
        let rows = blocks[0].statements();
        vec![
            block(211, vec![literal(2), rows[1].clone()], go(1)),
            block(212, vec![rows[2].clone()], SemanticTerminatorKindV1::Return),
        ]
    });
    assert_eq!(direct(owner.semantic_ssa(), 1, 0), Some(2));
    assert_eq!(query(&owner, 1, 0, Role::RvalueOperand(0)), Ok(false));
    assert_eq!(direct(owner.semantic_ssa(), 0, 0), None);
}

#[test]
fn original_literal_u64_width_is_taken_from_actual_original_type() {
    let owner = changed_ssa(|types, locals, blocks| {
        let ty = SemanticTypeIdV1::from_index(3);
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([203; 32]),
            SemanticLayoutIdentityV1::from_sha256([203; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(8),
                8,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::integer(false, 64, 8),
                    SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 64,
            }),
        ));
        let old = &locals[2];
        locals[2] = SemanticLocalDeclV1::new(old.identity(), ty, old.role(), old.source());
        let mut rows = blocks[0].statements().to_vec();
        rows[0] = assignment(2, ty, SemanticRvalueKindV1::Use(constant(ty, 2, 8)));
        replace_block(blocks, rows);
    })
    .unwrap();
    assert_eq!(direct(&owner, 0, 2), Some(2));
}

#[test]
fn original_literal_wrong_site_role_array_and_index_are_not_interchangeable() {
    let owner = array_owner(ArrayCase::ValueRead { local_index: true });
    for (statement, role, array, index) in [
        (0, Role::RvalueOperand(0), 1, 2),
        (1, Role::RvalueOperand(0), 1, 2),
        (2, Role::RvalueOperand(1), 1, 2),
        (2, Role::Destination, 1, 2),
        (2, Role::RvalueOperand(0), 3, 2),
        (2, Role::RvalueOperand(0), 1, 3),
    ] {
        assert_eq!(
            original(owner.semantic_ssa(), 0, statement, role, array, index, WORK)
                .0
                .unwrap(),
            None
        );
    }
    assert!(query(&owner, 0, 2, Role::RvalueOperand(1)).is_err());
}

#[test]
fn original_literal_second_operand_cannot_reuse_first_operand_event() {
    let owner = changed_ssa(|_, _, blocks| {
        let mut rows = blocks[0].statements().to_vec();
        rows[2] = assignment(
            3,
            ARRAY_SCALAR,
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::Add,
                left: constant(ARRAY_SCALAR, 1, 4),
                right: SemanticOperandV1::Copy(array_place(true)),
            },
        );
        replace_block(blocks, rows);
    })
    .unwrap();
    assert_eq!(direct(&owner, 0, 2), None);
    assert_eq!(
        original(&owner, 0, 2, Role::RvalueOperand(1), 1, 2, WORK)
            .0
            .unwrap(),
        None
    );
}

#[test]
fn original_literal_same_block_redefinition_is_not_a_site_witness() {
    let owner = changed_ssa(|_, _, blocks| {
        let rows = blocks[0].statements().to_vec();
        replace_block(
            blocks,
            vec![
                literal(0),
                rows[1].clone(),
                rows[2].clone(),
                literal(2),
                rows[2].clone(),
            ],
        );
    })
    .unwrap();
    assert_eq!(direct(&owner, 0, 2), None);
    assert_eq!(direct(&owner, 0, 4), None);
}

#[test]
fn original_literal_dynamic_and_cast_definitions_remain_unsupported() {
    for cast in [false, true] {
        let owner = changed_ssa(|_, _, blocks| {
            let mut rows = blocks[0].statements().to_vec();
            rows[0] = assignment(
                2,
                ARRAY_SCALAR,
                if cast {
                    SemanticRvalueKindV1::Cast {
                        kind: SemanticCastKindV1::Integer,
                        operand: constant(ARRAY_SCALAR, 2, 4),
                    }
                } else {
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::Add,
                        left: constant(ARRAY_SCALAR, 1, 4),
                        right: constant(ARRAY_SCALAR, 1, 4),
                    }
                },
            );
            replace_block(blocks, rows);
        })
        .unwrap();
        assert_eq!(direct(&owner, 0, 2), None);
    }
    // The original SSA owner can represent the expression above, but complete
    // lowering must retain its exact Rust bounds guard before any array query.
    let result = array_owner_with_cfg(
        ArrayCase::ValueRead { local_index: true },
        false,
        |blocks| {
            let mut rows = blocks[0].statements().to_vec();
            rows[0] = assignment(
                2,
                ARRAY_SCALAR,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::Add,
                    left: constant(ARRAY_SCALAR, 1, 4),
                    right: constant(ARRAY_SCALAR, 1, 4),
                },
            );
            vec![block(211, rows, SemanticTerminatorKindV1::Return)]
        },
    );
    assert!(matches!(
        result,
        Err(ProductionPreRankedKirErrorV1::Lowering(
            ProductionSemanticKirErrorV1::Unsupported {
                function: 0,
                block: Some(0),
                statement: Some(2),
                detail: "dynamic local array access lacks its retained exact Rust bounds guard",
            }
        ))
    ));
}

#[test]
fn original_literal_phi_does_not_become_a_constant_witness() {
    let owner = changed_ssa(|_, _, blocks| {
        let rows = blocks[0].statements().to_vec();
        *blocks = vec![
            block(
                211,
                vec![rows[1].clone()],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: constant(ARRAY_SCALAR, 0, 4),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            edge(SemanticEdgeRoleV1::SwitchValue, 1),
                        )],
                        edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                    )
                    .unwrap(),
                },
            ),
            block(212, vec![literal(0)], go(3)),
            block(213, vec![literal(2)], go(3)),
            block(214, vec![rows[2].clone()], SemanticTerminatorKindV1::Return),
        ];
    })
    .unwrap();
    assert_eq!(direct(&owner, 3, 0), None);
}

fn with_borrow(definition_prefix: bool, alias_index: bool) -> ProductionSemanticSsaOwnerV1 {
    changed_ssa(|types, locals, blocks| {
        let ty = SemanticTypeIdV1::from_index(3);
        types.push(
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([203; 32]),
                SemanticLayoutIdentityV1::from_sha256([203; 32]),
                SemanticTypeLayoutV1::new_with_backend_repr(
                    Some(8),
                    8,
                    SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                        SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
                    )),
                    false,
                )
                .unwrap(),
                SemanticTypeShapeV1::Pointer(
                    SemanticPointerTypeV1::new_with_kind(
                        ARRAY_SCALAR,
                        SemanticPointerKindV1::Reference,
                        SemanticMutabilityV1::Immutable,
                        0,
                        64,
                        SemanticPointerMetadataV1::None,
                    )
                    .unwrap(),
                ),
            )
            .with_rustc_abi_properties(
                SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                    Some(
                        SemanticAbiPointeeInfoV1::new(
                            SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                            4,
                            4,
                        )
                        .unwrap(),
                    ),
                    None,
                ),
            ),
        );
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([224; 32]),
            ty,
            SemanticLocalRoleV1::Temporary,
            SemanticSourceProvenanceV1::unavailable(),
        ));
        let rows = blocks[0].statements().to_vec();
        let borrow = assignment(
            4,
            ty,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: place(if alias_index { 2 } else { 3 }, ARRAY_SCALAR),
            },
        );
        if definition_prefix {
            *blocks = vec![
                block(
                    211,
                    vec![
                        assignment(
                            3,
                            ARRAY_SCALAR,
                            SemanticRvalueKindV1::Use(constant(ARRAY_SCALAR, 1, 4)),
                        ),
                        borrow,
                        rows[0].clone(),
                        rows[1].clone(),
                    ],
                    go(1),
                ),
                block(212, vec![rows[2].clone()], SemanticTerminatorKindV1::Return),
            ];
        } else {
            replace_block(
                blocks,
                vec![
                    rows[0].clone(),
                    rows[1].clone(),
                    assignment(
                        3,
                        ARRAY_SCALAR,
                        SemanticRvalueKindV1::Use(constant(ARRAY_SCALAR, 1, 4)),
                    ),
                    borrow,
                    rows[2].clone(),
                ],
            );
        }
    })
    .unwrap()
}

#[test]
fn original_literal_use_and_definition_borrow_prefixes_refuse_without_elision_guess() {
    assert_eq!(direct(&with_borrow(false, false), 0, 4), None);
    assert_eq!(direct(&with_borrow(true, false), 1, 0), None);
}

#[test]
fn original_literal_aliased_index_remains_unsupported() {
    assert_eq!(direct(&with_borrow(false, true), 0, 4), None);
}

#[test]
fn original_literal_signed_index_is_refused_by_original_semantic_admission() {
    let result = changed_ssa(|types, _, _| {
        types[1] = SemanticTypeDeclV1::new(
            types[1].identity(),
            types[1].layout_identity(),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(4),
                4,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::integer(true, 32, 4),
                    SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: true,
                bits: 32,
            }),
        );
    });
    assert!(matches!(
        result,
        Err(
            fe2o3_mir_model::semantic_mir_v1::SemanticMirErrorV1::InvalidTypeOperation {
                operation: fe2o3_mir_model::semantic_mir_v1::SemanticTypeOperationV1::Projection,
                ..
            }
        )
    ));
}

#[test]
fn original_literal_owner_root_and_body_custody_is_not_bypassed() {
    let owner = array_owner(ArrayCase::ValueRead { local_index: true });
    for (root, body) in [(1, 0), (0, 1)] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR + retained(&owner)).unwrap();
        assert!(
            owner
                .materialized_private_array_constant_index(
                    SemanticFunctionIdV1::from_index(root),
                    SemanticFunctionIdV1::from_index(body),
                    Site::Statement {
                        block: SsaBlockIdV1::new(0),
                        statement: 2
                    },
                    Role::RvalueOperand(0),
                    &mut budget,
                )
                .is_err()
        );
    }
    let foreign = changed_owner(|blocks| {
        let mut rows = blocks[0].statements().to_vec();
        rows[0] = assignment(
            2,
            ARRAY_SCALAR,
            SemanticRvalueKindV1::Cast {
                kind: SemanticCastKindV1::Integer,
                operand: constant(ARRAY_SCALAR, 2, 4),
            },
        );
        vec![block(211, rows, SemanticTerminatorKindV1::Return)]
    });
    assert_eq!(direct(owner.semantic_ssa(), 0, 2), Some(0));
    assert_eq!(direct(foreign.semantic_ssa(), 0, 2), None);
}

#[test]
fn original_literal_work_exact_and_one_under_preserve_live_storage() {
    let owner = array_owner(ArrayCase::ValueRead { local_index: true });
    let (result, needed) = original(
        owner.semantic_ssa(),
        0,
        2,
        Role::RvalueOperand(0),
        1,
        2,
        WORK,
    );
    assert_eq!(result.unwrap(), Some(0));
    assert!(needed > 0);
    assert_eq!(
        original(
            owner.semantic_ssa(),
            0,
            2,
            Role::RvalueOperand(0),
            1,
            2,
            needed
        )
        .0
        .unwrap(),
        Some(0)
    );
    for limit in [0, needed - 1] {
        assert!(matches!(
            original(
                owner.semantic_ssa(),
                0,
                2,
                Role::RvalueOperand(0),
                1,
                2,
                limit
            )
            .0,
            Err(
                fe2o3_pliron::ProductionSemanticSsaOccurrenceErrorV1::Resource(
                    fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Work(_)
                )
            )
        ));
    }
}

#[test]
fn original_literal_out_of_bounds_still_fails_original_lowering() {
    let result = array_owner_with_cfg(
        ArrayCase::ValueRead { local_index: true },
        false,
        |blocks| {
            let mut rows = blocks[0].statements().to_vec();
            rows[0] = literal(8);
            vec![block(211, rows, SemanticTerminatorKindV1::Return)]
        },
    );
    assert!(result.is_err());
}
