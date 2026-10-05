use super::*;
use crate::{BasicBlock, IntrinsicOperation, Kernel, Signature, Terminator, ValueDef};

fn noop(domain: LaunchDomain) -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("physical_envelope");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    module.kernels.push(Kernel::new("root", "entry", domain));
    module
}

fn exact(module: &Module, rank: u8, extents: [u64; 3]) -> FormalMemoryObligationAnalysis {
    derive_kernel_memory_obligations_for_launch(
        module,
        &module.kernels[0].id,
        ExplicitLaunchExtent::Exact { rank, extents },
        FormalIndexWidth::Bits64,
    )
    .unwrap()
}

fn envelope(module: &Module, rank: u8, extents: [u64; 3]) -> FormalMemoryObligationAnalysis {
    derive_kernel_memory_obligations_from_verified_for_physical_envelope_v2(
        verify_module_ref(module).unwrap(),
        &module.kernels[0].id,
        FormalPhysicalLaunchEnvelopeV2::new(rank, extents),
        FormalIndexWidth::Bits64,
    )
    .unwrap()
}

#[test]
fn physical_envelope_covers_padded_and_extra_groups_without_source_rewrite() {
    for (items, count) in [(64, 1088), (65, 128), (128, 128)] {
        let module = noop(LaunchDomain::D1 {
            x: LaunchExtent::Static(items),
        });
        let original = module.clone();
        let analysis = envelope(&module, 1, [count, 1, 1]);
        assert!(analysis.is_complete());
        assert_eq!(
            analysis.obligations().invocations().unwrap(),
            InvocationRange1d::from_count(count).unwrap()
        );
        assert_eq!(module, original);
        assert_eq!(
            module.kernels[0].domain,
            LaunchDomain::D1 {
                x: LaunchExtent::Static(items)
            }
        );
        let exact_analysis = exact(&module, 1, [count, 1, 1]);
        if u64::from(items) == count {
            assert_eq!(analysis, exact_analysis);
        } else {
            assert_eq!(
                exact_analysis.incomplete_reasons(),
                &[FormalMemoryIncompleteReason::StaticLaunchExtentMismatch {
                    expected: items,
                    actual: count
                }]
            );
        }
    }
}

#[test]
fn physical_envelope_preserves_invalid_shape_and_insufficient_coverage_reasons() {
    let module = noop(LaunchDomain::D1 {
        x: LaunchExtent::Static(64),
    });
    for (rank, extents) in [
        (0, [64, 1, 1]),
        (4, [64, 1, 1]),
        (2, [64, 1, 1]),
        (1, [0, 1, 1]),
        (1, [64, 2, 1]),
        (1, [63, 1, 1]),
    ] {
        let original = exact(&module, rank, extents);
        let bounded = envelope(&module, rank, extents);
        assert!(!bounded.is_complete());
        assert_eq!(bounded, original);
    }
    let module = noop(LaunchDomain::D2 {
        x: LaunchExtent::Dynamic,
        y: LaunchExtent::Dynamic,
    });
    let overflowing = [u64::MAX, 2, 1];
    assert_eq!(
        envelope(&module, 2, overflowing),
        exact(&module, 2, overflowing)
    );
    assert_eq!(
        envelope(&module, 2, overflowing).incomplete_reasons(),
        &[FormalMemoryIncompleteReason::LaunchExtentOverflow {
            rank: 2,
            extents: overflowing
        }]
    );
}

#[test]
fn physical_envelope_agrees_with_exact_for_dynamic_and_equal_static_coverage() {
    for domain in [
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
        LaunchDomain::D1 {
            x: LaunchExtent::Static(128),
        },
    ] {
        let module = noop(domain);
        assert_eq!(
            envelope(&module, 1, [128, 1, 1]),
            exact(&module, 1, [128, 1, 1])
        );
        let input = FormalPhysicalLaunchEnvelopeV2::new(1, [128, 1, 1]);
        assert_eq!(input.rank(), 1);
        assert_eq!(input.extents(), [128, 1, 1]);
        assert_eq!(
            derive_kernel_memory_obligations_for_physical_envelope_v2(
                &module,
                &module.kernels[0].id,
                input,
                FormalIndexWidth::Bits64
            )
            .unwrap(),
            envelope(&module, 1, [128, 1, 1]),
        );
    }
}

