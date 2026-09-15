//! Ranked guard components, not substitutes for authenticated CPU bindings.
use super::*;
use crate::reference_effect_v1::{
    MAX_REFERENCE_SYMBOLIC_WORK_NODES_V2, ReferenceOperandV1, ReferenceValueV1,
};
use dialect_kernel::AccessKindAttr;

fn local(id: u32) -> ProductionRankedValueV1 {
    ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(id))
}
fn empty(terminator: ProductionRankedTerminatorV1) -> ProductionRankedBlockV1 {
    ProductionRankedBlockV1::new(vec![], terminator)
}
fn fixture() -> (
    ProductionRankedKernelV1,
    ReferenceEffectIrV1,
    RankedGpuWriteV2,
) {
    let mut operations = vec![ProductionRankedOperationV1::InvocationIndex {
        result: ProductionRankedValueIdV1::new(0),
        dimension: 0,
        launch_extent: 1024,
    }];
    for i in 0..3 {
        operations.push(ProductionRankedOperationV1::ViewInSpace {
            result: ProductionRankedValueIdV1::new(i + 1),
            element_width: 32,
            writable: i == 2,
            shape: vec![DYNAMIC_EXTENT],
            dynamic_extents: vec![ProductionRankedValueV1::Argument(i)],
            memory_space: MemorySpaceAttr::Global,
            allocation_origin: i as u64 + 1,
            noalias_class: i as u64 + 1,
        });
    }
    operations.push(ProductionRankedOperationV1::IndexConstant {
        result: ProductionRankedValueIdV1::new(4),
        value: 1024,
    });
    let mut blocks = (0..3)
        .map(|i| {
            empty(ProductionRankedTerminatorV1::IndexEqual {
                lhs: ProductionRankedValueV1::Argument(i),
                rhs: local(4),
                true_block: i + 1,
                false_block: 11,
            })
        })
        .collect::<Vec<_>>();
    blocks[0] = ProductionRankedBlockV1::new(operations, blocks[0].terminator().clone());
    blocks.push(empty(ProductionRankedTerminatorV1::IndexLessThan {
        lhs: local(0),
        rhs: local(4),
        true_block: 4,
        false_block: 11,
    }));
    for i in 0..3 {
        blocks.push(empty(ProductionRankedTerminatorV1::IndexLessThan {
            lhs: local(0),
            rhs: ProductionRankedValueV1::Argument(i),
            true_block: i + 5,
            false_block: 11,
        }));
    }
    blocks.push(empty(ProductionRankedTerminatorV1::AnalysisSplit {
        control_dependencies: vec![],
        first_block: 8,
        second_block: 9,
    }));
    blocks.push(empty(ProductionRankedTerminatorV1::Branch { target: 10 }));
    blocks.push(empty(ProductionRankedTerminatorV1::Branch { target: 8 }));
    blocks.push(ProductionRankedBlockV1::new(
        vec![ProductionRankedOperationV1::Access {
            kind: AccessKindAttr::Write,
            view: local(3),
            indices: vec![local(0)],
        }],
        ProductionRankedTerminatorV1::Return,
    ));
    blocks.push(empty(ProductionRankedTerminatorV1::Return));
    let kernel = ProductionRankedKernelV1::new("guarded_point", 3, blocks).unwrap();
    let ir = ReferenceEffectIrV1 {
        argument_count: 4,
        local_count: 5,
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
        blocks: Box::default(),
        loop_summaries: Box::default(),
        observable_output_effects: Box::default(),
    };
    let write = RankedGpuWriteV2 {
        block: 10,
        operation: 0,
        allocation_origin: 3,
        view: local(3),
        indices: vec![local(0)],
        value: Err("guard component only"),
    };
    (kernel, ir, write)
}

