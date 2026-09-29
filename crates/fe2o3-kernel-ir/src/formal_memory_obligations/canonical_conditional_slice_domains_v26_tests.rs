#[path = "canonical_conditional_slice_retained_v26_tests.rs"]
mod retained_custody_tests;

fn batch_launch_v26() -> ExplicitLaunchExtent {
    ExplicitLaunchExtent::Exact {
        rank: 3,
        extents: [64, 1, 1],
    }
}

fn run_batch_v26<T>(
    module: &Module,
    launch: ExplicitLaunchExtent,
    width: FormalIndexWidth,
    work_limit: usize,
    storage_limit: usize,
    consume: impl for<'s, 'g> FnOnce(
        &CheckedCanonicalConditionalSliceDomainsV26<'s, 'g>,
        &mut Budget<'_>,
    ) -> Result<T>,
) -> (Result<Option<T>>, usize, usize) {
    let (owner, credit) = owner(module);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(credit + 17).unwrap();
    let result = with_canonical_guarded_global_reads_v18(
        &owner,
        Default::default(),
        &mut budget,
        |reads, budget| {
            with_canonical_guarded_global_stores_v24(
                &owner,
                Default::default(),
                budget,
                |stores, budget| {
                    with_canonical_conditional_slice_domains_v26(
                        reads,
                        stores,
                        &[launch],
                        width,
                        budget,
                        consume,
                    )
                },
            )
        },
    );
    assert_eq!(budget.storage(), credit + 17);
    (result, budget.work(), budget.peak_storage())
}

#[test]
fn conditional_slice_batch_has_exact_occurrences_and_retains_all_runtime_premises() {
    let module = fixture(Axis::X, AccessMode::ReadWrite, true);
    let (result, _, _) = run_batch_v26(
        &module,
        batch_launch_v26(),
        FormalIndexWidth::Bits64,
        100_000_000,
        100_000_000,
        |batch, budget| {
            assert_eq!(batch.function_count(budget)?, 1);
            assert_eq!(batch.access_count(budget)?, 2);
            let store = batch.access_at(coordinate(2), budget)?.unwrap();
            let read = batch.access_at(coordinate(3), budget)?.unwrap();
            assert!(matches!(
                store.domain(),
                CanonicalConditionalSliceDomainV26::Store(_)
            ));
            assert!(matches!(
                read.domain(),
                CanonicalConditionalSliceDomainV26::Read(_)
            ));
            assert_eq!(store.invocation_projection(), Some((Axis::X, ValueId(2))));
            assert_eq!(read.invocation_projection(), Some((Axis::X, ValueId(2))));
            assert!(batch.access_at(coordinate(1), budget)?.is_none());
            let row = batch.parameter(FunctionCoordinate(0), 0, budget)?.unwrap();
            assert_eq!(
                (row.function(), row.parameter(), row.value()),
                (FunctionCoordinate(0), 0, ValueId(0))
            );
            assert_eq!(
                (row.scalar(), row.access(), row.reads(), row.writes()),
                (ScalarType::U32, AccessMode::ReadWrite, 1, 1)
            );
            assert!(row.requires_valid_aligned_extent());
            assert!(row.requires_initialized_extent());
            assert!(row.requires_exclusive_runtime_binding());
            assert!(row.requires_exact_launch_binding());
            assert_eq!(
                batch.function_conditions(FunctionCoordinate(0), budget)?,
                Some((batch_launch_v26(), FormalIndexWidth::Bits64, 1, 1))
            );
            assert!(!batch.runtime_requirements_are_discharged());
            assert!(!batch.grants_artifact_or_launch_authority());
            Ok(())
        },
    );
    assert!(matches!(result, Ok(Some(()))), "{result:?}");
}

