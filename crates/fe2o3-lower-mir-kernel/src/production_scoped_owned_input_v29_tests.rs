fn owned_input_payload(input: &OwnedExecutionInputV29) -> usize {
    size_of::<OwnedExecutionInputV29>()
        + input.roots.capacity() * size_of::<ScopedRootRecipeV29>()
        + input.classes.capacity() * size_of::<ProductionScopeCallableCandidateV29>()
        + input.events.capacity() * size_of::<crate::ProductionScopeEventCandidateV29>()
        + input.launch.capacity() * size_of::<crate::ProductionSourceLaunchRootV1>()
        + input.kernel_argument_abi.as_ref().map_or(0, |profile| profile.retained_storage())
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
        let (candidate, roots, _) = with_scoped_source_cleanup_v29(
            budget,
            scratch_floor,
            |cleanup, budget| scoped_source_candidate_v29(&alternate, limits, cleanup, budget),
        )
        .unwrap();
        assert!(
            pending
                .graph
                .matches_module_with_budget_v18(&candidate, budget)
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
                .with_source::<()>(source.owner, source.launch, &mut foreign, |_, _| {
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
                .with_source::<()>(source.owner, source.launch, budget, |_, _| {
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
            } else if mode == 1 {
                assert!(matches!(
                    result,
                    Ok(Err(ScopedModuleErrorV29::Source(
                        ProductionSemanticKirErrorV1::Unsupported {
                            function: 0,
                            block: None,
                            statement: None,
                            detail: "scoped module differs from its complete source roster",
                        }
                    )))
                ));
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

#[test]
fn owned_execution_input_capture_cleans_partial_allocation_at_exact_stage_limits() {
    for mode in 0..4 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        with_module_fixture(ModuleFixture::Mixed, &mut budget, |source, budget| {
            let capture_work = 79
                + size_of::<ScopedRootRecipeV29>()
                + 7 * size_of::<ProductionScopeCallableCandidateV29>()
                + 4 * size_of::<crate::ProductionScopeEventCandidateV29>()
                + 3 * size_of::<crate::ProductionSourceLaunchRootV1>();
            let capture_storage = size_of::<OwnedExecutionInputV29>()
                + size_of::<ScopedRootRecipeV29>()
                + 7 * size_of::<ProductionScopeCallableCandidateV29>()
                + 4 * size_of::<crate::ProductionScopeEventCandidateV29>()
                + 3 * size_of::<crate::ProductionSourceLaunchRootV1>();
            if mode == 3 {
                assert!(budget.reserve_storage(usize::MAX).is_err());
                assert!(budget.charge_work(usize::MAX).is_err());
            }
            // Raise the live floor above fixture scratch, then leave only this
            // capture's independently counted allowance in the original ledger.
            let allowance = capture_storage - usize::from(mode == 2);
            let filler = MODULE_LIMIT - budget.storage() - allowance;
            budget.reserve_storage(filler).unwrap();
            budget
                .charge_work(MODULE_LIMIT - budget.work() - capture_work + usize::from(mode == 1))
                .unwrap();
            let floor = budget.storage();
            let before = budget.work();
            match OwnedExecutionInputV29::capture(source, budget) {
                Ok(input) => {
                    assert!(mode == 0 || mode == 3);
                    assert_eq!(input.retained_storage, capture_storage);
                    assert_eq!(budget.storage(), MODULE_LIMIT);
                    assert_eq!(budget.work() - before, capture_work);
                    drop(input);
                    budget.release_storage(capture_storage).unwrap();
                }
                Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error)) => {
                    if mode == 1 {
                        assert!(matches!(error, ArgumentResourceV1::Work(_)));
                        // The final identity-copy charge follows all four allocations.
                        assert_eq!(budget.work() - before, capture_work - 64);
                        assert_eq!(budget.peak_storage(), MODULE_LIMIT);
                    } else {
                        assert_eq!(mode, 2);
                        assert!(matches!(error, ArgumentResourceV1::Storage(_)));
                        assert!(budget.peak_storage() > floor);
                        assert_eq!(budget.failed_storage(), Some(MODULE_LIMIT + 1));
                    }
                }
                other => panic!("unexpected capture outcome: {}", other.is_ok()),
            }
            assert_eq!(budget.storage(), floor);
            if mode == 3 {
                assert_eq!(budget.failed_storage(), Some(usize::MAX));
            }
            budget.release_storage(filler).unwrap();
        })
        .unwrap();
        assert_eq!(budget.storage(), 0);
        if mode == 3 {
            assert_eq!(work.failed_work(), Some(usize::MAX));
        }
    }
}

fn execution_input_comparison_work_v18(input: ProductionExecutionSourceInputV29<'_>) -> usize {
    100
        + input.classes.len() * size_of::<ProductionScopeCallableCandidateV29>()
        + input.events.len() * size_of::<crate::ProductionScopeEventCandidateV29>()
        + input.roots.iter().map(|root| {
            size_of::<ScopedRootRecipeV29>() + 39 + 8 * root.helper_arguments.len()
        }).sum::<usize>()
}

#[test]
fn execution_input_comparison_checks_complete_genuine_census_without_allocation() {
    for kind in [ModuleFixture::Ordinary, ModuleFixture::Mixed, ModuleFixture::Array] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        with_module_fixture(kind, &mut budget, |source, budget| {
            let owned = OwnedExecutionInputV29::capture(source, budget).unwrap();
            let before = (budget.work(), budget.storage(), budget.peak_storage());
            owned.check_candidate_v18(source.owner, source.input, budget).unwrap();
            assert_eq!(budget.work() - before.0, execution_input_comparison_work_v18(source.input));
            assert_eq!((budget.storage(), budget.peak_storage()), (before.1, before.2));
            let retained = owned.retained_storage;
            drop(owned);
            budget.release_storage(retained).unwrap();
        }).unwrap();
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn execution_input_comparison_rejects_source_census_and_exact_root_substitutions() {
    use crate::ProductionContextRootErrorV29 as Error;
    for fault in 0..29 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        with_module_fixture(ModuleFixture::Mixed, &mut budget, |source, budget| {
            let owned = OwnedExecutionInputV29::capture(source, budget).unwrap();
            owned.check_candidate_v18(source.owner, source.input, budget).unwrap();
            let other = module_fixture_owner(ModuleFixture::Array);
            let mut sha = *source.input.semantic_sha256;
            sha[0] ^= 1;
            let mut roots = source.input.roots.to_vec();
            let mut classes = source.input.classes.to_vec();
            let mut events = source.input.events.to_vec();
            let mut arguments = roots[0].helper_arguments.to_vec();
            let mut candidate = source.input;
            let mut owner = source.owner;
            let expected = match fault {
                0 => { candidate.semantic_sha256 = &sha; Error::Source }
                1 => { owner = &other; Error::Source }
                2 => { roots.clear(); Error::RootCensus }
                3 => { roots.push(roots[0]); Error::RootCensus }
                4 => { classes.pop(); Error::CallableCensus }
                5 => { classes.push(ProductionScopeCallableCandidateV29::Ordinary); Error::CallableCensus }
                6 => { classes.swap(0, 2); Error::CallableCensus }
                7 => { events.pop(); Error::ScopeEventCensus }
                8 => { events.push(events[0]); Error::ScopeEventCensus }
                9 => { events.swap(0, 1); Error::ScopeEventCensus }
                10 => { events[0].statement_count += 1; Error::ScopeEventCensus }
                11 => { roots[0].semantic_sha256 = &sha; Error::RootCensus }
                12 => { roots[0].root = SemanticFunctionIdV1::from_index(0); Error::RootCensus }
                13 => { roots[0].root_identity = source.owner.source_semantic().functions()[0].identity(); Error::RootCensus }
                14 => { roots[0].helper = SemanticFunctionIdV1::from_index(0); Error::RootCensus }
                15 => { roots[0].helper_identity = source.owner.source_semantic().functions()[0].identity(); Error::RootCensus }
                16 => { roots[0].issuer = SemanticCallableIdV1::from_index(0); Error::RootCensus }
                17 => { roots[0].issuer_identity = source.owner.source_semantic().functions()[0].identity(); Error::RootCensus }
                18 => { roots[0].context_type = U32; Error::RootCensus }
                19 => { roots[0].context_identity = source.owner.source_semantic().types()[U32.index() as usize].identity(); Error::RootCensus }
                20 => { roots[0].issuance.statement_count += 1; Error::RootCensus }
                21 => { roots[0].helper_call.destination = SemanticLocalIdV1::from_index(1); Error::RootCensus }
                22 => { roots[0].helper_context_local = SemanticLocalIdV1::from_index(1); Error::RootCensus }
                23 => { arguments.clear(); roots[0].helper_arguments = &arguments; Error::Arguments }
                24 => {
                    let SemanticOperandV1::Move(place) = &arguments[0] else { panic!("context move"); };
                    arguments[0] = SemanticOperandV1::Copy(place.clone());
                    roots[0].helper_arguments = &arguments;
                    Error::Arguments
                }
                25 => {
                    let SemanticOperandV1::Move(place) = &arguments[0] else { panic!("context move"); };
                    arguments[0] = SemanticOperandV1::Move(SemanticPlaceV1::new(
                        SemanticLocalIdV1::from_index(place.local().index() + 1), Vec::new(), place.ty(),
                    ).unwrap());
                    roots[0].helper_arguments = &arguments;
                    Error::Arguments
                }
                26 => { roots[0].issuance.target = SemanticBlockIdV1::from_index(0); Error::RootCensus }
                27 => { roots[0].helper_call.target = SemanticBlockIdV1::from_index(0); Error::RootCensus }
                28 => {
                    let SemanticOperandV1::Move(place) = &arguments[0] else { panic!("context move"); };
                    let projection = SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), place.ty()).unwrap();
                    arguments[0] = SemanticOperandV1::Move(SemanticPlaceV1::new(
                        place.local(), vec![projection; 1024], place.ty(),
                    ).unwrap());
                    roots[0].helper_arguments = &arguments;
                    Error::Arguments
                }
                _ => unreachable!(),
            };
            candidate.roots = &roots;
            candidate.classes = &classes;
            candidate.events = &events;
            let before = (budget.storage(), budget.peak_storage());
            assert_eq!(owned.check_candidate_v18(owner, candidate, budget), Err(expected), "fault {fault}");
            assert_eq!((budget.storage(), budget.peak_storage()), before);
            // Relative comparison does not itself poison the owner or mint a receipt.
            owned.check_candidate_v18(source.owner, source.input, budget).unwrap();
            let retained = owned.retained_storage;
            drop(owned);
            budget.release_storage(retained).unwrap();
        }).unwrap();
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn execution_input_comparison_exact_work_zero_scratch_and_lost_custody_boundaries() {
    use crate::ProductionContextRootErrorV29 as Error;
    for mode in 0..4 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, MODULE_LIMIT);
        with_module_fixture(ModuleFixture::Mixed, &mut budget, |source, budget| {
            let owned = OwnedExecutionInputV29::capture(source, budget).unwrap();
            let comparison_work = execution_input_comparison_work_v18(source.input);
            if mode < 2 {
                let filler = MODULE_LIMIT - budget.storage();
                budget.reserve_storage(filler).unwrap();
                budget.charge_work(MODULE_LIMIT - budget.work() - comparison_work + usize::from(mode == 1)).unwrap();
                let before = (budget.work(), budget.storage(), budget.peak_storage());
                let result = owned.check_candidate_v18(source.owner, source.input, budget);
                if mode == 0 {
                    result.unwrap();
                    assert_eq!(budget.work() - before.0, comparison_work);
                } else {
                    assert!(matches!(result, Err(Error::Resource(ArgumentResourceV1::Work(_)))));
                }
                assert_eq!((budget.storage(), budget.peak_storage()), (before.1, before.2));
                budget.release_storage(filler).unwrap();
            } else if mode == 2 {
                foreign.reserve_storage(owned.retained_storage).unwrap();
                let before = (foreign.work(), foreign.storage(), budget.work(), budget.storage());
                assert_eq!(owned.check_candidate_v18(source.owner, source.input, &mut foreign), Err(Error::Resource(ArgumentResourceV1::Accounting)));
                assert_eq!((foreign.work(), foreign.storage(), budget.work(), budget.storage()), before);
            } else {
                let released = budget.storage() - owned.retained_storage + 1;
                budget.release_storage(released).unwrap();
                let before = (budget.work(), budget.storage());
                assert_eq!(owned.check_candidate_v18(source.owner, source.input, budget), Err(Error::Resource(ArgumentResourceV1::Accounting)));
                assert_eq!((budget.work(), budget.storage()), before);
                budget.reserve_storage(released).unwrap();
            }
            let retained = owned.retained_storage;
            drop(owned);
            budget.release_storage(retained).unwrap();
        }).unwrap();
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn source_owned_execution_input_comparison_retains_first_refusal() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = prepared_source_fixture(ModuleFixture::Mixed, false, &mut budget);
    let completed = std::cell::Cell::new(false);
    let result = prepared.with_checked_source_v18(&mut budget, |view, budget| {
        let owned = &view.owner.inner.source.input;
        let owner = view.source_ssa(budget)?;
        let mut roots: Vec<_> = owned.roots.iter().map(|root| {
            root.borrow(&owned.semantic_sha256, owner).unwrap()
        }).collect();
        let candidate = ProductionExecutionSourceInputV29 {
            semantic_sha256: &owned.semantic_sha256,
            roots: &roots,
            classes: &owned.classes,
            events: &owned.events,
        };
        view.check_execution_input_v18(candidate, budget)?;
        roots[0].helper_context_local = SemanticLocalIdV1::from_index(1);
        let candidate = ProductionExecutionSourceInputV29 {
            semantic_sha256: &owned.semantic_sha256,
            roots: &roots,
            classes: &owned.classes,
            events: &owned.events,
        };
        assert!(matches!(view.check_execution_input_v18(candidate, budget),
            Err(ProductionSourceOwnedViewErrorV18::Binding("execution source candidate differs from retained input"))));
        let before = (budget.work(), budget.storage());
        assert!(matches!(view.check_execution_input_v18(candidate, budget),
            Err(ProductionSourceOwnedViewErrorV18::Binding("execution source candidate differs from retained input"))));
        assert!(matches!(view.root_count(budget),
            Err(ProductionSourceOwnedViewErrorV18::Binding("execution source candidate differs from retained input"))));
        assert_eq!((budget.work(), budget.storage()), before);
        completed.set(true);
        Ok(())
    });
    assert!(completed.get());
    assert!(matches!(result,
        Err(ProductionSourceOwnedViewErrorV18::Binding("execution source candidate differs from retained input"))));
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn source_owned_execution_input_comparison_work_refusal_keeps_projection_cleanup_eligible() {
    for short in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = prepared_source_fixture(ModuleFixture::Mixed, false, &mut budget);
        let completed = std::cell::Cell::new(false);
        let result = prepared.with_checked_source_v18(&mut budget, |view, budget| {
            view.check_query_v18(budget)?;
            let owner = view.source_ssa(budget)?;
            view.check_original_source(owner, budget)?;
            let owned = &view.owner.inner.source.input;
            let roots: Vec<_> = owned.roots.iter().map(|root| {
                root.borrow(&owned.semantic_sha256, owner).unwrap()
            }).collect();
            let candidate = ProductionExecutionSourceInputV29 {
                semantic_sha256: &owned.semantic_sha256,
                roots: &roots,
                classes: &owned.classes,
                events: &owned.events,
            };
            let allowance = 1 + execution_input_comparison_work_v18(candidate) - usize::from(short);
            budget.charge_work(MODULE_LIMIT - budget.work() - allowance)?;
            let before = (budget.work(), budget.storage());
            let checked = view.check_execution_input_v18(candidate, budget);
            if short {
                assert!(matches!(checked,
                    Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(_)))));
                assert!(matches!(view.check_execution_input_v18(candidate, budget),
                    Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(_)))));
            } else {
                checked?;
                assert_eq!(budget.work() - before.0, allowance);
            }
            assert_eq!(budget.storage(), before.1);
            assert!(!view.cleanup.is_denied());
            completed.set(true);
            Ok(())
        });
        assert!(completed.get());
        if short {
            assert!(matches!(result,
                Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(_)))));
        } else {
            result.unwrap();
        }
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}
