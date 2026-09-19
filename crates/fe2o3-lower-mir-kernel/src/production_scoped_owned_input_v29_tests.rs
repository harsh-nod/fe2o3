fn owned_input_payload(input: &OwnedExecutionInputV29) -> usize {
    size_of::<OwnedExecutionInputV29>()
        + input.roots.capacity() * size_of::<ScopedRootRecipeV29>()
        + input.classes.capacity() * size_of::<ProductionScopeCallableCandidateV29>()
        + input.events.capacity() * size_of::<crate::ProductionScopeEventCandidateV29>()
        + input.launch.capacity() * size_of::<crate::ProductionSourceLaunchRootV1>()
}

#[test]
fn owned_execution_input_reborrows_source_and_retains_real_module_output() {
    for kind in [
        ModuleFixture::Ordinary,
        ModuleFixture::Mixed,
        ModuleFixture::Array,
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        with_module_fixture(kind, &mut budget, |source, budget| {
            let floor = budget.storage();
            let before = budget.work();
            let owned = OwnedExecutionInputV29::capture(source, budget).unwrap();
            assert_eq!(owned.retained_storage, owned_input_payload(&owned));
            assert_eq!(budget.storage(), floor + owned.retained_storage);
            if matches!(kind, ModuleFixture::Mixed) {
                assert_eq!(
                    [
                        owned.roots.len(),
                        owned.classes.len(),
                        owned.events.len(),
                        owned.launch.len()
                    ],
                    [1, 7, 4, 3]
                );
                assert_eq!(
                    budget.work() - before,
                    79 + size_of::<ScopedRootRecipeV29>()
                        + 7 * size_of::<ProductionScopeCallableCandidateV29>()
                        + 4 * size_of::<crate::ProductionScopeEventCandidateV29>()
                        + 3 * size_of::<crate::ProductionSourceLaunchRootV1>()
                );
            }
            let input_floor = budget.storage();
            let output = owned
                .with_source(source.owner, source.launch, budget, |rebuilt, budget| {
                    assert!(std::ptr::eq(rebuilt.owner, source.owner));
                    assert_eq!(rebuilt.input.classes, source.input.classes);
                    assert_eq!(rebuilt.input.events, source.input.events);
                    for (actual, original) in rebuilt.input.roots.iter().zip(source.input.roots) {
                        assert!(std::ptr::eq(
                            actual.helper_arguments,
                            original.helper_arguments
                        ));
                    }
                    admit_pending_scoped_module_v29(
                        rebuilt,
                        ProductionSemanticKirLimitsV1::default(),
                        budget,
                    )
                })
                .unwrap();
            assert_eq!(budget.storage(), input_floor + output.retained_storage);
            check_scoped_module(&output, source, kind);
            let retained = output.retained_storage;
            drop(output);
            budget.release_storage(retained).unwrap();
            let retained = owned.retained_storage;
            drop(owned);
            budget.release_storage(retained).unwrap();
            assert_eq!(budget.storage(), floor);
        })
        .unwrap();
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn owned_execution_input_rejects_source_and_recipe_substitution_before_visit() {
    for fault in 0..8 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        with_module_fixture(ModuleFixture::Mixed, &mut budget, |source, budget| {
            let mut owned = OwnedExecutionInputV29::capture(source, budget).unwrap();
            let other = module_fixture_owner(ModuleFixture::Array);
            match fault {
                0 => owned.semantic_sha256[0] ^= 1,
                1 => owned.ssa = other.identity(),
                2 => owned.roots[0].helper_call.block = SemanticBlockIdV1::from_index(2),
                3 => {
                    owned.roots[0].helper_identity =
                        other.source_semantic().functions()[0].identity()
                }
                4 => owned.classes[2] = ProductionScopeCallableCandidateV29::Ordinary,
                5 => {
                    owned.events.pop();
                }
                6 => owned.events[1] = owned.events[0],
                7 => {}
                _ => unreachable!(),
            }
            let floor = budget.storage();
            let mut visited = false;
            assert!(
                owned
                    .with_source(
                        if fault == 7 { &other } else { source.owner },
                        source.launch,
                        budget,
                        |_, _| {
                            visited = true;
                            Ok(())
                        }
                    )
                    .is_err(),
                "fault {fault}"
            );
            assert!(!visited);
            assert_eq!(budget.storage(), floor);
            let retained = owned.retained_storage;
            drop(owned);
            budget.release_storage(retained).unwrap();
        })
        .unwrap();
        assert_eq!(budget.storage(), 0);
    }
}

fn alternate_module_launch(
    source: &ExecutionLifecycleSourceV29<'_>,
) -> ProductionSourceLaunchRosterV1 {
    let semantic = source.owner.source_semantic();
    let inputs: Vec<_> = semantic
        .roots()
        .iter()
        .enumerate()
        .map(|(ordinal, root)| {
            let entry = semantic.functions()[root.index() as usize]
                .kernel_entry()
                .unwrap();
            ProductionSourceLaunchRootInputV1::new(
                std::str::from_utf8(entry.export_symbol().as_bytes()).unwrap(),
                *entry.kernel_binding_identity().as_bytes(),
                ProductionSourceLaunchInputV1::new(
                    1,
                    Some([64, 1, 1]),
                    [if ordinal == 1 { 4 } else { ordinal as u32 + 1 }, 1, 1],
                ),
            )
        })
        .collect();
    ProductionSourceLaunchRosterV1::try_new(semantic, &inputs).unwrap()
}

#[test]
fn owned_execution_input_rejects_launch_substitution_even_when_graph_is_identical() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    with_module_fixture(ModuleFixture::Mixed, &mut budget, |source, budget| {
        let floor = budget.storage();
        let owned = OwnedExecutionInputV29::capture(source, budget).unwrap();
        let launch = alternate_module_launch(source);
        let alternate =
            ExecutionLifecycleSourceV29::new(source.owner, &launch, source.input, budget).unwrap();
        let limits = ProductionSemanticKirLimitsV1::default();
        let pending = admit_pending_scoped_module_v29(source, limits, budget).unwrap();
        let scratch_floor = budget.storage();
        let emitted = scoped_module_roots_v29(&alternate, limits, budget).unwrap();
        let (candidate, roots) =
            scoped_module_candidate_v29(&alternate, emitted, limits, budget).unwrap();
        assert!(
            pending
                .graph
                .matches_module_with_budget_v15(&candidate, budget)
                .unwrap()
        );
        drop((candidate, roots));
        budget
            .release_storage(budget.storage() - scratch_floor)
            .unwrap();
        let before = budget.work();
        let mut visited = false;
        assert!(
            owned
                .with_source(source.owner, &launch, budget, |_, _| {
                    visited = true;
                    Ok(())
                })
                .is_err()
        );
        assert!(!visited);
        assert_eq!(
            budget.work() - before,
            98 + 3 * size_of::<crate::ProductionSourceLaunchRootV1>()
        );
        let retained = pending.retained_storage + owned.retained_storage;
        drop((pending, owned));
        budget.release_storage(retained).unwrap();
        assert_eq!(budget.storage(), floor);
    })
    .unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn owned_execution_input_error_and_panic_refund_only_view_storage() {
    for retained in [0, 19] {
        for panic in [false, true] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
            let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
            with_module_fixture(ModuleFixture::Mixed, &mut budget, |source, budget| {
                let owned = OwnedExecutionInputV29::capture(source, budget).unwrap();
                let floor = budget.storage();
                let mut output = None;
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    owned.with_source(source.owner, source.launch, budget, |_, budget| {
                        let mut rows = emission_vec_v1::<u8>(retained, budget)?;
                        rows.resize(retained, 7);
                        output = Some(rows);
                        if panic {
                            std::panic::panic_any(1729_u32);
                        }
                        Err::<(), _>(scoped_module_error_v29().into())
                    })
                }));
                if panic {
                    assert_eq!(result.unwrap_err().downcast_ref::<u32>(), Some(&1729));
                } else {
                    assert!(result.unwrap().is_err());
                }
                let visitor_storage = output.as_ref().unwrap().capacity();
                assert_eq!(budget.storage(), floor + visitor_storage);
                drop(output.take());
                budget.release_storage(visitor_storage).unwrap();
                let retained = owned.retained_storage;
                drop(owned);
                budget.release_storage(retained).unwrap();
            })
            .unwrap();
            assert_eq!(budget.storage(), 0);
        }
    }
}

