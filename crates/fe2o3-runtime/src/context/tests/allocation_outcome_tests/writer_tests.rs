use super::*;
use fe2o3_runtime_model::{ContextAllocationStateV1, ContextWriterKindV1, ContextWriterStateV1};

type Context = RuntimeContextV1<AllocationOnlyBackend>;
type Failure = MockMemoryFailure;

fn fixture(
    credits: bool,
    writers: usize,
) -> (Context, RuntimeAllocationIdV1, RuntimeAllocationIdV1) {
    let mut context =
        Context::open_with_version_journal_v1(AllocationOnlyBackend::default(), 2, writers)
            .unwrap();
    let device = context.devices()[0].id();
    if credits {
        context
            .configure_allocation_admission_v1(device, 16, 2)
            .unwrap();
    }
    let a = context
        .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 8, 8)
        .unwrap();
    let b = context
        .allocate(device, RuntimeMemoryKindV1::HostVisible, 8, 8)
        .unwrap();
    (context, a, b)
}

fn state(context: &Context, id: RuntimeAllocationIdV1) -> ContextAllocationStateV1 {
    context
        .versions
        .as_ref()
        .unwrap()
        .journal_for_test()
        .lookup_allocation(context.allocations[&id].journal.unwrap())
        .unwrap()
}

fn diagnostic(
    context: &mut Context,
    failure: Failure,
    release: bool,
) -> (*const Diagnostic, Arc<AtomicUsize>) {
    let drops = Arc::new(AtomicUsize::new(0));
    let error = Box::new(Diagnostic {
        drops: drops.clone(),
    });
    let pointer = &*error as *const Diagnostic;
    if release {
        context.backend.release_failure = failure;
        context.backend.release_diagnostic = Some(error);
    } else {
        context.backend.write_failure = failure;
        context.backend.write_diagnostic = Some(error);
        context.backend.write_prefix = 2;
    }
    (pointer, drops)
}

fn assert_diagnostic(
    result: std::thread::Result<Result<(), RuntimeErrorV1<Box<Diagnostic>>>>,
    failure: Failure,
    pointer: *const Diagnostic,
    drops: &AtomicUsize,
) {
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    let error = match (failure, result) {
        (Failure::Rejected, Ok(Err(RuntimeErrorV1::BackendRejected(error))))
        | (Failure::Quiescent, Ok(Err(RuntimeErrorV1::BackendQuiescent(error))))
        | (Failure::Terminal, Ok(Err(RuntimeErrorV1::BackendTerminal(error)))) => error,
        (Failure::Panic, Err(payload)) => payload.downcast::<Diagnostic>().unwrap(),
        _ => panic!("wrong failure class"),
    };
    assert_eq!(&*error as *const Diagnostic, pointer);
    drop(error);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

fn make_unknown(context: &mut Context, allocation: RuntimeAllocationIdV1) {
    let (pointer, drops) = diagnostic(context, Failure::Quiescent, false);
    let result = catch_unwind(AssertUnwindSafe(|| {
        context.write_allocation(allocation, 2, &[0x5a; 4])
    }));
    assert_diagnostic(result, Failure::Quiescent, pointer, &drops);
}

#[test]
fn synchronous_writes_burn_genuine_ids_and_epochs_but_rejections_preserve_lineage() {
    let (mut context, a, _) = fixture(true, 1);
    for (epoch, failure, lineage) in [
        (1, Failure::None, 1),
        (2, Failure::Rejected, 1),
        (3, Failure::None, 3),
    ] {
        let next = context.next_identity;
        let memory = context.backend.inner.memory.clone();
        let credits = context
            .allocation_admission_usage_v1(context.devices()[0].id())
            .unwrap();
        if failure == Failure::None {
            context.write_allocation(a, 0, &[epoch as u8; 8]).unwrap();
        } else {
            let (pointer, drops) = diagnostic(&mut context, failure, false);
            let result = catch_unwind(AssertUnwindSafe(|| {
                context.write_allocation(a, 0, &[epoch as u8; 8])
            }));
            assert_diagnostic(result, failure, pointer, &drops);
            assert_eq!(context.backend.inner.memory, memory);
        }
        let observed = state(&context, a);
        assert_eq!(
            (
                observed.attempt_epoch,
                observed.content_lineage,
                observed.pending_writer
            ),
            (epoch, lineage, None)
        );
        assert_eq!(context.next_identity, next + 1);
        assert_eq!(
            context
                .versions
                .as_ref()
                .unwrap()
                .journal_for_test()
                .registration_watermark(),
            next
        );
        assert_eq!(context.version_journal_writer_records_v1(), Some(0));
        assert_eq!(
            context
                .allocation_admission_usage_v1(context.devices()[0].id())
                .unwrap(),
            credits
        );
    }
    assert_eq!(context.backend.write_calls, 3);
    assert!(context.cleanup().is_complete());
}

#[test]
fn invalid_write_range_foreign_id_and_exhausted_id_reject_before_writer_or_backend() {
    let (mut context, a, _) = fixture(false, 1);
    for (allocation, offset, bytes, expected) in [
        (a, 0, &[][..], RuntimeValidationErrorV1::InvalidRange),
        (a, 7, &[1, 2][..], RuntimeValidationErrorV1::InvalidRange),
        (
            a,
            u64::MAX,
            &[1][..],
            RuntimeValidationErrorV1::InvalidRange,
        ),
        (
            RuntimeAllocationIdV1 {
                context_generation: a.context_generation + 1,
                ..a
            },
            0,
            &[1][..],
            RuntimeValidationErrorV1::UnknownAllocation,
        ),
    ] {
        let before = (
            context.next_identity,
            state(&context, a),
            context.backend.inner.memory.clone(),
        );
        assert!(
            matches!(context.write_allocation(allocation, offset, bytes), Err(RuntimeErrorV1::Validation(error)) if error == expected)
        );
        assert_eq!(
            (
                context.next_identity,
                state(&context, a),
                context.backend.inner.memory.clone()
            ),
            before
        );
    }
    context.next_identity = u64::MAX;
    let before = state(&context, a);
    assert!(matches!(
        context.write_allocation(a, 0, &[1]),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::Capacity
        ))
    ));
    assert_eq!(context.next_identity, u64::MAX);
    assert_eq!(state(&context, a), before);
    assert_eq!(context.version_journal_writer_records_v1(), Some(0));
    assert_eq!(context.backend.write_calls, 0);
    assert!(!context.is_terminal());
}

