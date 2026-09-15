use super::*;

fn constant(scalar: ReferenceScalarTypeV1, bits: u128) -> ReferenceOperandV1 {
    ReferenceOperandV1::Constant(ReferenceConstantV1::Scalar { scalar, bits })
}

fn field(field: u32) -> ReferenceOperandV1 {
    ReferenceOperandV1::Copy(ReferencePlaceV1 {
        local: 2,
        projection: vec![ReferencePlaceProjectionV1::Field(field)].into_boxed_slice(),
    })
}

fn fixture(
    operation: ReferenceBinaryOpV1,
    lhs: ReferenceOperandV1,
    rhs: ReferenceOperandV1,
) -> ReferenceEffectIrV1 {
    ReferenceEffectIrV1 {
        argument_count: 1,
        local_count: 3,
        relations: vec![ReferenceArgumentRelationV1::PointCoordinate {
            reference_argument: 0,
            axis: 0,
        }]
        .into_boxed_slice(),
        blocks: vec![ReferenceBlockV1 {
            block: 0,
            assignments: vec![ReferenceAssignmentV1 {
                statement: 0,
                destination: ReferencePlaceV1 {
                    local: 2,
                    projection: Box::default(),
                },
                value: ReferenceValueV1::Binary {
                    operation,
                    lhs,
                    rhs,
                    checked: true,
                },
            }]
            .into_boxed_slice(),
            terminator: ReferenceTerminatorV1::Return,
        }]
        .into_boxed_slice(),
        loop_summaries: Box::default(),
        observable_output_effects: Box::default(),
    }
}

fn resolve(
    ir: &ReferenceEffectIrV1,
    operand: &ReferenceOperandV1,
    used: &mut usize,
) -> Result<ReferenceEffectExpressionV1, ReferenceBindingErrorV1> {
    ReferenceExpressionResolverV1::new(ir)?.resolve_operand_inner_v1(
        operand,
        &mut BTreeSet::new(),
        used,
        0,
    )
}

fn bits(expression: ReferenceEffectExpressionV1) -> (ReferenceScalarTypeV1, u128) {
    let ReferenceEffectExpressionV1::Constant(ReferenceConstantV1::Scalar { scalar, bits }) =
        expression
    else {
        panic!("exact constant projection");
    };
    (scalar, bits)
}

#[test]
fn checked_constant_pair_projects_value_and_real_overflow_for_every_unsigned_width() {
    for scalar in [
        ReferenceScalarTypeV1::U8,
        ReferenceScalarTypeV1::U16,
        ReferenceScalarTypeV1::U32,
        ReferenceScalarTypeV1::U64,
        ReferenceScalarTypeV1::Usize,
    ] {
        let mask = reference_scalar_mask_v2(scalar).unwrap();
        for (operation, lhs, rhs, value, overflow) in [
            (ReferenceBinaryOpV1::Add, 16, 16, 32, false),
            (ReferenceBinaryOpV1::Multiply, 4, 4, 16, false),
            (ReferenceBinaryOpV1::Subtract, 16, 4, 12, false),
            (ReferenceBinaryOpV1::Add, mask, 1, 0, true),
            (ReferenceBinaryOpV1::Subtract, 0, 1, mask, true),
            (ReferenceBinaryOpV1::Multiply, mask, 2, mask - 1, true),
        ] {
            let ir = fixture(operation, constant(scalar, lhs), constant(scalar, rhs));
            let before = ir.canonical_sha256_v1();
            for moved in [false, true] {
                let operand = |index| match field(index) {
                    ReferenceOperandV1::Copy(place) if moved => ReferenceOperandV1::Move(place),
                    operand => operand,
                };
                assert_eq!(
                    reference_constant_bits_v2(&resolve(&ir, &operand(0), &mut 0).unwrap())
                        .unwrap(),
                    (scalar, value)
                );
                assert_eq!(
                    bits(resolve(&ir, &operand(1), &mut 0).unwrap()),
                    (ReferenceScalarTypeV1::Bool, u128::from(overflow))
                );
            }
            assert_eq!(ir.canonical_sha256_v1(), before);
        }
    }
}

