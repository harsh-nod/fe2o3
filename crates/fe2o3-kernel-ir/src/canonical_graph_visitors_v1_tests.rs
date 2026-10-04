use super::*;
use crate::{
    AssemblySourceIdentity, Atomic, AtomicKind, Barrier, BarrierSemantics, Constant, Convergence,
    CopyNonOverlappingContract, Fence, FixedVectorTypeV12, Gfx950LdsTransposeFormatV1,
    Gfx950LdsTransposeOperationV1, InlineAssembly, InlineAssemblyTarget, IntegerSwitchCase,
    IntrinsicOperation, MatrixElement, MatrixOperation, MemoryAccess, MemoryLayout, ScalarType,
    SwitchCase, Type, VectorLayoutV12, VectorLoadOperationV12, VectorMemoryAccessV12,
    VectorStoreOperationV12, VolatileAccessContract, WorkgroupBarrier, WorkgroupMemory,
    WorkgroupMemoryExtent,
};

#[test]
fn all_terminators_preserve_successor_order_arguments_and_early_failure() {
    let terms = [
        Terminator::Branch {
            target: BlockId(99),
            arguments: vec![ValueId(100)],
        },
        Terminator::ConditionalBranch {
            condition: ValueId(1),
            then_target: BlockId(9),
            then_arguments: vec![ValueId(3)],
            else_target: BlockId(9),
            else_arguments: vec![ValueId(4)],
        },
        Terminator::Switch {
            selector: ValueId(1),
            cases: vec![
                SwitchCase {
                    value: 0,
                    target: BlockId(9),
                    arguments: vec![ValueId(5)],
                },
                SwitchCase {
                    value: 1,
                    target: BlockId(9),
                    arguments: vec![ValueId(6)],
                },
            ],
            default_target: BlockId(8),
            default_arguments: vec![ValueId(7)],
        },
        Terminator::IntegerSwitch {
            selector: ValueId(1),
            cases: vec![
                IntegerSwitchCase {
                    value: Constant::U32(0),
                    target: BlockId(9),
                    arguments: vec![ValueId(5)],
                },
                IntegerSwitchCase {
                    value: Constant::U32(1),
                    target: BlockId(9),
                    arguments: vec![ValueId(6)],
                },
            ],
            default_target: BlockId(8),
            default_arguments: vec![ValueId(7)],
        },
        Terminator::Return {
            values: vec![ValueId(1)],
        },
        Terminator::Unreachable,
    ];
    for term in &terms {
        let mut visited = Vec::new();
        term.try_visit_edges_v1(|target, arguments| {
            visited.push((target, arguments));
            Ok::<_, ()>(())
        })
        .unwrap();
        assert_eq!(
            visited.iter().map(|row| row.0).collect::<Vec<_>>(),
            term.successors()
        );
        let expected: Vec<&[ValueId]> = match term {
            Terminator::Branch { arguments, .. } => vec![arguments],
            Terminator::ConditionalBranch {
                then_arguments,
                else_arguments,
                ..
            } => vec![then_arguments, else_arguments],
            Terminator::Switch {
                cases,
                default_arguments,
                ..
            } => cases
                .iter()
                .map(|case| case.arguments.as_slice())
                .chain([default_arguments.as_slice()])
                .collect(),
            Terminator::IntegerSwitch {
                cases,
                default_arguments,
                ..
            } => cases
                .iter()
                .map(|case| case.arguments.as_slice())
                .chain([default_arguments.as_slice()])
                .collect(),
            Terminator::Return { .. } | Terminator::Unreachable => vec![],
        };
        assert_eq!(visited.len(), expected.len());
        for ((_, actual), expected) in visited.iter().zip(expected) {
            assert!(std::ptr::eq(*actual, expected));
        }
        let mut visits = 0;
        let result = term.try_visit_edges_v1(|_, _| {
            visits += 1;
            Err::<(), _>(19)
        });
        assert_eq!(visits, usize::from(!visited.is_empty()));
        assert_eq!(result, if visited.is_empty() { Ok(()) } else { Err(19) });
    }
}