#[test]
fn ambiguous_partial_writes_preserve_payload_neighbors_and_unknown_ownership() {
    for credits in [false, true] {
        for failure in [Failure::Quiescent, Failure::Terminal, Failure::Panic] {
            let (mut context, a, b) = fixture(credits, 2);
            context.write_allocation(a, 0, &[0x31; 8]).unwrap();
            context.write_allocation(b, 0, &[0x72; 8]).unwrap();
            let a_record = context.allocations[&a];
            let b_state = state(&context, b);
            let next = context.next_identity;
            let (pointer, drops) = diagnostic(&mut context, failure, false);
            let result = catch_unwind(AssertUnwindSafe(|| {
                context.write_allocation(a, 2, &[0x5a; 4])
            }));
            assert_diagnostic(result, failure, pointer, &drops);
            let terminal = failure != Failure::Quiescent;
            assert_eq!(context.is_terminal(), terminal);
            assert_eq!(context.allocations[&a], a_record);
            assert_eq!(
                context.backend.inner.memory[&a_record.backend_allocation],
                [0x31, 0x31, 0x5a, 0x5a, 0x31, 0x31, 0x31, 0x31]
            );
            assert_eq!(
                context.backend.inner.memory[&context.allocations[&b].backend_allocation],
                [0x72; 8]
            );
            assert_eq!(state(&context, b), b_state);
            let observed = state(&context, a);
            assert_eq!((observed.attempt_epoch, observed.content_lineage), (2, 1));
            let writer = observed.pending_writer.unwrap();
            assert_eq!(
                (
                    writer.key.context_generation,
                    writer.key.local,
                    writer.key.kind
                ),
                (
                    context.context_generation,
                    next,
                    ContextWriterKindV1::Synchronous
                )
            );
            assert_eq!(
                context
                    .versions
                    .as_ref()
                    .unwrap()
                    .journal_for_test()
                    .lookup_writer(writer),
                Ok(ContextWriterStateV1::Unknown { member_count: 1 })
            );
            assert_eq!(context.version_journal_writer_records_v1(), Some(1));
            let expected = if terminal {
                RuntimeValidationErrorV1::ContextTerminal
            } else {
                RuntimeValidationErrorV1::ContextReserved
            };
            assert!(
                matches!(context.write_allocation(a, 0, &[9]), Err(RuntimeErrorV1::Validation(error)) if error == expected)
            );
            assert_eq!(context.backend.write_calls, 3);
            assert_eq!(context.next_identity, next + 1);
            if credits {
                let usage = context
                    .allocation_admission_usage_v1(context.devices()[0].id())
                    .unwrap()
                    .unwrap();
                assert_eq!(usage.retained_records, if terminal { 1 } else { 2 });
                assert_eq!(usage.quarantined_records, usize::from(terminal));
                assert_eq!(
                    usage.used,
                    RuntimeResourceVectorV1::ZERO
                        .with(K::RequestedAllocationBytes, 16)
                        .with(K::AllocationRecords, 2)
                );
            }
            if terminal {
                let report = context.cleanup();
                assert!(!report.is_complete());
                assert_eq!(report.writer_journal_records_v1(), 1);
                assert_eq!(context.backend.release_calls, 0);
            }
        }
    }
}

