// The separate fixture owner holds projected operand borrows, like the backend's
// retained receipt. It is not the SSA owner consumed by the public constructor.
fn with_pending_api_input<'work, R>(
    kind: ModuleFixture,
    preexisting: bool,
    budget: &mut ArgumentBudgetV1<'work>,
    use_input: impl FnOnce(
        ProductionSemanticSsaOwnerV1,
        ProductionSourceLaunchRosterV1,
        ProductionExecutionSourceInputV29<'_>,
        usize,
        &mut ArgumentBudgetV1<'work>,
    ) -> R,
) -> R {
    let projection_owner = module_fixture_owner(kind);
    with_module_fixture_view(&projection_owner, kind, budget, |source, budget| {
        let mut owner = module_fixture_owner(kind);
        let (_, launch) = with_module_fixture_view(&owner, kind, budget, |_, _| ()).unwrap();
        let capture = if preexisting {
            let receipt = owner
                .try_capture_occurrences_with_budget_v1(budget)
                .unwrap();
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            receipt.retained_storage()
        } else {
            0
        };
        use_input(owner, launch, source.input, capture, budget)
    })
    .unwrap()
    .0
}

#[test]
fn pending_scoped_api_owns_projected_input_and_retains_only_its_adopted_reservation() {
    for kind in [
        ModuleFixture::Ordinary,
        ModuleFixture::Mixed,
        ModuleFixture::Array,
    ] {
        for preexisting in [false, true] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
            let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
            budget.reserve_storage(MODULE_FLOOR).unwrap();
            let (pending, capture) = with_pending_api_input(
                kind,
                preexisting,
                &mut budget,
                |owner, launch, input, capture, budget| {
                    let digest = *owner.source_semantic_sha256();
                    let pending =
                        ProductionPendingScopedSourceOwnerV29::try_materialize_with_budget(
                            owner,
                            launch,
                            input,
                            ProductionSemanticKirLimitsV1::default(),
                            budget,
                        )
                        .unwrap();
                    assert_eq!(pending.source_semantic_sha256(), &digest);
                    assert_eq!(
                        budget.storage(),
                        MODULE_FLOOR + capture + pending.adopted_storage()
                    );
                    assert_eq!(pending.assertion_attachment_count(), 2);
                    let expected = if matches!(kind, ModuleFixture::Ordinary) {
                        2
                    } else {
                        3
                    };
                    assert_eq!(pending.pending_module().kernels.len(), expected);
                    assert_eq!(pending.pending_module().functions.len(), expected + 1);
                    (pending, capture)
                },
            );
            // All projection borrows and backing are gone before replay.
            let identity = *pending.pending_identity();
            let graph = pending.pending_module() as *const Module;
            let floor = budget.storage();
            pending.replay_with_budget(&mut budget).unwrap();
            assert_eq!(pending.pending_identity(), &identity);
            assert_eq!(pending.pending_module() as *const Module, graph);
            assert_eq!(budget.storage(), floor);
            let retained = pending.adopted_storage();
            drop(pending);
            budget.release_storage(retained).unwrap();
            assert_eq!(budget.storage(), MODULE_FLOOR + capture);
            budget.release_storage(capture).unwrap();
        }
    }
}

#[test]
fn pending_scoped_api_rejects_missing_preexisting_capture_before_allocating_input() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    with_pending_api_input(
        ModuleFixture::Mixed,
        true,
        &mut budget,
        |owner, launch, input, capture, budget| {
            assert!(capture > 0);
            budget.release_storage(capture).unwrap();
            let before = budget.work();
            let result = ProductionPendingScopedSourceOwnerV29::try_materialize_with_budget(
                owner,
                launch,
                input,
                ProductionSemanticKirLimitsV1::default(),
                budget,
            );
            assert!(matches!(
                result,
                Err(ProductionPendingScopedSourceErrorV29::Source(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Accounting
                    )
                ))
            ));
            assert_eq!(budget.storage(), 0);
            assert_eq!(budget.work(), before);
        },
    );
}

