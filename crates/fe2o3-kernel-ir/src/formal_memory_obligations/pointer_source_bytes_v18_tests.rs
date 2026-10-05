use super::*;
use crate::{
    BasicBlock, Constant, Kernel, Signature, StorageLayoutKindV1, StorageLayoutLimitsV1,
    StorageLayoutV1, Terminator, ValueDef,
};

mod access_tests {
    include!("access_source_bytes_v18_tests.rs");
}

mod body_tests {
    include!("body_source_v19_tests.rs");
}

mod original {
    use std::result::Result;
    include!("pointer_original_reference_v18_tests.rs");
    include!("pointer_observation_v18_tests.rs");

    // The shared guarded helper uses the current nominal error; the frozen
    // reference algorithm owns a separate error with the same two payloads.
    impl From<crate::formal_memory_obligations::pointer_derivation::AccessDerivationError>
        for AccessDerivationError
    {
        fn from(
            error: crate::formal_memory_obligations::pointer_derivation::AccessDerivationError,
        ) -> Self {
            use crate::formal_memory_obligations::pointer_derivation::AccessDerivationError as Current;
            match error {
                Current::Incomplete(reason) => Self::Incomplete(reason),
                Current::Resource(error) => Self::Resource(error),
            }
        }
    }
}

#[test]
fn original_pointer_oracle_preserves_both_shared_guard_error_payloads() {
    use crate::formal_memory_obligations::pointer_derivation::AccessDerivationError as Current;
    let reason = FormalMemoryIncompleteReason::UnsupportedPointerDerivation {
        location: FunctionOperationLocation {
            block: BlockId(91),
            operation_index: 7,
        },
        pointer: ValueId(123),
    };
    match original::AccessDerivationError::from(Current::Incomplete(reason.clone())) {
        original::AccessDerivationError::Incomplete(actual) => assert_eq!(actual, reason),
        original::AccessDerivationError::Resource(_) => panic!("semantic reason changed category"),
    }
    assert!(matches!(
        original::AccessDerivationError::from(Current::Resource(ResourceError::Arithmetic)),
        original::AccessDerivationError::Resource(ResourceError::Arithmetic)
    ));
}