#[test]
fn unknown_writer_capacity_returns_only_after_confirmed_disposal() {
    let (mut context, a, b) = fixture(true, 1);
    make_unknown(&mut context, a);
    let before = (
        context.next_identity,
        state(&context, b),
        context.backend.write_calls,
    );
    assert!(matches!(
        context.write_allocation(b, 0, &[1]),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::Capacity
        ))
    ));
    assert_eq!(
        (
            context.next_identity,
            state(&context, b),
            context.backend.write_calls
        ),
        before
    );
    context.release_allocation(a).unwrap();
    assert_eq!(context.version_journal_writer_records_v1(), Some(0));
    assert_eq!(
        context
            .version_journal_usage_v1()
            .unwrap()
            .allocation_records,
        1
    );
    context.write_allocation(b, 0, &[3]).unwrap();
    assert_eq!(state(&context, b).content_lineage, 1);
    assert!(context.cleanup().is_complete());
}

#[test]
fn unknown_disposal_retries_retain_exact_handles_and_credits_until_success() {
    for credits in [false, true] {
        for cleanup in [false, true] {
            for failure in [Failure::Rejected, Failure::Quiescent] {
                let (mut context, a, b) = fixture(credits, 1);
                context.release_allocation(b).unwrap();
                make_unknown(&mut context, a);
                let record = context.allocations[&a];
                let before = state(&context, a);
                let usage = context
                    .allocation_admission_usage_v1(context.devices()[0].id())
                    .unwrap();
                let (pointer, drops) = diagnostic(&mut context, failure, true);
                if cleanup {
                    let mut report = context.cleanup();
                    assert!(!report.is_complete());
                    assert_eq!(report.writer_journal_records_v1(), 1);
                    assert_eq!(report.allocation_journal_records_v1(), 1);
                    assert_eq!(report.failures.len(), 1);
                    let error = match report.failures.pop().unwrap().failure {
                        RuntimeBackendFailureV1::Rejected(error)
                            if failure == Failure::Rejected =>
                        {
                            error
                        }
                        RuntimeBackendFailureV1::Quiescent(error)
                            if failure == Failure::Quiescent =>
                        {
                            error
                        }
                        _ => panic!("wrong disposal failure"),
                    };
                    assert_eq!(&*error as *const Diagnostic, pointer);
                    drop(error);
                    assert_eq!(drops.load(Ordering::SeqCst), 1);
                } else {
                    let result = catch_unwind(AssertUnwindSafe(|| context.release_allocation(a)));
                    assert_diagnostic(result, failure, pointer, &drops);
                }
                assert!(!context.is_terminal());
                assert_eq!(context.allocations[&a], record);
                assert_eq!(state(&context, a), before);
                assert!(
                    context
                        .backend
                        .inner
                        .memory
                        .contains_key(&record.backend_allocation)
                );
                assert_eq!(
                    context
                        .allocation_admission_usage_v1(context.devices()[0].id())
                        .unwrap(),
                    usage
                );
                if cleanup {
                    assert!(context.cleanup().is_complete());
                } else {
                    context.release_allocation(a).unwrap();
                }
                assert!(
                    !context
                        .backend
                        .inner
                        .memory
                        .contains_key(&record.backend_allocation)
                );
                assert_eq!(context.version_journal_writer_records_v1(), Some(0));
                assert_eq!(
                    context
                        .version_journal_usage_v1()
                        .unwrap()
                        .allocation_records,
                    0
                );
                assert_eq!(context.backend.release_calls, 3);
                assert!(context.cleanup().is_complete());
                assert_eq!(context.backend.release_calls, 3);
                if credits {
                    assert_eq!(
                        context
                            .allocation_admission_usage_v1(context.devices()[0].id())
                            .unwrap()
                            .unwrap()
                            .used,
                        RuntimeResourceVectorV1::ZERO
                    );
                }
            }
        }
    }
}

