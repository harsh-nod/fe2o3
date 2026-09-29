fn ingress_cleanup_v29() -> ScopedSourceCleanupV29 {
    ScopedSourceCleanupV29 {
        denied: std::cell::Cell::new(false),
        fault: std::cell::RefCell::new(None),
        fault_storage: std::cell::Cell::new(None),
        fault_skip: std::cell::Cell::new(0),
    }
}

struct IngressCaptureV29<'a> {
    dropped: &'a std::cell::Cell<usize>,
    cleanup: Option<&'a ScopedSourceCleanupV29>,
}

impl IngressCaptureV29<'_> {
    fn invoked(&self) {
        panic!("refused preparation invoked its consumer");
    }
}

impl Drop for IngressCaptureV29<'_> {
    fn drop(&mut self) {
        self.dropped.set(self.dropped.get() + 1);
        if let Some(cleanup) = self.cleanup {
            cleanup.deny_refund();
        }
        panic!("uncalled consumer destructor");
    }
}

#[test]
fn source_cleanup_initial_boundary_refusals_keep_typed_error_after_hostile_capture_drop() {
    let boundary_header = size_of::<ScopedSourceCleanupBoundaryV29>()
        + size_of::<std::thread::Result<Result<(), ArgumentResourceV1>>>();
    for invalid_floor in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_FLOOR + boundary_header - 1);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let dropped = std::cell::Cell::new(0);
        let capture = IngressCaptureV29 {
            dropped: &dropped,
            cleanup: None,
        };
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_scoped_source_cleanup_v29(
                &mut budget,
                MODULE_FLOOR + usize::from(invalid_floor),
                move |_, _| {
                    capture.invoked();
                    Ok::<(), ArgumentResourceV1>(())
                },
            )
        }));
        assert_eq!(dropped.get(), 1);
        let error = caught
            .expect("capture panic replaced the first refusal")
            .unwrap_err();
        if invalid_floor {
            assert_eq!(error, ArgumentResourceV1::Accounting);
            assert_eq!(budget.failed_storage(), None);
        } else {
            assert!(matches!(error, ArgumentResourceV1::Storage(error)
                if error.actual() == MODULE_FLOOR + boundary_header
                    && error.limit() == MODULE_FLOOR + boundary_header - 1));
            assert_eq!(
                budget.failed_storage(),
                Some(MODULE_FLOOR + boundary_header)
            );
        }
        assert_eq!((budget.work(), budget.storage()), (0, MODULE_FLOOR));
    }
}