type Observation = (
    std::result::Result<FormalAllocationIdentity, FormalMemoryIncompleteReason>,
    std::result::Result<(FormalAllocationIdentity, ByteExpression), FormalMemoryIncompleteReason>,
);
fn pointer(space: AddressSpace) -> Type {
    Type::pointer(Type::Scalar(ScalarType::U32), space, AccessMode::ReadWrite)
}
fn op(id: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(id), ty), kind)
}
fn index(id: u32, value: u64) -> Operation {
    op(
        id,
        Type::INDEX,
        OperationKind::Constant(Constant::Index(value)),
    )
}
fn gep(id: u32, base: u32, offset: u32) -> Operation {
    op(
        id,
        pointer(AddressSpace::Global),
        OperationKind::GetElementPointer {
            base: ValueId(base),
            offset: ValueId(offset),
        },
    )
}
fn branch(target: u32, arguments: &[u32]) -> Option<Terminator> {
    Some(Terminator::Branch {
        target: BlockId(target),
        arguments: arguments.iter().map(|&v| ValueId(v)).collect(),
    })
}
fn module(blocks: Vec<BasicBlock>) -> Module {
    let mut module = Module::new("paid_pointer_source");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(
            vec![
                pointer(AddressSpace::Global),
                pointer(AddressSpace::Global),
                Type::BOOL,
                Type::INDEX,
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
        blocks,
    ));
    module.kernels.push(Kernel::new(
        "root",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module.storage_layouts.push(StorageLayoutV1 {
        size: 4,
        alignment: 4,
        kind: StorageLayoutKindV1::Scalar(ScalarType::U32),
    });
    module
}
fn straight() -> Module {
    let mut entry = BasicBlock::new(BlockId(0));
    let generic = pointer(AddressSpace::Generic);
    entry.operations = vec![
        index(10, 2),
        gep(11, 0, 10),
        op(
            12,
            generic.clone(),
            OperationKind::Cast {
                kind: CastKind::PointerToGeneric,
                value: ValueId(11),
                to: generic,
            },
        ),
        gep(13, 1, 3),
    ];
    entry.terminator = Some(Terminator::Return { values: vec![] });
    module(vec![entry])
}
fn diamond(different_root: bool, same_value: bool) -> Module {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = vec![
        index(10, 2),
        gep(11, 0, 10),
        gep(12, if different_root { 1 } else { 0 }, 10),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(2),
        then_target: BlockId(90),
        then_arguments: vec![],
        else_target: BlockId(3),
        else_arguments: vec![],
    });
    let mut left = BasicBlock::new(BlockId(90));
    left.terminator = branch(7000, &[11]);
    let mut right = BasicBlock::new(BlockId(3));
    right.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(2),
        then_target: BlockId(7000),
        then_arguments: vec![ValueId(if same_value { 11 } else { 12 })],
        else_target: BlockId(7000),
        else_arguments: vec![ValueId(if same_value { 11 } else { 12 })],
    });
    let mut join = BasicBlock::new(BlockId(7000));
    join.parameters
        .push(ValueDef::new(ValueId(20), pointer(AddressSpace::Global)));
    join.terminator = Some(Terminator::Return { values: vec![] });
    module(vec![entry, left, right, join])
}
fn with_owner<R>(module: &Module, run: impl FnOnce(&VerifiedCanonicalKernelIrModuleV18) -> R) -> R {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let (owner, receipt) =
        VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
            module,
            StorageLayoutLimitsV1 {
                rows: 64,
                edges: 256,
                containment_depth: 32,
                object_bytes: 4096,
            },
            &mut budget,
        )
        .unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let result = run(&owner);
    drop(owner);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 0);
    result
}
fn compare(module: &Module, values: &[u32]) -> Vec<Observation> {
    with_owner(module, |owner| {
        let values = values.iter().map(|&v| ValueId(v)).collect::<Vec<_>>();
        let location = FunctionOperationLocation {
            block: BlockId(0),
            operation_index: 0,
        };
        let source = &owner.module().functions[0];
        let expected = original::observe_for_test_v18(source, &values, location);
        let legacy = crate::formal_memory_obligations::pointer_derivation::observe_for_test_v18(
            source, &values, location,
        );
        assert_eq!(legacy, expected);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        budget.reserve_storage(23).unwrap();
        let mut affine =
            ActualOwnerAffineV18::build(owner, 0, ControlFlowLimits::DEFAULT, &mut budget).unwrap();
        let mut slots = affine.private_slots(owner, 0, &mut budget).unwrap();
        let floor = budget.storage();
        let mut pointers = slots.pointers(owner, 0, &mut budget).unwrap();
        let retained = budget.storage();
        let observed = values
            .iter()
            .map(|&value| {
                let allocation = pointers
                    .allocation(owner, 0, value, &mut budget)
                    .unwrap()
                    .map_err(|failure| failure.materialize(location));
                let expression = pointers
                    .expression(owner, 0, value, &mut budget)
                    .unwrap()
                    .map(|row| (row.allocation, row.byte_offset.into_byte_expression()))
                    .map_err(|failure| failure.materialize(location));
                assert_eq!(budget.storage(), retained);
                (allocation, expression)
            })
            .collect::<Vec<_>>();
        assert_eq!(observed, expected);
        pointers.release(&mut budget).unwrap();
        assert_eq!(budget.storage(), floor);
        slots.release(&mut budget).unwrap();
        affine.release(&mut budget).unwrap();
        assert_eq!(budget.storage(), 23);
        observed
    })
}

