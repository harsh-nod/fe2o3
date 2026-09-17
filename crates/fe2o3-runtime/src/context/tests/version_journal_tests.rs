use super::*;

fn configured(capacity: usize) -> RuntimeContextV1<MockBackend> {
    RuntimeContextV1::open_with_version_journal_v1(MockBackend::default(), capacity, 2).unwrap()
}

fn allocate(
    context: &mut RuntimeContextV1<MockBackend>,
) -> Result<RuntimeAllocationIdV1, RuntimeErrorV1<MockError>> {
    context.allocate(
        context.devices()[0].id(),
        RuntimeMemoryKindV1::DeviceLocal,
        8,
        8,
    )
}

#[test]
fn journal_constructor_is_fresh_bounded_and_returns_backend_on_failure() {
    for (a, w) in [(0, 1), (1, 0), (usize::MAX, 1), (1, usize::MAX)] {
        let backend = MockBackend {
            next: 73,
            ..MockBackend::default()
        };
        let Err(failure) = RuntimeContextV1::open_with_version_journal_v1(backend, a, w) else {
            panic!("invalid capacity admitted");
        };
        let (backend, error) = failure.into_parts();
        assert!(matches!(
            error,
            RuntimeErrorV1::Validation(RuntimeValidationErrorV1::Capacity)
        ));
        assert_eq!(backend.next, 73);
        assert_eq!(backend.enumeration_calls, 0);
    }
    let backend = MockBackend {
        device_name_len: MAX_RUNTIME_DEVICE_NAME_BYTES_V1 + 1,
        next: 81,
        ..MockBackend::default()
    };
    let Err(failure) = RuntimeContextV1::open_with_version_journal_v1(backend, 1, 1) else {
        panic!("invalid description admitted");
    };
    let (backend, error) = failure.into_parts();
    assert_eq!(backend.next, 81);
    assert_eq!(backend.enumeration_calls, 1);
    assert!(matches!(
        error,
        RuntimeErrorV1::Validation(RuntimeValidationErrorV1::InvalidBackendDescription)
    ));
    let context = configured(3);
    assert_eq!(context.next_identity, 1);
    assert_eq!(
        context.version_journal_usage_v1(),
        Some(RuntimeContextJournalUsageV1 {
            allocation_capacity: 3,
            writer_capacity: 2,
            allocation_records: 0,
            provisional_records: 0,
        })
    );
    assert!(context.shutdown().is_ok());
    assert!(
        RuntimeContextV1::open(MockBackend::default())
            .unwrap()
            .version_journal_usage_v1()
            .is_none()
    );
}

#[test]
fn journal_capacity_precedes_ids_and_backend_and_disposal_permits_only_fresh_ids() {
    let mut context = configured(1);
    let first = allocate(&mut context).unwrap();
    let first_reference = context.allocations[&first].journal.unwrap();
    let next = context.next_identity;
    assert!(matches!(
        allocate(&mut context),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::Capacity
        ))
    ));
    assert_eq!(context.next_identity, next);
    assert_eq!(context.backend.allocation_calls, 1);
    assert_eq!(
        context
            .version_journal_usage_v1()
            .unwrap()
            .provisional_records,
        0
    );
    context.release_allocation(first).unwrap();
    let second = allocate(&mut context).unwrap();
    let second_reference = context.allocations[&second].journal.unwrap();
    assert_eq!(first_reference.slot, second_reference.slot);
    assert_ne!(first, second);
    assert_ne!(first_reference.key, second_reference.key);
    assert!(context.release_allocation(first).is_err());
    let record = context.allocations[&second];
    assert!(
        context
            .versions
            .as_ref()
            .unwrap()
            .validate_live(first, &record)
            .is_err()
    );
    assert!(context.cleanup().is_complete());
    assert_eq!(
        context
            .version_journal_usage_v1()
            .unwrap()
            .allocation_records,
        0
    );
}

