use super::*;

type Request = (
    FunctionOperationLocation,
    ValueId,
    FormalMemoryAccessKind,
    MemoryAccess,
    Option<ValueId>,
    bool,
);

fn direct_accesses() -> (Module, Vec<Request>) {
    let mut module = straight();
    let operations = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
    operations.push(op(
        20,
        Type::Scalar(ScalarType::U32),
        OperationKind::Constant(Constant::U32(0)),
    ));
    operations.push(op(
        21,
        Type::Scalar(ScalarType::U32),
        OperationKind::Load {
            pointer: ValueId(11),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    ));
    operations.push(Operation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(12),
            value: ValueId(20),
            access: MemoryAccess::new(AddressSpace::Generic, 4),
        },
    ));
    operations.push(op(
        22,
        Type::Scalar(ScalarType::U32),
        OperationKind::Load {
            pointer: ValueId(13),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    ));
    operations.push(op(
        23,
        Type::Scalar(ScalarType::U32),
        OperationKind::GuardedLoad {
            pointer: ValueId(13),
            predicate: ValueId(2),
            fallback: ValueId(20),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    ));
    let requests = [
        (
            5,
            11,
            FormalMemoryAccessKind::Read,
            AddressSpace::Global,
            None,
            false,
        ),
        (
            6,
            12,
            FormalMemoryAccessKind::Write,
            AddressSpace::Generic,
            None,
            false,
        ),
        (
            7,
            13,
            FormalMemoryAccessKind::Read,
            AddressSpace::Global,
            None,
            false,
        ),
        (
            8,
            13,
            FormalMemoryAccessKind::Read,
            AddressSpace::Global,
            Some(ValueId(2)),
            false,
        ),
        (
            8,
            13,
            FormalMemoryAccessKind::Read,
            AddressSpace::Global,
            Some(ValueId(2)),
            true,
        ),
    ]
    .into_iter()
    .map(
        |(operation, pointer, kind, space, predicate, conservative)| {
            (
                FunctionOperationLocation::new(BlockId(0), operation),
                ValueId(pointer),
                kind,
                MemoryAccess::new(space, 4),
                predicate,
                conservative,
            )
        },
    )
    .collect();
    (module, requests)
}

fn run(
    owner: &VerifiedCanonicalKernelIrModuleV18,
    requests: &[Request],
    work_limit: usize,
    storage_limit: usize,
) -> (
    Result<Vec<std::result::Result<FormalMemoryAccess, FormalMemoryIncompleteReason>>>,
    usize,
    usize,
) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(23).unwrap();
    let result = (|| {
        let mut affine =
            ActualOwnerAffineV18::build(owner, 0, ControlFlowLimits::DEFAULT, &mut budget)?;
        let mut slots = affine.private_slots(owner, 0, &mut budget)?;
        let mut pointers = slots.pointers(owner, 0, &mut budget)?;
        let parent_floor = budget.storage();
        let mut accesses = pointers.accesses(owner, 0, &mut budget)?;
        let retained = budget.storage();
        let mut observed = Vec::new();
        for &(location, _, _, _, _, conservative) in requests {
            let result = if conservative {
                accesses.conservative_guarded_read(
                    owner,
                    0,
                    location,
                    InvocationRange1d::new(0, 8).unwrap(),
                    &mut budget,
                )
            } else {
                accesses.access(
                    owner,
                    0,
                    location,
                    InvocationRange1d::new(0, 8).unwrap(),
                    &mut budget,
                )
            };
            assert_eq!(budget.storage(), retained);
            match result {
                Ok(value) => observed.push(value),
                Err(error) => {
                    assert!(matches!(accesses.access(owner, 0, location,
                        InvocationRange1d::new(0, 8).unwrap(), &mut budget), Err(ref again) if again == &error));
                    accesses.release(&mut budget)?;
                    pointers.release(&mut budget)?;
                    slots.release(&mut budget)?;
                    affine.release(&mut budget)?;
                    return Err(error);
                }
            }
        }
        accesses.release(&mut budget)?;
        assert_eq!(budget.storage(), parent_floor);
        pointers.release(&mut budget)?;
        slots.release(&mut budget)?;
        affine.release(&mut budget)?;
        Ok(observed)
    })();
    // A constructor denial can retire partial owners under the caller's outer
    // scratch scope; all references above have left scope before this refund.
    budget.rollback_storage(23).unwrap();
    (result, budget.work(), budget.peak_storage())
}

