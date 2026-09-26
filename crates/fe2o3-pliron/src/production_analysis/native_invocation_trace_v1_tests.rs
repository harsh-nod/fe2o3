use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirInventoryV1 as Inventory, CanonicalRankedMetadataV1 as Metadata,
    CanonicalRankedViewErrorV1, build_canonical_ranked_candidate_v1,
    with_checked_canonical_ranked_view_v1,
};
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, Atomic, AtomicKind, BasicBlock, BlockId,
    CanonicalKernelIrWorkBudgetV1 as Work, Constant, Function, Kernel, LaunchDomain, LaunchExtent,
    MemoryAccess, MemoryOrdering, Module, Operation, OperationKind, ScalarType, Signature,
    SynchronizationScope, Terminator, Type, ValueDef, ValueId,
};

pub(super) const WORK: usize = 1 << 48;
pub(super) const STORAGE: usize = 1 << 32;

pub(super) fn owner(module: &Module) -> (Owner, usize) {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (owner, storage) =
        Owner::from_module_ref_with_verification_budget_v12(module, &mut budget).unwrap();
    (owner, storage.retained_storage())
}

pub(super) fn with_checked<T>(
    module: &Module,
    run: impl FnOnce(&mut CheckedCanonicalRankedViewV1<'_, '_, '_, '_>, &mut Budget<'_>) -> T,
) -> T {
    with_checked_space(module, None, run)
}

pub(super) fn with_checked_space<T>(
    module: &Module,
    spare_storage: Option<usize>,
    run: impl FnOnce(&mut CheckedCanonicalRankedViewV1<'_, '_, '_, '_>, &mut Budget<'_>) -> T,
) -> T {
    let (owner, storage) = owner(module);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(storage).unwrap();
    let (inventory, inventory_storage) = Inventory::derive(&owner, &mut budget).unwrap();
    budget
        .reserve_storage(inventory_storage.retained_storage())
        .unwrap();
    let metadata = Metadata::new(&owner, &[]);
    let metadata_storage = metadata.storage_extent(&mut budget).unwrap();
    budget.reserve_storage(metadata_storage).unwrap();
    let (candidate, candidate_storage) =
        build_canonical_ranked_candidate_v1(&inventory, &metadata, &mut budget).unwrap();
    budget
        .reserve_storage(candidate_storage.retained_storage())
        .unwrap();
    // This independently stated same-host Accounting field roster is checked
    // at callback entry. It is never obtained from a successful trace run.
    let foundation_header = size_of::<(
        usize,
        Ledger,
        usize,
        usize,
        Option<CanonicalRankedViewErrorV1>,
        bool,
    )>() + size_of::<CheckedCanonicalRankedViewV1<'_, '_, '_, '_>>();
    let padding = spare_storage.map_or(0, |spare| {
        STORAGE - budget.storage() - foundation_header - spare
    });
    budget.reserve_storage(padding).unwrap();
    let floor = budget.storage();
    let result = with_checked_canonical_ranked_view_v1(
        &inventory,
        &metadata,
        &candidate,
        &mut budget,
        |checked, budget| {
            assert_eq!(budget.storage(), floor + foundation_header);
            Ok::<_, CanonicalRankedViewErrorV1>(run(checked, budget))
        },
    )
    .unwrap();
    assert_eq!(budget.storage(), floor);
    budget.release_storage(padding).unwrap();
    drop(candidate);
    budget
        .release_storage(candidate_storage.retained_storage())
        .unwrap();
    drop(metadata);
    budget.release_storage(metadata_storage).unwrap();
    drop(inventory);
    budget
        .release_storage(inventory_storage.retained_storage())
        .unwrap();
    drop(owner);
    budget.release_storage(storage).unwrap();
    assert_eq!(budget.storage(), 0);
    result
}

pub(super) fn noop() -> Module {
    let mut block = BasicBlock::new(BlockId(9));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("native-trace");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    module.kernels.push(Kernel::new(
        "root",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(1),
        },
    ));
    module
}

pub(super) fn memory() -> Module {
    let mut module = noop();
    let scalar = Type::Scalar(ScalarType::U32);
    module.functions[0].signature.parameters = vec![Type::pointer(
        scalar.clone(),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    )];
    let body = module.functions[0].body.as_mut().unwrap();
    body.parameters = vec![ValueId(1)];
    body.blocks[0].operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(2), scalar),
            OperationKind::Load {
                pointer: ValueId(1),
                access: MemoryAccess {
                    address_space: AddressSpace::Global,
                    alignment: 4,
                    volatile: true,
                },
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(1),
                value: ValueId(2),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    module
}

#[test]
fn native_store_and_return_have_nonempty_exact_subjects() {
    with_checked(&memory(), |checked, budget| {
        let original = checked.inventory(budget).unwrap().owner();
        let bytes = original.canonical().canonical_bytes().to_vec();
        with_canonical_invocation_traces_v1(checked, budget, |view, budget| {
            assert!(std::ptr::eq(original, view.owner(budget)?));
            assert_eq!(
                view.attempt(0, budget)?,
                CanonicalInvocationTraceAttemptV1::Complete {
                    invocations: 1,
                    events: 3
                }
            );
            let read = view.event(0, 0, 0, budget)?;
            assert_eq!(read.kind, Some(NativeEventKindV1::Load));
            let CanonicalNativeSubjectV1::Operation(operation) = read.subject else {
                panic!("expected read")
            };
            assert!(matches!(
                operation.kind,
                OperationKind::Load {
                    access: MemoryAccess {
                        volatile: true,
                        alignment: 4,
                        ..
                    },
                    ..
                }
            ));
            assert_eq!(read.address.unwrap().base_subject, ValueId(1));
            assert_eq!(
                view.event(0, 0, 1, budget)?.kind,
                Some(NativeEventKindV1::Store)
            );
            assert_eq!(
                view.event(0, 0, 2, budget)?.kind,
                Some(NativeEventKindV1::Return)
            );
            assert!(
                view.remaining_obligations(0, budget)?
                    .unwrap()
                    .provenance_and_alias
            );
            Ok(())
        })
        .unwrap();
        assert_eq!(original.canonical().canonical_bytes(), bytes);
    });
}

#[test]
fn alloca_is_an_event_and_not_an_alias_proof() {
    let mut module = noop();
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .push(Operation::effect_free(
            ValueDef::new(
                ValueId(3),
                Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Private,
                    AccessMode::ReadWrite,
                ),
            ),
            OperationKind::Alloca {
                element: Type::Scalar(ScalarType::U32),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ));
    with_checked(&module, |checked, budget| {
        with_canonical_invocation_traces_v1(checked, budget, |view, budget| {
            assert_eq!(
                view.attempt(0, budget)?,
                CanonicalInvocationTraceAttemptV1::Complete {
                    invocations: 1,
                    events: 2
                }
            );
            assert_eq!(
                view.event(0, 0, 0, budget)?.kind,
                Some(NativeEventKindV1::Alloca)
            );
            assert!(
                view.remaining_obligations(0, budget)?
                    .unwrap()
                    .initialization_and_lifetime
            );
            Ok(())
        })
        .unwrap();
    });
}

pub(super) fn atomic(kind: AtomicKind) -> Module {
    let mut module = memory();
    let body = module.functions[0].body.as_mut().unwrap();
    let scalar = Type::Scalar(ScalarType::U32);
    let value = (kind != AtomicKind::Load).then_some(ValueId(2));
    let compare = (kind == AtomicKind::CompareExchange).then_some(ValueId(3));
    let mut results = if kind == AtomicKind::Store {
        vec![]
    } else {
        vec![ValueDef::new(ValueId(4), scalar.clone())]
    };
    if kind == AtomicKind::CompareExchange {
        results.push(ValueDef::new(ValueId(5), Type::BOOL));
    }
    body.blocks[0].operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(2), scalar.clone()),
            OperationKind::Constant(Constant::U32(7)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(3), scalar),
            OperationKind::Constant(Constant::U32(9)),
        ),
        Operation::new(
            results,
            OperationKind::Atomic(Atomic {
                kind,
                pointer: ValueId(1),
                value,
                compare,
                access: MemoryAccess::new(AddressSpace::Global, 4),
                scope: SynchronizationScope::Device,
                ordering: MemoryOrdering::SequentiallyConsistent,
                failure_ordering: compare.map(|_| MemoryOrdering::Acquire),
            }),
        ),
    ];
    module
}

