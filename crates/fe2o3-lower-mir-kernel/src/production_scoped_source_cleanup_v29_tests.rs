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

#[test]
fn source_cleanup_transport_donor_recovery_does_not_reset_denial() {
    for skip in [0, 1, 3] {
        for undercut in [false, true] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(CLEANUP_TILE_LIMIT_V29);
            let mut budget = ArgumentBudgetV1::new(&mut work, CLEANUP_TILE_LIMIT_V29);
            budget.reserve_storage(MODULE_FLOOR).unwrap();
            let pending =
                super::super::tile_schedule_tests::cleanup_pending_source_v29(&mut budget);
            let identity = *pending.pending_identity();
            let entry = budget.storage();
            let rollback = entry - pending.adopted_storage();
            let mut donor = Some(pending);
            let observed = CleanupObservationV29::default();
            let result = cleanup_probe_v29(
                &mut budget,
                rollback,
                skip,
                ScopedSourceCleanupFaultV29::Error { undercut },
                &observed,
                |cleanup, budget| {
                    ProductionTileScalarTransportOwnerV29::try_from_pending_with_cleanup_v29(
                        &mut donor,
                        ProductionTileScalarOrderV29::Blocked,
                        cleanup,
                        budget,
                    )
                },
            )
            .unwrap();
            let phase = match skip {
                0 => ProductionTileScalarPhaseV29::Preparation,
                1 => ProductionTileScalarPhaseV29::Materialization,
                3 => ProductionTileScalarPhaseV29::Replay,
                _ => unreachable!(),
            };
            assert!(
                matches!(result, Err(ProductionTileScalarTransportErrorV29::Candidate {
                phase: actual, reason: "source",
            }) if actual == phase)
            );
            assert_eq!(donor.as_ref().unwrap().pending_identity(), &identity);
            assert_eq!(observed.denied.get(), undercut);
            assert_eq!(
                budget.storage(),
                if undercut {
                    observed.storage.get().unwrap()
                } else {
                    entry
                }
            );
            if !undercut {
                donor
                    .as_ref()
                    .unwrap()
                    .replay_with_budget(&mut budget)
                    .unwrap();
                assert_eq!(budget.storage(), entry);
            }
        }
    }
}

#[test]
fn source_cleanup_transport_keeps_each_existing_panic_boundary() {
    for skip in [1, 3] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(CLEANUP_TILE_LIMIT_V29);
        let mut budget = ArgumentBudgetV1::new(&mut work, CLEANUP_TILE_LIMIT_V29);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let pending = super::super::tile_schedule_tests::cleanup_pending_source_v29(&mut budget);
        let rollback = budget.storage() - pending.adopted_storage();
        let mut donor = Some(pending);
        let observed = CleanupObservationV29::default();
        let (payload, address, drops) = cleanup_panic_v29(200 + skip);
        let result = cleanup_probe_v29(
            &mut budget,
            rollback,
            skip,
            ScopedSourceCleanupFaultV29::Panic {
                undercut: true,
                payload,
            },
            &observed,
            |cleanup, budget| {
                ProductionTileScalarTransportOwnerV29::try_from_pending_with_cleanup_v29(
                    &mut donor,
                    ProductionTileScalarOrderV29::Blocked,
                    cleanup,
                    budget,
                )
            },
        );
        assert!(observed.denied.get());
        assert_eq!(budget.storage(), observed.storage.get().unwrap());
        if skip == 1 {
            assert!(donor.is_none());
            match result {
                Err(payload) => require_cleanup_panic_v29(payload, address, 200 + skip, &drops),
                Ok(_) => panic!("materialization producer panic was replaced"),
            }
        } else {
            assert!(donor.is_some());
            assert!(matches!(
                result,
                Ok(Err(ProductionTileScalarTransportErrorV29::Panicked))
            ));
            assert_eq!(drops.load(std::sync::atomic::Ordering::SeqCst), 1);
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
fn source_cleanup_checked_transport_cannot_refund_a_foreign_source_callback() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(CLEANUP_TILE_LIMIT_V29);
    let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(CLEANUP_TILE_LIMIT_V29);
    let mut budget = ArgumentBudgetV1::new(&mut work, CLEANUP_TILE_LIMIT_V29);
    let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, CLEANUP_TILE_LIMIT_V29);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    foreign.reserve_storage(97).unwrap();
    let pending = super::super::tile_schedule_tests::cleanup_pending_source_v29(&mut budget);
    let owner = ProductionTileScalarTransportOwnerV29::try_from_pending_with_budget_v29(
        &mut Some(pending),
        ProductionTileScalarOrderV29::Blocked,
        &mut budget,
    )
    .unwrap();
    let source = owning_source_fixture(ModuleFixture::Ordinary, true, &mut budget).unwrap();
    let floor = budget.storage();
    owner
        .with_checked_transport_v29(&mut budget, |_, _| Ok(()))
        .unwrap();
    assert_eq!(budget.storage(), floor);
    let seen = std::cell::Cell::new(0);
    let result = with_scoped_source_cleanup_v29(&mut budget, floor, |cleanup, budget| {
        owner.with_checked_transport_with_cleanup_v29(cleanup, budget, |_, budget| {
            let result = cleanup_real_source_callback_v29(&source, cleanup, budget, |budget| {
                seen.set(budget.storage());
                std::mem::swap(budget, &mut foreign);
                Err(unsupported(0, None, None, "selected source cleanup error"))
            });
            require_cleanup_source_error_v29(result);
            assert!(cleanup.is_denied());
            Err::<(), _>(ProductionTileScalarTransportErrorV29::Binding(
                "selected source error",
            ))
        })
    });
    assert_eq!(
        result,
        Err(ProductionTileScalarTransportErrorV29::Binding(
            "selected source error"
        ))
    );
    assert_eq!(budget.storage(), 97);
    assert_eq!(foreign.storage(), seen.get());
}

