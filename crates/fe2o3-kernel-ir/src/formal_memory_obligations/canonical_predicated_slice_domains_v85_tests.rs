use super::*;

fn run_batch<T>(
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
            with_canonical_predicated_global_stores_v84(
                &owner,
                Default::default(),
                budget,
                |stores, budget| {
                    crate::with_canonical_predicated_conditional_slice_domains_v85(
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

fn launch() -> ExplicitLaunchExtent {
    ExplicitLaunchExtent::Exact {
        rank: 3,
        extents: [64, 1, 1],
    }
}

#[test]
fn predicated_batch_v85_keeps_complete_census_and_independent_runtime_premises() {
    for shape in 0..5 {
        for access in [AccessMode::WriteOnly, AccessMode::ReadWrite] {
            let mut module = predicated_fixture(Axis::X, access, shape);
            let at = predicated_coordinate(&module);
            let ops = &mut module.functions[0].body.as_mut().unwrap().blocks
                [at.block.block as usize]
                .operations;
            ops.push(ops.last().unwrap().clone());
            let (result, _, _) = run_batch(
                &module,
                launch(),
                FormalIndexWidth::Bits64,
                100_000_000,
                100_000_000,
                |batch, budget| {
                    assert_eq!(batch.access_count(budget)?, 2);
                    let first = batch.access_at(at, budget)?.unwrap();
                    let second = batch
                        .access_at(
                            Coordinate {
                                operation: at.operation + 1,
                                ..at
                            },
                            budget,
                        )?
                        .unwrap();
                    for item in [first, second] {
                        let CanonicalConditionalSliceDomainV26::Store(domain) = item.domain()
                        else {
                            panic!()
                        };
                        assert_eq!(domain.path(), FormalGuardedPathV1::ExplicitPredicate);
                        assert_eq!(domain.index(), ValueId(2));
                        assert_eq!(item.invocation_projection(), Some((Axis::X, ValueId(2))));
                    }
                    let parameter = batch.parameter(FunctionCoordinate(0), 0, budget)?.unwrap();
                    assert_eq!((parameter.reads(), parameter.writes()), (0, 2));
                    assert_eq!(parameter.access(), access);
                    assert!(parameter.requires_valid_aligned_extent());
                    assert!(!parameter.requires_initialized_extent());
                    assert!(parameter.requires_exclusive_runtime_binding());
                    assert!(parameter.requires_exact_launch_binding());
                    assert!(!batch.runtime_requirements_are_discharged());
                    assert!(!batch.grants_artifact_or_launch_authority());
                    Ok(())
                },
            );
            assert!(matches!(result, Ok(Some(()))), "shape {shape}: {result:?}");
        }
    }
}

#[test]
fn predicated_batch_v85_legacy_profile_stays_closed_without_dropping_effects() {
    let module = predicated_fixture(Axis::X, AccessMode::WriteOnly, 1);
    let (owner, credit) = owner(&module);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
    let mut budget = Budget::new(&mut work, 100_000_000);
    budget.reserve_storage(credit + 17).unwrap();
    let result = with_canonical_guarded_global_reads_v18(
        &owner,
        Default::default(),
        &mut budget,
        |reads, budget| {
            with_canonical_predicated_global_stores_v84(
                &owner,
                Default::default(),
                budget,
                |stores, budget| {
                    assert_eq!(
                        stores.function_effects(FunctionCoordinate(0), budget)?,
                        (1, 0, 0)
                    );
                    with_canonical_conditional_slice_domains_v26(
                        reads,
                        stores,
                        &[launch()],
                        FormalIndexWidth::Bits64,
                        budget,
                        |_, _| -> Result<()> { panic!("legacy collector admitted explicit store") },
                    )
                },
            )
        },
    );
    assert!(matches!(result, Ok(None)));
    assert_eq!(budget.storage(), credit + 17);
    let ordinary = fixture(Axis::X, AccessMode::ReadWrite, true);
    assert!(matches!(
        run_batch(
            &ordinary,
            launch(),
            FormalIndexWidth::Bits64,
            100_000_000,
            100_000_000,
            |batch, budget| {
                assert_eq!(batch.access_count(budget)?, 2);
                Ok(())
            }
        )
        .0,
        Ok(Some(()))
    ));
}

#[test]
fn predicated_batch_v85_refuses_unbound_launch_noninjective_and_extra_effects() {
    for mutation in 0..6 {
        let mut module = predicated_fixture(Axis::X, AccessMode::ReadWrite, 1);
        let at = predicated_coordinate(&module);
        let ops = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
        match mutation {
            0 => {
                ops.iter_mut()
                    .find(|op| op.results.first().is_some_and(|r| r.id == ValueId(2)))
                    .unwrap()
                    .kind = OperationKind::Constant(Constant::Index(0))
            }
            1 => {
                let OperationKind::GuardedStore { access, .. } =
                    &mut ops[at.operation as usize].kind
                else {
                    panic!()
                };
                access.volatile = true;
            }
            2 => ops.push(Operation::new(
                vec![],
                OperationKind::Store {
                    pointer: ValueId(6),
                    value: ValueId(1),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            )),
            3 => ops.push(op(
                40,
                Type::Scalar(ScalarType::U32),
                OperationKind::Load {
                    pointer: ValueId(6),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            )),
            _ => (),
        }
        let extent = if mutation == 4 {
            ExplicitLaunchExtent::Unknown
        } else {
            launch()
        };
        let width = if mutation == 5 {
            FormalIndexWidth::Unknown
        } else {
            FormalIndexWidth::Bits64
        };
        let (result, _, _) = run_batch(
            &module,
            extent,
            width,
            100_000_000,
            100_000_000,
            |_, _| -> Result<()> { panic!("incomplete family admitted") },
        );
        assert!(
            matches!(result, Ok(None)),
            "mutation {mutation}: {result:?}"
        );
    }
}

#[test]
fn predicated_batch_v85_whole_scope_exact_and_one_short_resource_boundaries() {
    for shape in 0..5 {
        let module = predicated_fixture(Axis::X, AccessMode::WriteOnly, shape);
        let run = |work, storage| {
            run_batch(
                &module,
                launch(),
                FormalIndexWidth::Bits64,
                work,
                storage,
                |batch, budget| {
                    assert_eq!(batch.access_count(budget)?, 1);
                    assert_eq!(
                        batch
                            .parameter(FunctionCoordinate(0), 0, budget)?
                            .unwrap()
                            .writes(),
                        1
                    );
                    Ok(())
                },
            )
        };
        let (wide, work, storage) = run(100_000_000, 100_000_000);
        assert!(matches!(wide, Ok(Some(()))));
        let (exact, actual_work, actual_storage) = run(work, storage);
        assert!(matches!(exact, Ok(Some(()))));
        assert_eq!((actual_work, actual_storage), (work, storage));
        assert!(matches!(
            run(work - 1, storage).0,
            Err(Failure::Resource(ResourceError::Work(_)))
        ));
        assert!(matches!(
            run(work, storage - 1).0,
            Err(Failure::Resource(ResourceError::Storage { .. }))
        ));
    }
}