#[test]
fn conditional_slice_batch_checks_every_matching_store_pair() {
    let mut module = fixture(Axis::X, AccessMode::ReadWrite, true);
    let body = module.functions[0].body.as_mut().unwrap();
    let store = body.blocks[1].operations[2].clone();
    body.blocks[1].operations.push(store);
    let (result, _, _) = run_batch_v26(
        &module,
        batch_launch_v26(),
        FormalIndexWidth::Bits64,
        100_000_000,
        100_000_000,
        |batch, budget| {
            assert_eq!(batch.access_count(budget)?, 3);
            assert_eq!(
                batch
                    .parameter(FunctionCoordinate(0), 0, budget)?
                    .unwrap()
                    .writes(),
                2
            );
            assert!(batch.access_at(coordinate(4), budget)?.is_some());
            Ok(())
        },
    );
    assert!(matches!(result, Ok(Some(()))), "{result:?}");
}

fn batch_cross_axis_read_v26() -> Module {
    let mut module = fixture(Axis::X, AccessMode::ReadWrite, true);
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks[0].operations.push(op(
        8,
        Type::INDEX,
        OperationKind::Intrinsic(IntrinsicOperation::new(
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Global,
                axis: Axis::Y,
            },
            Type::INDEX,
        )),
    ));
    body.blocks[1].operations.pop();
    body.blocks[1].operations.push(op(
        9,
        Type::BOOL,
        OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs: ValueId(8),
            rhs: ValueId(3),
        },
    ));
    body.blocks[1].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(9),
        then_target: BlockId(40),
        then_arguments: vec![],
        else_target: BlockId(30),
        else_arguments: vec![],
    });
    let mut read = BasicBlock::new(BlockId(40));
    read.operations = vec![
        op(
            10,
            Type::pointer(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Global,
                AccessMode::ReadWrite,
            ),
            OperationKind::GetElementPointer {
                base: ValueId(5),
                offset: ValueId(8),
            },
        ),
        op(
            11,
            Type::Scalar(ScalarType::U32),
            OperationKind::Load {
                pointer: ValueId(10),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    read.terminator = Some(Terminator::Return { values: vec![] });
    body.blocks.push(read);
    module
}

#[test]
fn conditional_slice_batch_refuses_individually_guarded_cross_axis_conflicts() {
    let module = batch_cross_axis_read_v26();
    let (owner, credit) = owner(&module);
    run(&owner, credit, |stores, budget| {
        with_canonical_guarded_global_reads_v18(
            &owner,
            Default::default(),
            budget,
            |reads, budget| {
                let at = Coordinate {
                    block: BlockCoordinate {
                        function: FunctionCoordinate(0),
                        block: 3,
                    },
                    operation: 1,
                };
                let CanonicalGuardedGlobalReadOutcomeV1::ProvedLocalConditions(fact) =
                    reads.read_at(at, budget)?
                else {
                    panic!("read must independently satisfy its local guard");
                };
                assert_eq!(
                    stores.read_invocation_projection(&fact, budget)?,
                    Some((Axis::Y, ValueId(8)))
                );
                assert!(
                    with_canonical_conditional_slice_domains_v26(
                        reads,
                        stores,
                        &[batch_launch_v26()],
                        FormalIndexWidth::Bits64,
                        budget,
                        |_, _| -> Result<()> {
                            panic!("conflicting whole family entered consumer")
                        }
                    )?
                    .is_none()
                );
                Ok(())
            },
        )
    })
    .unwrap();
}

#[test]
fn conditional_slice_batch_refuses_volatile_effects_raw_pointers_and_unbound_store_launches() {
    let mut volatile = fixture(Axis::X, AccessMode::ReadWrite, true);
    let OperationKind::Store { access, .. } =
        &mut volatile.functions[0].body.as_mut().unwrap().blocks[1].operations[2].kind
    else {
        unreachable!()
    };
    access.volatile = true;
    let mut raw = fixture(Axis::X, AccessMode::ReadWrite, false);
    raw.functions[0].signature.parameters[0] = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    );
    let body = raw.functions[0].body.as_mut().unwrap();
    body.blocks.truncate(1);
    body.blocks[0].operations = vec![Operation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(0),
            value: ValueId(1),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    )];
    body.blocks[0].terminator = Some(Terminator::Return { values: vec![] });
    for module in [&volatile, &raw] {
        let (result, _, _) = run_batch_v26(
            module,
            batch_launch_v26(),
            FormalIndexWidth::Bits64,
            100_000_000,
            100_000_000,
            |_, _| -> Result<()> { panic!("unsupported effect entered consumer") },
        );
        assert!(matches!(result, Ok(None)), "{result:?}");
    }
    let module = fixture(Axis::X, AccessMode::ReadWrite, true);
    for (launch, width) in [
        (batch_launch_v26(), FormalIndexWidth::Unknown),
        (
            ExplicitLaunchExtent::Exact {
                rank: 3,
                extents: [64, 2, 1],
            },
            FormalIndexWidth::Bits64,
        ),
        (
            ExplicitLaunchExtent::Exact {
                rank: 3,
                extents: [(1_u64 << 32) + 1, 1, 1],
            },
            FormalIndexWidth::Bits32,
        ),
    ] {
        let (result, _, _) = run_batch_v26(
            &module,
            launch,
            width,
            100_000_000,
            100_000_000,
            |_, _| -> Result<()> { panic!("unbound launch entered consumer") },
        );
        assert!(matches!(result, Ok(None)), "{result:?}");
    }
}