#[test]
fn journal_retains_no_handle_ambiguity_with_and_without_allocation_credits() {
    for credits in [false, true] {
        for failure in [
            MockMemoryFailure::Rejected,
            MockMemoryFailure::Quiescent,
            MockMemoryFailure::Terminal,
            MockMemoryFailure::Panic,
        ] {
            let mut context = configured(1);
            if credits {
                context
                    .configure_allocation_admission_v1(context.devices()[0].id(), 8, 1)
                    .unwrap();
            }
            context.backend.allocation_failure = failure;
            let result = catch_unwind(AssertUnwindSafe(|| allocate(&mut context)));
            if failure == MockMemoryFailure::Panic {
                let payload = result.unwrap_err();
                assert_eq!(
                    payload.downcast_ref::<&str>(),
                    Some(&"scripted allocation adapter panic")
                );
            } else {
                assert!(result.unwrap().is_err());
            }
            assert_eq!(context.next_identity, 2);
            assert!(context.allocations.is_empty());
            if failure == MockMemoryFailure::Rejected {
                assert_eq!(
                    context
                        .version_journal_usage_v1()
                        .unwrap()
                        .allocation_records,
                    0
                );
                assert!(context.cleanup().is_complete());
                assert_eq!(allocate(&mut context).unwrap().get(), 2);
                assert!(context.cleanup().is_complete());
            } else {
                let usage = context.version_journal_usage_v1().unwrap();
                assert_eq!(usage.allocation_records, 1);
                assert_eq!(usage.provisional_records, 1);
                let report = context.cleanup();
                assert!(report.retained().is_empty());
                assert_eq!(report.allocation_journal_records_v1(), 1);
                assert_eq!(report.allocation_credit_records_v1(), usize::from(credits));
                assert!(!report.is_complete());
                assert_eq!(
                    context.is_terminal(),
                    failure != MockMemoryFailure::Quiescent
                );
                assert!(allocate(&mut context).is_err());
                assert_eq!(context.backend.allocation_calls, 1);
                assert_eq!(context.next_identity, 2);
                assert!(context.shutdown().is_err());
            }
        }
    }
}

#[test]
fn journal_failed_disposal_retains_exact_records_and_successful_retry_retires() {
    for cleanup in [false, true] {
        for failure in [
            MockMemoryFailure::Rejected,
            MockMemoryFailure::Quiescent,
            MockMemoryFailure::Terminal,
            MockMemoryFailure::Panic,
        ] {
            let mut context = configured(1);
            let id = allocate(&mut context).unwrap();
            let reference = context.allocations[&id].journal;
            context.backend.release_allocation_failure = failure;
            let result = catch_unwind(AssertUnwindSafe(|| {
                if cleanup {
                    assert!(!context.cleanup().is_complete());
                } else {
                    assert!(context.release_allocation(id).is_err());
                }
            }));
            assert_eq!(result.is_err(), failure == MockMemoryFailure::Panic);
            assert_eq!(context.allocations[&id].journal, reference);
            assert_eq!(
                context
                    .version_journal_usage_v1()
                    .unwrap()
                    .allocation_records,
                1
            );
            assert_eq!(
                context
                    .version_journal_usage_v1()
                    .unwrap()
                    .provisional_records,
                0
            );
            if matches!(
                failure,
                MockMemoryFailure::Rejected | MockMemoryFailure::Quiescent
            ) {
                assert!(context.cleanup().is_complete());
                assert_eq!(
                    context
                        .version_journal_usage_v1()
                        .unwrap()
                        .allocation_records,
                    0
                );
            } else {
                assert!(context.is_terminal());
                assert!(!context.cleanup().is_complete());
            }
        }
    }
}

