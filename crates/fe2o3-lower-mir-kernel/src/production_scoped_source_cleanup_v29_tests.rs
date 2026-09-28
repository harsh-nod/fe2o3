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
