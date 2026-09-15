use super::*;

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

fn assignment(statement: u32, local: u32, value: ReferenceValueV1) -> ReferenceAssignmentV1 {
    ReferenceAssignmentV1 {
        statement,
        destination: place(local),
        value,
    }
}

pub(crate) fn fixture() -> ReferenceEffectIrV1 {
    let mut assignments = Vec::new();
    for reference_argument in 1..=3 {
        assignments.push(assignment(
            assignments.len() as u32,
            reference_argument + 4,
            ReferenceValueV1::InputLength { reference_argument },
        ));
    }
    for (local, lhs, rhs, operation) in [
        (8, operand(5), integer(1024), ReferenceBinaryOpV1::Equal),
        (9, operand(6), integer(1024), ReferenceBinaryOpV1::Equal),
        (10, operand(7), integer(1024), ReferenceBinaryOpV1::Equal),
        (11, operand(1), integer(1024), ReferenceBinaryOpV1::LessThan),
        (12, operand(1), operand(5), ReferenceBinaryOpV1::LessThan),
        (13, operand(1), operand(6), ReferenceBinaryOpV1::LessThan),
        (14, operand(1), operand(7), ReferenceBinaryOpV1::LessThan),
    ] {
        assignments.push(assignment(
            assignments.len() as u32,
            local,
            ReferenceValueV1::Binary {
                operation,
                lhs,
                rhs,
                checked: false,
            },
        ));
    }
    let mut blocks = (0..4)
        .map(|block| ReferenceBlockV1 {
            block,
            assignments: Box::default(),
            terminator: ReferenceTerminatorV1::Switch {
                discriminant: operand(8 + block),
                values: vec![(0, 8)].into_boxed_slice(),
                otherwise: block + 1,
            },
        })
        .collect::<Vec<_>>();
    blocks[0].assignments = assignments.into_boxed_slice();
    for block in 4..7 {
        blocks.push(ReferenceBlockV1 {
            block,
            assignments: Box::default(),
            terminator: ReferenceTerminatorV1::Assert {
                condition: operand(block + 8),
                expected: true,
                success: block + 1,
                bounds_check: Some(ReferenceBoundsCheckV1 {
                    index: operand(1),
                    length: operand(block + 1),
                }),
            },
        });
    }
    let indexed = |local| ReferencePlaceV1 {
        local,
        projection: vec![
            ReferencePlaceProjectionV1::Dereference,
            ReferencePlaceProjectionV1::Index(1),
        ]
        .into_boxed_slice(),
    };
    blocks.push(ReferenceBlockV1 {
        block: 7,
        assignments: vec![
            assignment(
                0,
                15,
                ReferenceValueV1::Use(ReferenceOperandV1::Copy(indexed(2))),
            ),
            assignment(
                1,
                16,
                ReferenceValueV1::Use(ReferenceOperandV1::Copy(indexed(3))),
            ),
            ReferenceAssignmentV1 {
                statement: 2,
                destination: indexed(4),
                value: ReferenceValueV1::Binary {
                    operation: ReferenceBinaryOpV1::Add,
                    lhs: operand(15),
                    rhs: operand(16),
                    checked: false,
                },
            },
        ]
        .into_boxed_slice(),
        terminator: ReferenceTerminatorV1::Goto { target: 8 },
    });
    blocks.push(ReferenceBlockV1 {
        block: 8,
        assignments: Box::default(),
        terminator: ReferenceTerminatorV1::Return,
    });
    ReferenceEffectIrV1 {
        argument_count: 4,
        local_count: 18,
        relations: vec![
            ReferenceArgumentRelationV1::PointCoordinate {
                reference_argument: 0,
                axis: 0,
            },
            ReferenceArgumentRelationV1::SharedSliceInput {
                argument: 0,
                element: ReferenceScalarTypeV1::F32,
            },
            ReferenceArgumentRelationV1::SharedSliceInput {
                argument: 1,
                element: ReferenceScalarTypeV1::F32,
            },
            ReferenceArgumentRelationV1::InvocationDisjointOutputSlice1D {
                argument: 2,
                element: ReferenceScalarTypeV1::F32,
            },
        ]
        .into_boxed_slice(),
        blocks: blocks.into_boxed_slice(),
        loop_summaries: Box::default(),
        observable_output_effects: Box::default(),
    }
}

