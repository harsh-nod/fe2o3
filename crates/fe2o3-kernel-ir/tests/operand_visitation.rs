use std::collections::BTreeSet;

use fe2o3_kernel_ir::*;

fn assert_visitation(kind: OperationKind, expected: &[u32]) {
    let expected = expected.iter().copied().map(ValueId).collect::<Vec<_>>();
    assert_eq!(kind.operands(), expected);
    assert_eq!(kind.operand_count(), expected.len());

    let mut visited = Vec::new();
    kind.visit_operands(|value| visited.push(value));
    assert_eq!(visited, expected);
    visited.clear();
    let completed = kind.try_visit_operands(|value| {
        visited.push(value);
        Ok::<_, (usize, ValueId)>(())
    });
    assert_eq!(completed, Ok(()));
    assert_eq!(visited, expected);

    // Every rejection carries its exact input and stops before the next callback.
    for stop in 0..expected.len() {
        visited.clear();
        let rejected = kind.try_visit_operands(|value| {
            let ordinal = visited.len();
            visited.push(value);
            if ordinal == stop {
                Err((ordinal, value))
            } else {
                Ok(())
            }
        });
        assert_eq!(rejected, Err((stop, expected[stop])));
        assert_eq!(visited, expected[..=stop]);
    }
}

#[test]
fn operand_free_families_never_invoke_the_callback() {
    let semantics =
        BarrierSemantics::new(MemoryOrdering::AcquireRelease, [AddressSpace::Workgroup]);
    let mut kinds = vec![
        OperationKind::Constant(Constant::Index(0)),
        OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        OperationKind::Barrier(Barrier {
            execution_scope: SynchronizationScope::Workgroup,
            memory_scope: SynchronizationScope::Workgroup,
            semantics: semantics.clone(),
        }),
        OperationKind::Fence(Fence {
            memory_scope: SynchronizationScope::Workgroup,
            semantics: semantics.clone(),
        }),
        OperationKind::WorkgroupBarrier(WorkgroupBarrier {
            memory_scope: SynchronizationScope::Workgroup,
            semantics,
            convergence: Convergence::uniform(SynchronizationScope::Workgroup),
        }),
    ];
    for extent in [
        WorkgroupMemoryExtent::Static(1),
        WorkgroupMemoryExtent::Dynamic,
        WorkgroupMemoryExtent::DynamicAtLeast(1),
    ] {
        kinds.push(OperationKind::WorkgroupMemory(WorkgroupMemory {
            element: Type::INDEX,
            extent,
            alignment: 8,
        }));
    }
    for kind in kinds {
        assert_visitation(kind.clone(), &[]);
        assert_eq!(
            kind.try_visit_operands(|_| Err("unexpected operand")),
            Ok(())
        );
    }
}

#[test]
fn scalar_and_memory_access_families_preserve_exact_order_and_duplicates() {
    let a = ValueId(u32::MAX);
    let b = ValueId(0);
    let c = ValueId(7);
    let access = MemoryAccess::new(AddressSpace::Global, 4);
    let cases = vec![
        (
            OperationKind::Unary {
                op: UnaryOp::Not,
                operand: a,
            },
            vec![a.0],
        ),
        (
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: a,
                rhs: b,
            },
            vec![a.0, b.0],
        ),
        (
            OperationKind::Binary {
                op: BinaryOp::Checked(CheckedBinaryOperator::Add),
                lhs: a,
                rhs: a,
            },
            vec![a.0, a.0],
        ),
        (
            OperationKind::Compare {
                predicate: ComparePredicate::Equal,
                lhs: a,
                rhs: b,
            },
            vec![a.0, b.0],
        ),
        (
            OperationKind::Cast {
                kind: CastKind::Truncate,
                value: a,
                to: Type::BOOL,
            },
            vec![a.0],
        ),
        (
            OperationKind::Select {
                condition: c,
                true_value: a,
                false_value: a,
            },
            vec![c.0, a.0, a.0],
        ),
        (
            OperationKind::Call {
                callee: FunctionId::new("callee"),
                arguments: vec![a, b, a],
            },
            vec![a.0, b.0, a.0],
        ),
        (
            OperationKind::Call {
                callee: FunctionId::new("empty"),
                arguments: vec![],
            },
            vec![],
        ),
        (
            OperationKind::Alloca {
                element: Type::INDEX,
                count: None,
                address_space: AddressSpace::Private,
                alignment: 8,
            },
            vec![],
        ),
        (
            OperationKind::Alloca {
                element: Type::INDEX,
                count: Some(a),
                address_space: AddressSpace::Private,
                alignment: 8,
            },
            vec![a.0],
        ),
        (OperationKind::SliceLength { slice: a }, vec![a.0]),
        (OperationKind::SliceData { slice: b }, vec![b.0]),
        (
            OperationKind::GetElementPointer { base: a, offset: b },
            vec![a.0, b.0],
        ),
        (OperationKind::Load { pointer: a, access }, vec![a.0]),
        (
            OperationKind::GuardedLoad {
                pointer: a,
                predicate: b,
                fallback: c,
                access,
            },
            vec![a.0, b.0, c.0],
        ),
        (
            OperationKind::GuardedStore {
                pointer: a,
                predicate: b,
                value: c,
                access,
            },
            vec![a.0, b.0, c.0],
        ),
        (
            OperationKind::Store {
                pointer: a,
                value: b,
                access,
            },
            vec![a.0, b.0],
        ),
    ];
    for (kind, expected) in cases {
        assert_visitation(kind, &expected);
    }
}

