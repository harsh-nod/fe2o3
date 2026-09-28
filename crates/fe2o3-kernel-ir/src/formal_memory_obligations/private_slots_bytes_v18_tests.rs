use super::*;
use crate::{
    BasicBlock, Kernel, Signature, StorageLayoutKindV1, StorageLayoutLimitsV1, StorageLayoutV1,
    ValueDef,
};

// Immutable pre-refactor production algorithm, not a second admission path.
#[path = "private_slots_original_reference_v18_tests.rs"]
mod original;

fn alloca(id: u32, element: Type) -> Operation {
    Operation::effect_free(
        ValueDef::new(
            ValueId(id),
            Type::pointer(
                element.clone(),
                AddressSpace::Private,
                AccessMode::ReadWrite,
            ),
        ),
        OperationKind::Alloca {
            element,
            count: None,
            address_space: AddressSpace::Private,
            alignment: 8,
        },
    )
}
fn store(pointer: u32, value: u32, space: AddressSpace) -> Operation {
    Operation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(pointer),
            value: ValueId(value),
            access: MemoryAccess::new(space, 4),
        },
    )
}
fn load(id: u32, pointer: u32, space: AddressSpace) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), Type::Scalar(ScalarType::U32)),
        OperationKind::Load {
            pointer: ValueId(pointer),
            access: MemoryAccess::new(space, 4),
        },
    )
}
fn branch(target: u32) -> Option<Terminator> {
    Some(Terminator::Branch {
        target: BlockId(target),
        arguments: vec![],
    })
}
fn module(blocks: Vec<BasicBlock>) -> Module {
    let mut module = Module::new("paid_private_slots");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(
            vec![
                Type::Scalar(ScalarType::U32),
                Type::Scalar(ScalarType::U32),
                Type::BOOL,
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2)],
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
    entry.operations = vec![
        alloca(10, Type::Scalar(ScalarType::U32)),
        load(19, 10, AddressSpace::Private),
        store(10, 0, AddressSpace::Private),
    ];
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(10)],
    });
    let mut next = BasicBlock::new(BlockId(1));
    next.parameters.push(ValueDef::new(
        ValueId(11),
        Type::pointer(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Private,
            AccessMode::ReadWrite,
        ),
    ));
    let generic = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Generic,
        AccessMode::ReadWrite,
    );
    next.operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(12), generic.clone()),
            OperationKind::Cast {
                kind: CastKind::PointerToGeneric,
                value: ValueId(11),
                to: generic,
            },
        ),
        load(20, 12, AddressSpace::Generic),
        store(12, 1, AddressSpace::Generic),
        load(21, 12, AddressSpace::Generic),
    ];
    next.terminator = Some(Terminator::Return { values: vec![] });
    module(vec![entry, next])
}
fn diamond(mode: u8) -> Module {
    let mut entry = BasicBlock::new(BlockId(0));
    entry
        .operations
        .push(alloca(10, Type::Scalar(ScalarType::U32)));
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(2),
        then_target: BlockId(90),
        then_arguments: vec![],
        else_target: BlockId(3),
        else_arguments: vec![],
    });
    let mut left = BasicBlock::new(BlockId(90));
    left.operations.push(store(10, 0, AddressSpace::Private));
    left.terminator = branch(9000);
    let mut right = BasicBlock::new(BlockId(3));
    if mode != 2 {
        right
            .operations
            .push(store(10, u32::from(mode != 0), AddressSpace::Private));
    }
    right.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(2),
        then_target: BlockId(9000),
        then_arguments: vec![],
        else_target: BlockId(9000),
        else_arguments: vec![],
    });
    let mut merge = BasicBlock::new(BlockId(9000));
    merge.operations.push(load(20, 10, AddressSpace::Private));
    merge.terminator = Some(Terminator::Return { values: vec![] });
    module(vec![entry, left, right, merge])
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
fn compare_original(owner: &VerifiedCanonicalKernelIrModuleV18, expected: &[(u32, Option<u32>)]) {
    let source = &owner.module().functions[0];
    let (definitions, _) = collect_definitions(source).unwrap();
    let types = collect_types(source);
    let mut original_reasons = BTreeSet::new();
    let original_slots = original::classify_eligible_private_slots(
        source,
        &definitions,
        &types,
        &mut original_reasons,
    );
    let original_loads =
        original::collect_private_load_sources(source, &definitions, &types, &original_slots);
    let mut current_reasons = BTreeSet::new();
    let current_slots = private_slots::classify_eligible_private_slots(
        source,
        &definitions,
        &types,
        &mut current_reasons,
    );
    let current_loads =
        private_slots::collect_private_load_sources(source, &definitions, &types, &current_slots);
    assert_eq!(current_slots, original_slots);
    assert_eq!(current_reasons, original_reasons);
    assert_eq!(current_loads, original_loads);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(23).unwrap();
    let mut affine =
        ActualOwnerAffineV18::build(owner, 0, ControlFlowLimits::DEFAULT, &mut budget).unwrap();
    let floor = budget.storage();
    let mut paid = affine.private_slots(owner, 0, &mut budget).unwrap();
    let mut observed_reasons = BTreeSet::new();
    let mut observed_slots = BTreeSet::new();
    for ordinal in 0.. {
        let Some((slot, escape)) = paid.escape(owner, 0, ordinal, &mut budget).unwrap() else {
            break;
        };
        assert_eq!(
            paid.eligible(owner, 0, slot, &mut budget).unwrap(),
            escape.is_none()
        );
        if let Some((location, pointer)) = escape {
            observed_reasons.insert(FormalMemoryIncompleteReason::UnsupportedPointerDerivation {
                location,
                pointer,
            });
        } else {
            observed_slots.insert(slot);
        }
    }
    assert_eq!(observed_slots, original_slots);
    assert_eq!(observed_reasons, original_reasons);
    for (value, expected) in expected {
        assert_eq!(
            original_loads.get(&ValueId(*value)).copied(),
            expected.map(ValueId)
        );
        assert_eq!(
            paid.load_source(owner, 0, ValueId(*value), &mut budget)
                .unwrap(),
            expected.map(ValueId)
        );
    }
    assert_eq!(
        paid.loads
            .iter()
            .map(|row| (row.value, row.source))
            .collect::<BTreeMap<_, _>>(),
        original_loads
    );
    paid.release(&mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
    affine.release(&mut budget).unwrap();
    assert_eq!(budget.storage(), 23);
}

#[test]
fn actual_private_slots_follow_cross_block_phi_casts_and_reaching_stores() {
    with_owner(&straight(), |owner| {
        compare_original(owner, &[(19, None), (20, Some(0)), (21, Some(1))])
    });
}

#[test]
fn actual_private_slots_join_all_predecessors_and_preserve_uninitialized_paths() {
    for mode in 0..3 {
        with_owner(&diamond(mode), |owner| {
            compare_original(owner, &[(20, (mode == 0).then_some(0))])
        });
    }
}

#[test]
fn actual_private_slots_compute_loop_fixed_point_not_physical_precedence() {
    for changed in [false, true] {
        let mut entry = BasicBlock::new(BlockId(0));
        entry.operations = vec![
            alloca(10, Type::Scalar(ScalarType::U32)),
            store(10, 0, AddressSpace::Private),
        ];
        entry.terminator = branch(8);
        let mut header = BasicBlock::new(BlockId(8));
        header.operations = vec![
            load(20, 10, AddressSpace::Private),
            store(10, u32::from(changed), AddressSpace::Private),
        ];
        header.terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(2),
            then_target: BlockId(8),
            then_arguments: vec![],
            else_target: BlockId(2),
            else_arguments: vec![],
        });
        let mut exit = BasicBlock::new(BlockId(2));
        exit.operations.push(load(21, 10, AddressSpace::Private));
        exit.terminator = Some(Terminator::Return { values: vec![] });
        with_owner(&module(vec![entry, header, exit]), |owner| {
            compare_original(
                owner,
                &[
                    (20, (!changed).then_some(0)),
                    (21, Some(u32::from(changed))),
                ],
            )
        });
    }
}