fn expected_guard() -> ReferencePathPredicateV1 {
    // Independent point-reference source has these four conditions. Bounds
    // assertions and output-view capacity are deliberately not premises.
    let mut guard = ReferencePathPredicateV1::unconditional_v1();
    for i in 0..4 {
        let condition = ReferenceEffectExpressionV1::Binary {
            operation: if i == 3 {
                ReferenceBinaryOpV1::LessThan
            } else {
                ReferenceBinaryOpV1::Equal
            },
            lhs: Box::new(if i == 3 {
                ReferenceEffectExpressionV1::PointCoordinate { axis: 0 }
            } else {
                ReferenceEffectExpressionV1::InputLength {
                    reference_argument: i + 1,
                }
            }),
            rhs: Box::new(ReferenceEffectExpressionV1::Constant(
                ReferenceConstantV1::Scalar {
                    scalar: ReferenceScalarTypeV1::Usize,
                    bits: 1024,
                },
            )),
            checked: false,
        };
        guard = reference_predicate_and_atom_v1(
            &guard,
            reference_boolean_guard_atom_v1(condition, true),
        )
        .unwrap();
    }
    guard
}

fn adjacent_fixture(
    threshold: u64,
    reversed: bool,
) -> (
    ProductionRankedKernelV1,
    ReferenceEffectIrV1,
    RankedGpuWriteV2,
) {
    let (kernel, ir, write) = fixture();
    let mut blocks = kernel.blocks().to_vec();
    let mut operations = blocks[0].operations().to_vec();
    operations.push(ProductionRankedOperationV1::IndexConstant {
        result: ProductionRankedValueIdV1::new(5),
        value: threshold,
    });
    blocks[0] = ProductionRankedBlockV1::new(operations, blocks[0].terminator().clone());
    blocks[3] = empty(ProductionRankedTerminatorV1::IndexLessThan {
        lhs: local(5),
        rhs: local(0),
        true_block: if reversed { 4 } else { 11 },
        false_block: if reversed { 11 } else { 4 },
    });
    (
        ProductionRankedKernelV1::new("guarded_point", 3, blocks).unwrap(),
        ir,
        write,
    )
}

#[test]
fn source_guard_adjacent_threshold_pairs_exact_cpu_and_rejects_missing_point() {
    use crate::reference_effect_bijection_v1::{
        CompilerExtractedGpuOutputEffectV1, ReferenceEffectBijectionErrorV1,
        establish_reference_effect_bijection_v1,
    };
    use crate::reference_effect_v1::{ReferenceGuardAtomV1, ReferenceOutputWriteV1};
    let (kernel, ir, write) = adjacent_fixture(1023, false);
    let original = kernel.blocks().to_vec();
    let guard = gpu_write_path_predicate_v2(&kernel, &ir, &write).unwrap();
    assert_eq!(guard.clauses[0].atoms.len(), 4);
    assert_eq!(guard, expected_guard());
    assert_eq!(kernel.blocks(), original, "original branches remain intact");
    let coordinate = ReferenceOutputCoordinateV1::LogicalPoint(
        vec![ReferenceEffectExpressionV1::PointCoordinate { axis: 0 }].into_boxed_slice(),
    );
    let zero = ReferenceConstantV1::Scalar {
        scalar: ReferenceScalarTypeV1::F32,
        bits: 0,
    };
    let mut cpu = ReferenceOutputWriteV1 {
        argument: 2,
        block: 7,
        statement: 0,
        coordinate: coordinate.clone(),
        guard: expected_guard(),
        rhs: ReferenceEffectExpressionV1::Constant(zero.clone()),
        value: ReferenceValueV1::Use(ReferenceOperandV1::Constant(zero)),
    };
    let gpu = CompilerExtractedGpuOutputEffectV1 {
        output_argument: 2,
        block: write.block as u32,
        operation: 0,
        coordinate,
        guard,
    };
    assert_eq!(
        establish_reference_effect_bijection_v1(&[cpu.clone()], &[gpu.clone()])
            .unwrap()
            .len(),
        1
    );
    for atom in &mut cpu.guard.clauses[0].atoms {
        if let ReferenceGuardAtomV1::SwitchValueSet {
            discriminant:
                ReferenceEffectExpressionV1::Binary {
                    operation: ReferenceBinaryOpV1::LessThan,
                    rhs,
                    ..
                },
            ..
        } = atom
        {
            **rhs = ReferenceEffectExpressionV1::Constant(ReferenceConstantV1::Scalar {
                scalar: ReferenceScalarTypeV1::Usize,
                bits: 1023,
            });
        }
    }
    assert!(matches!(
        establish_reference_effect_bijection_v1(&[cpu], &[gpu]),
        Err(ReferenceEffectBijectionErrorV1::GuardMismatch { .. })
    ));
}