#[test]
fn owned_execution_input_checks_ledger_and_live_reservation_before_work() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, MODULE_LIMIT);
    foreign.reserve_storage(900_000).unwrap();
    foreign.charge_work(7).unwrap();
    with_module_fixture(ModuleFixture::Mixed, &mut budget, |source, budget| {
        assert!(matches!(
            OwnedExecutionInputV29::capture(source, &mut foreign),
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting
                )
            )
        ));
        let owned = OwnedExecutionInputV29::capture(source, budget).unwrap();
        assert!(
            owned
                .with_source(source.owner, source.launch, &mut foreign, |_, _| {
                    panic!("foreign visitor");
                })
                .is_err()
        );
        assert_eq!(foreign.storage(), 900_000);
        assert_eq!(foreign.work(), 7);
        let released = budget.storage() - owned.retained_storage + 1;
        budget.release_storage(released).unwrap();
        let before = budget.work();
        assert!(
            owned
                .with_source(source.owner, source.launch, budget, |_, _| {
                    panic!("unreserved visitor");
                })
                .is_err()
        );
        assert_eq!(budget.work(), before);
        budget.reserve_storage(released).unwrap();
        let retained = owned.retained_storage;
        drop(owned);
        budget.release_storage(retained).unwrap();
    })
    .unwrap();
}

