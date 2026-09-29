const CLEANUP_TILE_LIMIT_V29: usize = 100_000_000;

#[derive(Default)]
struct CleanupObservationV29 {
    denied: std::cell::Cell<bool>,
    storage: std::cell::Cell<Option<usize>>,
}

struct CleanupObserverV29<'a> {
    cleanup: &'a ScopedSourceCleanupV29,
    observed: &'a CleanupObservationV29,
}

impl Drop for CleanupObserverV29<'_> {
    fn drop(&mut self) {
        self.observed.denied.set(self.cleanup.is_denied());
        self.observed.storage.set(self.cleanup.fault_storage.get());
    }
}

fn cleanup_probe_v29<'work, T, E: From<ArgumentResourceV1>>(
    budget: &mut ArgumentBudgetV1<'work>,
    rollback: usize,
    skip: usize,
    fault: ScopedSourceCleanupFaultV29,
    observed: &CleanupObservationV29,
    run: impl FnOnce(&ScopedSourceCleanupV29, &mut ArgumentBudgetV1<'work>) -> Result<T, E>,
) -> std::thread::Result<Result<T, E>> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_scoped_source_cleanup_v29(budget, rollback, |cleanup, budget| {
            assert!(cleanup.fault.replace(Some(fault)).is_none());
            cleanup.fault_skip.set(skip);
            let _observer = CleanupObserverV29 { cleanup, observed };
            run(cleanup, budget)
        })
    }))
}

fn require_cleanup_source_error_v29<T>(result: Result<T, ScopedModuleErrorV29>) {
    match result {
        Err(ScopedModuleErrorV29::Source(ProductionSemanticKirErrorV1::Unsupported {
            detail,
            ..
        })) => assert_eq!(detail, "selected source cleanup error"),
        Err(error) => panic!("selected source error was replaced: {error:?}"),
        Ok(value) => {
            drop(value);
            panic!("source fault returned success")
        }
    }
}

#[test]
fn source_cleanup_real_constructor_and_reconstruction_preserve_first_error_and_custody() {
    for kind in [
        ModuleFixture::Ordinary,
        ModuleFixture::Array,
        ModuleFixture::Mixed,
    ] {
        for skip in [0, 1] {
            for undercut in [false, true] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
                let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
                budget.reserve_storage(MODULE_FLOOR).unwrap();
                let source = owning_source_fixture(kind, true, &mut budget).unwrap();
                let rollback = budget.storage() - source.input.retained_storage;
                let mut donor = Some(source);
                let observed = CleanupObservationV29::default();
                let result = cleanup_probe_v29(
                    &mut budget,
                    rollback,
                    skip,
                    ScopedSourceCleanupFaultV29::Error { undercut },
                    &observed,
                    |cleanup, budget| {
                        SourceOwnedScopedModuleV29::try_new_with_cleanup(
                            &mut donor,
                            ProductionSemanticKirLimitsV1::default(),
                            cleanup,
                            budget,
                        )
                    },
                )
                .unwrap();
                require_cleanup_source_error_v29(result);
                assert!(donor.is_none(), "original source adoption changed");
                assert!(
                    observed.storage.get().is_some(),
                    "real source producer was not reached"
                );
                assert_eq!(observed.denied.get(), undercut);
                assert_eq!(
                    budget.storage(),
                    if undercut {
                        observed.storage.get().unwrap()
                    } else {
                        rollback
                    }
                );
            }
        }
    }
}