#[test]
fn source_guard_adjacent_missing_length_threshold_polarity_or_path_keeps_mismatch() {
    for mutation in 0..4 {
        let (kernel, ir, write) =
            adjacent_fixture(if mutation == 0 { 1022 } else { 1023 }, mutation == 1);
        let mut blocks = kernel.blocks().to_vec();
        if mutation == 2 {
            blocks[0] = ProductionRankedBlockV1::new(
                blocks[0].operations().to_vec(),
                ProductionRankedTerminatorV1::Branch { target: 1 },
            );
        }
        if mutation == 3 {
            blocks[3] = empty(ProductionRankedTerminatorV1::Branch { target: 4 });
        }
        let kernel = ProductionRankedKernelV1::new("guarded_point", 3, blocks).unwrap();
        let actual = gpu_write_path_predicate_v2(&kernel, &ir, &write).unwrap();
        assert_ne!(actual, expected_guard(), "mutation {mutation}");
        if mutation == 0 {
            // point < 1023 proves the subsequent point < length=1024 checks,
            // but must not erase the different observable write domain.
            let missing_last_point = ReferenceEffectExpressionV1::Binary {
                operation: ReferenceBinaryOpV1::LessThan,
                lhs: Box::new(ReferenceEffectExpressionV1::PointCoordinate { axis: 0 }),
                rhs: Box::new(ReferenceEffectExpressionV1::Constant(
                    ReferenceConstantV1::Scalar {
                        scalar: ReferenceScalarTypeV1::Usize,
                        bits: 1023,
                    },
                )),
                checked: false,
            };
            assert_eq!(actual.clauses.len(), 1);
            assert_eq!(actual.clauses[0].atoms.len(), 4);
            assert!(actual.clauses[0].atoms.contains(
                &reference_boolean_guard_atom_v1(missing_last_point, true)
            ));
            continue;
        }
        assert!(
            actual.clauses[0].atoms.iter().any(|atom| matches!(atom,
                crate::reference_effect_v1::ReferenceGuardAtomV1::SwitchValueSet {
                    discriminant: ReferenceEffectExpressionV1::Binary {
                        operation: ReferenceBinaryOpV1::LessThan, rhs, ..
                    }, ..
                } if matches!(rhs.as_ref(), ReferenceEffectExpressionV1::InputLength { .. })
            )),
            "unproved length guards remain"
        );
    }
}

fn adjacent_condition(scalar: ReferenceScalarTypeV1, bits: u128) -> ReferenceEffectExpressionV1 {
    ReferenceEffectExpressionV1::Binary {
        operation: ReferenceBinaryOpV1::LessThan,
        lhs: Box::new(ReferenceEffectExpressionV1::Constant(
            ReferenceConstantV1::Scalar { scalar, bits },
        )),
        rhs: Box::new(ReferenceEffectExpressionV1::PointCoordinate { axis: 0 }),
        checked: false,
    }
}

#[test]
fn source_guard_adjacent_unsigned_boundary_equivalence_and_maximum() {
    for threshold in [0, 1, 1023, u64::MAX - 1] {
        let (condition, yes, no) = canonical_point_threshold(
            adjacent_condition(ReferenceScalarTypeV1::Usize, threshold.into()),
            7,
            9,
            &mut ReferenceSymbolicWorkBudgetV2::default(),
        )
        .unwrap();
        assert_eq!((yes, no), (9, 7));
        let ReferenceEffectExpressionV1::Binary { lhs, rhs, .. } = condition else {
            panic!()
        };
        assert_eq!(
            *lhs,
            ReferenceEffectExpressionV1::PointCoordinate { axis: 0 }
        );
        let ReferenceEffectExpressionV1::Constant(ReferenceConstantV1::Scalar {
            scalar: ReferenceScalarTypeV1::Usize,
            bits,
        }) = *rhs
        else {
            panic!()
        };
        assert_eq!(bits, u128::from(threshold) + 1);
        for point in [0, 1, threshold, threshold + 1, u64::MAX] {
            assert_eq!(
                if threshold < point { 7 } else { 9 },
                if u128::from(point) < bits { yes } else { no }
            );
        }
    }
    for bits in [u128::from(u64::MAX), u128::from(u64::MAX) + 1, u128::MAX] {
        let original = adjacent_condition(ReferenceScalarTypeV1::Usize, bits);
        assert_eq!(
            canonical_point_threshold(
                original.clone(),
                7,
                9,
                &mut ReferenceSymbolicWorkBudgetV2::default()
            )
            .unwrap(),
            (original, 7, 9)
        );
    }
}