#[test]
fn every_atomic_variant_retains_its_full_canonical_payload() {
    for kind in [
        AtomicKind::Load,
        AtomicKind::Store,
        AtomicKind::Exchange,
        AtomicKind::CompareExchange,
        AtomicKind::Add,
        AtomicKind::Subtract,
        AtomicKind::Min,
        AtomicKind::Max,
        AtomicKind::BitAnd,
        AtomicKind::BitOr,
        AtomicKind::BitXor,
    ] {
        let module = atomic(kind);
        with_checked(&module, |checked, budget| {
            with_canonical_invocation_traces_v1(checked, budget, |view, budget| {
                assert_eq!(
                    view.attempt(0, budget)?,
                    CanonicalInvocationTraceAttemptV1::Complete {
                        invocations: 1,
                        events: 2
                    }
                );
                let event = view.event(0, 0, 0, budget)?;
                assert_eq!(event.kind, Some(NativeEventKindV1::Atomic));
                let CanonicalNativeSubjectV1::Operation(operation) = event.subject else {
                    panic!("atomic")
                };
                assert_eq!(
                    operation,
                    &module.functions[0].body.as_ref().unwrap().blocks[0].operations[2]
                );
                assert!(
                    view.remaining_obligations(0, budget)?
                        .unwrap()
                        .memory_order_and_atomic_outcomes
                );
                Ok(())
            })
            .unwrap();
        });
    }
}