#[test]
fn output_slice_retains_point_metadata_guards_and_ordered_inputs() {
    let ir = fixture();
    let writes = ir.observable_output_writes_v1().unwrap();
    assert_eq!(writes.len(), 1);
    assert_eq!(
        writes[0].coordinate,
        ReferenceOutputCoordinateV1::LogicalPoint(
            vec![ReferenceEffectExpressionV1::PointCoordinate { axis: 0 }].into_boxed_slice()
        )
    );
    assert_eq!(writes[0].guard.clauses.len(), 1);
    assert_eq!(writes[0].guard.clauses[0].atoms.len(), 4);
    let ReferenceEffectExpressionV1::Binary {
        operation: ReferenceBinaryOpV1::Add,
        lhs,
        rhs,
        checked: false,
    } = &writes[0].rhs
    else {
        panic!("ordered addition")
    };
    assert!(matches!(
        **lhs,
        ReferenceEffectExpressionV1::InputLoad {
            reference_argument: 1,
            ..
        }
    ));
    assert!(matches!(
        **rhs,
        ReferenceEffectExpressionV1::InputLoad {
            reference_argument: 2,
            ..
        }
    ));
    assert_eq!(
        ir.resolved_bounds_checks_with_budget_v1(&mut ReferenceSymbolicWorkBudgetV2::default())
            .unwrap()
            .len(),
        3
    );
}

#[test]
fn output_slice_distinct_identity_is_not_scalar_or_legacy_slice() {
    let ir = fixture();
    for relation in [
        ReferenceArgumentRelationV1::DisjointOutputSlice {
            argument: 2,
            element: ReferenceScalarTypeV1::F32,
        },
        ReferenceArgumentRelationV1::InvocationDisjointOutputCoordinate1D {
            argument: 2,
            element: ReferenceScalarTypeV1::F32,
        },
    ] {
        let mut changed = ir.clone();
        changed.relations[3] = relation;
        assert_ne!(ir.canonical_sha256_v1(), changed.canonical_sha256_v1());
    }
}

#[test]
fn output_slice_rejects_nonpoint_coordinate_and_point_reassignment() {
    let mut ir = fixture();
    ir.blocks[7].assignments[2].destination.projection[1] = ReferencePlaceProjectionV1::Index(5);
    assert!(
        ir.observable_output_writes_v1()
            .unwrap_err()
            .to_string()
            .contains("exact source point")
    );
    let mut ir = fixture();
    ir.blocks[0].assignments[0].destination = place(1);
    assert!(
        ir.observable_output_writes_v1()
            .unwrap_err()
            .to_string()
            .contains("immutable arguments")
    );
}

#[test]
fn output_slice_rejects_read_escape_rebind_and_extra_write() {
    for mutation in 0..4 {
        let mut ir = fixture();
        match mutation {
            0 => {
                let ReferenceValueV1::Use(ReferenceOperandV1::Copy(p)) =
                    &mut ir.blocks[7].assignments[0].value
                else {
                    unreachable!()
                };
                p.local = 4;
            }
            1 => ir.blocks[7].assignments[0].value = ReferenceValueV1::Use(operand(4)),
            2 => ir.blocks[7].assignments[0].destination = place(4),
            3 => {
                let mut a = ir.blocks[7].assignments.to_vec();
                a.push(a[2].clone());
                ir.blocks[7].assignments = a.into_boxed_slice();
            }
            _ => unreachable!(),
        }
        assert!(
            ir.observable_output_writes_v1()
                .unwrap_err()
                .to_string()
                .contains("no output reads or escapes"),
            "mutation {mutation}"
        );
    }
}

#[test]
fn output_slice_wrong_length_and_unexecuted_write_fail_independent_pairing() {
    use crate::reference_effect_bijection_v1::{
        CompilerExtractedGpuOutputEffectV1, ReferenceEffectBijectionErrorV1,
        establish_reference_effect_bijection_v1,
    };
    let reference = fixture().observable_output_writes_v1().unwrap();
    let mut gpu = CompilerExtractedGpuOutputEffectV1 {
        output_argument: 2,
        block: 7,
        operation: 0,
        coordinate: reference[0].coordinate.clone(),
        guard: reference[0].guard.clone(),
    };
    establish_reference_effect_bijection_v1(&reference, &[gpu.clone()]).unwrap();
    let mut wrong_length = fixture();
    let ReferenceValueV1::Binary { rhs, .. } = &mut wrong_length.blocks[0].assignments[5].value
    else {
        unreachable!()
    };
    *rhs = integer(1023);
    let wrong = wrong_length.observable_output_writes_v1().unwrap();
    assert!(matches!(
        establish_reference_effect_bijection_v1(&wrong, &[gpu.clone()]),
        Err(ReferenceEffectBijectionErrorV1::GuardMismatch { .. })
    ));
    gpu.guard = ReferencePathPredicateV1::unconditional_v1();
    assert!(matches!(
        establish_reference_effect_bijection_v1(&reference, &[gpu]),
        Err(ReferenceEffectBijectionErrorV1::GuardMismatch { .. })
    ));
}