#[test]
fn source_guard_adjacent_nonunsigned_nonpoint_checked_or_wrong_operation_unchanged() {
    for scalar in [
        ReferenceScalarTypeV1::I64,
        ReferenceScalarTypeV1::U32,
        ReferenceScalarTypeV1::F32,
        ReferenceScalarTypeV1::Bool,
    ] {
        let original = adjacent_condition(scalar, 1);
        assert_eq!(
            canonical_point_threshold(
                original.clone(),
                7,
                9,
                &mut ReferenceSymbolicWorkBudgetV2::default()
            )
            .unwrap(),
            (original, 7, 9)
        );
    }
    for mutation in 0..3 {
        let mut original = adjacent_condition(ReferenceScalarTypeV1::Usize, 1023);
        let ReferenceEffectExpressionV1::Binary {
            operation,
            rhs,
            checked,
            ..
        } = &mut original
        else {
            panic!()
        };
        match mutation {
            0 => *operation = ReferenceBinaryOpV1::Equal,
            1 => *checked = true,
            2 => {
                **rhs = ReferenceEffectExpressionV1::InputLength {
                    reference_argument: 1,
                }
            }
            _ => unreachable!(),
        }
        assert_eq!(
            canonical_point_threshold(
                original.clone(),
                7,
                9,
                &mut ReferenceSymbolicWorkBudgetV2::default()
            )
            .unwrap(),
            (original, 7, 9)
        );
    }
}

#[test]
fn source_guard_adjacent_charges_inherited_work_without_reset() {
    let mut budget = ReferenceSymbolicWorkBudgetV2::default();
    budget
        .charge_v2(MAX_REFERENCE_SYMBOLIC_WORK_NODES_V2 - 4)
        .unwrap();
    canonical_point_threshold(
        adjacent_condition(ReferenceScalarTypeV1::Usize, 1023),
        7,
        9,
        &mut budget,
    )
    .unwrap();
    assert!(
        canonical_point_threshold(
            adjacent_condition(ReferenceScalarTypeV1::Usize, 1023),
            7,
            9,
            &mut budget
        )
        .is_err()
    );
}

#[test]
fn source_guard_empty_option_split_preserves_four_exact_source_predicates() {
    let (kernel, ir, write) = fixture();
    let retained = kernel.blocks().to_vec();
    assert_eq!(
        gpu_write_path_predicate_v2(&kernel, &ir, &write).unwrap(),
        expected_guard()
    );
    assert_eq!(kernel.blocks(), retained);
    assert!(reconverges(&kernel, 8, 9, &mut ReferenceSymbolicWorkBudgetV2::default()).unwrap());
    assert!(reconverges(&kernel, 9, 8, &mut ReferenceSymbolicWorkBudgetV2::default()).unwrap());
}

#[test]
fn source_guard_does_not_erase_effectful_diverging_or_conditional_arms() {
    for mutation in 0..4 {
        let (kernel, ir, write) = fixture();
        let mut blocks = kernel.blocks().to_vec();
        blocks[9] = match mutation {
            0 => empty(ProductionRankedTerminatorV1::Return),
            1 => ProductionRankedBlockV1::new(
                vec![ProductionRankedOperationV1::Access {
                    kind: AccessKindAttr::Read,
                    view: local(1),
                    indices: vec![local(0)],
                }],
                ProductionRankedTerminatorV1::Branch { target: 8 },
            ),
            2 => empty(ProductionRankedTerminatorV1::IndexEqual {
                lhs: local(0),
                rhs: local(4),
                true_block: 8,
                false_block: 11,
            }),
            3 => empty(ProductionRankedTerminatorV1::Trap),
            _ => unreachable!(),
        };
        let kernel = ProductionRankedKernelV1::new("guarded_point", 3, blocks).unwrap();
        assert!(
            !reconverges(&kernel, 8, 9, &mut ReferenceSymbolicWorkBudgetV2::default()).unwrap()
        );
        assert!(
            matches!(
                gpu_write_path_predicate_v2(&kernel, &ir, &write),
                Err(ProductionReferenceEffectJoinErrorV2::UnsupportedGpuEffect {
                    detail: "GPU guard contains a nonrepresentable analysis split",
                    ..
                })
            ),
            "mutation {mutation}"
        );
    }
}