#[test]
fn checked_constant_pair_does_not_assume_unknown_overflow() {
    let ir = fixture(
        ReferenceBinaryOpV1::Multiply,
        ReferenceOperandV1::Copy(ReferencePlaceV1 {
            local: 1,
            projection: Box::default(),
        }),
        constant(ReferenceScalarTypeV1::Usize, 16),
    );
    assert!(matches!(
        resolve(&ir, &field(0), &mut 0).unwrap(),
        ReferenceEffectExpressionV1::Binary { checked: true, .. }
    ));
    assert!(resolve(&ir, &field(1), &mut 0).is_err());
    assert!(resolve(&ir, &field(2), &mut 0).is_err());
}

#[test]
fn checked_constant_pair_rejects_wrong_types_operations_and_unchecked_definitions() {
    for (lhs, rhs, operation) in [
        (
            constant(ReferenceScalarTypeV1::U8, 256),
            constant(ReferenceScalarTypeV1::U8, 1),
            ReferenceBinaryOpV1::Add,
        ),
        (
            constant(ReferenceScalarTypeV1::U8, 1),
            constant(ReferenceScalarTypeV1::U32, 1),
            ReferenceBinaryOpV1::Add,
        ),
        (
            constant(ReferenceScalarTypeV1::I32, 1),
            constant(ReferenceScalarTypeV1::I32, 1),
            ReferenceBinaryOpV1::Add,
        ),
        (
            constant(ReferenceScalarTypeV1::F32, 1),
            constant(ReferenceScalarTypeV1::F32, 1),
            ReferenceBinaryOpV1::Add,
        ),
        (
            constant(ReferenceScalarTypeV1::Bool, 1),
            constant(ReferenceScalarTypeV1::Bool, 1),
            ReferenceBinaryOpV1::Add,
        ),
        (
            constant(ReferenceScalarTypeV1::Usize, 1),
            constant(ReferenceScalarTypeV1::Usize, 0),
            ReferenceBinaryOpV1::Divide,
        ),
    ] {
        assert!(resolve(&fixture(operation, lhs, rhs), &field(1), &mut 0).is_err());
    }
    let mut ir = fixture(
        ReferenceBinaryOpV1::Add,
        constant(ReferenceScalarTypeV1::Usize, 1),
        constant(ReferenceScalarTypeV1::Usize, 1),
    );
    let ReferenceValueV1::Binary { checked, .. } = &mut ir.blocks[0].assignments[0].value else {
        unreachable!()
    };
    *checked = false;
    for index in [0, 1] {
        assert!(resolve(&ir, &field(index), &mut 0).is_err());
    }
}

#[test]
fn checked_constant_pair_keeps_ambiguous_definitions_and_cycles_closed() {
    let mut ir = fixture(
        ReferenceBinaryOpV1::Add,
        constant(ReferenceScalarTypeV1::Usize, 1),
        constant(ReferenceScalarTypeV1::Usize, 1),
    );
    let mut repeated = ir.blocks[0].assignments[0].clone();
    repeated.statement = 1;
    ir.blocks[0].assignments =
        vec![ir.blocks[0].assignments[0].clone(), repeated].into_boxed_slice();
    for index in [0, 1] {
        let error = resolve(&ir, &field(index), &mut 0).unwrap_err();
        assert!(error.to_string().contains("has no unique definition"));
    }
    let cyclic = fixture(
        ReferenceBinaryOpV1::Add,
        field(0),
        constant(ReferenceScalarTypeV1::Usize, 1),
    );
    assert!(resolve(&cyclic, &field(1), &mut 0).is_err());
}