#[test]
fn conditional_slice_batch_whole_scope_exact_and_one_short_limits() {
    let module = fixture(Axis::X, AccessMode::ReadWrite, true);
    let query = |batch: &CheckedCanonicalConditionalSliceDomainsV26<'_, '_>,
                 budget: &mut Budget<'_>| {
        assert!(batch.access_at(coordinate(2), budget)?.is_some());
        Ok(())
    };
    let (result, work, peak) = run_batch_v26(
        &module,
        batch_launch_v26(),
        FormalIndexWidth::Bits64,
        100_000_000,
        100_000_000,
        query,
    );
    result.unwrap().unwrap();
    let (result, exact_work, exact_peak) = run_batch_v26(
        &module,
        batch_launch_v26(),
        FormalIndexWidth::Bits64,
        work,
        peak,
        query,
    );
    result.unwrap().unwrap();
    assert_eq!((work, peak), (exact_work, exact_peak));
    let (result, accepted, _) = run_batch_v26(
        &module,
        batch_launch_v26(),
        FormalIndexWidth::Bits64,
        work - 1,
        peak,
        query,
    );
    assert!(matches!(result, Err(Failure::Resource(_))));
    assert!(accepted < work);
    let (result, _, accepted) = run_batch_v26(
        &module,
        batch_launch_v26(),
        FormalIndexWidth::Bits64,
        work,
        peak - 1,
        query,
    );
    assert!(matches!(result, Err(Failure::Resource(_))));
    assert!(accepted < peak);
}

#[test]
fn conditional_slice_batch_drains_result_when_parent_query_poison_is_ignored() {
    struct Hostile(std::rc::Rc<std::cell::Cell<usize>>);
    impl Drop for Hostile {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
            panic!("hostile rejected callback value");
        }
    }
    let (owner, credit) = owner(&fixture(Axis::X, AccessMode::ReadWrite, true));
    let dropped = std::rc::Rc::new(std::cell::Cell::new(0));
    let direct_checked = std::cell::Cell::new(false);
    let result = run(&owner, credit, |stores, budget| {
        with_canonical_guarded_global_reads_v18(
            &owner,
            Default::default(),
            budget,
            |reads, budget| {
                let direct = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    with_canonical_conditional_slice_domains_v26(
                        reads,
                        stores,
                        &[batch_launch_v26()],
                        FormalIndexWidth::Bits64,
                        budget,
                        |_, budget| {
                            let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
                            let mut foreign = Budget::new(&mut work, 100_000_000);
                            foreign.reserve_storage(budget.storage())?;
                            assert!(reads.owner(&mut foreign).is_err());
                            Ok(Hostile(dropped.clone()))
                        },
                    )
                }));
                let Ok(result) = direct else {
                    panic!("batch leaked rejected-value Drop panic");
                };
                assert!(matches!(
                    &result,
                    Err(Failure::Resource(ResourceError::Accounting))
                ));
                direct_checked.set(true);
                result
            },
        )
    });
    assert!(matches!(result, Err(Failure::Resource(_))));
    assert_eq!(dropped.get(), 1);
    assert!(direct_checked.get());
}