#[test]
fn source_attempt_initial_and_preflight_refusals_keep_uncalled_captures_in_custody() {
    // Denied entry, undercut floor, one-short disposal work, and one-short header.
    for phase in 0..4 {
        let cleanup = ingress_cleanup_v29();
        if phase == 0 {
            cleanup.deny_refund();
        }
        let dropped = std::cell::Cell::new(0);
        let capture = IngressCaptureV29 {
            dropped: &dropped,
            cleanup: None,
        };
        let run = move |_: &mut ArgumentBudgetV1<'_>| {
            capture.invoked();
            Ok::<(), ArgumentResourceV1>(())
        };
        fn header<F>(_: &F) -> usize {
            scoped_source_attempt_header_oracle_v29::<(), ArgumentResourceV1, F>()
        }
        let header = header(&run);
        let work_limit = if phase == 2 {
            32 + 2 + 1 + 4 - 1
        } else {
            MODULE_LIMIT
        };
        let storage_limit = if phase == 3 {
            MODULE_FLOOR + header - 1
        } else {
            MODULE_LIMIT
        };
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            scoped_source_attempt_v29(
                &cleanup,
                &mut budget,
                MODULE_FLOOR + usize::from(phase == 1),
                run,
            )
        }));
        assert_eq!(dropped.get(), 1);
        let error = caught
            .expect("capture panic replaced the selected refusal")
            .unwrap_err();
        match phase {
            0 | 1 => {
                assert_eq!(error, ArgumentResourceV1::Accounting);
                assert!(cleanup.is_denied());
                assert_eq!(budget.work(), 0);
            }
            2 => {
                assert!(matches!(error, ArgumentResourceV1::Work(error)
                    if error.limit() == work_limit));
                assert!(budget.failed_work().is_some());
                assert!(!cleanup.is_denied());
            }
            3 => {
                assert!(matches!(error, ArgumentResourceV1::Storage(error)
                    if error.actual() == MODULE_FLOOR + header
                        && error.limit() == storage_limit));
                assert_eq!(budget.failed_storage(), Some(MODULE_FLOOR + header));
                assert_eq!(budget.work(), 32 + 2 + 1 + 4);
                assert!(!cleanup.is_denied());
            }
            _ => unreachable!(),
        }
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn source_attempt_constructor_error_and_raw_panic_drop_uncalled_capture_before_refund() {
    for raw_panic in [false, true] {
        for deny in [false, true] {
            let cleanup = ingress_cleanup_v29();
            let dropped = std::cell::Cell::new(0);
            let capture = SourceCallbackCustodyV29::new(IngressCaptureV29 {
                dropped: &dropped,
                cleanup: deny.then_some(&cleanup),
            });
            let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
            let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
            budget.reserve_storage(MODULE_FLOOR).unwrap();
            let retained = std::cell::Cell::new(0);
            let retained_ref = &retained;
            let run = move |budget: &mut ArgumentBudgetV1<'_>| {
                capture.prepare(|| -> Result<(), ArgumentResourceV1> {
                    budget.reserve_storage(71)?;
                    retained_ref.set(budget.storage());
                    if raw_panic {
                        std::panic::resume_unwind(Box::new(271_1797usize));
                    }
                    Err(ArgumentResourceV1::Accounting)
                })
            };
            let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                scoped_source_attempt_v29(&cleanup, &mut budget, MODULE_FLOOR, run)
            }));
            assert_eq!(dropped.get(), 1);
            assert!(retained.get() > MODULE_FLOOR + 71);
            assert_eq!(cleanup.is_denied(), deny);
            assert_eq!(
                budget.storage(),
                if deny { retained.get() } else { MODULE_FLOOR }
            );
            assert_eq!(budget.work(), 32 + 2 + 1 + 4);
            match caught {
                Err(payload) if raw_panic => {
                    assert_eq!(*payload.downcast::<usize>().unwrap(), 271_1797);
                }
                Ok(Err(ArgumentResourceV1::Accounting)) if !raw_panic => {}
                _ => panic!("constructor's selected result was replaced"),
            }
        }
    }
}

#[test]
fn source_attempt_error_conversion_panic_follows_protected_uncalled_capture_disposal() {
    struct ConversionPanic;
    impl From<ArgumentResourceV1> for ConversionPanic {
        fn from(_: ArgumentResourceV1) -> Self {
            std::panic::resume_unwind(Box::new(271_1798usize));
        }
    }
    for deny in [false, true] {
        let cleanup = ingress_cleanup_v29();
        let dropped = std::cell::Cell::new(0);
        let capture = IngressCaptureV29 {
            dropped: &dropped,
            cleanup: deny.then_some(&cleanup),
        };
        let mut work = CanonicalKernelIrWorkBudgetV1::new(32 + 2 + 1 + 4 - 1);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            scoped_source_attempt_v29(&cleanup, &mut budget, MODULE_FLOOR, move |_| {
                capture.invoked();
                Ok::<(), ConversionPanic>(())
            })
        }));
        assert_eq!(dropped.get(), 1);
        assert_eq!(cleanup.is_denied(), deny);
        assert_eq!(budget.storage(), MODULE_FLOOR);
        assert!(budget.failed_work().is_some());
        let Err(payload) = caught else {
            panic!("error conversion panic did not propagate");
        };
        assert_eq!(*payload.downcast::<usize>().unwrap(), 271_1798);
    }
}