#[test]
fn pending_scoped_api_consuming_failures_preserve_the_entry_floor() {
    for preexisting in [false, true] {
        for late in [false, true] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
            let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
            budget.reserve_storage(MODULE_FLOOR).unwrap();
            with_pending_api_input(
                ModuleFixture::Mixed,
                preexisting,
                &mut budget,
                |owner, launch, input, capture, budget| {
                    let mut digest = *input.semantic_sha256;
                    if !late {
                        digest[0] ^= 1;
                    }
                    let input = ProductionExecutionSourceInputV29 {
                        semantic_sha256: &digest,
                        ..input
                    };
                    let limits = if late {
                        ProductionSemanticKirLimitsV1::new_with_max_operations(0, 128, 128, 1024)
                    } else {
                        ProductionSemanticKirLimitsV1::default()
                    };
                    let result = ProductionPendingScopedSourceOwnerV29::try_materialize_with_budget(
                        owner, launch, input, limits, budget,
                    );
                    assert!(result.is_err());
                    assert_eq!(budget.storage(), MODULE_FLOOR + capture);
                    budget.release_storage(capture).unwrap();
                },
            );
        }
    }
}

#[test]
fn pending_scoped_api_late_reconstruction_panic_drops_before_refunding() {
    for preexisting in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        with_pending_api_input(
            ModuleFixture::Mixed,
            preexisting,
            &mut budget,
            |owner, launch, input, capture, budget| {
                OWNING_REPLAY_ROOT_VISITS.set(0);
                let previous =
                    SCOPED_SLOT_OBSERVER_V29.replace(Some(panic_during_owning_reconstruction));
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    ProductionPendingScopedSourceOwnerV29::try_materialize_with_budget(
                        owner,
                        launch,
                        input,
                        ProductionSemanticKirLimitsV1::default(),
                        budget,
                    )
                }));
                SCOPED_SLOT_OBSERVER_V29.set(previous);
                assert!(result.is_err());
                assert_eq!(OWNING_REPLAY_ROOT_VISITS.get(), 6);
                assert_eq!(budget.storage(), MODULE_FLOOR + capture);
                budget.release_storage(capture).unwrap();
            },
        );
    }
}

#[test]
fn pending_scoped_api_replay_refuses_a_foreign_ledger_without_work() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    let owner = with_pending_api_input(
        ModuleFixture::Mixed,
        false,
        &mut budget,
        |owner, launch, input, _, budget| {
            ProductionPendingScopedSourceOwnerV29::try_materialize_with_budget(
                owner,
                launch,
                input,
                ProductionSemanticKirLimitsV1::default(),
                budget,
            )
            .unwrap()
        },
    );
    let retained = owner.adopted_storage();
    let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, MODULE_LIMIT);
    foreign.reserve_storage(retained).unwrap();
    assert!(owner.replay_with_budget(&mut foreign).is_err());
    assert_eq!(foreign.work(), 0);
    assert_eq!(foreign.storage(), retained);
    drop(owner);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), 0);
}

fn pending_api_probe(preexisting: bool, allowance: Option<(usize, usize)>) -> (bool, usize, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    with_pending_api_input(
        ModuleFixture::Array,
        preexisting,
        &mut budget,
        |owner, launch, input, _, budget| {
            budget
                .reserve_storage(budget.peak_storage() + 1 - budget.storage())
                .unwrap();
            if let Some((work_left, storage_left)) = allowance {
                budget
                    .charge_work(MODULE_LIMIT - budget.work() - work_left)
                    .unwrap();
                budget
                    .reserve_storage(MODULE_LIMIT - budget.storage() - storage_left)
                    .unwrap();
            }
            let entry = budget.storage();
            let before = budget.work();
            let result = ProductionPendingScopedSourceOwnerV29::try_materialize_with_budget(
                owner,
                launch,
                input,
                ProductionSemanticKirLimitsV1::default(),
                budget,
            );
            let okay = result.is_ok();
            if let Ok(owner) = result {
                let retained = owner.adopted_storage();
                assert_eq!(budget.storage(), entry + retained);
                drop(owner);
                budget.release_storage(retained).unwrap();
            }
            assert_eq!(budget.storage(), entry);
            let measured = (okay, budget.work() - before, budget.peak_storage() - entry);
            budget.release_storage(entry).unwrap();
            measured
        },
    )
}

#[test]
fn pending_scoped_api_obeys_exact_and_one_short_complete_transaction_budgets() {
    for preexisting in [false, true] {
        let measured = pending_api_probe(preexisting, None);
        assert!(measured.0 && measured.1 > 0 && measured.2 > 0);
        assert_eq!(
            pending_api_probe(preexisting, Some((measured.1, measured.2))),
            measured
        );
        assert!(!pending_api_probe(preexisting, Some((measured.1 - 1, measured.2))).0);
        assert!(!pending_api_probe(preexisting, Some((measured.1, measured.2 - 1))).0);
        assert!(!pending_api_probe(preexisting, Some((0, measured.2))).0);
        assert!(!pending_api_probe(preexisting, Some((measured.1, 0))).0);
    }
}