#[test]
fn same_preserved_atomic_family_keeps_distinct_metadata_and_repeated_alias_slots() {
    let mut module = atomic(AtomicKind::CompareExchange);
    let operations = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
    let mut second = operations[2].clone();
    second.results[0].id = ValueId(6);
    second.results[1].id = ValueId(7);
    let OperationKind::Atomic(atomic) = &mut second.kind else {
        unreachable!()
    };
    atomic.compare = atomic.value;
    atomic.scope = SynchronizationScope::System;
    atomic.ordering = MemoryOrdering::AcquireRelease;
    atomic.failure_ordering = Some(MemoryOrdering::Relaxed);
    operations.push(second);
    with_checked(&module, |checked, budget| {
        with_canonical_invocation_traces_v1(checked, budget, |view, budget| {
            assert_eq!(
                view.attempt(0, budget)?,
                CanonicalInvocationTraceAttemptV1::Complete {
                    invocations: 1,
                    events: 3
                }
            );
            for (event_index, operation_index) in [(0, 2), (1, 3)] {
                let event = view.event(0, 0, event_index, budget)?;
                assert_eq!(event.kind, Some(NativeEventKindV1::Atomic));
                assert_eq!(event.operation, operation_index);
                let CanonicalNativeSubjectV1::Operation(subject) = event.subject else {
                    panic!("atomic subject")
                };
                assert_eq!(
                    subject,
                    &module.functions[0].body.as_ref().unwrap().blocks[0].operations
                        [operation_index]
                );
            }
            assert!(
                view.remaining_obligations(0, budget)?
                    .unwrap()
                    .memory_order_and_atomic_outcomes
            );
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn workgroup_barrier_retains_one_arrival_and_all_address_spaces() {
    use fe2o3_kernel_ir::{Barrier, BarrierSemantics, Fence, WorkgroupSize};
    let mut module = noop();
    module.kernels[0].domain = LaunchDomain::D1 {
        x: LaunchExtent::Static(4),
    };
    module.kernels[0].workgroup_size = Some(WorkgroupSize::new(4, 1, 1));
    let semantics = BarrierSemantics::new(
        MemoryOrdering::AcquireRelease,
        [AddressSpace::Workgroup, AddressSpace::Global],
    );
    module.functions[0].body.as_mut().unwrap().blocks[0].operations = vec![
        Operation::new(
            vec![],
            OperationKind::Barrier(Barrier {
                execution_scope: SynchronizationScope::Workgroup,
                memory_scope: SynchronizationScope::Workgroup,
                semantics: semantics.clone(),
            }),
        ),
        Operation::new(
            vec![],
            OperationKind::Fence(Fence {
                memory_scope: SynchronizationScope::Workgroup,
                semantics,
            }),
        ),
    ];
    with_checked(&module, |checked, budget| {
        with_canonical_invocation_traces_v1(checked, budget, |view, budget| {
            assert_eq!(
                view.attempt(0, budget)?,
                CanonicalInvocationTraceAttemptV1::Complete {
                    invocations: 4,
                    events: 12
                }
            );
            assert_eq!(
                view.event(0, 0, 0, budget)?.synchronization_address_spaces,
                Some(6)
            );
            assert_eq!(
                view.event(0, 0, 1, budget)?.synchronization_address_spaces,
                Some(6)
            );
            assert_eq!(view.barrier_divergent(0, budget)?, Some(false));
            assert_eq!(view.geometry(0, budget)?.unwrap().subgroup_size, None);
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn repeated_roots_are_separate_attempts_and_unused_functions_remain_visible() {
    let mut module = noop();
    module.kernels.push(Kernel::new(
        "second",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(2),
        },
    ));
    let mut block = BasicBlock::new(BlockId(3));
    block.terminator = Some(Terminator::Return { values: vec![] });
    module.functions.push(Function::internal_helper(
        "unused",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    module.functions.push(Function::declaration(
        "external_unused",
        Signature::new(vec![], vec![]),
    ));
    with_checked(&module, |checked, budget| {
        with_canonical_invocation_traces_v1(checked, budget, |view, budget| {
            assert_eq!(view.function_count(budget)?, 3);
            assert_eq!(view.operation_count(1, budget)?, 1);
            assert!(view.is_declaration(2, budget)?);
            assert_eq!(
                view.function_census(2, budget)?,
                CanonicalNativeFunctionCensusV1::Declaration
            );
            assert_eq!(view.root_count(budget)?, 2);
            assert_eq!(
                view.attempt(0, budget)?,
                CanonicalInvocationTraceAttemptV1::Complete {
                    invocations: 1,
                    events: 1
                }
            );
            assert_eq!(
                view.attempt(1, budget)?,
                CanonicalInvocationTraceAttemptV1::Complete {
                    invocations: 2,
                    events: 2
                }
            );
            Ok(())
        })
        .unwrap();
    });
}