#[test]
fn source_guard_wrong_length_coordinate_bypass_and_reversed_edge_do_not_match() {
    for mutation in 0..6 {
        let (kernel, ir, write) = fixture();
        let mut blocks = kernel.blocks().to_vec();
        let site = if mutation == 4 { 3 } else { 0 };
        let old = blocks[site].terminator().clone();
        let terminal = match (mutation, old) {
            (0, _) => ProductionRankedTerminatorV1::Branch { target: 1 },
            (1, ProductionRankedTerminatorV1::IndexEqual { lhs, rhs, .. }) => {
                ProductionRankedTerminatorV1::IndexEqual {
                    lhs,
                    rhs,
                    true_block: 11,
                    false_block: 1,
                }
            }
            (2, ProductionRankedTerminatorV1::IndexEqual { lhs, .. }) => {
                ProductionRankedTerminatorV1::IndexEqual {
                    lhs,
                    rhs: local(0),
                    true_block: 1,
                    false_block: 11,
                }
            }
            (3, ProductionRankedTerminatorV1::IndexEqual { rhs, .. }) => {
                ProductionRankedTerminatorV1::IndexEqual {
                    lhs: ProductionRankedValueV1::Argument(1),
                    rhs,
                    true_block: 1,
                    false_block: 11,
                }
            }
            (4, _) => ProductionRankedTerminatorV1::Branch { target: 4 },
            (5, _) => ProductionRankedTerminatorV1::Branch { target: 10 },
            _ => unreachable!(),
        };
        blocks[site] = ProductionRankedBlockV1::new(blocks[site].operations().to_vec(), terminal);
        let kernel = ProductionRankedKernelV1::new("guarded_point", 3, blocks).unwrap();
        assert_ne!(
            gpu_write_path_predicate_v2(&kernel, &ir, &write).unwrap(),
            expected_guard(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn source_guard_cycles_and_existing_work_exhaustion_are_not_pruned() {
    let (kernel, ir, write) = fixture();
    let mut budget = ReferenceSymbolicWorkBudgetV2::default();
    budget
        .charge_v2(MAX_REFERENCE_SYMBOLIC_WORK_NODES_V2)
        .unwrap();
    assert!(reconverges(&kernel, 8, 9, &mut budget).is_err());
    let mut blocks = kernel.blocks().to_vec();
    blocks[9] = empty(ProductionRankedTerminatorV1::Branch { target: 9 });
    let kernel = ProductionRankedKernelV1::new("guarded_point", 3, blocks).unwrap();
    assert!(!reconverges(&kernel, 8, 9, &mut ReferenceSymbolicWorkBudgetV2::default()).unwrap());
    assert!(matches!(
        gpu_write_path_predicate_v2(&kernel, &ir, &write),
        Err(ProductionReferenceEffectJoinErrorV2::UnsupportedGpuEffect {
            detail: "GPU write guard CFG contains a cycle",
            ..
        })
    ));
}

#[test]
fn source_guard_bound_substitution_requires_every_preceding_clause() {
    let (_, _, _) = fixture();
    let mut guard = expected_guard();
    let condition = ReferenceEffectExpressionV1::Binary {
        operation: ReferenceBinaryOpV1::LessThan,
        lhs: Box::new(ReferenceEffectExpressionV1::PointCoordinate { axis: 0 }),
        rhs: Box::new(ReferenceEffectExpressionV1::InputLength {
            reference_argument: 3,
        }),
        checked: false,
    };
    assert!(
        preceding_bound(
            &guard,
            &condition,
            4,
            &mut ReferenceSymbolicWorkBudgetV2::default()
        )
        .unwrap()
    );
    reference_predicate_or_assign_v1(&mut guard, ReferencePathPredicateV1::unconditional_v1())
        .unwrap();
    assert!(
        !preceding_bound(
            &guard,
            &condition,
            4,
            &mut ReferenceSymbolicWorkBudgetV2::default()
        )
        .unwrap()
    );
}