#[test]
fn terminal_or_panicking_unknown_disposal_retains_custody_and_blocks_retries() {
    for credits in [false, true] {
        for failure in [Failure::Terminal, Failure::Panic] {
            let (mut context, a, _) = fixture(credits, 1);
            make_unknown(&mut context, a);
            let before = state(&context, a);
            let memory = context.backend.inner.memory.clone();
            let (pointer, drops) = diagnostic(&mut context, failure, true);
            let result = catch_unwind(AssertUnwindSafe(|| context.release_allocation(a)));
            assert_diagnostic(result, failure, pointer, &drops);
            assert!(context.is_terminal());
            assert_eq!(state(&context, a), before);
            assert_eq!(context.backend.inner.memory, memory);
            assert!(matches!(
                context.release_allocation(a),
                Err(RuntimeErrorV1::Validation(
                    RuntimeValidationErrorV1::ContextTerminal
                ))
            ));
            assert_eq!(context.cleanup().writer_journal_records_v1(), 1);
            assert_eq!(context.backend.release_calls, 1);
            if credits {
                let usage = context
                    .allocation_admission_usage_v1(context.devices()[0].id())
                    .unwrap()
                    .unwrap();
                assert_eq!((usage.retained_records, usage.quarantined_records), (1, 1));
            }
        }
    }
}

#[test]
fn pending_host_write_blocks_its_disposal_but_cleanup_continues_unrelated_allocations() {
    let (mut context, a, b) = fixture(true, 1);
    let record = context.allocations[&a];
    let _ticket = context
        .begin_journal_host_write_v1(a, &record)
        .unwrap()
        .unwrap();
    assert!(matches!(
        context.release_allocation(a),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::ContextReserved
        ))
    ));
    assert_eq!(context.backend.release_calls, 0);
    let report = context.cleanup();
    assert!(!report.is_complete());
    assert!(!context.is_terminal());
    assert_eq!(report.writer_journal_records_v1(), 1);
    assert_eq!(report.allocation_journal_records_v1(), 1);
    assert!(context.allocations.contains_key(&a));
    assert!(!context.allocations.contains_key(&b));
    assert_eq!(context.backend.release_calls, 1);
}