#[test]
fn memory_intrinsic_families_preserve_all_operand_roles() {
    let element = MemoryElementType::Scalar(ScalarType::U32);
    let layout = MemoryLayout::new(4, 4);
    let cases = [
        (
            MemoryIntrinsicOperation::PointerDistance {
                pointer: ValueId(9),
                origin: ValueId(2),
                kind: PointerDistanceKind::Signed,
                unit: PointerDistanceUnit::Elements,
                element,
                address_space: AddressSpace::Global,
                layout,
                contract: PointerDistanceContract::supported_rust(PointerDistanceKind::Signed),
            },
            vec![9, 2],
        ),
        (
            MemoryIntrinsicOperation::VolatileLoad {
                pointer: ValueId(8),
                element,
                address_space: AddressSpace::Global,
                layout,
                contract: VolatileAccessContract::rust_allocation_load(),
            },
            vec![8],
        ),
        (
            MemoryIntrinsicOperation::VolatileStore {
                pointer: ValueId(4),
                value: ValueId(3),
                element,
                address_space: AddressSpace::Global,
                layout,
                contract: VolatileAccessContract::rust_allocation_store(),
            },
            vec![4, 3],
        ),
        (
            MemoryIntrinsicOperation::CopyNonOverlapping {
                source: ValueId(7),
                destination: ValueId(1),
                count: ValueId(0),
                element,
                source_address_space: AddressSpace::Global,
                destination_address_space: AddressSpace::Global,
                layout,
                contract: CopyNonOverlappingContract::supported_rust(),
            },
            vec![7, 1, 0],
        ),
    ];
    for (kind, expected) in cases {
        assert_visitation(OperationKind::MemoryIntrinsic(kind), &expected);
    }
}

#[test]
fn matrix_and_lds_transpose_families_preserve_fragment_and_state_order() {
    assert_visitation(
        OperationKind::Matrix(MatrixOperation::multiply_accumulate(
            [1, 2, 3, 4].map(ValueId),
            [5, 6, 7, 8].map(ValueId),
            [9, 10, 11, 12].map(ValueId),
        )),
        &(1..=12).collect::<Vec<_>>(),
    );
    assert_visitation(
        OperationKind::Matrix(MatrixOperation::scaled_multiply_accumulate_fp8_e4m3(
            [1, 2, 3, 4, 5, 6, 7, 8].map(ValueId),
            [9, 10, 11, 12, 13, 14, 15, 16].map(ValueId),
            [17, 18, 19, 20].map(ValueId),
        )),
        &(1..=20).collect::<Vec<_>>(),
    );
    assert_visitation(
        OperationKind::Matrix(MatrixOperation::lds_load(ValueId(3), MatrixElement::F32)),
        &[3],
    );
    assert_visitation(
        OperationKind::Matrix(MatrixOperation::lds_store(
            ValueId(5),
            [4, 3, 2, 1].map(ValueId),
            MatrixElement::F32,
        )),
        &[5, 4, 3, 2, 1],
    );
    let format = Gfx950LdsTransposeFormatV1::Fp8E4M3;
    for (kind, expected) in [
        (
            Gfx950LdsTransposeOperationKindV1::Current { format },
            vec![],
        ),
        (
            Gfx950LdsTransposeOperationKindV1::Stage {
                format,
                storage: ValueId(8),
                source_slice: ValueId(7),
                offset: ValueId(6),
                rows: ValueId(5),
                columns: ValueId(4),
                stride: ValueId(3),
                token_base: ValueId(2),
                reduction_base: ValueId(1),
            },
            vec![8, 7, 6, 5, 4, 3, 2, 1],
        ),
        (
            Gfx950LdsTransposeOperationKindV1::Publish {
                format,
                storage: ValueId(0),
            },
            vec![0],
        ),
        (
            Gfx950LdsTransposeOperationKindV1::Read {
                format,
                storage: ValueId(u32::MAX),
            },
            vec![u32::MAX],
        ),
    ] {
        assert_visitation(
            OperationKind::Gfx950LdsTranspose(Gfx950LdsTransposeOperationV1::full(kind)),
            &expected,
        );
    }
}