#[test]
fn conditional_slice_batch_drains_uninvoked_capture_without_replacing_entry_or_build_refusal() {
    struct Hostile(std::rc::Rc<std::cell::Cell<usize>>);
    impl Drop for Hostile {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
            panic!("hostile uncalled capture");
        }
    }
    let module = fixture(Axis::X, AccessMode::ReadWrite, true);
    let (first, credit) = owner(&module);
    let (other, other_credit) = owner(&module);
    for foreign_owner in [false, true] {
        let dropped = std::rc::Rc::new(std::cell::Cell::new(0));
        let direct_checked = std::cell::Cell::new(false);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
        let mut budget = Budget::new(&mut work, 100_000_000);
        budget.reserve_storage(credit + other_credit + 17).unwrap();
        let result = with_canonical_guarded_global_reads_v18(
            &first,
            Default::default(),
            &mut budget,
            |reads, budget| {
                with_canonical_guarded_global_stores_v24(
                    if foreign_owner { &other } else { &first },
                    Default::default(),
                    budget,
                    |stores, budget| {
                        if !foreign_owner {
                            budget.charge_work(100_000_000 - budget.work() - 2)?;
                        }
                        let hostile = Hostile(dropped.clone());
                        let direct = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            with_canonical_conditional_slice_domains_v26(
                                reads,
                                stores,
                                &[batch_launch_v26()],
                                FormalIndexWidth::Bits64,
                                budget,
                                move |_, _| -> Result<()> {
                                    std::hint::black_box(&hostile);
                                    panic!("refused construction invoked callback")
                                },
                            )
                        }));
                        let Ok(result) = direct else {
                            panic!("batch leaked uncalled-capture Drop panic");
                        };
                        if foreign_owner {
                            assert!(matches!(
                                &result,
                                Err(Failure::Resource(ResourceError::Accounting))
                            ));
                        } else {
                            assert!(
                                matches!(&result, Err(Failure::Resource(ResourceError::Work(error))) if error.limit() == 100_000_000 && error.actual() == 100_000_004)
                            );
                        }
                        direct_checked.set(true);
                        result
                    },
                )
            },
        );
        assert!(matches!(result, Err(Failure::Resource(_))));
        assert_eq!(dropped.get(), 1);
        assert!(direct_checked.get());
        assert_eq!(budget.storage(), credit + other_credit + 17);
    }
}

#[test]
fn conditional_slice_batch_preserves_parent_query_refusal_when_callback_panics() {
    let (owner, credit) = owner(&fixture(Axis::X, AccessMode::ReadWrite, true));
    let direct_checked = std::cell::Cell::new(false);
    let result = run(&owner, credit, |stores, budget| {
        with_canonical_guarded_global_reads_v18(
            &owner,
            Default::default(),
            budget,
            |reads, budget| {
                let direct = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    with_canonical_conditional_slice_domains_v26(
                        reads,
                        stores,
                        &[batch_launch_v26()],
                        FormalIndexWidth::Bits64,
                        budget,
                        |_, budget| -> Result<()> {
                            let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
                            let mut foreign = Budget::new(&mut work, 100_000_000);
                            foreign.reserve_storage(budget.storage())?;
                            assert!(stores.owner(&mut foreign).is_err());
                            panic!("callback ignored parent refusal then panicked")
                        },
                    )
                }));
                let Ok(result) = direct else {
                    panic!("batch leaked callback panic");
                };
                assert!(matches!(
                    &result,
                    Err(Failure::Resource(ResourceError::Accounting))
                ));
                direct_checked.set(true);
                result
            },
        )
    });
    assert!(matches!(result, Err(Failure::Resource(_))));
    assert!(direct_checked.get());
}