#[test]
fn physical_envelope_mixed_ranked_static_dynamic_bounds_flatten_without_truncation() {
    for (domain, rank, extents, count) in [
        (
            LaunchDomain::D2 {
                x: LaunchExtent::Static(2),
                y: LaunchExtent::Dynamic,
            },
            2,
            [4, 3, 1],
            12,
        ),
        (
            LaunchDomain::D3 {
                x: LaunchExtent::Dynamic,
                y: LaunchExtent::Static(2),
                z: LaunchExtent::Static(3),
            },
            3,
            [5, 4, 6],
            120,
        ),
    ] {
        let module = noop(domain);
        let original = module.clone();
        let report = envelope(&module, rank, extents);
        assert!(report.is_complete());
        assert_eq!(
            report.obligations().invocations().unwrap(),
            InvocationRange1d::from_count(count).unwrap()
        );
        assert!(report.obligations().accesses().is_empty());
        assert_eq!(
            row_major_invocation_index(
                rank,
                extents,
                [extents[0] - 1, extents[1] - 1, extents[2] - 1],
            ),
            Some(count - 1)
        );
        assert!(!exact(&module, rank, extents).is_complete());
        assert_eq!(module, original);
    }
}

#[test]
fn physical_envelope_refuses_insufficient_static_coverage_on_y_and_z() {
    for (domain, rank, extents, axis, expected, actual) in [
        (
            LaunchDomain::D2 {
                x: LaunchExtent::Dynamic,
                y: LaunchExtent::Static(3),
            },
            2,
            [5, 2, 1],
            Axis::Y,
            3,
            2,
        ),
        (
            LaunchDomain::D3 {
                x: LaunchExtent::Static(2),
                y: LaunchExtent::Dynamic,
                z: LaunchExtent::Static(5),
            },
            3,
            [3, 4, 4],
            Axis::Z,
            5,
            4,
        ),
    ] {
        let module = noop(domain);
        let original = module.clone();
        let report = envelope(&module, rank, extents);
        assert!(!report.is_complete());
        assert!(report.obligations().invocations().is_none());
        assert_eq!(
            report.incomplete_reasons(),
            &[
                FormalMemoryIncompleteReason::StaticLaunchAxisExtentMismatch {
                    axis,
                    expected,
                    actual,
                }
            ]
        );
        assert_eq!(module, original);
    }
}