#[test]
fn checked_constant_pair_spends_existing_budget_and_preserves_failed_assertion() {
    let mut ir = fixture(
        ReferenceBinaryOpV1::Add,
        constant(ReferenceScalarTypeV1::U8, 255),
        constant(ReferenceScalarTypeV1::U8, 1),
    );
    let mut used = MAX_REFERENCE_EXPRESSION_NODES_V1;
    assert!(resolve(&ir, &field(1), &mut used).is_err());
    assert!(used > MAX_REFERENCE_EXPRESSION_NODES_V1);
    ir.blocks[0].terminator = ReferenceTerminatorV1::Assert {
        condition: field(1),
        expected: false,
        success: 1,
        bounds_check: None,
    };
    ir.blocks = vec![
        ir.blocks[0].clone(),
        ReferenceBlockV1 {
            block: 1,
            assignments: Box::default(),
            terminator: ReferenceTerminatorV1::Return,
        },
    ]
    .into_boxed_slice();
    let paths = reference_block_path_predicates_v1(&ir).unwrap();
    assert!(
        paths[1].is_unreachable_v1(),
        "a constant overflow cannot enter the success path"
    );
}

#[test]
fn constant_assertions_keep_only_the_reachable_success_edge() {
    let ir = fixture(
        ReferenceBinaryOpV1::Multiply,
        constant(ReferenceScalarTypeV1::Usize, 16),
        constant(ReferenceScalarTypeV1::Usize, 16),
    );
    let resolver = ReferenceExpressionResolverV1::new(&ir).unwrap();
    for bits in [0, 1] {
        for expected in [false, true] {
            let edges = reference_guarded_edges_v1(
                &ReferenceTerminatorV1::Assert {
                    condition: constant(ReferenceScalarTypeV1::Bool, bits),
                    expected,
                    success: 1,
                    bounds_check: None,
                },
                &resolver,
                &mut ReferenceSymbolicWorkBudgetV2::default(),
            )
            .unwrap();
            assert_eq!(
                edges,
                if (bits == 1) == expected {
                    vec![(1, None)]
                } else {
                    vec![]
                }
            );
        }
    }
}

#[test]
fn unresolved_assertions_do_not_become_constant_facts() {
    let ir = fixture(
        ReferenceBinaryOpV1::Add,
        constant(ReferenceScalarTypeV1::Usize, 1),
        constant(ReferenceScalarTypeV1::Usize, 1),
    );
    let resolver = ReferenceExpressionResolverV1::new(&ir).unwrap();
    for condition in [
        constant(ReferenceScalarTypeV1::Bool, 2),
        constant(ReferenceScalarTypeV1::Usize, 1),
        ReferenceOperandV1::Copy(ReferencePlaceV1 {
            local: 1,
            projection: Box::default(),
        }),
    ] {
        let edges = reference_guarded_edges_v1(
            &ReferenceTerminatorV1::Assert {
                condition,
                expected: true,
                success: 1,
                bounds_check: None,
            },
            &resolver,
            &mut ReferenceSymbolicWorkBudgetV2::default(),
        )
        .unwrap();
        assert!(matches!(
            edges.as_slice(),
            [(1, Some(ReferenceGuardAtomV1::Assert { .. }))]
        ));
    }
}

#[test]
fn bounds_assertions_are_not_assumptions_and_constant_folding_spends_shared_work() {
    let ir = fixture(
        ReferenceBinaryOpV1::Add,
        constant(ReferenceScalarTypeV1::Usize, 1),
        constant(ReferenceScalarTypeV1::Usize, 1),
    );
    let resolver = ReferenceExpressionResolverV1::new(&ir).unwrap();
    let mut assertion = ReferenceTerminatorV1::Assert {
        condition: constant(ReferenceScalarTypeV1::Bool, 0),
        expected: true,
        success: 1,
        bounds_check: Some(ReferenceBoundsCheckV1 {
            index: constant(ReferenceScalarTypeV1::Usize, 9),
            length: constant(ReferenceScalarTypeV1::Usize, 8),
        }),
    };
    let mut budget = ReferenceSymbolicWorkBudgetV2::default();
    assert_eq!(
        reference_guarded_edges_v1(&assertion, &resolver, &mut budget).unwrap(),
        vec![(1, None)]
    );
    let ReferenceTerminatorV1::Assert { bounds_check, .. } = &mut assertion else {
        unreachable!()
    };
    *bounds_check = None;
    budget
        .charge_v2(MAX_REFERENCE_SYMBOLIC_WORK_NODES_V2)
        .unwrap();
    assert!(reference_guarded_edges_v1(&assertion, &resolver, &mut budget).is_err());
}