#[test]
fn source_cleanup_owner_replay_keeps_the_original_owner_and_denied_balance() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    let source = owning_source_fixture(ModuleFixture::Ordinary, true, &mut budget).unwrap();
    let owner = SourceOwnedScopedModuleV29::try_new(
        &mut Some(source),
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap();
    let identity = *owner.pending.graph.identity();
    let bytes = owner.pending.graph.canonical_bytes().as_ptr();
    let floor = budget.storage();
    owner.replay(&mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
    let observed = CleanupObservationV29::default();
    let result = cleanup_probe_v29(
        &mut budget,
        floor,
        0,
        ScopedSourceCleanupFaultV29::Error { undercut: true },
        &observed,
        |cleanup, budget| owner.replay_with_cleanup(cleanup, budget),
    )
    .unwrap();
    require_cleanup_source_error_v29(result);
    assert!(observed.denied.get());
    assert_eq!(budget.storage(), observed.storage.get().unwrap());
    assert_eq!(owner.pending.graph.identity(), &identity);
    assert_eq!(owner.pending.graph.canonical_bytes().as_ptr(), bytes);
}

#[derive(Debug)]
struct CleanupPanicV29 {
    serial: usize,
    drops: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl Drop for CleanupPanicV29 {
    fn drop(&mut self) {
        self.drops.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

fn cleanup_panic_v29(
    serial: usize,
) -> (
    Box<dyn std::any::Any + Send>,
    usize,
    std::sync::Arc<std::sync::atomic::AtomicUsize>,
) {
    let drops = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let payload = Box::new(CleanupPanicV29 {
        serial,
        drops: drops.clone(),
    });
    let address = std::ptr::from_ref(payload.as_ref()) as usize;
    (payload, address, drops)
}

fn require_cleanup_panic_v29(
    payload: Box<dyn std::any::Any + Send>,
    address: usize,
    serial: usize,
    drops: &std::sync::atomic::AtomicUsize,
) {
    let payload = payload.downcast::<CleanupPanicV29>().unwrap();
    assert_eq!(std::ptr::from_ref(payload.as_ref()) as usize, address);
    assert_eq!(payload.serial, serial);
    assert_eq!(drops.load(std::sync::atomic::Ordering::SeqCst), 0);
    drop(payload);
    assert_eq!(drops.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[test]
fn source_cleanup_real_source_replay_preserves_original_boxed_panic() {
    for skip in [0, 1] {
        for undercut in [false, true] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
            let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
            budget.reserve_storage(MODULE_FLOOR).unwrap();
            let source = owning_source_fixture(ModuleFixture::Ordinary, true, &mut budget).unwrap();
            let rollback = budget.storage() - source.input.retained_storage;
            let mut donor = Some(source);
            let observed = CleanupObservationV29::default();
            let (payload, address, drops) = cleanup_panic_v29(91 + skip);
            let result = cleanup_probe_v29(
                &mut budget,
                rollback,
                skip,
                ScopedSourceCleanupFaultV29::Panic { undercut, payload },
                &observed,
                |cleanup, budget| {
                    SourceOwnedScopedModuleV29::try_new_with_cleanup(
                        &mut donor,
                        ProductionSemanticKirLimitsV1::default(),
                        cleanup,
                        budget,
                    )
                },
            );
            assert!(donor.is_none());
            assert_eq!(observed.denied.get(), undercut);
            assert_eq!(
                budget.storage(),
                if undercut {
                    observed.storage.get().unwrap()
                } else {
                    rollback
                }
            );
            match result {
                Err(payload) => require_cleanup_panic_v29(payload, address, 91 + skip, &drops),
                Ok(_) => panic!("raw producer panic was replaced"),
            }
        }
    }
}

fn cleanup_real_source_callback_v29<'work>(
    source: &ScopedSourceInputsV29,
    cleanup: &ScopedSourceCleanupV29,
    budget: &mut ArgumentBudgetV1<'work>,
    callback: impl FnOnce(&mut ArgumentBudgetV1<'work>) -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ScopedModuleErrorV29> {
    source.input.with_source_with_cleanup(
        &source.owner,
        &source.launch,
        cleanup,
        budget,
        |view, budget| {
            with_scoped_source_layouts_v29(
                view,
                ProductionSemanticKirLimitsV1::default(),
                cleanup,
                budget,
                |demands, layouts, budget| {
                    let before = budget.storage();
                    let roots = scoped_module_roots_v29(
                        view,
                        demands,
                        layouts,
                        ProductionSemanticKirLimitsV1::default(),
                        budget,
                    )?;
                    let candidate = scoped_module_candidate_v29(
                        view,
                        roots,
                        ProductionSemanticKirLimitsV1::default(),
                        budget,
                    )?;
                    let scratch = budget.storage() - before;
                    drop(candidate);
                    callback(budget)?;
                    Ok(scratch)
                },
                |scratch, layouts, demands, budget| {
                    layouts.release(budget)?;
                    demands.discard(budget)?;
                    budget.release_storage(scratch)?;
                    Ok(())
                },
            )
            .map_err(ScopedModuleErrorV29::from)
        },
    )
}

#[test]
fn source_cleanup_foreign_ledger_preserves_source_error_and_raw_panic() {
    for panic in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        foreign.reserve_storage(97).unwrap();
        let source = owning_source_fixture(ModuleFixture::Ordinary, true, &mut budget).unwrap();
        let floor = budget.storage();
        let original = budget.work_ledger_identity_v1();
        let other = foreign.work_ledger_identity_v1();
        let original_storage = std::cell::Cell::new(0);
        let denied = std::cell::Cell::new(false);
        let (payload, address, drops) = cleanup_panic_v29(321);
        let mut payload = Some(payload);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_scoped_source_cleanup_v29(&mut budget, floor, |cleanup, budget| {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    cleanup_real_source_callback_v29(&source, cleanup, budget, |budget| {
                        let selected = unsupported(0, None, None, "selected source cleanup error");
                        original_storage.set(budget.storage());
                        std::mem::swap(budget, &mut foreign);
                        if panic {
                            drop(selected);
                            std::panic::resume_unwind(payload.take().unwrap());
                        }
                        Err(selected)
                    })
                }));
                denied.set(cleanup.is_denied());
                match result {
                    Ok(result) => result,
                    Err(payload) => std::panic::resume_unwind(payload),
                }
            })
        }));
        assert!(denied.get());
        assert_eq!(budget.storage(), 97);
        assert_eq!(foreign.storage(), original_storage.get());
        assert!(budget.work_ledger_identity_v1() == other);
        assert!(foreign.work_ledger_identity_v1() == original);
        if panic {
            require_cleanup_panic_v29(result.unwrap_err(), address, 321, &drops);
        } else {
            require_cleanup_source_error_v29(result.unwrap());
            drop(payload);
            assert_eq!(drops.load(std::sync::atomic::Ordering::SeqCst), 1);
        }
    }
}