#[test]
fn source_cleanup_header_is_charged_once_and_obeys_independent_exact_limits() {
    let header = size_of::<ScopedSourceCleanupBoundaryV29>()
        + size_of::<std::thread::Result<Result<(), ScopedModuleErrorV29>>>();
    for (allowance, work_allowance) in [(header, 5), (header - 1, 5), (header, 4)] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_allowance);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_FLOOR + allowance);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let entered = std::cell::Cell::new(false);
        let result = with_scoped_source_cleanup_v29::<(), ScopedModuleErrorV29>(
            &mut budget,
            MODULE_FLOOR,
            |cleanup, budget| {
                entered.set(true);
                assert_eq!(budget.storage(), MODULE_FLOOR + header);
                let floor = budget.storage();
                scoped_source_attempt_v29(cleanup, budget, floor, |budget| {
                    scoped_source_attempt_v29(cleanup, budget, floor, |budget| {
                        assert_eq!(budget.storage(), MODULE_FLOOR + header);
                        budget.charge_work(5)?;
                        Ok(())
                    })
                })
            },
        );
        assert_eq!(budget.storage(), MODULE_FLOOR);
        if allowance < header {
            assert!(!entered.get());
            assert!(matches!(
                result,
                Err(ScopedModuleErrorV29::Source(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Storage(_)
                    )
                ))
            ));
        } else if work_allowance < 5 {
            assert!(entered.get());
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
            assert_eq!(budget.peak_storage(), MODULE_FLOOR + header);
            assert_eq!(budget.work(), 5);
        }
    }
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

#[test]
fn source_cleanup_transport_header_does_not_mask_missing_retained_owner_storage() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(CLEANUP_TILE_LIMIT_V29);
    let mut budget = ArgumentBudgetV1::new(&mut work, CLEANUP_TILE_LIMIT_V29);
    let pending = super::super::tile_schedule_tests::cleanup_pending_source_v29(&mut budget);
    let owner = ProductionTileScalarTransportOwnerV29::try_from_pending_with_budget_v29(
        &mut Some(pending),
        ProductionTileScalarOrderV29::Blocked,
        &mut budget,
    )
    .unwrap();
    budget
        .release_storage(budget.storage() - (owner.adopted_storage() - 1))
        .unwrap();
    let entry = budget.storage();
    let work = budget.work();
    let result: Result<(), ProductionTileScalarTransportErrorV29> = owner
        .with_checked_transport_v29(&mut budget, |_, _| {
            panic!("missing incoming owner credit reached the checked callback")
        });
    assert!(matches!(
        result,
        Err(ProductionTileScalarTransportErrorV29::Resource(
            ArgumentResourceV1::Accounting
        ))
    ));
    assert_eq!(budget.storage(), entry);
    assert_eq!(budget.work(), work);
    assert!(budget.failed_storage().is_none());
}