#[test]
fn actual_pointer_source_matches_original_gep_cast_root_and_unknown_diagnostics() {
    let rows = compare(&straight(), &[0, 1, 11, 12, 13, 999, 12, 11]);
    assert_eq!(rows[0].0.as_ref().unwrap().parameter_index(), 0);
    assert_eq!(rows[1].0.as_ref().unwrap().parameter_index(), 1);
    assert_eq!(rows[2].1.as_ref().unwrap().1, ByteExpression::constant(8));
    assert_eq!(rows[2], rows[3]);
    assert!(matches!(
        rows[4].1,
        Err(FormalMemoryIncompleteReason::UnsupportedIndexExpression {
            index: ValueId(3),
            ..
        })
    ));
    assert!(matches!(
        rows[5].0,
        Err(FormalMemoryIncompleteReason::UnsupportedPointerDerivation {
            pointer: ValueId(999),
            ..
        })
    ));
}

#[test]
fn actual_pointer_source_allocation_phi_does_not_invent_unique_address() {
    let rows = compare(&diamond(false, false), &[20, 11, 12, 20]);
    assert_eq!(rows[0].0.as_ref().unwrap().parameter_index(), 0);
    assert!(matches!(
        rows[0].1,
        Err(FormalMemoryIncompleteReason::UnsupportedPointerDerivation {
            pointer: ValueId(20),
            ..
        })
    ));
    let equal = compare(&diamond(false, true), &[20, 11, 20]);
    assert_eq!(equal[0], equal[1]);
}

#[test]
fn actual_pointer_source_mixed_roots_keep_original_first_failure_and_cache_order() {
    compare(&diamond(true, false), &[20, 11, 12, 20]);
    let rows = compare(&diamond(true, false), &[11, 12, 20, 20]);
    assert!(rows[2].0.is_err());
    assert_eq!(rows[2], rows[3]);
}

#[test]
fn actual_pointer_source_invariant_loop_phi_uses_root_reachability() {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.terminator = branch(1, &[0]);
    let mut loop_block = BasicBlock::new(BlockId(1));
    loop_block
        .parameters
        .push(ValueDef::new(ValueId(20), pointer(AddressSpace::Global)));
    loop_block.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(2),
        then_target: BlockId(1),
        then_arguments: vec![ValueId(20)],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    let mut exit = BasicBlock::new(BlockId(2));
    exit.terminator = Some(Terminator::Return { values: vec![] });
    let rows = compare(&module(vec![entry, loop_block, exit]), &[20, 0, 20]);
    assert_eq!(rows[0], rows[1]);
}

#[test]
fn actual_pointer_source_consumes_cross_block_private_pointer_store_facts() {
    let element = pointer(AddressSpace::Global);
    let private = Type::pointer(
        element.clone(),
        AddressSpace::Private,
        AccessMode::ReadWrite,
    );
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = vec![
        op(
            10,
            private,
            OperationKind::Alloca {
                element: element.clone(),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 8,
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(10),
                value: ValueId(0),
                access: MemoryAccess::new(AddressSpace::Private, 8),
            },
        ),
    ];
    entry.terminator = branch(1, &[]);
    let mut next = BasicBlock::new(BlockId(1));
    next.operations.push(op(
        20,
        element,
        OperationKind::Load {
            pointer: ValueId(10),
            access: MemoryAccess::new(AddressSpace::Private, 8),
        },
    ));
    next.terminator = Some(Terminator::Return { values: vec![] });
    let rows = compare(&module(vec![entry, next]), &[20, 0, 20]);
    assert_eq!(rows[0], rows[1]);
}

#[test]
fn actual_pointer_source_slice_data_reuses_allocation_closure() {
    let mut source = straight();
    source.functions[0].signature.parameters[0] = Type::slice(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    );
    let block = &mut source.functions[0].body.as_mut().unwrap().blocks[0];
    block.operations = vec![
        op(
            10,
            pointer(AddressSpace::Global),
            OperationKind::SliceData { slice: ValueId(0) },
        ),
        index(11, 3),
        gep(12, 10, 11),
    ];
    let rows = compare(&source, &[0, 10, 12, 10]);
    assert!(rows[0].0.is_ok());
    assert!(rows[0].1.is_err());
    assert_eq!(rows[2].1.as_ref().unwrap().1, ByteExpression::constant(12));
}

