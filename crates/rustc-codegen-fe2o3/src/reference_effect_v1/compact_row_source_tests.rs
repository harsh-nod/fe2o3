//! Synthetic IR consumer fixtures, not source-authentication receipts.
use super::*;
use crate::collector::exclusive_reference_v1::ExclusiveOutputSourceV1;

fn place(local: u32) -> ReferencePlaceV1 {
    ReferencePlaceV1 {
        local,
        projection: Box::default(),
    }
}
fn operand(local: u32) -> ReferenceOperandV1 {
    ReferenceOperandV1::Copy(place(local))
}
fn integer(bits: u128) -> ReferenceOperandV1 {
    ReferenceOperandV1::Constant(ReferenceConstantV1::Scalar {
        scalar: ReferenceScalarTypeV1::Usize,
        bits,
    })
}

pub(crate) fn fixture() -> ReferenceEffectIrV1 {
    let mut assignments = vec![];
    for (local, argument) in [(4, 1), (5, 2)] {
        assignments.push(ReferenceAssignmentV1 {
            statement: assignments.len() as u32,
            destination: place(local),
            value: ReferenceValueV1::InputLength {
                reference_argument: argument,
            },
        });
    }
    for (local, operation, lhs, rhs) in [
        (6, ReferenceBinaryOpV1::Divide, operand(1), integer(64)),
        (7, ReferenceBinaryOpV1::Remainder, operand(1), integer(64)),
        (8, ReferenceBinaryOpV1::Multiply, operand(6), integer(16)),
        (9, ReferenceBinaryOpV1::Add, operand(8), operand(7)),
        (10, ReferenceBinaryOpV1::Equal, operand(4), integer(256)),
        (11, ReferenceBinaryOpV1::Equal, operand(5), integer(256)),
        (12, ReferenceBinaryOpV1::LessThan, operand(6), integer(16)),
        (13, ReferenceBinaryOpV1::LessThan, operand(7), integer(16)),
        (14, ReferenceBinaryOpV1::LessThan, operand(9), operand(4)),
        (15, ReferenceBinaryOpV1::LessThan, operand(9), operand(5)),
    ] {
        assignments.push(ReferenceAssignmentV1 {
            statement: assignments.len() as u32,
            destination: place(local),
            value: ReferenceValueV1::Binary {
                operation,
                lhs,
                rhs,
                checked: false,
            },
        });
    }
    let mut blocks = (0..4)
        .map(|block| ReferenceBlockV1 {
            block,
            assignments: Box::default(),
            terminator: ReferenceTerminatorV1::Switch {
                discriminant: operand(10 + block),
                values: vec![(0, 7)].into_boxed_slice(),
                otherwise: block + 1,
            },
        })
        .collect::<Vec<_>>();
    blocks[0].assignments = assignments.into_boxed_slice();
    for block in 4..6 {
        blocks.push(ReferenceBlockV1 {
            block,
            assignments: Box::default(),
            terminator: ReferenceTerminatorV1::Assert {
                condition: operand(10 + block),
                expected: true,
                success: block + 1,
                bounds_check: Some(ReferenceBoundsCheckV1 {
                    index: operand(9),
                    length: operand(block),
                }),
            },
        });
    }
    let indexed = |local| ReferencePlaceV1 {
        local,
        projection: vec![
            ReferencePlaceProjectionV1::Dereference,
            ReferencePlaceProjectionV1::Index(9),
        ]
        .into_boxed_slice(),
    };
    blocks.push(ReferenceBlockV1 {
        block: 6,
        assignments: vec![ReferenceAssignmentV1 {
            statement: 0,
            destination: indexed(3),
            value: ReferenceValueV1::Use(ReferenceOperandV1::Copy(indexed(2))),
        }]
        .into_boxed_slice(),
        terminator: ReferenceTerminatorV1::Goto { target: 7 },
    });
    blocks.push(ReferenceBlockV1 {
        block: 7,
        assignments: Box::default(),
        terminator: ReferenceTerminatorV1::Return,
    });
    ReferenceEffectIrV1 {
        argument_count: 3,
        local_count: 16,
        relations: vec![
            ReferenceArgumentRelationV1::PointCoordinate {
                reference_argument: 0,
                axis: 0,
            },
            ReferenceArgumentRelationV1::SharedSliceInput {
                argument: 0,
                element: ReferenceScalarTypeV1::U32,
            },
            ReferenceArgumentRelationV1::ExclusivePrimitiveOutputSlice1D {
                argument: 1,
                element: ReferenceScalarTypeV1::U32,
                source: ExclusiveOutputSourceV1::test_only_v1(53, 1, ReferenceScalarTypeV1::U32),
            },
        ]
        .into_boxed_slice(),
        blocks: blocks.into_boxed_slice(),
        loop_summaries: Box::default(),
        observable_output_effects: Box::default(),
    }
}