fn callback_custody_finish_header_oracle_v29<T, E>() -> usize {
    type Payload = Box<dyn std::any::Any + Send>;
    2 * size_of::<std::thread::Result<Result<T, E>>>()
        + size_of::<Result<T, E>>()
        + size_of::<std::thread::Result<()>>()
        + size_of::<Payload>()
        + size_of::<&mut Option<()>>()
        + size_of::<std::panic::AssertUnwindSafe<&mut Option<()>>>()
        + size_of::<[Option<Payload>; 2]>()
        + size_of::<std::panic::AssertUnwindSafe<[Option<Payload>; 2]>>()
        + 2 * size_of::<Payload>()
        + size_of::<std::panic::AssertUnwindSafe<Payload>>()
        + 2 * size_of::<Result<(), Payload>>()
        + size_of::<std::ops::Range<usize>>()
        + size_of::<usize>()
        + size_of::<bool>()
}

#[test]
fn source_callback_visitor_disposal_has_independent_exact_and_short_bounds() {
    type Value = [u64; 53];
    type Error = [u64; 97];
    let header = callback_custody_finish_header_oracle_v29::<Value, Error>();
    // Four protected payload retries and four envelope steps. This visitor
    // uses reference disposal, not the separate 32-attempt value finalizer.
    for (work_limit, storage_limit, success) in [
        (4 + 4, MODULE_FLOOR + header, true),
        (4 + 4 - 1, MODULE_FLOOR + header, false),
        (4 + 4, MODULE_FLOOR + header - 1, false),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let result = source_callback_custody_finish_preflight_v29::<Value, Error>(&mut budget)
            .and_then(|bytes| {
                assert_eq!(bytes, header);
                budget.reserve_storage(bytes)
            });
        assert_eq!(result.is_ok(), success);
        if success {
            assert_eq!(budget.storage(), MODULE_FLOOR + header);
            budget.release_storage(header).unwrap();
        } else if work_limit < 4 + 4 {
            assert!(matches!(result, Err(ArgumentResourceV1::Work(error))
                if error.actual() == 4 + 4 && error.limit() == work_limit));
        } else {
            assert!(matches!(result, Err(ArgumentResourceV1::Storage(error))
                if error.actual() == MODULE_FLOOR + header && error.limit() == storage_limit));
        }
        assert_eq!(budget.work(), if work_limit == 4 + 4 { 4 + 4 } else { 0 });
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn source_callback_visitor_drop_preserves_first_refusal_and_never_returns_false_success() {
    for phase in 0..3 {
        let capture_drops = std::cell::Cell::new(0);
        let result_drops = std::cell::Cell::new(0);
        let capture = SourceCallbackCustodyV29::new(SourceQueryDropV1751 {
            drops: &capture_drops,
            deny: None,
            payload: Some(Box::new(0x1797_0001_u64)),
        });
        let selected = match phase {
            0 | 1 => {
                let value = SourceQueryDropV1751 {
                    drops: &result_drops,
                    deny: None,
                    payload: Some(Box::new(0x1797_0002_u64)),
                };
                if phase == 0 {
                    Ok(Ok(value))
                } else {
                    Ok(Err(value))
                }
            }
            _ => Err(Box::new(0x1797_0003_u64) as Box<dyn std::any::Any + Send>),
        };
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let headers = source_callback_custody_finish_preflight_v29::<
            SourceQueryDropV1751<'_>,
            SourceQueryDropV1751<'_>,
        >(&mut budget)
        .unwrap();
        budget.reserve_storage(headers).unwrap();
        let result = capture.finish(selected);
        assert_eq!(capture_drops.get(), 1);
        assert_eq!(budget.storage(), MODULE_FLOOR + headers);
        match (phase, result) {
            (0, Err(payload)) => {
                assert_eq!(result_drops.get(), 1);
                assert_eq!(*payload.downcast::<u64>().unwrap(), 0x1797_0001);
            }
            (1, Ok(Err(value))) => {
                assert_eq!(result_drops.get(), 0);
                assert_eq!(value.payload.as_deref(), Some(&0x1797_0002));
                source_reference_discard_v29(value);
                assert_eq!(result_drops.get(), 1);
            }
            (2, Err(payload)) => {
                assert_eq!(result_drops.get(), 0);
                assert_eq!(*payload.downcast::<u64>().unwrap(), 0x1797_0003);
            }
            (_, unexpected) => {
                source_reference_discard_v29(unexpected);
                panic!("capture destruction replaced the first refusal or returned success");
            }
        }
        budget.release_storage(headers).unwrap();
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}