#[test]
fn source_cleanup_corrupted_success_cannot_escape_as_a_valid_source_result() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    let source = owning_source_fixture(ModuleFixture::Ordinary, true, &mut budget).unwrap();
    let floor = budget.storage();
    with_scoped_source_cleanup_v29(&mut budget, floor, |cleanup, budget| {
        cleanup_real_source_callback_v29(&source, cleanup, budget, |_| Ok(()))
    })
    .unwrap();
    assert_eq!(budget.storage(), floor);
    let result = with_scoped_source_cleanup_v29(&mut budget, floor, |cleanup, budget| {
        let result = cleanup_real_source_callback_v29(&source, cleanup, budget, |budget| {
            budget.release_storage(budget.storage() - (floor + 1))?;
            Ok(())
        });
        assert!(cleanup.is_denied());
        result
    });
    assert!(matches!(
        result,
        Err(ScopedModuleErrorV29::Source(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Accounting
            )
        ))
    ));
    assert_eq!(budget.storage(), floor + 1);
}

#[test]
fn source_cleanup_header_is_charged_once_and_obeys_independent_exact_limits() {
    struct State {
        stage: std::cell::Cell<usize>,
        floors: [usize; 3],
    }
    fn run(
        state: &State,
        cleanup: &ScopedSourceCleanupV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ScopedModuleErrorV29> {
        state.stage.set(1);
        assert_eq!(budget.storage(), state.floors[0]);
        let outer = move |budget: &mut ArgumentBudgetV1<'_>| {
            state.stage.set(2);
            assert_eq!(budget.storage(), state.floors[1]);
            let inner = move |budget: &mut ArgumentBudgetV1<'_>| {
                state.stage.set(3);
                assert_eq!(budget.storage(), state.floors[2]);
                budget.charge_work(5)?;
                Ok::<(), ScopedModuleErrorV29>(())
            };
            assert_eq!(std::mem::size_of_val(&inner), size_of::<&State>());
            let floor = budget.storage();
            scoped_source_attempt_v29(cleanup, budget, floor, inner)
        };
        assert_eq!(
            std::mem::size_of_val(&outer),
            size_of::<(&State, &ScopedSourceCleanupV29)>()
        );
        let floor = budget.storage();
        scoped_source_attempt_v29(cleanup, budget, floor, outer)
    }
    let header = size_of::<ScopedSourceCleanupBoundaryV29>()
        + size_of::<std::thread::Result<Result<(), ScopedModuleErrorV29>>>();
    let callback =
        header + cleanup_callback_header_oracle_v1766::<(), ScopedModuleErrorV29, &State>();
    let outer = callback
        + cleanup_attempt_header_oracle_v1766::<
            (),
            ScopedModuleErrorV29,
            (&State, &ScopedSourceCleanupV29),
        >();
    let complete =
        outer + cleanup_attempt_header_oracle_v1766::<(), ScopedModuleErrorV29, &State>();
    for (allowance, work_allowance, stage, denied) in [
        (header - 1, 5, 0, Some(header)),
        (callback - 1, 5, 0, Some(callback)),
        (outer - 1, 5, 1, Some(outer)),
        (complete - 1, 5, 2, Some(complete)),
        (complete, 5, 3, None),
        (complete, 4, 3, None),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_allowance + 32 + 2 + 1 + 4);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_FLOOR + allowance);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let state = State {
            stage: std::cell::Cell::new(0),
            floors: [callback, outer, complete].map(|bytes| MODULE_FLOOR + bytes),
        };
        let state = &state;
        let consume = move |cleanup: &ScopedSourceCleanupV29, budget: &mut ArgumentBudgetV1<'_>| {
            run(state, cleanup, budget)
        };
        assert_eq!(std::mem::size_of_val(&consume), size_of::<&State>());
        let result = with_scoped_source_cleanup_v29::<(), ScopedModuleErrorV29>(
            &mut budget,
            MODULE_FLOOR,
            consume,
        );
        assert_eq!(budget.storage(), MODULE_FLOOR);
        assert_eq!(state.stage.get(), stage);
        if let Some(denied) = denied {
            assert!(matches!(
                result,
                Err(ScopedModuleErrorV29::Source(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Storage(error)
                    )
                )) if error.actual() == MODULE_FLOOR + denied
                    && error.limit() == MODULE_FLOOR + allowance
            ));
            assert_eq!(budget.failed_storage(), Some(MODULE_FLOOR + denied));
            assert_eq!(
                budget.work(),
                if allowance < header {
                    0
                } else {
                    32 + 2 + 1 + 4
                }
            );
        } else if work_allowance < 5 {
            assert!(matches!(
                result,
                Err(ScopedModuleErrorV29::Source(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Work(_)
                    )
                ))
            ));
        } else {
            result.unwrap();
            assert_eq!(budget.peak_storage(), MODULE_FLOOR + complete);
            assert_eq!(budget.work(), 5 + 32 + 2 + 1 + 4);
        }
    }
}