#[test]
fn output_slice_swapped_inputs_remain_distinct_reference_rhs() {
    let ir = fixture();
    let mut changed = ir.clone();
    let ReferenceValueV1::Binary { lhs, rhs, .. } = &mut changed.blocks[7].assignments[2].value
    else {
        unreachable!()
    };
    std::mem::swap(lhs, rhs);
    assert_ne!(
        ir.observable_output_writes_v1().unwrap()[0].rhs,
        changed.observable_output_writes_v1().unwrap()[0].rhs
    );
}

#[test]
fn output_slice_custody_and_predicates_share_exhausted_budget() {
    let ir = fixture();
    let mut work = ReferenceSymbolicWorkBudgetV2::default();
    work.charge_v2(MAX_REFERENCE_SYMBOLIC_WORK_NODES_V2)
        .unwrap();
    assert!(
        validate(&ir, &mut work)
            .unwrap_err()
            .to_string()
            .contains("cumulative expression work")
    );
}

#[test]
fn output_slice_rejects_a_backedge_before_effect_admission() {
    let mut ir = fixture();
    ir.blocks[7].terminator = ReferenceTerminatorV1::Goto { target: 0 };
    assert!(
        ir.observable_output_writes_v1()
            .unwrap_err()
            .to_string()
            .contains("contains a cycle")
    );
}

fn discharge_output_bounds(
    ir: &ReferenceEffectIrV1,
) -> Result<(), crate::production_reference_bounds_v2::ReferenceBoundsDischargeErrorV2> {
    use crate::production_reference_bounds_v2::{
        CompilerOwnedOutputDomainV2, discharge_reference_bounds_over_ranked_domains_v2,
    };
    use fe2o3_pliron::{
        ProductionRankedBlockV1, ProductionRankedKernelV1, ProductionRankedOperationV1,
        ProductionRankedTerminatorV1, ProductionRankedValueIdV1, ProductionRankedValueV1,
    };
    let views = (0..3)
        .map(|index| ProductionRankedOperationV1::View {
            result: ProductionRankedValueIdV1::new(index),
            element_width: 32,
            writable: index == 2,
            shape: vec![dialect_kernel::DYNAMIC_EXTENT],
            dynamic_extents: vec![ProductionRankedValueV1::Argument(index)],
            allocation_origin: u64::from(index) + 1,
            noalias_class: u64::from(index) + 1,
        })
        .collect();
    let kernel = ProductionRankedKernelV1::new(
        "output_slice_bounds",
        3,
        vec![ProductionRankedBlockV1::new(
            views,
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap();
    let writes = ir.observable_output_writes_v1().unwrap();
    discharge_reference_bounds_over_ranked_domains_v2(
        &kernel,
        ir,
        &[CompilerOwnedOutputDomainV2 {
            reference: &writes[0],
            ranked_view: ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(2)),
        }],
    )
}

#[test]
fn output_slice_all_three_retained_bounds_follow_exact_cpu_guards() {
    discharge_output_bounds(&fixture()).unwrap();
    for block in 4..7 {
        let mut ir = fixture();
        ir.blocks[block].terminator = ReferenceTerminatorV1::Goto {
            target: block as u32 + 1,
        };
        let error = discharge_output_bounds(&ir).unwrap_err();
        assert!(
            error
                .detail()
                .contains("has no exact retained bounds assertion"),
            "block {block}: {error:?}"
        );
    }
}

#[test]
fn output_slice_bounds_do_not_use_own_output_extent_or_assertions_as_assumptions() {
    for mutation in 0..4 {
        let mut ir = fixture();
        match mutation {
            0 => ir.blocks[2].terminator = ReferenceTerminatorV1::Goto { target: 3 },
            1 => ir.blocks[3].terminator = ReferenceTerminatorV1::Goto { target: 4 },
            2 => {
                ir.blocks[2].terminator = ReferenceTerminatorV1::Switch {
                    discriminant: operand(10),
                    values: vec![(0, 3)].into_boxed_slice(),
                    otherwise: 8,
                }
            }
            3 => {
                let ReferenceValueV1::Binary { rhs, .. } = &mut ir.blocks[0].assignments[5].value
                else {
                    unreachable!()
                };
                *rhs = integer(1023);
            }
            _ => unreachable!(),
        }
        let error = discharge_output_bounds(&ir).unwrap_err();
        assert!(
            error
                .detail()
                .contains("not a preceding CPU predicate on every path"),
            "mutation {mutation}: {error:?}"
        );
    }
}