fn compare_accesses(
    module: &Module,
    requests: &[Request],
) -> Vec<std::result::Result<FormalMemoryAccess, FormalMemoryIncompleteReason>> {
    with_owner(module, |owner| {
        let source = &owner.module().functions[0];
        let invocations = InvocationRange1d::new(0, 8).unwrap();
        let expected = original::observe_access_for_test_v18(source, true, invocations, requests);
        let legacy =
            crate::formal_memory_obligations::pointer_derivation::observe_access_for_test_v18(
                source,
                true,
                invocations,
                requests,
            );
        assert_eq!(legacy, expected);
        let (actual, _, _) = run(owner, requests, usize::MAX, usize::MAX);
        let actual = actual.unwrap();
        assert_eq!(actual, expected);
        actual
    })
}

#[test]
fn paid_accesses_match_original_pointer_planes_dynamic_refusal_and_conservative_read() {
    let (module, requests) = direct_accesses();
    let actual = compare_accesses(&module, &requests);
    assert!(actual[0].is_ok());
    assert_eq!(
        actual[1].as_ref().unwrap().address_space,
        AddressSpace::Global
    );
    assert!(matches!(
        actual[2],
        Err(FormalMemoryIncompleteReason::UnsupportedIndexExpression { .. })
    ));
    assert!(matches!(
        actual[3],
        Err(FormalMemoryIncompleteReason::UnsupportedIndexExpression { .. })
    ));
    assert_eq!(
        actual[4].as_ref().unwrap().byte_offset,
        ByteExpression::Unbounded
    );
}

#[test]
fn paid_accesses_preserve_original_dominating_and_switch_guard_recipes() {
    for switch in [
        None,
        Some(Type::Scalar(ScalarType::U32)),
        Some(Type::Scalar(ScalarType::U64)),
    ] {
        let module = crate::formal_memory_obligations::guarded_access_v1::tests::fixture(switch);
        let request = (
            FunctionOperationLocation::new(BlockId(1), 0),
            ValueId(8),
            FormalMemoryAccessKind::Write,
            MemoryAccess::new(AddressSpace::Global, 4),
            None,
            false,
        );
        let actual = compare_accesses(&module, &[request]);
        assert!(matches!(
            actual[0].as_ref().unwrap().domain,
            FormalAccessDomainV1::SliceBounded(_)
        ));
    }
}

#[test]
fn paid_accesses_keep_exact_guard_path_refusal_then_allow_a_valid_query() {
    let mut module = crate::formal_memory_obligations::guarded_access_v1::tests::fixture(None);
    let blocks = &mut module.functions[0].body.as_mut().unwrap().blocks;
    let operation = blocks[1].operations[0].clone();
    blocks[2].operations.push(operation);
    let request = |block| {
        (
            FunctionOperationLocation::new(BlockId(block), 0),
            ValueId(8),
            FormalMemoryAccessKind::Write,
            MemoryAccess::new(AddressSpace::Global, 4),
            None,
            false,
        )
    };
    let observed = compare_accesses(&module, &[request(2), request(1)]);
    assert_eq!(
        observed[0],
        Err(FormalMemoryIncompleteReason::GuardedAccessPathUnavailable {
            location: FunctionOperationLocation::new(BlockId(2), 0),
            predicate: ValueId(4),
        })
    );
    assert!(matches!(
        observed[1].as_ref().unwrap().domain,
        FormalAccessDomainV1::SliceBounded(_)
    ));
}