fn cleanup_callback_header_oracle_v1766<T, E, F>() -> usize {
    type Capture<'a, 'w, F> = (
        F,
        &'a ScopedSourceCleanupBoundaryV29,
        &'a mut ArgumentBudgetV1<'w>,
        Result<(), ArgumentResourceV1>,
    );
    size_of::<F>()
        + std::mem::align_of::<F>()
        + size_of::<Capture<'_, '_, F>>()
        + size_of::<std::panic::AssertUnwindSafe<Capture<'_, '_, F>>>()
        + size_of::<(F, &ScopedSourceCleanupV29, &mut ArgumentBudgetV1<'_>)>()
        + size_of::<T>()
        + size_of::<E>()
        + size_of::<Result<T, E>>()
        + size_of::<Result<(), ArgumentResourceV1>>()
        + size_of::<Result<usize, ArgumentResourceV1>>()
        + size_of::<Box<dyn std::any::Any + Send>>()
        + 2 * size_of::<usize>()
        + source_owned_finish_header_oracle_v26::<T, E>()
}

fn cleanup_attempt_header_oracle_v1766<T, E, F>() -> usize {
    type Frame<'a, 'w, F> = (
        &'a ScopedSourceCleanupV29,
        &'a mut ArgumentBudgetV1<'w>,
        usize,
        F,
    );
    type Capture<'a, 'w, F> = (
        F,
        &'a mut ArgumentBudgetV1<'w>,
        &'a mut usize,
        &'a mut usize,
    );
    size_of::<F>()
        + std::mem::align_of::<F>()
        + size_of::<Frame<'_, '_, F>>()
        + size_of::<Capture<'_, '_, F>>()
        + size_of::<std::panic::AssertUnwindSafe<Capture<'_, '_, F>>>()
        + size_of::<std::thread::Result<Result<T, E>>>()
        + size_of::<Result<T, E>>()
        + size_of::<T>()
        + size_of::<E>()
        + size_of::<Box<dyn std::any::Any + Send>>()
        + size_of::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>()
        + 7 * size_of::<usize>()
        + size_of::<bool>()
        + size_of::<Result<usize, ArgumentResourceV1>>()
        + size_of::<Result<(), ArgumentResourceV1>>()
}

#[test]
fn source_cleanup_new_settlement_preflight_preserves_refusal_through_uncalled_capture_drop() {
    struct Capture<'a>(&'a std::cell::Cell<bool>);
    impl Capture<'_> {
        fn invoked(&self) {
            panic!("preflight refusal invoked its callback");
        }
    }
    impl Drop for Capture<'_> {
        fn drop(&mut self) {
            self.0.set(true);
            panic!("uncalled cleanup callback destructor");
        }
    }
    let dropped = std::cell::Cell::new(false);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(32 + 2 + 1 + 4 - 1);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let boundary =
        ScopedSourceCleanupBoundaryV29::new::<(), ArgumentResourceV1>(MODULE_FLOOR, &mut budget)
            .unwrap();
    let capture = Capture(&dropped);
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        boundary.run(&mut budget, move |_, _| {
            capture.invoked();
            Ok::<_, ArgumentResourceV1>(())
        })
    }));
    assert!(dropped.get());
    assert!(matches!(caught, Ok(Err(ArgumentResourceV1::Work(_)))));
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn source_cleanup_header_cannot_cover_one_byte_missing_incoming_reservation() {
    for replay in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        let source = owning_source_fixture(ModuleFixture::Ordinary, true, &mut budget).unwrap();
        let mut donor = Some(source);
        let owner = if replay {
            Some(
                SourceOwnedScopedModuleV29::try_new(
                    &mut donor,
                    ProductionSemanticKirLimitsV1::default(),
                    &mut budget,
                )
                .unwrap(),
            )
        } else {
            None
        };
        let required = if let Some(owner) = &owner {
            owner.retained_storage + owner.capture.preexisting_storage()
        } else {
            let source = donor.as_ref().unwrap();
            source.input.retained_storage
                + source
                    .owner
                    .occurrence_storage()
                    .unwrap()
                    .retained_storage()
        };
        budget
            .release_storage(budget.storage() - (required - 1))
            .unwrap();
        let storage = budget.storage();
        let work = budget.work();
        let result = if let Some(owner) = &owner {
            owner.replay(&mut budget)
        } else {
            SourceOwnedScopedModuleV29::try_new(
                &mut donor,
                ProductionSemanticKirLimitsV1::default(),
                &mut budget,
            )
            .map(|value| drop(value))
        };
        assert!(matches!(
            result,
            Err(ScopedModuleErrorV29::Source(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting
                )
            ))
        ));
        assert_eq!(budget.storage(), storage);
        assert_eq!(budget.work(), work);
        assert!(budget.failed_storage().is_none());
        assert_eq!(donor.is_some(), !replay);
    }
}

#[test]
fn source_cleanup_header_refusal_keeps_the_original_constructor_adoption_contract() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    let source = owning_source_fixture(ModuleFixture::Ordinary, true, &mut budget).unwrap();
    let inherited = source.input.retained_storage;
    let header = size_of::<ScopedSourceCleanupBoundaryV29>()
        + size_of::<std::thread::Result<Result<SourceOwnedScopedModuleV29, ScopedModuleErrorV29>>>(
        );
    budget
        .reserve_storage(MODULE_LIMIT - budget.storage() - (header - 1))
        .unwrap();
    let entry = budget.storage();
    let before_work = budget.work();
    let mut donor = Some(source);
    let result = SourceOwnedScopedModuleV29::try_new(
        &mut donor,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    );
    assert!(matches!(
        result,
        Err(ScopedModuleErrorV29::Source(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Storage(_)
            )
        ))
    ));
    assert!(donor.is_none());
    assert_eq!(budget.storage(), entry - inherited);
    assert_eq!(budget.work(), before_work);
    assert!(budget.failed_storage().is_some());
}