#[test]
fn borrowed_local_effects_match_allocating_order_and_stop_before_second_effect() {
    let access = MemoryAccess {
        address_space: AddressSpace::Global,
        alignment: 4,
        volatile: true,
    };
    let vector_access = VectorMemoryAccessV12::new(
        FixedVectorTypeV12::new(ScalarType::U32, 4, VectorLayoutV12::Contiguous),
        access,
    );
    let semantics = BarrierSemantics::new(
        MemoryOrdering::AcquireRelease,
        [AddressSpace::Global, AddressSpace::Workgroup],
    );
    let format = Gfx950LdsTransposeFormatV1::Fp8E4M3;
    // These local visitor fixtures deliberately do not claim module validity.
    // The connected-owner inventory tests exercise verified modules separately.
    let mut kinds = vec![
        OperationKind::Alloca {
            element: Type::INDEX,
            count: None,
            address_space: AddressSpace::Private,
            alignment: 8,
        },
        OperationKind::Load {
            pointer: ValueId(1),
            access,
        },
        OperationKind::GuardedLoad {
            pointer: ValueId(1),
            predicate: ValueId(2),
            fallback: ValueId(3),
            access,
        },
        OperationKind::Store {
            pointer: ValueId(1),
            value: ValueId(2),
            access,
        },
        OperationKind::GuardedStore {
            pointer: ValueId(1),
            predicate: ValueId(2),
            value: ValueId(3),
            access,
        },
        OperationKind::VectorLoad(VectorLoadOperationV12::new(ValueId(1), vector_access)),
        OperationKind::VectorStore(VectorStoreOperationV12::new(
            ValueId(1),
            ValueId(2),
            vector_access,
        )),
        OperationKind::Atomic(Atomic {
            kind: AtomicKind::CompareExchange,
            pointer: ValueId(1),
            value: Some(ValueId(2)),
            compare: Some(ValueId(3)),
            access,
            scope: SynchronizationScope::Device,
            ordering: MemoryOrdering::AcquireRelease,
            failure_ordering: Some(MemoryOrdering::Acquire),
        }),
        OperationKind::Barrier(Barrier {
            execution_scope: SynchronizationScope::Workgroup,
            memory_scope: SynchronizationScope::Device,
            semantics: semantics.clone(),
        }),
        OperationKind::Fence(Fence {
            memory_scope: SynchronizationScope::Device,
            semantics: semantics.clone(),
        }),
        OperationKind::WorkgroupBarrier(WorkgroupBarrier {
            memory_scope: SynchronizationScope::Workgroup,
            semantics,
            convergence: Convergence::uniform(SynchronizationScope::Workgroup),
        }),
        OperationKind::WorkgroupMemory(WorkgroupMemory {
            element: Type::INDEX,
            extent: WorkgroupMemoryExtent::Static(4),
            alignment: 8,
        }),
        OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::CopyNonOverlapping {
            source: ValueId(1),
            destination: ValueId(2),
            count: ValueId(3),
            element: MemoryElementType::Scalar(ScalarType::U32),
            source_address_space: AddressSpace::Global,
            destination_address_space: AddressSpace::Workgroup,
            layout: MemoryLayout::new(4, 4),
            contract: CopyNonOverlappingContract::supported_rust(),
        }),
        OperationKind::Matrix(MatrixOperation::lds_load(ValueId(1), MatrixElement::Bf16)),
        OperationKind::Matrix(MatrixOperation::lds_store(
            ValueId(1),
            [ValueId(2); 4],
            MatrixElement::Bf16,
        )),
        OperationKind::Matrix(MatrixOperation::multiply_accumulate(
            [ValueId(1); 4],
            [ValueId(2); 4],
            [ValueId(3); 4],
        )),
        OperationKind::Matrix(MatrixOperation::scaled_multiply_accumulate_fp8_e4m3(
            [ValueId(1); 8],
            [ValueId(2); 8],
            [ValueId(3); 4],
        )),
        OperationKind::Gfx950LdsTranspose(Gfx950LdsTransposeOperationV1::full(
            Gfx950LdsTransposeOperationKindV1::Current { format },
        )),
        OperationKind::Gfx950LdsTranspose(Gfx950LdsTransposeOperationV1::full(
            Gfx950LdsTransposeOperationKindV1::Stage {
                format,
                storage: ValueId(1),
                source_slice: ValueId(2),
                offset: ValueId(3),
                rows: ValueId(4),
                columns: ValueId(5),
                stride: ValueId(6),
                token_base: ValueId(7),
                reduction_base: ValueId(8),
            },
        )),
        OperationKind::Gfx950LdsTranspose(Gfx950LdsTransposeOperationV1::full(
            Gfx950LdsTransposeOperationKindV1::Publish {
                format,
                storage: ValueId(1),
            },
        )),
        OperationKind::Gfx950LdsTranspose(Gfx950LdsTransposeOperationV1::full(
            Gfx950LdsTransposeOperationKindV1::Read {
                format,
                storage: ValueId(1),
            },
        )),
        OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        OperationKind::Intrinsic(IntrinsicOperation::launch_extent_1d()),
        OperationKind::Call {
            callee: "unresolved".into(),
            arguments: vec![ValueId(1)],
        },
        OperationKind::InlineAssembly(InlineAssembly {
            target: InlineAssemblyTarget::AmdGpuGfx942,
            source: AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [4; 32]),
            mnemonic: "inert-test-payload".into(),
            operands: vec![],
            options: BTreeSet::new(),
            declared_effects: BTreeSet::from([
                AssemblyEffect::ReadGlobal,
                AssemblyEffect::WriteGlobal,
                AssemblyEffect::ReadWorkgroup,
                AssemblyEffect::WriteWorkgroup,
                AssemblyEffect::Atomic,
                AssemblyEffect::Barrier,
                AssemblyEffect::ControlFlow,
            ]),
        }),
    ];
    for element in [
        MemoryElementType::Unit,
        MemoryElementType::Scalar(ScalarType::U32),
    ] {
        kinds.push(OperationKind::MemoryIntrinsic(
            MemoryIntrinsicOperation::VolatileLoad {
                pointer: ValueId(1),
                element,
                address_space: AddressSpace::Global,
                layout: MemoryLayout::new(
                    if element == MemoryElementType::Unit {
                        0
                    } else {
                        4
                    },
                    4,
                ),
                contract: if element == MemoryElementType::Unit {
                    VolatileAccessContract::zero_sized_aligned_no_access()
                } else {
                    VolatileAccessContract::rust_allocation_load()
                },
            },
        ));
        kinds.push(OperationKind::MemoryIntrinsic(
            MemoryIntrinsicOperation::VolatileStore {
                pointer: ValueId(1),
                value: ValueId(2),
                element,
                address_space: AddressSpace::Global,
                layout: MemoryLayout::new(
                    if element == MemoryElementType::Unit {
                        0
                    } else {
                        4
                    },
                    4,
                ),
                contract: if element == MemoryElementType::Unit {
                    VolatileAccessContract::zero_sized_aligned_no_access()
                } else {
                    VolatileAccessContract::rust_allocation_store()
                },
            },
        ));
    }
    for kind in kinds {
        let operation = Operation::new(vec![], kind);
        let mut borrowed = Vec::new();
        operation
            .try_visit_local_memory_effects_v1(|effect| {
                borrowed.push(effect);
                Ok::<_, ()>(())
            })
            .unwrap();
        assert_eq!(
            borrowed
                .iter()
                .map(|effect| (*effect).to_owned())
                .collect::<Vec<_>>(),
            operation.memory_effects()
        );
        let mut visits = 0;
        let stopped = operation.try_visit_local_memory_effects_v1(|_| {
            visits += 1;
            Err::<(), _>(23)
        });
        assert_eq!(visits, usize::from(!borrowed.is_empty()));
        assert_eq!(stopped, if borrowed.is_empty() { Ok(()) } else { Err(23) });
        if let OperationKind::Barrier(barrier) = &operation.kind {
            assert!(
                matches!(borrowed[0], KirLocalMemoryEffectRefV1::Synchronize {
                address_spaces: KirAddressSpacesRefV1::Borrowed(spaces), ..
            } if std::ptr::eq(spaces, &barrier.semantics.address_spaces))
            );
        }
        if let OperationKind::Atomic(atomic) = &operation.kind {
            assert_eq!(atomic.failure_ordering, Some(MemoryOrdering::Acquire));
        }
        // A legacy volatile Load is still a Read in memory_effects; retaining
        // the exact operation is required to inspect the original access flag.
        if let OperationKind::Load { access, .. } = &operation.kind {
            assert!(access.volatile);
            assert!(matches!(
                borrowed[0],
                KirLocalMemoryEffectRefV1::Read(AddressSpace::Global)
            ));
        }
    }
}

#[test]
fn coordinate_kinds_do_not_conflate_function_arguments_block_arguments_and_results() {
    use crate::{
        CanonicalKirBlockCoordinateV1 as Block, CanonicalKirDefinitionCoordinateV1 as Definition,
        CanonicalKirFunctionCoordinateV1 as Function,
        CanonicalKirOperationCoordinateV1 as Operation,
    };
    let function = Function(5);
    let block = Block { function, block: 7 };
    let operation = Operation {
        block,
        operation: 11,
    };
    let identities = BTreeSet::from([
        Definition::FunctionArgument {
            function,
            argument: 0,
        },
        Definition::BlockArgument { block, argument: 0 },
        Definition::Result {
            operation,
            result: 0,
        },
        Definition::Result {
            operation,
            result: 1,
        },
    ]);
    assert_eq!(identities.len(), 4);
}