#[test]
fn actual_private_slots_keep_first_original_escape_and_ignore_dead_blocks() {
    let mut source = straight();
    let body = source.functions[0].body.as_mut().unwrap();
    let pointer_type = body.blocks[0].operations[0].results[0].ty.clone();
    body.blocks[0]
        .operations
        .insert(1, alloca(30, pointer_type));
    body.blocks[0]
        .operations
        .insert(2, store(30, 10, AddressSpace::Private));
    body.blocks[0]
        .operations
        .insert(3, store(30, 10, AddressSpace::Private));
    let mut dead = BasicBlock::new(BlockId(999));
    dead.operations
        .push(alloca(900, Type::Scalar(ScalarType::U32)));
    dead.terminator = Some(Terminator::Unreachable);
    body.blocks.push(dead);
    with_owner(&source, |owner| {
        compare_original(owner, &[(19, None), (20, None), (21, None)])
    });
}

#[test]
fn actual_private_slot_empty_phase_has_independent_work_and_storage_equations() {
    let mut block = BasicBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Return { values: vec![] });
    with_owner(&module(vec![block]), |owner| {
        // Entry1, slots allocation1, two reachability scans5+5, terminal1,
        // loads and filtered-slot allocations2, settlement1 = 16.
        for short in [false, true] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut budget = Budget::new(&mut work, 1_000_000);
            let mut affine =
                ActualOwnerAffineV18::build(owner, 0, ControlFlowLimits::DEFAULT, &mut budget)
                    .unwrap();
            let radix_frame = size_of::<
                std::thread::Result<std::result::Result<(), VerificationResourceError>>,
            >() + size_of::<(&mut [Slot], &mut Budget<'static>)>();
            let independent_frame = size_of::<(
                &mut ActualOwnerAffineV18<'static, 'static>,
                usize,
                Vec<Slot>,
                Vec<LoadSource>,
                usize,
                usize,
            )>() + 3 * size_of::<usize>()
                + size_of::<meter::LiveGuardMeter<'static, 'static>>()
                + size_of::<std::thread::Result<Result<(Vec<Slot>, Vec<LoadSource>, usize)>>>()
                + size_of::<Result<ActualOwnerPrivateSlotsV18<'static, 'static, 'static>>>()
                + 4 * size_of::<usize>();
            assert_eq!(frame_bytes_v18(), independent_frame);
            let vec_header = size_of::<Vec<Slot>>();
            let phase_peak = independent_frame + (vec_header + radix_frame).max(3 * vec_header);
            let padding =
                budget.storage_limit() - budget.storage() - phase_peak + usize::from(short);
            budget.reserve_storage(padding).unwrap();
            let floor = budget.storage();
            let before = budget.work();
            match affine.private_slots(owner, 0, &mut budget) {
                Ok(paid) => {
                    assert!(!short);
                    assert_eq!(budget.work() - before, 16);
                    assert_eq!(budget.storage(), floor + independent_frame + 2 * vec_header);
                    assert_eq!(budget.peak_storage(), 1_000_000);
                    paid.release(&mut budget).unwrap();
                }
                Err(error) => {
                    assert!(short);
                    assert!(matches!(
                        error,
                        Failure::Resource(ResourceError::Storage {
                            actual: 1_000_001,
                            limit: 1_000_000
                        })
                    ));
                }
            }
            assert_eq!(budget.storage(), floor);
            affine.release(&mut budget).unwrap();
            assert_eq!(budget.storage(), padding);
        }
    });
}