fn conflict_source() -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let op = |id, ty, kind| Operation::effect_free(ValueDef::new(ValueId(id), ty), kind);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        op(
            2,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        op(3, Type::INDEX, OperationKind::Constant(Constant::Index(3))),
        op(
            4,
            pointer.clone(),
            OperationKind::SliceData { slice: ValueId(0) },
        ),
        op(
            5,
            pointer.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(4),
                offset: ValueId(2),
            },
        ),
        op(
            6,
            pointer,
            OperationKind::GetElementPointer {
                base: ValueId(4),
                offset: ValueId(3),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(5),
                value: ValueId(1),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
        op(
            7,
            scalar.clone(),
            OperationKind::Load {
                pointer: ValueId(6),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("static_extra_group_conflict");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(
            vec![
                Type::slice(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite),
                scalar,
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    ));
    module.kernels.push(Kernel::new(
        "root",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(2),
        },
    ));
    module
}

#[test]
fn physical_envelope_retains_extra_grid_cross_access_conflict_missing_at_static_extent() {
    let module = conflict_source();
    let original = module.clone();
    let small = exact(&module, 1, [2, 1, 1]);
    let full = envelope(&module, 1, [4, 1, 1]);
    assert!(small.is_complete() && full.is_complete());
    assert!(small.obligations().inter_invocation_conflicts().is_empty());
    assert_eq!(small.obligations().accesses().len(), 2);
    assert_eq!(full.obligations().accesses().len(), 2);
    let [conflict] = full.obligations().inter_invocation_conflicts() else {
        panic!("full physical envelope must retain the cross-access pair")
    };
    assert_eq!(
        conflict.left(),
        FunctionOperationLocation::new(BlockId(0), 5)
    );
    assert_eq!(
        conflict.right(),
        FunctionOperationLocation::new(BlockId(0), 6)
    );
    assert_eq!(module, original);
    assert_eq!(
        exact(&module, 1, [4, 1, 1]).incomplete_reasons(),
        &[FormalMemoryIncompleteReason::StaticLaunchExtentMismatch {
            expected: 2,
            actual: 4
        }]
    );
}

#[test]
fn physical_envelope_preserves_conflicting_stores_outside_original_static_extent() {
    let mut module = conflict_source();
    let block = &mut module.functions[0].body.as_mut().unwrap().blocks[0];
    block.operations.insert(
        4,
        Operation::effect_free(
            ValueDef::new(ValueId(8), Type::INDEX),
            OperationKind::Binary {
                op: crate::BinaryOp::Add,
                lhs: ValueId(2),
                rhs: ValueId(3),
            },
        ),
    );
    let OperationKind::GetElementPointer { offset, .. } = &mut block.operations[5].kind else {
        panic!("original second pointer offset")
    };
    *offset = ValueId(8);
    block.operations[7] = Operation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(6),
            value: ValueId(1),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    );
    let original = module.clone();
    let small = exact(&module, 1, [2, 1, 1]);
    let full = envelope(&module, 1, [4, 1, 1]);
    assert!(small.is_complete() && full.is_complete());
    assert!(small.obligations().inter_invocation_conflicts().is_empty());
    let [conflict] = full.obligations().inter_invocation_conflicts() else {
        panic!("x and x+3 writes overlap between invocations three and zero")
    };
    assert_eq!(
        conflict.left(),
        FunctionOperationLocation::new(BlockId(0), 6)
    );
    assert_eq!(
        conflict.right(),
        FunctionOperationLocation::new(BlockId(0), 7)
    );
    assert_eq!(module, original);
    assert_eq!(
        exact(&module, 1, [4, 1, 1]).incomplete_reasons(),
        &[FormalMemoryIncompleteReason::StaticLaunchExtentMismatch {
            expected: 2,
            actual: 4
        }],
    );
}

#[test]
fn physical_envelope_retains_unknown_width_and_original_verification_failures() {
    let module = noop(LaunchDomain::D1 {
        x: LaunchExtent::Static(64),
    });
    let input = FormalPhysicalLaunchEnvelopeV2::new(1, [128, 1, 1]);
    let analysis = derive_kernel_memory_obligations_for_physical_envelope_v2(
        &module,
        &module.kernels[0].id,
        input,
        FormalIndexWidth::Unknown,
    )
    .unwrap();
    assert_eq!(
        analysis.incomplete_reasons(),
        &[FormalMemoryIncompleteReason::UnsupportedIndexWidth {
            width: FormalIndexWidth::Unknown
        }]
    );
    assert!(matches!(
        derive_kernel_memory_obligations_for_physical_envelope_v2(
            &module,
            &KernelId::new("foreign"),
            input,
            FormalIndexWidth::Bits64
        ),
        Err(FormalMemoryObligationError::MissingKernel { .. })
    ));
    let mut invalid = module;
    invalid.functions.clear();
    assert!(matches!(
        derive_kernel_memory_obligations_for_physical_envelope_v2(
            &invalid,
            &invalid.kernels[0].id,
            input,
            FormalIndexWidth::Bits64
        ),
        Err(FormalMemoryObligationError::InvalidModule(_))
    ));
}