#[test]
fn wave_families_preserve_predicate_value_and_lane_order() {
    for (kind, expected) in [
        (WaveOperationKind::LaneId, vec![]),
        (
            WaveOperationKind::Ballot {
                predicate: ValueId(9),
            },
            vec![9],
        ),
        (
            WaveOperationKind::Any {
                predicate: ValueId(8),
            },
            vec![8],
        ),
        (
            WaveOperationKind::All {
                predicate: ValueId(7),
            },
            vec![7],
        ),
        (
            WaveOperationKind::ShuffleIndex {
                value: ValueId(6),
                source_lane: ValueId(5),
                tile_width: 64,
            },
            vec![6, 5],
        ),
        (
            WaveOperationKind::ReduceF32 {
                value: ValueId(4),
                tile_width: 64,
                kind: WaveF32ReductionKindV1::Sum,
            },
            vec![4],
        ),
        (
            WaveOperationKind::BroadcastF32 {
                value: ValueId(3),
                source_lane: ValueId(2),
                tile_width: 64,
            },
            vec![3, 2],
        ),
    ] {
        assert_visitation(
            OperationKind::Wave(WaveOperation::full(kind, WaveWidth::Wave64)),
            &expected,
        );
    }
}

#[test]
fn atomic_options_and_assembly_non_operands_keep_exact_prefixes() {
    for value in [None, Some(ValueId(0))] {
        for compare in [None, Some(ValueId(u32::MAX))] {
            let mut expected = vec![7];
            expected.extend(value.map(|id| id.0));
            expected.extend(compare.map(|id| id.0));
            assert_visitation(
                OperationKind::Atomic(Atomic {
                    kind: AtomicKind::CompareExchange,
                    pointer: ValueId(7),
                    value,
                    compare,
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                    scope: SynchronizationScope::Device,
                    ordering: MemoryOrdering::AcquireRelease,
                    failure_ordering: Some(MemoryOrdering::Acquire),
                }),
                &expected,
            );
        }
    }
    assert_visitation(
        OperationKind::InlineAssembly(InlineAssembly {
            target: InlineAssemblyTarget::AmdGpuGfx942,
            source: AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [4; 32]),
            mnemonic: "v_add_u32".to_owned(),
            operands: vec![
                AssemblyOperand::output(0, AssemblyConstraint::Vgpr32),
                AssemblyOperand::input(ValueId(u32::MAX), AssemblyConstraint::Vgpr32),
                AssemblyOperand {
                    kind: AssemblyOperandKind::ImmediateI32(-1),
                    constraint: AssemblyConstraint::ImmediateI32,
                },
                AssemblyOperand {
                    kind: AssemblyOperandKind::InOut {
                        input: ValueId(0),
                        result_index: 1,
                    },
                    constraint: AssemblyConstraint::Vgpr32,
                },
                AssemblyOperand::input(ValueId(u32::MAX), AssemblyConstraint::Vgpr32),
            ],
            options: BTreeSet::new(),
            declared_effects: BTreeSet::new(),
        }),
        &[u32::MAX, 0, u32::MAX],
    );
}

#[test]
fn verifier_keeps_duplicate_use_diagnostics_after_visitation() {
    let mut block = BasicBlock::new(BlockId(9));
    block.operations.push(Operation::new(
        vec![ValueDef::new(ValueId(1), Type::INDEX)],
        OperationKind::Binary {
            op: BinaryOp::Add,
            lhs: ValueId(u32::MAX),
            rhs: ValueId(u32::MAX),
        },
    ));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("operand-diagnostics");
    module.functions.push(Function::definition(
        "entry",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    let expected = Diagnostic {
        location: DiagnosticLocation {
            module: module.id.clone(),
            function: Some(FunctionId::new("entry")),
            kernel: None,
            block: Some(BlockId(9)),
            operation: Some(0),
        },
        code: DiagnosticCode::UndefinedValue,
        message: "SSA value %4294967295 is not defined in this function".to_owned(),
    };
    assert_eq!(
        verify_module(&module).unwrap_err().diagnostics(),
        &[expected.clone(), expected],
    );
}