#[test]
fn disabled_journal_preserves_write_ids_and_legacy_panic_behavior() {
    for failure in [
        Failure::None,
        Failure::Rejected,
        Failure::Quiescent,
        Failure::Terminal,
        Failure::Panic,
    ] {
        let mut context = Context::open(AllocationOnlyBackend::default()).unwrap();
        let device = context.devices()[0].id();
        let a = context
            .allocate(device, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        let next = context.next_identity;
        if failure == Failure::None {
            context.write_allocation(a, 0, &[1; 8]).unwrap();
        } else {
            let (pointer, drops) = diagnostic(&mut context, failure, false);
            let result = catch_unwind(AssertUnwindSafe(|| context.write_allocation(a, 0, &[1; 8])));
            assert_diagnostic(result, failure, pointer, &drops);
        }
        assert_eq!(context.next_identity, next);
        assert_eq!(context.version_journal_writer_records_v1(), None);
        assert_eq!(context.is_terminal(), failure == Failure::Terminal);
    }
}

#[test]
fn unsupported_unknown_rosters_keep_all_members_while_cleanup_releases_unrelated_ids() {
    for (kind, count) in [
        (ContextWriterKindV1::Synchronous, 2),
        (ContextWriterKindV1::Submission, 1),
    ] {
        let mut context =
            Context::open_with_version_journal_v1(AllocationOnlyBackend::default(), 3, 1).unwrap();
        let device = context.devices()[0].id();
        context
            .configure_allocation_admission_v1(device, 24, 3)
            .unwrap();
        let ids: Vec<_> = (0..3)
            .map(|_| {
                context
                    .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 8, 8)
                    .unwrap()
            })
            .collect();
        let writer = context.retain_test_writer_v1(&ids[..count], kind);
        let expected: Vec<_> = ids[..count]
            .iter()
            .map(|id| (*id, context.allocations[id], state(&context, *id)))
            .collect();
        let next = context.next_identity;
        let memory = context.backend.inner.memory.clone();
        let credits = context.allocation_admission_usage_v1(device).unwrap();
        for id in &ids[..count] {
            assert!(matches!(
                context.release_allocation(*id),
                Err(RuntimeErrorV1::Validation(
                    RuntimeValidationErrorV1::Unsupported
                ))
            ));
        }
        assert_eq!(context.backend.release_calls, 0);
        assert_eq!(context.backend.inner.memory, memory);
        assert_eq!(
            context.allocation_admission_usage_v1(device).unwrap(),
            credits
        );
        for _ in 0..2 {
            let report = context.cleanup();
            assert!(!report.is_complete());
            assert!(report.failures.is_empty());
            assert!(!context.is_terminal());
            assert_eq!(report.writer_journal_records_v1(), 1);
            assert_eq!(report.allocation_journal_records_v1(), count);
            assert_eq!(context.backend.release_calls, 3 - count);
            assert_eq!(context.next_identity, next);
            assert_eq!(
                context
                    .versions
                    .as_ref()
                    .unwrap()
                    .journal_for_test()
                    .lookup_writer(writer),
                Ok(ContextWriterStateV1::Unknown {
                    member_count: count
                })
            );
            for (id, record, before) in &expected {
                assert_eq!(context.allocations[id], *record);
                assert_eq!(state(&context, *id), *before);
                assert_eq!(
                    context.backend.inner.memory[&record.backend_allocation],
                    memory[&record.backend_allocation]
                );
            }
            let usage = context
                .allocation_admission_usage_v1(device)
                .unwrap()
                .unwrap();
            assert_eq!(usage.retained_records, count);
            assert_eq!(
                usage.used,
                RuntimeResourceVectorV1::ZERO
                    .with(K::RequestedAllocationBytes, 8 * count as u64)
                    .with(K::AllocationRecords, count as u64)
            );
        }
    }
}

#[test]
fn disabled_journal_with_inconsistent_allocation_reference_seals_before_backend_write() {
    let (tracked, tracked_a, _) = fixture(false, 1);
    let mut context = Context::open(AllocationOnlyBackend::default()).unwrap();
    let device = context.devices()[0].id();
    let a = context
        .allocate(device, RuntimeMemoryKindV1::HostVisible, 8, 8)
        .unwrap();
    context.allocations.get_mut(&a).unwrap().journal = tracked.allocations[&tracked_a].journal;
    assert!(matches!(
        context.write_allocation(a, 0, &[1]),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::InvalidBackendDescription
        ))
    ));
    assert!(context.is_terminal());
    assert_eq!(context.backend.write_calls, 0);
}

#[test]
fn empty_unknown_writer_prevents_false_complete_cleanup_or_backend_return() {
    let (mut context, _, _) = fixture(true, 1);
    let writer = context.retain_test_writer_v1(&[], ContextWriterKindV1::Submission);
    let report = context.cleanup();
    assert!(!report.is_complete());
    assert!(!context.is_terminal());
    assert_eq!(report.allocation_journal_records_v1(), 0);
    assert_eq!(report.allocation_credit_records_v1(), 0);
    assert_eq!(report.writer_journal_records_v1(), 1);
    assert_eq!(context.backend.release_calls, 2);
    assert_eq!(
        context
            .versions
            .as_ref()
            .unwrap()
            .journal_for_test()
            .lookup_writer(writer),
        Ok(ContextWriterStateV1::Unknown { member_count: 0 })
    );
    let Err(failure) = context.shutdown() else {
        panic!("retained writer permitted backend return");
    };
    assert_eq!(failure.report.writer_journal_records_v1(), 1);
    assert_eq!(failure.context.backend.release_calls, 2);
}