#[test]
fn actual_private_slots_preserve_sparse_reversed_block_and_value_orders() {
    let mut entry = BasicBlock::new(BlockId(0));
    for slot in [900_001, 400_003, 100_007] {
        entry
            .operations
            .push(alloca(slot, Type::Scalar(ScalarType::U32)));
        entry.operations.push(store(slot, 0, AddressSpace::Private));
    }
    entry.terminator = branch(11);
    let mut blocks = vec![entry];
    for ordinal in (1..=16).rev() {
        let mut block = BasicBlock::new(BlockId(ordinal * 11));
        block.terminator = if ordinal == 16 {
            Some(Terminator::Return { values: vec![] })
        } else {
            branch((ordinal + 1) * 11)
        };
        if ordinal == 16 {
            for (result, slot) in [(20, 900_001), (21, 400_003), (22, 100_007)] {
                block
                    .operations
                    .push(load(result, slot, AddressSpace::Private));
            }
        }
        blocks.push(block);
    }
    with_owner(&module(blocks), |owner| {
        compare_original(owner, &[(20, Some(0)), (21, Some(0)), (22, Some(0))])
    });
}

#[test]
fn actual_private_slot_empty_phase_independent_sixteen_work_cut_is_exact() {
    let mut block = BasicBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Return { values: vec![] });
    with_owner(&module(vec![block]), |owner| {
        let constructor = {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut budget = Budget::new(&mut work, usize::MAX);
            let affine =
                ActualOwnerAffineV18::build(owner, 0, ControlFlowLimits::DEFAULT, &mut budget)
                    .unwrap();
            let before = budget.work();
            affine.release(&mut budget).unwrap();
            assert_eq!(budget.storage(), 0);
            before
        };
        for short in [false, true] {
            let mut work =
                CanonicalKernelIrWorkBudgetV1::new(constructor + 16 - usize::from(short));
            let mut budget = Budget::new(&mut work, usize::MAX);
            let mut affine =
                ActualOwnerAffineV18::build(owner, 0, ControlFlowLimits::DEFAULT, &mut budget)
                    .unwrap();
            let floor = budget.storage();
            match affine.private_slots(owner, 0, &mut budget) {
                Ok(paid) => {
                    assert!(!short);
                    assert_eq!(budget.work(), constructor + 16);
                    paid.release(&mut budget).unwrap();
                }
                Err(error) => {
                    assert!(short);
                    assert!(matches!(error, Failure::Resource(ResourceError::Work(_))));
                    assert_eq!(budget.failed_work(), Some(constructor + 16));
                }
            }
            assert_eq!(budget.storage(), floor);
            affine.release(&mut budget).unwrap();
            assert_eq!(budget.storage(), 0);
        }
    });
}