#[test]
fn paid_accesses_store_only_queries_do_not_rescan_the_original_operation_body() {
    for count in [1, 16, 256, 1024] {
        let mut block = BasicBlock::new(BlockId(0));
        block.operations.push(op(
            10,
            Type::Scalar(ScalarType::U32),
            OperationKind::Constant(Constant::U32(7)),
        ));
        for _ in 0..count {
            block.operations.push(Operation::new(
                vec![],
                OperationKind::Store {
                    pointer: ValueId(0),
                    value: ValueId(10),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            ));
        }
        block.terminator = Some(Terminator::Return { values: vec![] });
        let module = module(vec![block]);
        with_owner(&module, |owner| {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut budget = Budget::new(&mut work, usize::MAX);
            let mut affine =
                ActualOwnerAffineV18::build(owner, 0, ControlFlowLimits::DEFAULT, &mut budget)
                    .unwrap();
            let mut slots = affine.private_slots(owner, 0, &mut budget).unwrap();
            let mut pointers = slots.pointers(owner, 0, &mut budget).unwrap();
            let mut accesses = pointers.accesses(owner, 0, &mut budget).unwrap();
            let retained = budget.storage();
            let before = budget.work();
            for operation in 1..=count {
                let location = FunctionOperationLocation::new(BlockId(0), operation);
                let row = accesses
                    .access(
                        owner,
                        0,
                        location,
                        InvocationRange1d::new(0, 8).unwrap(),
                        &mut budget,
                    )
                    .unwrap()
                    .unwrap();
                assert_eq!(row.location, location);
                assert_eq!(row.kind, FormalMemoryAccessKind::Write);
                assert_eq!(row.allocation.parameter_index, 0);
                assert_eq!(
                    row.byte_offset,
                    ByteExpression::Affine {
                        constant: 0,
                        invocation_coefficient: 0
                    }
                );
                assert_eq!(row.domain, FormalAccessDomainV1::LaunchEnvelope);
                assert_eq!(budget.storage(), retained);
            }
            // Source/value/CFG indices have constant size in this family;
            // each direct query is bounded independently of the store census.
            assert!(budget.work() - before <= 512 * count);
            accesses.release(&mut budget).unwrap();
            pointers.release(&mut budget).unwrap();
            slots.release(&mut budget).unwrap();
            affine.release(&mut budget).unwrap();
            assert_eq!(budget.storage(), 0);
        });
    }
}

#[test]
fn paid_accesses_exact_and_one_short_cumulative_construction_and_query_limits() {
    for (module, requests) in [direct_accesses(), {
        let module = crate::formal_memory_obligations::guarded_access_v1::tests::fixture(None);
        (
            module,
            vec![(
                FunctionOperationLocation::new(BlockId(1), 0),
                ValueId(8),
                FormalMemoryAccessKind::Write,
                MemoryAccess::new(AddressSpace::Global, 4),
                None,
                false,
            )],
        )
    }] {
        with_owner(&module, |owner| {
            let (expected, work, storage) = run(owner, &requests, usize::MAX, usize::MAX);
            let expected = expected.unwrap();
            assert_eq!(run(owner, &requests, work, storage).0.unwrap(), expected);
            assert!(matches!(run(owner, &requests, work - 1, storage).0,
                Err(Failure::Resource(ResourceError::Work(error)))
                    if error.limit() == work - 1 && error.actual() > error.limit()));
            assert!(matches!(run(owner, &requests, work, storage - 1).0,
                Err(Failure::Resource(ResourceError::Storage { actual, limit }))
                    if limit == storage - 1 && actual > limit));
        });
    }
}

#[test]
fn paid_accesses_preserve_runtime_read_fallback_and_volatile_refusal() {
    for scalar in [
        ScalarType::U8,
        ScalarType::U32,
        ScalarType::U64,
        ScalarType::F32,
    ] {
        for mode in [AccessMode::ReadOnly, AccessMode::ReadWrite] {
            for volatile in [false, true] {
                let mut module = crate::formal_memory_obligations::guarded_access_v1::runtime_slice_read_v1::tests::fixture(scalar, mode);
                let operation =
                    &mut module.functions[0].body.as_mut().unwrap().blocks[1].operations[1];
                let OperationKind::Load { access, .. } = &mut operation.kind else {
                    panic!("original runtime read fixture");
                };
                access.volatile = volatile;
                let request = (
                    FunctionOperationLocation::new(BlockId(20), 1),
                    ValueId(9),
                    FormalMemoryAccessKind::Read,
                    *access,
                    None,
                    false,
                );
                let observed = compare_accesses(&module, &[request]);
                if volatile {
                    assert_eq!(
                        observed[0],
                        Err(FormalMemoryIncompleteReason::UnsupportedIndexExpression {
                            location: FunctionOperationLocation::new(BlockId(20), 0),
                            index: ValueId(1),
                            allocation: FormalAllocationIdentity { parameter_index: 0 },
                        })
                    );
                } else {
                    assert!(matches!(
                        observed[0].as_ref().unwrap().domain,
                        FormalAccessDomainV1::RuntimeSliceReadBounded(_)
                    ));
                }
            }
        }
    }
}

#[test]
fn paid_accesses_preserve_legacy_write_only_guard_accounting() {
    fn reference<const STORES: bool>(owner: &VerifiedCanonicalKernelIrModuleV18) -> (usize, usize) {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        budget.reserve_storage(23).unwrap();
        let mut affine =
            ActualOwnerAffineV18::build(owner, 0, ControlFlowLimits::DEFAULT, &mut budget).unwrap();
        let mut slots = affine.private_slots(owner, 0, &mut budget).unwrap();
        let pointers = slots.pointers(owner, 0, &mut budget).unwrap();
        let parent_floor = budget.storage();
        type Owner = accesses::ActualOwnerAccessesV18<'static, 'static, 'static, 'static, 'static>;
        let frame = size_of::<Owner>()
            + size_of::<Result<Owner>>()
            + size_of::<std::thread::Result<Result<Option<GuardedAnalysisV1<'static, ()>>>>>()
            + size_of::<(
                &mut ActualOwnerPointersV18<'static, 'static, 'static, 'static>,
                &mut Budget<'static>,
                &VerifiedCanonicalKernelIrModuleV18,
                usize,
            )>();
        budget.reserve_storage(frame).unwrap();
        budget.charge_work(4).unwrap();
        let retained_affine = &mut pointers.slots.affine;
        let source = retained_affine.source;
        let (flow, origins) = retained_affine
            .context
            .guarded_inputs(source, &mut budget)
            .unwrap();
        let mut meter = meter::LiveGuardMeter::new(&mut budget, usize::MAX, usize::MAX, usize::MAX);
        meter
            .storage(size_of::<
                GuardedControlCollectionV1<meter::LiveGuardMeter<'_, '_>>,
            >())
            .unwrap();
        let GuardedControlCollectionV1::Selected(seed) =
            GuardedControlV1::collect_preserving_ledger(source, flow, meter).unwrap()
        else {
            panic!("the genuine Load must select the historical guarded analysis");
        };
        let entry = seed.entry;
        let mut guarded = GuardedAnalysisV1::empty(seed, true);
        canonical_reads::collect_actual_definitions(&mut guarded, source).unwrap();
        guarded
            .collect_parameters_and_truths(source, entry)
            .unwrap();
        guarded
            .ledger
            .reserve(&mut guarded.runtime_reads.origins, origins.len())
            .unwrap();
        guarded.ledger.charge(origins.len()).unwrap();
        guarded.runtime_reads.origins.extend_from_slice(origins);
        guarded
            .collect_runtime_access_guards_v24::<STORES>(source)
            .unwrap();
        guarded.collect_recipes().unwrap();
        // Retire the real borrowed reference facts before refunding their scope.
        drop(guarded);
        budget.rollback_storage(parent_floor).unwrap();
        pointers.release(&mut budget).unwrap();
        slots.release(&mut budget).unwrap();
        affine.release(&mut budget).unwrap();
        assert_eq!(budget.storage(), 23);
        (budget.work(), budget.peak_storage())
    }

    let mut module =
        crate::formal_memory_obligations::guarded_access_v1::runtime_slice_read_v1::tests::fixture(
            ScalarType::U32,
            AccessMode::ReadOnly,
        );
    // A real read keeps the historical constructor selected, but its dominating
    // condition concerns a different, WriteOnly slice. It is not a read bound.
    module.functions[0].signature.parameters[3] = Type::slice(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::WriteOnly,
    );
    module.functions[0].body.as_mut().unwrap().blocks[0].operations[0].kind =
        OperationKind::SliceLength { slice: ValueId(3) };
    with_owner(&module, |owner| {
        let expected = reference::<false>(owner);
        let stores = reference::<true>(owner);
        assert!(
            stores.0 > expected.0,
            "the counterfactual must exercise the extra guard"
        );
        let (observed, work, storage) = run(owner, &[], usize::MAX, usize::MAX);
        assert!(observed.unwrap().is_empty());
        assert_eq!((work, storage), expected);
        assert!(run(owner, &[], expected.0, expected.1).0.is_ok());
        assert!(matches!(
            run(owner, &[], expected.0 - 1, expected.1).0,
            Err(Failure::Resource(ResourceError::Work(_)))
        ));
        assert!(matches!(
            run(owner, &[], expected.0, expected.1 - 1).0,
            Err(Failure::Resource(ResourceError::Storage { .. }))
        ));
    });
}

#[test]
fn paid_accesses_unselected_constructor_has_independent_exact_frame_and_work_bounds() {
    const LIMIT: usize = 100_000_000;
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        op(
            10,
            Type::Scalar(ScalarType::U32),
            OperationKind::Constant(Constant::U32(7)),
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(0),
                value: ValueId(10),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let module = module(vec![block]);
    with_owner(&module, |owner| {
        for boundary in 0..3 {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            let mut affine =
                ActualOwnerAffineV18::build(owner, 0, ControlFlowLimits::DEFAULT, &mut budget)
                    .unwrap();
            let mut slots = affine.private_slots(owner, 0, &mut budget).unwrap();
            let mut pointers = slots.pointers(owner, 0, &mut budget).unwrap();
            type Owner =
                accesses::ActualOwnerAccessesV18<'static, 'static, 'static, 'static, 'static>;
            // Owner, returned owner result, construction unwind/result, exact
            // closure captures and the original control-selection carrier.
            let needed_storage = size_of::<Owner>()
                + size_of::<Result<Owner>>()
                + size_of::<std::thread::Result<Result<Option<GuardedAnalysisV1<'static, ()>>>>>()
                + size_of::<(
                    &mut ActualOwnerPointersV18<'static, 'static, 'static, 'static>,
                    &mut Budget<'static>,
                    &VerifiedCanonicalKernelIrModuleV18,
                    usize,
                )>()
                + size_of::<GuardedControlCollectionV1<meter::LiveGuardMeter<'static, 'static>>>();
            // Entry 4, control selection 32, one block 2 and two operations 16.
            let needed_work = 4 + 32 + 2 + 2 * 8;
            budget
                .charge_work(LIMIT - budget.work() - needed_work + usize::from(boundary == 1))
                .unwrap();
            let pressure = LIMIT - budget.storage() - needed_storage + usize::from(boundary == 2);
            budget.reserve_storage(pressure).unwrap();
            let floor = budget.storage();
            match pointers.accesses(owner, 0, &mut budget) {
                Ok(accesses) => {
                    assert_eq!(boundary, 0);
                    assert_eq!(budget.work(), LIMIT);
                    assert_eq!(budget.storage(), LIMIT);
                    accesses.release(&mut budget).unwrap();
                }
                Err(error) => match (boundary, error) {
                    (1, Failure::Resource(ResourceError::Work(error))) => {
                        assert_eq!((error.actual(), error.limit()), (LIMIT + 1, LIMIT));
                    }
                    (2, Failure::Resource(ResourceError::Storage { actual, limit })) => {
                        assert_eq!((actual, limit), (LIMIT + 1, LIMIT));
                    }
                    (_, error) => panic!("unexpected access constructor refusal: {error:?}"),
                },
            }
            assert_eq!(budget.storage(), floor);
            pointers.release(&mut budget).unwrap();
            slots.release(&mut budget).unwrap();
            affine.release(&mut budget).unwrap();
            assert_eq!(budget.storage(), pressure);
            budget.release_storage(pressure).unwrap();
            assert_eq!(budget.storage(), 0);
        }
    });
}

#[test]
fn paid_accesses_bind_same_owner_root_actual_operation_and_original_budget() {
    let (module, requests) = direct_accesses();
    with_owner(&module, |owner| {
        with_owner(&module, |foreign| {
            for fault in 0..7 {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
                let mut budget = Budget::new(&mut work, usize::MAX);
                let mut affine =
                    ActualOwnerAffineV18::build(owner, 0, ControlFlowLimits::DEFAULT, &mut budget)
                        .unwrap();
                let mut slots = affine.private_slots(owner, 0, &mut budget).unwrap();
                let mut pointers = slots.pointers(owner, 0, &mut budget).unwrap();
                let mut accesses = pointers.accesses(owner, 0, &mut budget).unwrap();
                let location = requests[0].0;
                let invocations = InvocationRange1d::new(0, 8).unwrap();
                let before = (budget.work(), budget.storage());
                let mut other_work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
                let mut other = Budget::new(&mut other_work, usize::MAX);
                assert!(
                    accesses
                        .access(owner, 0, location, invocations, &mut other)
                        .is_err()
                );
                assert_eq!((other.work(), other.storage()), (0, 0));
                assert_eq!((budget.work(), budget.storage()), before);
                assert!(
                    accesses
                        .access(owner, 0, location, invocations, &mut budget)
                        .unwrap()
                        .is_ok()
                );
                let mut expected = Failure::Resource(ResourceError::Accounting);
                let wrong = match fault {
                    0 => accesses.access(foreign, 0, location, invocations, &mut budget),
                    1 => accesses.access(owner, 1, location, invocations, &mut budget),
                    2 => accesses.access(
                        owner,
                        0,
                        FunctionOperationLocation::new(BlockId(0), 0),
                        invocations,
                        &mut budget,
                    ),
                    3 => accesses.conservative_guarded_read(
                        owner,
                        0,
                        location,
                        invocations,
                        &mut budget,
                    ),
                    _ => {
                        if fault == 5 {
                            expected = budget.charge_work(usize::MAX).unwrap_err().into();
                        } else if fault == 6 {
                            expected = budget.reserve_storage(usize::MAX).unwrap_err().into();
                        }
                        budget.release_storage(1).unwrap();
                        let result = accesses.access(owner, 0, location, invocations, &mut budget);
                        budget.reserve_storage(1).unwrap();
                        result
                    }
                };
                assert_eq!(wrong, Err(expected.clone()));
                assert_eq!(
                    accesses.access(owner, 0, location, invocations, &mut budget),
                    Err(expected)
                );
                accesses.release(&mut budget).unwrap();
                pointers.release(&mut budget).unwrap();
                slots.release(&mut budget).unwrap();
                affine.release(&mut budget).unwrap();
                assert_eq!(budget.storage(), 0);
            }
        })
    });
}