pub(crate) fn writes(ir: &ReferenceEffectIrV1) -> Vec<ReferenceOutputWriteV1> {
    ir.observable_output_writes_v1().unwrap()
}

#[test]
fn compact_row_source_retains_guards_bounds_and_exact_input_index() {
    let ir = fixture();
    let original = ir.clone();
    let outputs = writes(&ir);
    let [output] = outputs.as_slice() else {
        panic!("one output")
    };
    let ReferenceOutputCoordinateV1::CompactRowsUsize1D(mapping) = output.coordinate else {
        panic!("exact compact row")
    };
    assert_eq!(
        (mapping.axis(), mapping.divisor(), mapping.stride()),
        (0, 64, 16)
    );
    assert_eq!(output.guard.clauses.len(), 1);
    assert_eq!(output.guard.clauses[0].atoms.len(), 4);
    let ReferenceEffectExpressionV1::InputLoad {
        reference_argument: 1,
        index,
    } = &output.rhs
    else {
        panic!("original input")
    };
    assert_eq!(
        **index,
        mapping
            .expression(&mut ReferenceSymbolicWorkBudgetV2::default())
            .unwrap()
    );
    assert_eq!(
        ir.resolved_bounds_checks_with_budget_v1(&mut ReferenceSymbolicWorkBudgetV2::default())
            .unwrap()
            .len(),
        2
    );
    assert_eq!(ir, original);
}

#[test]
fn compact_row_source_never_remaps_invocation_disjoint_carrier() {
    let mut ir = fixture();
    ir.relations[2] = ReferenceArgumentRelationV1::InvocationDisjointOutputSlice1D {
        argument: 1,
        element: ReferenceScalarTypeV1::U32,
    };
    assert!(
        ir.observable_output_writes_v1()
            .unwrap_err()
            .to_string()
            .contains("exact source point coordinate")
    );
}

#[test]
fn compact_row_source_rejects_symbolic_assert_even_after_origin_normalization() {
    let mut ir = fixture();
    ir.blocks[3].terminator = ReferenceTerminatorV1::Assert {
        condition: operand(13),
        expected: true,
        success: 4,
        bounds_check: None,
    };
    let paths = reference_block_path_predicates_v1(&ir).unwrap();
    let resolver = ReferenceExpressionResolverV1::new(&ir).unwrap();
    let expression = resolver.resolve_local_v1(9).unwrap();
    assert!(
        CompactRowsUsize1D::from_expression(
            &expression,
            &paths[6],
            &mut ReferenceSymbolicWorkBudgetV2::default()
        )
        .unwrap()
        .is_some(),
        "normalization intentionally erases the origin"
    );
    assert!(
        ir.observable_output_writes_v1()
            .unwrap_err()
            .to_string()
            .contains("raw nonbounds assertion")
    );
}

#[test]
fn compact_row_source_audits_raw_asserts_off_write_path_and_both_polarities() {
    for (bits, expected, accepts) in [
        (0, false, true),
        (1, true, true),
        (0, true, false),
        (1, false, false),
        (2, true, false),
    ] {
        let mut ir = fixture();
        let mut blocks = ir.blocks.to_vec();
        blocks.push(ReferenceBlockV1 {
            block: 8,
            assignments: Box::default(),
            terminator: ReferenceTerminatorV1::Assert {
                condition: ReferenceOperandV1::Constant(ReferenceConstantV1::Scalar {
                    scalar: ReferenceScalarTypeV1::Bool,
                    bits,
                }),
                expected,
                success: 7,
                bounds_check: None,
            },
        });
        ir.blocks = blocks.into_boxed_slice();
        let result = ir.observable_output_writes_v1();
        if accepts {
            assert_eq!(result.unwrap().len(), 1);
        } else {
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains("raw nonbounds assertion")
            );
        }
    }
}

