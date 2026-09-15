//! Component metadata/guard tests. These inputs are not source-authentication
//! receipts or successful numerical proof executions.
use super::*;
use dialect_kernel::AccessKindAttr;

fn fixture() -> (
    ProductionRankedKernelV1,
    ReferenceEffectIrV1,
    RankedGpuWriteV2,
) {
    let local = |id| ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(id));
    let mut operations = vec![ProductionRankedOperationV1::InvocationIndex {
        result: ProductionRankedValueIdV1::new(0),
        dimension: 0,
        launch_extent: 1024,
    }];
    for index in 0..3 {
        operations.push(ProductionRankedOperationV1::ViewInSpace {
            result: ProductionRankedValueIdV1::new(index + 1),
            element_width: 32,
            writable: index == 2,
            shape: vec![DYNAMIC_EXTENT],
            dynamic_extents: vec![ProductionRankedValueV1::Argument(index)],
            memory_space: MemorySpaceAttr::Global,
            allocation_origin: index as u64 + 1,
            noalias_class: index as u64 + 1,
        });
    }
    operations.push(ProductionRankedOperationV1::IndexConstant {
        result: ProductionRankedValueIdV1::new(4),
        value: 1024,
    });
    let kernel = ProductionRankedKernelV1::new(
        "point_slice",
        3,
        vec![
            ProductionRankedBlockV1::new(
                operations,
                ProductionRankedTerminatorV1::IndexEqual {
                    lhs: ProductionRankedValueV1::Argument(2),
                    rhs: local(4),
                    true_block: 1,
                    false_block: 4,
                },
            ),
            ProductionRankedBlockV1::new(
                vec![],
                ProductionRankedTerminatorV1::IndexLessThan {
                    lhs: local(0),
                    rhs: local(4),
                    true_block: 2,
                    false_block: 4,
                },
            ),
            ProductionRankedBlockV1::new(
                vec![],
                ProductionRankedTerminatorV1::IndexLessThan {
                    lhs: local(0),
                    rhs: ProductionRankedValueV1::Argument(2),
                    true_block: 3,
                    false_block: 4,
                },
            ),
            ProductionRankedBlockV1::new(
                vec![ProductionRankedOperationV1::Access {
                    kind: AccessKindAttr::Write,
                    view: local(3),
                    indices: vec![local(0)],
                }],
                ProductionRankedTerminatorV1::Return,
            ),
            ProductionRankedBlockV1::new(vec![], ProductionRankedTerminatorV1::Return),
        ],
    )
    .unwrap();
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
        block: 3,
        operation: 0,
        allocation_origin: 3,
        view: local(3),
        indices: vec![local(0)],
        value: Err("metadata/guard-only fixture"),
    };
    (kernel, ir, write)
}

#[test]
fn output_slice_gpu_guard_keeps_exact_output_length_and_no_write_domain() {
    let (kernel, ir, write) = fixture();
    let guard = gpu_write_path_predicate_v2(&kernel, &ir, &write).unwrap();
    assert_eq!(guard.clauses.len(), 1);
    assert_eq!(guard.clauses[0].atoms.len(), 2);
    let expected = ReferenceEffectExpressionV1::Binary {
        operation: ReferenceBinaryOpV1::Equal,
        lhs: Box::new(ReferenceEffectExpressionV1::InputLength {
            reference_argument: 3,
        }),
        rhs: Box::new(ReferenceEffectExpressionV1::Constant(
            ReferenceConstantV1::Scalar {
                scalar: ReferenceScalarTypeV1::Usize,
                bits: 1024,
            },
        )),
        checked: false,
    };
    assert!(
        guard.clauses[0]
            .atoms
            .contains(&reference_boolean_guard_atom_v1(expected, true))
    );
}

#[test]
fn output_slice_gpu_guard_rejects_wrong_role_and_metadata_alias() {
    for alias in [false, true] {
        let (kernel, ir, write) = fixture();
        let mut blocks = kernel.blocks().to_vec();
        let mut operations = blocks[0].operations().to_vec();
        let ProductionRankedOperationV1::ViewInSpace {
            writable,
            dynamic_extents,
            ..
        } = &mut operations[3]
        else {
            unreachable!()
        };
        if alias {
            dynamic_extents[0] = ProductionRankedValueV1::Argument(1);
        } else {
            *writable = false;
        }
        blocks[0] = ProductionRankedBlockV1::new(operations, blocks[0].terminator().clone());
        let result = ProductionRankedKernelV1::new("point_slice", 3, blocks);
        if !alias {
            assert!(matches!(
                result,
                Err(ProductionRankedKernelErrorV1::WriteThroughReadOnlyView)
            ));
            continue;
        }
        let kernel = result.unwrap();
        assert!(matches!(
            gpu_write_path_predicate_v2(&kernel, &ir, &write),
            Err(ProductionReferenceEffectJoinErrorV2::UnsupportedGpuEffect { .. })
        ));
    }
}

#[test]
fn output_slice_gpu_guard_bypass_and_reversed_edge_do_not_match() {
    let (kernel, ir, write) = fixture();
    let expected = gpu_write_path_predicate_v2(&kernel, &ir, &write).unwrap();
    for reverse in [false, true] {
        let mut blocks = kernel.blocks().to_vec();
        let terminator = if reverse {
            let ProductionRankedTerminatorV1::IndexEqual { lhs, rhs, .. } = blocks[0].terminator()
            else {
                unreachable!()
            };
            ProductionRankedTerminatorV1::IndexEqual {
                lhs: *lhs,
                rhs: *rhs,
                true_block: 4,
                false_block: 1,
            }
        } else {
            ProductionRankedTerminatorV1::Branch { target: 1 }
        };
        blocks[0] = ProductionRankedBlockV1::new(blocks[0].operations().to_vec(), terminator);
        let kernel = ProductionRankedKernelV1::new("point_slice", 3, blocks).unwrap();
        assert_ne!(
            gpu_write_path_predicate_v2(&kernel, &ir, &write).unwrap(),
            expected
        );
    }
}

#[test]
fn output_slice_gpu_guard_unknown_branch_still_rejects() {
    let (kernel, ir, write) = fixture();
    let mut blocks = kernel.blocks().to_vec();
    blocks[0] = ProductionRankedBlockV1::new(
        blocks[0].operations().to_vec(),
        ProductionRankedTerminatorV1::AnalysisSplit {
            control_dependencies: vec![],
            first_block: 1,
            second_block: 4,
        },
    );
    let kernel = ProductionRankedKernelV1::new("point_slice", 3, blocks).unwrap();
    assert!(matches!(
        gpu_write_path_predicate_v2(&kernel, &ir, &write),
        Err(ProductionReferenceEffectJoinErrorV2::UnsupportedGpuEffect {
            detail: "GPU guard contains a nonrepresentable analysis split",
            ..
        })
    ));
}