#[test]
fn actual_private_slot_constructor_exact_and_short_limits_preserve_custody() {
    with_owner(&diamond(0), |owner| {
        let (constructor, work_used, peak) = {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut budget = Budget::new(&mut work, usize::MAX);
            let mut affine =
                ActualOwnerAffineV18::build(owner, 0, ControlFlowLimits::DEFAULT, &mut budget)
                    .unwrap();
            let constructor = budget.work();
            budget.reserve_storage(1_000_000).unwrap();
            let paid = affine.private_slots(owner, 0, &mut budget).unwrap();
            let totals = (constructor, budget.work(), budget.peak_storage());
            paid.release(&mut budget).unwrap();
            affine.release(&mut budget).unwrap();
            budget.release_storage(1_000_000).unwrap();
            assert_eq!(budget.storage(), 0);
            totals
        };
        assert!(work_used > constructor);
        for (work_limit, storage_limit, success) in [
            (work_used, peak, true),
            (work_used - 1, peak, false),
            (work_used, peak - 1, false),
        ] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            let mut affine =
                ActualOwnerAffineV18::build(owner, 0, ControlFlowLimits::DEFAULT, &mut budget)
                    .unwrap();
            budget.reserve_storage(1_000_000).unwrap();
            let floor = budget.storage();
            let denial = match affine.private_slots(owner, 0, &mut budget) {
                Ok(paid) => {
                    assert!(success);
                    paid.release(&mut budget).unwrap();
                    None
                }
                Err(error) => {
                    assert!(!success);
                    Some(error)
                }
            };
            assert_eq!(budget.storage(), floor);
            if let Some(error) = denial {
                assert!(matches!(
                    error,
                    Failure::Resource(ResourceError::Work(_) | ResourceError::Storage { .. })
                ));
                let retry = affine.private_slots(owner, 0, &mut budget).err().unwrap();
                assert_eq!(retry, error);
            }
            affine.release(&mut budget).unwrap();
            budget.release_storage(1_000_000).unwrap();
            assert_eq!(budget.storage(), 0);
        }
    });
}

#[test]
fn actual_private_slots_bind_original_owner_root_floor_and_first_denial() {
    let source = straight();
    with_owner(&source, |owner| {
        with_owner(&source, |other| {
            for mode in 0..4 {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
                let mut budget = Budget::new(&mut work, usize::MAX);
                let mut affine =
                    ActualOwnerAffineV18::build(owner, 0, ControlFlowLimits::DEFAULT, &mut budget)
                        .unwrap();
                let mut paid = affine.private_slots(owner, 0, &mut budget).unwrap();
                let floor = budget.storage();
                let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
                let mut foreign = Budget::new(&mut foreign_work, usize::MAX);
                assert_eq!(
                    paid.load_source(owner, 0, ValueId(20), &mut foreign)
                        .unwrap_err(),
                    Failure::Resource(ResourceError::Accounting)
                );
                assert_eq!((foreign.work(), foreign.storage()), (0, 0));
                assert_eq!(
                    paid.load_source(owner, 0, ValueId(20), &mut budget)
                        .unwrap(),
                    Some(ValueId(0))
                );
                let expected = match mode {
                    0 => paid
                        .load_source(other, 0, ValueId(20), &mut budget)
                        .unwrap_err(),
                    1 => paid
                        .load_source(owner, 1, ValueId(20), &mut budget)
                        .unwrap_err(),
                    2 => {
                        budget.rollback_storage(floor - 1).unwrap();
                        paid.load_source(owner, 0, ValueId(20), &mut budget)
                            .unwrap_err()
                    }
                    _ => {
                        assert!(budget.charge_work(usize::MAX).is_err());
                        let original = paid
                            .load_source(owner, 0, ValueId(20), &mut budget)
                            .unwrap_err();
                        assert!(matches!(
                            original,
                            Failure::Resource(ResourceError::Work(_))
                        ));
                        budget.rollback_storage(floor - 1).unwrap();
                        assert_eq!(
                            paid.load_source(owner, 0, ValueId(20), &mut budget)
                                .unwrap_err(),
                            original
                        );
                        original
                    }
                };
                if mode >= 2 {
                    budget.reserve_storage(1).unwrap();
                }
                assert_eq!(
                    paid.load_source(owner, 0, ValueId(20), &mut budget)
                        .unwrap_err(),
                    expected
                );
                paid.release(&mut budget).unwrap();
                affine.release(&mut budget).unwrap();
                assert_eq!(budget.storage(), 0);
            }
        })
    });
}