#[test]
fn actual_pointer_source_retains_exact_original_phi_input_order_and_entry_empty_inputs() {
    let mut source = diamond(true, false).functions.remove(0);
    let body = source.body.as_mut().unwrap();
    body.blocks[0]
        .parameters
        .push(ValueDef::new(ValueId(99), pointer(AddressSpace::Global)));
    let (definitions, _) = collect_definitions(&source).unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let mut context = ByteSourceContextV2::build(&source, ControlFlowLimits::DEFAULT, &mut budget)
        .unwrap()
        .unwrap();
    for (&value, inputs) in &definitions.block_parameter_inputs {
        assert_eq!(
            context
                .phi_input_count(&source, value, &mut budget)
                .unwrap(),
            Some(inputs.len())
        );
        for (ordinal, &input) in inputs.iter().enumerate() {
            assert_eq!(
                context
                    .phi_input(&source, value, ordinal, &mut budget)
                    .unwrap(),
                Some(input)
            );
        }
        assert_eq!(
            context
                .phi_input(&source, value, inputs.len(), &mut budget)
                .unwrap(),
            None
        );
    }
    assert_eq!(
        context
            .phi_input_count(&source, ValueId(99), &mut budget)
            .unwrap(),
        Some(0)
    );
    assert_eq!(
        context
            .phi_input_count(&source, ValueId(999), &mut budget)
            .unwrap(),
        None
    );
    context.release(&mut budget).unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn actual_pointer_source_overflow_preserves_exact_definition_location() {
    let mut source = straight();
    let block = &mut source.functions[0].body.as_mut().unwrap().blocks[0];
    block.operations = vec![index(10, u64::MAX), gep(11, 0, 10)];
    let rows = compare(&source, &[11, 0, 11]);
    assert_eq!(rows[0].0.as_ref().unwrap().parameter_index(), 0);
    assert_eq!(
        rows[0].1,
        Err(FormalMemoryIncompleteReason::AddressArithmeticOverflow {
            location: FunctionOperationLocation::new(BlockId(0), 1),
        })
    );
}

#[test]
fn actual_pointer_source_sparse_long_gep_chain_uses_explicit_stack_and_cache() {
    let mut source = straight();
    let block = &mut source.functions[0].body.as_mut().unwrap().blocks[0];
    block.operations = vec![index(10, 1)];
    let mut base = 0;
    for ordinal in 0..64 {
        let next = u32::MAX - ordinal;
        block.operations.push(gep(next, base, 10));
        base = next;
    }
    let rows = compare(&source, &[base, 0, u32::MAX, base]);
    assert_eq!(rows[0].1.as_ref().unwrap().1, ByteExpression::constant(256));
    assert_eq!(rows[2].1.as_ref().unwrap().1, ByteExpression::constant(4));
    assert_eq!(rows[0], rows[3]);
}

#[test]
fn actual_pointer_source_unknown_query_has_independent_exact_work_and_header_boundaries() {
    with_owner(&straight(), |owner| {
        for mode in 0..3 {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut budget = Budget::new(&mut work, 2_000_000);
            let mut affine =
                ActualOwnerAffineV18::build(owner, 0, ControlFlowLimits::DEFAULT, &mut budget)
                    .unwrap();
            let mut slots = affine.private_slots(owner, 0, &mut budget).unwrap();
            let mut pointers = slots.pointers(owner, 0, &mut budget).unwrap();
            let frame = size_of::<
                Query<'static, 'static, 'static, 'static, 'static, 'static, 'static>,
            >() + size_of::<meter::LiveGuardMeter<'static, 'static>>()
                + size_of::<std::thread::Result<Result<CachedPointerDerivation<QueryValue>>>>()
                + size_of::<Result<CachedPointerDerivation<QueryValue>>>()
                + size_of::<(
                    &mut ActualOwnerPointersV18<'static, 'static, 'static, 'static>,
                    ValueId,
                    &mut Budget<'static>,
                    bool,
                )>();
            let padding =
                budget.storage_limit() - budget.storage() - frame + usize::from(mode == 2);
            budget.reserve_storage(padding).unwrap();
            let floor = budget.storage();
            let before = budget.work();
            // Upper-bound search for MAX advances left to middle+1 every time.
            let mut left = 0;
            let count = pointers.rows.len();
            let mut comparisons = 0;
            while left < count {
                left = left + (count - left) / 2 + 1;
                comparisons += 1;
            }
            let expected = 1 + 1 + comparisons + usize::from(count != 0);
            if mode == 1 {
                budget
                    .charge_work(usize::MAX - before - expected + 1)
                    .unwrap();
            }
            let result = pointers.allocation(owner, 0, ValueId(u32::MAX), &mut budget);
            if mode == 0 {
                assert_eq!(
                    result.unwrap(),
                    Err(PointerDerivationFailure::AtAccess(ValueId(u32::MAX)))
                );
                assert_eq!(budget.work() - before, expected);
                assert_eq!(budget.peak_storage(), floor + frame);
            } else {
                let error = result.err().unwrap();
                assert!(matches!(
                    error,
                    Failure::Resource(ResourceError::Work(_) | ResourceError::Storage { .. })
                ));
                assert_eq!(
                    pointers
                        .allocation(owner, 0, ValueId(0), &mut budget)
                        .err()
                        .unwrap(),
                    error
                );
            }
            assert_eq!(budget.storage(), floor);
            pointers.release(&mut budget).unwrap();
            slots.release(&mut budget).unwrap();
            affine.release(&mut budget).unwrap();
            assert_eq!(budget.storage(), padding);
        }
    });
}

#[test]
fn actual_pointer_source_measured_cumulative_query_exact_short_and_cache_retirement() {
    with_owner(&diamond(false, false), |owner| {
        let execute = |work_limit, storage_limit, success| {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            let mut affine =
                ActualOwnerAffineV18::build(owner, 0, ControlFlowLimits::DEFAULT, &mut budget)
                    .unwrap();
            let mut slots = affine.private_slots(owner, 0, &mut budget).unwrap();
            let mut pointers = slots.pointers(owner, 0, &mut budget).unwrap();
            budget.reserve_storage(1_000_000).unwrap();
            let floor = budget.storage();
            let result = pointers.allocation(owner, 0, ValueId(20), &mut budget);
            if success {
                assert!(result.unwrap().is_ok());
            } else {
                let error = result.err().unwrap();
                assert!(matches!(
                    error,
                    Failure::Resource(ResourceError::Work(_) | ResourceError::Storage { .. })
                ));
                assert_eq!(
                    pointers
                        .allocation(owner, 0, ValueId(0), &mut budget)
                        .err()
                        .unwrap(),
                    error
                );
            }
            assert_eq!(budget.storage(), floor);
            let metrics = (budget.work(), budget.peak_storage());
            pointers.release(&mut budget).unwrap();
            slots.release(&mut budget).unwrap();
            affine.release(&mut budget).unwrap();
            budget.release_storage(1_000_000).unwrap();
            assert_eq!(budget.storage(), 0);
            metrics
        };
        let (work, peak) = execute(usize::MAX, usize::MAX, true);
        assert_eq!(execute(work, peak, true), (work, peak));
        execute(work - 1, peak, false);
        execute(work, peak - 1, false);
    });
}

#[test]
fn actual_pointer_source_binds_owner_root_original_ledger_floor_and_sticky_refusal() {
    let source = straight();
    with_owner(&source, |owner| {
        with_owner(&source, |other| {
            for mode in 0..4 {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
                let mut budget = Budget::new(&mut work, usize::MAX);
                let mut affine =
                    ActualOwnerAffineV18::build(owner, 0, ControlFlowLimits::DEFAULT, &mut budget)
                        .unwrap();
                let mut slots = affine.private_slots(owner, 0, &mut budget).unwrap();
                let mut pointers = slots.pointers(owner, 0, &mut budget).unwrap();
                let retained = budget.storage();
                let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
                let mut foreign = Budget::new(&mut foreign_work, usize::MAX);
                assert_eq!(
                    pointers
                        .allocation(owner, 0, ValueId(0), &mut foreign)
                        .err()
                        .unwrap(),
                    Failure::Resource(ResourceError::Accounting)
                );
                assert_eq!((foreign.work(), foreign.storage()), (0, 0));
                assert!(
                    pointers
                        .allocation(owner, 0, ValueId(0), &mut budget)
                        .unwrap()
                        .is_ok()
                );
                if mode >= 2 {
                    if mode == 3 {
                        assert!(budget.charge_work(usize::MAX).is_err());
                    }
                    budget.rollback_storage(retained - 1).unwrap();
                }
                let error = pointers
                    .allocation(
                        if mode == 0 { other } else { owner },
                        usize::from(mode == 1),
                        ValueId(0),
                        &mut budget,
                    )
                    .err()
                    .unwrap();
                if mode == 3 {
                    assert!(matches!(error, Failure::Resource(ResourceError::Work(_))));
                } else {
                    assert_eq!(error, Failure::Resource(ResourceError::Accounting));
                }
                if mode >= 2 {
                    budget.reserve_storage(1).unwrap();
                }
                assert_eq!(
                    pointers
                        .allocation(owner, 0, ValueId(0), &mut budget)
                        .err()
                        .unwrap(),
                    error
                );
                pointers.release(&mut budget).unwrap();
                slots.release(&mut budget).unwrap();
                affine.release(&mut budget).unwrap();
                assert_eq!(budget.storage(), 0);
            }
        })
    });
}

#[test]
fn actual_pointer_source_wide_independent_queries_have_logarithmic_not_dense_set_cost() {
    let mut totals = Vec::new();
    for count in [512_u32, 1024] {
        let mut source = straight();
        let block = &mut source.functions[0].body.as_mut().unwrap().blocks[0];
        block.operations = vec![index(10, 1)];
        block
            .operations
            .extend((0..count).map(|ordinal| gep(100 + ordinal, 0, 10)));
        with_owner(&source, |owner| {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut budget = Budget::new(&mut work, usize::MAX);
            let mut affine =
                ActualOwnerAffineV18::build(owner, 0, ControlFlowLimits::DEFAULT, &mut budget)
                    .unwrap();
            let mut slots = affine.private_slots(owner, 0, &mut budget).unwrap();
            let mut pointers = slots.pointers(owner, 0, &mut budget).unwrap();
            pointers
                .expression(owner, 0, ValueId(0), &mut budget)
                .unwrap()
                .unwrap();
            let before = budget.work();
            let floor = budget.storage();
            let row_count = pointers.rows.len();
            // One binary lookup costs <= ceil(log2(N+1))+2. Each independent
            // GEP visits at most 15 dense indices, one block, two empty phi
            // tables, one touched slot, and three explicit stack frames.
            // 16 lookups + 64 constant work conservatively cover that roster.
            let mut height = 0_usize;
            let mut bound = 1_usize;
            while bound < row_count + 1 {
                bound *= 2;
                height += 1;
            }
            let per_query = 16 * (height + 2) + 64;
            for ordinal in 0..count {
                let start = budget.work();
                let row = pointers
                    .expression(owner, 0, ValueId(100 + ordinal), &mut budget)
                    .unwrap()
                    .unwrap();
                assert_eq!(
                    row.byte_offset.into_byte_expression(),
                    ByteExpression::constant(4)
                );
                assert!(
                    budget.work() - start <= per_query,
                    "independent query scanned unrelated source rows"
                );
                assert_eq!(budget.storage(), floor);
            }
            let total = budget.work() - before;
            assert!(total <= count as usize * per_query);
            totals.push(total);
            pointers.release(&mut budget).unwrap();
            slots.release(&mut budget).unwrap();
            affine.release(&mut budget).unwrap();
            assert_eq!(budget.storage(), 0);
        });
    }
    assert!(totals[1] > totals[0]);
    assert!(totals[1] < 3 * totals[0]);
}

#[test]
fn actual_pointer_source_sparse_sets_preserve_roles_reinsertion_and_sorted_members() {
    use engine::State;
    with_owner(&straight(), |owner| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        let mut affine =
            ActualOwnerAffineV18::build(owner, 0, ControlFlowLimits::DEFAULT, &mut budget).unwrap();
        let mut slots = affine.private_slots(owner, 0, &mut budget).unwrap();
        let mut pointers = slots.pointers(owner, 0, &mut budget).unwrap();
        let floor = budget.storage();
        {
            let mut query = Query {
                pointers: &mut pointers,
                budget: &mut budget,
            };
            let mut outer = query.set(engine::SetRole::ExpressionVisiting).unwrap();
            let mut visited = query.set(engine::SetRole::AllocationVisited).unwrap();
            let mut sources = query.set(engine::SetRole::AllocationSources).unwrap();
            let mut failed = query.set(engine::SetRole::AllocationFailure).unwrap();
            for value in [12, 0, 11, 0] {
                query.insert(&mut outer, ValueId(value)).unwrap();
            }
            for set in [&mut visited, &mut sources, &mut failed] {
                assert!(query.insert(set, ValueId(0)).unwrap());
            }
            query.remove(&mut outer, ValueId(0)).unwrap();
            assert_eq!(query.members(&outer).unwrap(), [ValueId(11), ValueId(12)]);
            assert!(query.insert(&mut outer, ValueId(0)).unwrap());
            assert!(!query.insert(&mut outer, ValueId(0)).unwrap());
            assert_eq!(outer.touched.len(), 3);
            assert_eq!(
                query.members(&outer).unwrap(),
                [ValueId(0), ValueId(11), ValueId(12)]
            );
            for set in [&visited, &sources, &failed] {
                assert_eq!(query.members(set).unwrap(), [ValueId(0)]);
            }
        }
        budget.rollback_storage(floor).unwrap();
        // Epoch marks from retired sets cannot contaminate subsequent queries.
        assert!(
            pointers
                .expression(owner, 0, ValueId(12), &mut budget)
                .unwrap()
                .is_ok()
        );
        assert_eq!(budget.storage(), floor);
        pointers.release(&mut budget).unwrap();
        slots.release(&mut budget).unwrap();
        affine.release(&mut budget).unwrap();
        assert_eq!(budget.storage(), 0);
    });
}

#[test]
fn actual_pointer_source_sparse_epoch_overflow_is_sticky_and_restores_scratch() {
    with_owner(&straight(), |owner| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        let mut affine =
            ActualOwnerAffineV18::build(owner, 0, ControlFlowLimits::DEFAULT, &mut budget).unwrap();
        let mut slots = affine.private_slots(owner, 0, &mut budget).unwrap();
        let mut pointers = slots.pointers(owner, 0, &mut budget).unwrap();
        let floor = budget.storage();
        pointers.next_epoch = usize::MAX;
        let error = pointers
            .expression(owner, 0, ValueId(11), &mut budget)
            .err()
            .unwrap();
        assert_eq!(error, Failure::Resource(ResourceError::Arithmetic));
        assert_eq!(budget.storage(), floor);
        pointers.next_epoch = 1;
        assert_eq!(
            pointers
                .allocation(owner, 0, ValueId(0), &mut budget)
                .err()
                .unwrap(),
            error
        );
        pointers.release(&mut budget).unwrap();
        slots.release(&mut budget).unwrap();
        affine.release(&mut budget).unwrap();
        assert_eq!(budget.storage(), 0);
    });
}