#[test]
fn compact_row_source_missing_or_reversed_lane_guard_rejects() {
    for reversed in [false, true] {
        let mut ir = fixture();
        ir.blocks[3].terminator = if reversed {
            ReferenceTerminatorV1::Switch {
                discriminant: operand(13),
                values: vec![(0, 4)].into_boxed_slice(),
                otherwise: 7,
            }
        } else {
            ReferenceTerminatorV1::Goto { target: 4 }
        };
        assert!(
            ir.observable_output_writes_v1()
                .unwrap_err()
                .to_string()
                .contains("guarded compact row")
        );
    }
}

#[test]
fn compact_row_source_duplicate_definition_and_output_escape_reject() {
    for escape in [false, true] {
        let mut ir = fixture();
        let mut assignments = ir.blocks[0].assignments.to_vec();
        assignments.push(ReferenceAssignmentV1 {
            statement: assignments.len() as u32,
            destination: place(if escape { 2 } else { 9 }),
            value: ReferenceValueV1::Use(integer(0)),
        });
        ir.blocks[0].assignments = assignments.into_boxed_slice();
        let error = ir.observable_output_writes_v1().unwrap_err().to_string();
        assert!(
            error.contains(if escape {
                "no output reads or escapes"
            } else {
                "multiple"
            }),
            "{error}"
        );
    }
}

#[test]
fn compact_row_source_budget_is_shared_not_refilled_after_assert_audit() {
    let ir = fixture();
    let resolver = ReferenceExpressionResolverV1::new(&ir).unwrap();
    let expression = resolver.resolve_local_v1(9).unwrap();
    let guard = reference_block_path_predicates_v1(&ir).unwrap().remove(6);
    let mut work = ReferenceSymbolicWorkBudgetV2::default();
    work.charge_v2(MAX_REFERENCE_SYMBOLIC_WORK_NODES_V2 - ir.blocks.len() - ir.relations.len())
        .unwrap();
    let error = output_slice_v1::coordinate(
        &ir,
        1,
        ReferenceOutputCoordinateV1::Dynamic(expression),
        &guard,
        &resolver,
        &mut work,
    )
    .unwrap_err();
    assert!(error.to_string().contains("cumulative expression work"));
    assert!(work.charge_v2(1).is_err());
}

#[test]
fn compact_row_source_digest_tag4_binds_map_and_original_guard() {
    let ir = fixture();
    let output = writes(&ir).remove(0);
    let hash = |value: &ReferenceOutputWriteV1| {
        let mut h = Sha256::new();
        digest_output_effect_v1(&mut h, value);
        h.finalize()
    };
    let original = hash(&output);
    let ReferenceOutputCoordinateV1::CompactRowsUsize1D(mapping) = output.coordinate else {
        panic!()
    };
    let mut changed = output.clone();
    changed.coordinate = ReferenceOutputCoordinateV1::Dynamic(
        mapping
            .expression(&mut ReferenceSymbolicWorkBudgetV2::default())
            .unwrap(),
    );
    assert_ne!(original, hash(&changed));
    changed = output.clone();
    changed.guard = ReferencePathPredicateV1::unconditional_v1();
    assert_ne!(original, hash(&changed));
}

#[test]
fn compact_row_source_assert_resolver_cannot_be_replaced_by_an_equal_body_owner() {
    let ir = fixture();
    let other = ir.clone();
    let resolver = ReferenceExpressionResolverV1::new(&other).unwrap();
    let expression = resolver.resolve_local_v1(9).unwrap();
    let guard = reference_block_path_predicates_v1(&ir).unwrap().remove(6);
    let error = output_slice_v1::coordinate(
        &ir,
        1,
        ReferenceOutputCoordinateV1::Dynamic(expression),
        &guard,
        &resolver,
        &mut ReferenceSymbolicWorkBudgetV2::default(),
    )
    .unwrap_err();
    assert_eq!(
        error.to_string(),
        "compact row assertion resolver belongs to another source owner"
    );
}