#[test]
fn owned_execution_input_never_refunds_a_replaced_ledger_or_swallows_panic() {
    for mode in 0..3 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, MODULE_LIMIT);
        foreign.reserve_storage(900_000).unwrap();
        foreign.charge_work(7).unwrap();
        with_module_fixture(ModuleFixture::Mixed, &mut budget, |source, budget| {
            let owned = OwnedExecutionInputV29::capture(source, budget).unwrap();
            let floor = budget.storage();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                owned.with_source(source.owner, source.launch, budget, |_, budget| {
                    std::mem::swap(budget, &mut foreign);
                    match mode {
                        0 => Ok(()),
                        1 => Err(scoped_module_error_v29().into()),
                        _ => std::panic::panic_any(1729_u32),
                    }
                })
            }));
            if mode == 2 {
                assert_eq!(result.unwrap_err().downcast_ref::<u32>(), Some(&1729));
            } else {
                assert!(matches!(
                    result,
                    Ok(Err(ScopedModuleErrorV29::Source(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Accounting
                        )
                    )))
                ));
            }
            assert_eq!(budget.storage(), 900_000);
            assert_eq!(budget.work(), 7);
            assert_eq!(budget.peak_storage(), 900_000);
            assert!(budget.failed_storage().is_none());
            std::mem::swap(budget, &mut foreign);
            assert!(budget.storage() > floor);
            budget.release_storage(budget.storage() - floor).unwrap();
            let retained = owned.retained_storage;
            drop(owned);
            budget.release_storage(retained).unwrap();
        })
        .unwrap();
        assert_eq!(budget.storage(), 0);
    }
}

fn owned_input_probe(
    work_limit: usize,
    storage_limit: usize,
) -> (
    Result<(), ScopedModuleErrorV29>,
    usize,
    usize,
    Option<usize>,
    Option<usize>,
) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let result = with_module_fixture(ModuleFixture::Mixed, &mut budget, |source, budget| {
        let floor = budget.storage();
        let owned = OwnedExecutionInputV29::capture(source, budget)?;
        let result = owned.with_source(source.owner, source.launch, budget, |_, _| Ok(()));
        let retained = owned.retained_storage;
        drop(owned);
        budget.release_storage(retained)?;
        assert_eq!(budget.storage(), floor);
        result
    })
    .and_then(|result| result);
    assert_eq!(budget.storage(), MODULE_FLOOR);
    let peak = budget.peak_storage();
    let failed_storage = budget.failed_storage();
    drop(budget);
    (
        result,
        work.work(),
        peak,
        work.failed_work(),
        failed_storage,
    )
}

#[test]
fn owned_execution_input_obeys_exact_and_one_short_resources() {
    let (result, work, storage, _, _) = owned_input_probe(MODULE_LIMIT, MODULE_LIMIT);
    result.unwrap();
    owned_input_probe(work, storage).0.unwrap();
    let short_work = owned_input_probe(work - 1, storage);
    assert!(short_work.0.is_err());
    assert!(short_work.3.is_some());
    let short_storage = owned_input_probe(work, storage - 1);
    assert!(short_storage.0.is_err());
    assert!(short_storage.4.is_some());
}