#[test]
fn journal_cleanup_retires_live_neighbor_but_preserves_unidentified_attempt() {
    for credits in [false, true] {
        let mut context = configured(2);
        let device = context.devices()[0].id();
        if credits {
            context
                .configure_allocation_admission_v1(device, 16, 2)
                .unwrap();
        }
        context.backend.allocation_failure = MockMemoryFailure::Quiescent;
        assert!(allocate(&mut context).is_err());
        let live = allocate(&mut context).unwrap();
        assert_eq!(live.get(), 2);
        assert_eq!(context.next_identity, 3);
        assert_eq!(
            context
                .version_journal_usage_v1()
                .unwrap()
                .allocation_records,
            2
        );
        assert_eq!(
            context
                .version_journal_usage_v1()
                .unwrap()
                .provisional_records,
            1
        );
        let report = context.cleanup();
        assert!(report.retained().is_empty());
        assert!(!report.is_complete());
        assert!(!report.is_terminal());
        assert_eq!(report.allocation_journal_records_v1(), 1);
        assert_eq!(report.allocation_credit_records_v1(), usize::from(credits));
        assert_eq!(
            context
                .version_journal_usage_v1()
                .unwrap()
                .provisional_records,
            1
        );
        assert_eq!(context.backend.memory.len(), 1);
        assert_eq!(context.backend.cleanup_log.len(), 1);
        assert!(context.shutdown().is_err());
    }
}

#[test]
fn journal_invalid_handle_retains_logical_and_backend_ownership() {
    for duplicate in [false, true] {
        let mut context = configured(2);
        let handle = if duplicate {
            let id = allocate(&mut context).unwrap();
            context.allocations[&id].backend_allocation
        } else {
            0
        };
        context.backend.handle_override = Some((MockHandleKind::Allocation, handle));
        assert!(allocate(&mut context).is_err());
        assert!(context.is_terminal());
        assert_eq!(context.allocations.len(), 1 + usize::from(duplicate));
        assert_eq!(
            context
                .version_journal_usage_v1()
                .unwrap()
                .allocation_records,
            context.allocations.len()
        );
        assert!(!context.cleanup().is_complete());
    }
}

#[test]
fn journal_preflight_failure_and_credit_rejection_leave_no_provisional_owner() {
    let mut context = configured(2);
    let device = context.devices()[0].id();
    context
        .configure_allocation_admission_v1(device, 1, 1)
        .unwrap();
    assert!(
        context
            .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 8, 3)
            .is_err()
    );
    assert_eq!(context.next_identity, 1);
    assert!(allocate(&mut context).is_err());
    assert_eq!(context.next_identity, 2);
    assert_eq!(context.backend.allocation_calls, 0);
    assert_eq!(
        context
            .version_journal_usage_v1()
            .unwrap()
            .allocation_records,
        0
    );
    assert!(context.cleanup().is_complete());
}

#[test]
fn journal_foreign_reference_is_rejected_before_backend_disposal() {
    for cleanup in [false, true] {
        let mut context = configured(1);
        let mut foreign = configured(1);
        let id = allocate(&mut context).unwrap();
        let foreign_id = allocate(&mut foreign).unwrap();
        context.allocations.get_mut(&id).unwrap().journal =
            foreign.allocations[&foreign_id].journal;
        if cleanup {
            assert!(!context.cleanup().is_complete());
        } else {
            assert!(context.release_allocation(id).is_err());
        }
        assert!(context.is_terminal());
        assert!(context.backend.cleanup_log.is_empty());
        assert_eq!(
            context
                .version_journal_usage_v1()
                .unwrap()
                .allocation_records,
            1
        );
        assert!(foreign.cleanup().is_complete());
    }
}

#[test]
fn journal_metadata_profile_does_not_restrict_writes_or_claim_content_versions() {
    let mut context = configured(1);
    let id = allocate(&mut context).unwrap();
    let before = context.version_journal_usage_v1();
    context.write_allocation(id, 1, &[9, 8]).unwrap();
    let mut bytes = [0; 8];
    context.read_allocation(id, 0, &mut bytes).unwrap();
    assert_eq!(bytes, [0, 9, 8, 0, 0, 0, 0, 0]);
    assert_eq!(context.version_journal_usage_v1(), before);
    assert!(context.cleanup().is_complete());
}
