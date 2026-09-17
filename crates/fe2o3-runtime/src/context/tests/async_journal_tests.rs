use super::*;
use fe2o3_runtime_model::{ContextAllocationStateV1, ContextWriterKindV1, ContextWriterStateV1};

type Context = RuntimeContextV1<MockBackend>;

pub(super) fn fixture(
    writers: usize,
) -> (
    Context,
    RuntimeStreamIdV1,
    RuntimeAllocationIdV1,
    TypedRuntimeKernelV1<AddArguments>,
) {
    let backend = MockBackend {
        next: 100,
        ..MockBackend::default()
    };
    let mut context = Context::open_with_version_journal_v1(backend, 128, writers).unwrap();
    let device = context.devices()[0].id();
    let stream = context.create_stream(device).unwrap();
    let allocation = context
        .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
        .unwrap();
    let module = context.load_module(device, b"object").unwrap();
    let kernel = context
        .resolve_kernel::<AddArguments>(module, "add")
        .unwrap();
    (context, stream, allocation, kernel)
}

pub(super) fn state(context: &Context, id: RuntimeAllocationIdV1) -> ContextAllocationStateV1 {
    context
        .versions
        .as_ref()
        .unwrap()
        .journal_for_test()
        .lookup_allocation(context.allocations[&id].journal.unwrap())
        .unwrap()
}

pub(super) fn writer_state(context: &Context, id: RuntimeAllocationIdV1) -> ContextWriterStateV1 {
    context
        .versions
        .as_ref()
        .unwrap()
        .journal_for_test()
        .lookup_writer(state(context, id).pending_writer.unwrap())
        .unwrap()
}

fn launch(
    context: &mut Context,
    stream: RuntimeStreamIdV1,
    allocation: RuntimeAllocationIdV1,
    kernel: &TypedRuntimeKernelV1<AddArguments>,
) -> RuntimeSubmissionV1<AddArguments> {
    context
        .launch(
            stream,
            kernel,
            &AddArguments {
                allocation,
                scalar: 1,
            },
            geometry(),
            &[],
        )
        .unwrap()
}

#[test]
fn async_writer_uses_exact_submission_identity_and_settles_once_through_event() {
    let (mut context, stream, allocation, kernel) = fixture(2);
    let next = context.next_identity;
    let mut submission = launch(&mut context, stream, allocation, &kernel);
    let writer = state(&context, allocation).pending_writer.unwrap();
    assert_eq!(
        writer.key.context_generation,
        submission.id.context_generation
    );
    assert_eq!(writer.key.local, submission.id.local);
    assert_eq!(writer.key.kind, ContextWriterKindV1::Submission);
    assert_ne!(writer.key.local, submission.backend_submission);
    assert_eq!(submission.id.local, next);
    assert_eq!(context.next_identity, next + 1);
    assert_eq!(state(&context, allocation).byte_extent, 64);
    assert_eq!(
        context.poll(&mut submission).unwrap(),
        RuntimePollV1::Pending
    );
    assert_eq!(state(&context, allocation).content_lineage, 0);
    let event = context.record_event(&submission).unwrap();
    assert_eq!(
        context.wait_event(event, Duration::ZERO).unwrap(),
        RuntimeCompletionStatusV1::Succeeded
    );
    assert_eq!(state(&context, allocation).content_lineage, 1);
    assert!(state(&context, allocation).pending_writer.is_none());
    assert_eq!(context.version_journal_writer_records_v1(), Some(0));
    assert_eq!(
        context.poll(&mut submission).unwrap(),
        RuntimePollV1::Succeeded
    );
    context.destroy_stream(stream).unwrap();
    assert_eq!(state(&context, allocation).content_lineage, 1);
    assert!(context.cleanup().is_complete());
}

pub(super) struct MixedArguments(pub(super) Vec<RuntimeMemoryRegionV1>);

impl RuntimeArgumentsV1 for MixedArguments {
    const SIGNATURE_V1: [u8; 32] = [41; 32];

    fn encode_explicit_kernarg_v1(&self) -> Vec<u8> {
        vec![0; self.0.len() * 8]
    }

    fn bindings_v1(&self) -> Vec<RuntimeBindingV1> {
        self.0
            .iter()
            .enumerate()
            .map(|(index, region)| RuntimeBindingV1 {
                region: *region,
                kernarg_byte_offset: index as u32 * 8,
            })
            .collect()
    }
}

pub(super) fn region(
    allocation: RuntimeAllocationIdV1,
    access: RuntimeAccessV1,
    byte_offset: u64,
) -> RuntimeMemoryRegionV1 {
    RuntimeMemoryRegionV1 {
        allocation,
        access,
        byte_offset,
        byte_len: 8,
    }
}

#[test]
fn async_writer_canonicalizes_duplicate_writable_allocations_and_skips_read_only() {
    let (mut context, stream, a, _) = fixture(1);
    let device = context.devices()[0].id();
    let b = context
        .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
        .unwrap();
    let read = context
        .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
        .unwrap();
    let module = context.load_module(device, b"mixed").unwrap();
    let kernel = context
        .resolve_kernel::<MixedArguments>(module, "mixed")
        .unwrap();
    let args = MixedArguments(vec![
        region(b, RuntimeAccessV1::Write, 0),
        region(read, RuntimeAccessV1::Read, 0),
        region(a, RuntimeAccessV1::ReadWrite, 8),
        region(a, RuntimeAccessV1::Write, 0),
    ]);
    let before = state(&context, read);
    let mut submission = context
        .launch(stream, &kernel, &args, geometry(), &[])
        .unwrap();
    assert_eq!(
        writer_state(&context, a),
        ContextWriterStateV1::Pending { member_count: 2 }
    );
    assert_eq!(
        state(&context, a).pending_writer,
        state(&context, b).pending_writer
    );
    assert_eq!(state(&context, read), before);
    context.wait(&mut submission, Duration::ZERO).unwrap();
    assert_eq!(state(&context, a).content_lineage, 1);
    assert_eq!(state(&context, b).content_lineage, 1);
    assert_eq!(state(&context, read), before);
    assert!(context.cleanup().is_complete());
}

#[test]
fn read_only_submission_needs_no_writer_capacity() {
    let (mut context, stream, a, kernel) = fixture(1);
    let mut writer = launch(&mut context, stream, a, &kernel);
    let device = context.devices()[0].id();
    let read = context
        .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
        .unwrap();
    let module = context.load_module(device, b"read").unwrap();
    let reader = context
        .resolve_kernel::<MixedArguments>(module, "read")
        .unwrap();
    let mut submission = context
        .launch(
            stream,
            &reader,
            &MixedArguments(vec![region(read, RuntimeAccessV1::Read, 0)]),
            geometry(),
            &[],
        )
        .unwrap();
    assert!(context.submissions[&submission.id].journal_writer.is_none());
    context.wait(&mut submission, Duration::ZERO).unwrap();
    assert_eq!(state(&context, read).attempt_epoch, 0);
    assert_eq!(context.version_journal_writer_records_v1(), Some(1));
    context.wait(&mut writer, Duration::ZERO).unwrap();
    assert!(context.cleanup().is_complete());
}

#[test]
fn copy_and_peer_copy_journal_only_the_full_destination() {
    for peer in [false, true] {
        let (mut context, local_stream, source, _) = fixture(1);
        let device = context.devices()[usize::from(peer)].id();
        let destination = context
            .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
            .unwrap();
        let stream = if peer {
            context.create_stream(device).unwrap()
        } else {
            local_stream
        };
        let source_region = region(source, RuntimeAccessV1::Read, 8);
        let destination_region = region(destination, RuntimeAccessV1::Write, 24);
        let before = state(&context, source);
        if peer {
            let mut submission = context
                .peer_copy(stream, source_region, destination_region, &[])
                .unwrap();
            assert_eq!(
                writer_state(&context, destination),
                ContextWriterStateV1::Pending { member_count: 1 }
            );
            context.wait(&mut submission, Duration::ZERO).unwrap();
        } else {
            let mut submission = context
                .copy_async(stream, source_region, destination_region, &[])
                .unwrap();
            assert_eq!(
                writer_state(&context, destination),
                ContextWriterStateV1::Pending { member_count: 1 }
            );
            context.wait(&mut submission, Duration::ZERO).unwrap();
        }
        assert_eq!(state(&context, destination).byte_extent, 64);
        assert_eq!(state(&context, destination).content_lineage, 1);
        assert_eq!(state(&context, source), before);
        assert!(context.cleanup().is_complete());
    }
}

#[test]
fn capacity_and_conflicting_writers_reject_before_identity_or_backend() {
    for writers in [1, 2] {
        let (mut context, stream, allocation, kernel) = fixture(writers);
        let mut submission = launch(&mut context, stream, allocation, &kernel);
        let before = (
            context.next_identity,
            context.backend.submit_count,
            state(&context, allocation),
        );
        let result = context.launch(
            stream,
            &kernel,
            &AddArguments {
                allocation,
                scalar: 1,
            },
            geometry(),
            &[],
        );
        let expected = if writers == 1 {
            RuntimeValidationErrorV1::Capacity
        } else {
            RuntimeValidationErrorV1::ContextReserved
        };
        assert!(matches!(result, Err(RuntimeErrorV1::Validation(error)) if error == expected));
        assert_eq!(
            (
                context.next_identity,
                context.backend.submit_count,
                state(&context, allocation)
            ),
            before
        );
        context.wait(&mut submission, Duration::ZERO).unwrap();
        assert!(context.cleanup().is_complete());
    }
}

#[test]
fn rejected_completion_observation_is_not_no_effect() {
    let (mut context, stream, allocation, kernel) = fixture(1);
    context.backend.first_wait_failure = MockWaitFailure::RejectFirst;
    let mut submission = launch(&mut context, stream, allocation, &kernel);
    let before = state(&context, allocation);
    assert!(matches!(
        context.wait(&mut submission, Duration::ZERO),
        Err(RuntimeErrorV1::BackendRejected(_))
    ));
    assert_eq!(state(&context, allocation), before);
    assert_eq!(
        writer_state(&context, allocation),
        ContextWriterStateV1::Pending { member_count: 1 }
    );
    context.wait(&mut submission, Duration::ZERO).unwrap();
    assert_eq!(state(&context, allocation).content_lineage, 1);
    assert!(context.cleanup().is_complete());
}

#[test]
fn explicit_unpublished_cancellation_settles_no_effect_but_too_late_does_not() {
    for cancellable in [false, true] {
        let (mut context, stream, allocation, kernel) = fixture(1);
        context.backend.cancel_before_publication = cancellable;
        let mut submission = launch(&mut context, stream, allocation, &kernel);
        context.cancel(&mut submission).unwrap();
        assert_eq!(state(&context, allocation).attempt_epoch, 1);
        assert_eq!(state(&context, allocation).content_lineage, 0);
        assert_eq!(
            state(&context, allocation).pending_writer.is_none(),
            cancellable
        );
        if !cancellable {
            context.wait(&mut submission, Duration::ZERO).unwrap();
        }
        assert!(context.cleanup().is_complete());
    }
}

#[test]
fn failed_or_resultless_completion_retains_unknown_after_metadata_release() {
    for outcome in 0..3 {
        let (mut context, stream, allocation, kernel) = fixture(1);
        let mut submission = launch(&mut context, stream, allocation, &kernel);
        match outcome {
            0 => {
                context.backend.wait_observation = Some(BackendPollV1::Failed { code: -2 });
                assert_eq!(
                    context.wait(&mut submission, Duration::ZERO).unwrap(),
                    RuntimePollV1::Failed { code: -2 }
                );
            }
            1 => {
                context.backend.first_wait_failure = MockWaitFailure::QuiescentFirst;
                assert!(matches!(
                    context.wait(&mut submission, Duration::ZERO),
                    Err(RuntimeErrorV1::BackendQuiescent(_))
                ));
            }
            _ => context.destroy_stream(stream).unwrap(),
        }
        assert_eq!(
            writer_state(&context, allocation),
            ContextWriterStateV1::Unknown { member_count: 1 }
        );
        context.release_submission(submission).unwrap();
        assert!(context.submissions.is_empty());
        assert_eq!(context.version_journal_writer_records_v1(), Some(1));
        assert_eq!(state(&context, allocation).content_lineage, 0);
        context.release_allocation(allocation).unwrap();
        let report = context.cleanup();
        assert!(report.is_complete());
        assert!(!report.is_terminal());
        assert_eq!(report.writer_journal_records_v1(), 0);
        assert!(!context.allocations.contains_key(&allocation));
    }
}

#[test]
fn invalid_submission_handles_preserve_attempted_and_existing_writers() {
    for duplicate in [false, true] {
        let (mut context, stream, a, kernel) = fixture(2);
        let handle = if duplicate {
            launch(&mut context, stream, a, &kernel).backend_submission
        } else {
            0
        };
        let device = context.devices()[0].id();
        let b = context
            .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
            .unwrap();
        context.backend.handle_override = Some((MockHandleKind::Submission, handle));
        assert!(matches!(
            context.launch(
                stream,
                &kernel,
                &AddArguments {
                    allocation: b,
                    scalar: 1
                },
                geometry(),
                &[]
            ),
            Err(RuntimeErrorV1::BackendProtocol(_))
        ));
        assert!(context.is_terminal());
        assert_eq!(context.submissions.len(), 1 + usize::from(duplicate));
        assert_eq!(
            writer_state(&context, b),
            ContextWriterStateV1::Unknown { member_count: 1 }
        );
        if duplicate {
            assert_eq!(
                writer_state(&context, a),
                ContextWriterStateV1::Unknown { member_count: 1 }
            );
        }
        assert!(!context.cleanup().is_complete());
    }
}

#[test]
fn initial_no_handle_failures_settle_only_definite_rejection() {
    for failure in [
        MockMemoryFailure::Rejected,
        MockMemoryFailure::Quiescent,
        MockMemoryFailure::Terminal,
        MockMemoryFailure::Panic,
    ] {
        let (mut context, stream, allocation, _) = fixture(1);
        let next = context.next_identity;
        let record = context.streams[&stream];
        let result = catch_unwind(AssertUnwindSafe(|| {
            context.submit_context_operation_v1::<()>(
                stream,
                record,
                &[allocation],
                None,
                None,
                |backend| {
                    backend.submit_count += 1;
                    mock_memory_failure_v1(failure).map(|()| 0)
                },
            )
        }));
        match (failure, result) {
            (MockMemoryFailure::Rejected, Ok(Err(RuntimeErrorV1::BackendRejected(error)))) => {
                assert_eq!(error.0, "allocation rejected")
            }
            (MockMemoryFailure::Quiescent, Ok(Err(RuntimeErrorV1::BackendQuiescent(error)))) => {
                assert_eq!(error.0, "allocation quiescent failure")
            }
            (MockMemoryFailure::Terminal, Ok(Err(RuntimeErrorV1::BackendTerminal(error)))) => {
                assert_eq!(error.0, "allocation terminal failure")
            }
            (MockMemoryFailure::Panic, Err(payload)) => assert_eq!(
                payload.downcast_ref::<&str>(),
                Some(&"scripted allocation adapter panic")
            ),
            _ => panic!("original failure was replaced"),
        }
        assert_eq!(context.next_identity, next + 1);
        assert_eq!(context.backend.submit_count, 1);
        assert!(context.submissions.is_empty());
        assert_eq!(state(&context, allocation).attempt_epoch, 1);
        assert_eq!(state(&context, allocation).content_lineage, 0);
        if failure == MockMemoryFailure::Rejected {
            assert_eq!(context.version_journal_writer_records_v1(), Some(0));
            assert!(context.cleanup().is_complete());
        } else {
            assert_eq!(
                writer_state(&context, allocation),
                ContextWriterStateV1::Unknown { member_count: 1 }
            );
            assert_eq!(
                context.is_terminal(),
                failure != MockMemoryFailure::Quiescent
            );
            assert_eq!(
                context.cleanup().is_complete(),
                failure == MockMemoryFailure::Quiescent
            );
        }
    }
}

#[test]
fn missing_expected_writer_cannot_publish_completion() {
    let (mut context, stream, allocation, kernel) = fixture(1);
    let mut submission = launch(&mut context, stream, allocation, &kernel);
    let called = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = called.clone();
    context
        .on_completion(&submission, move |_| {
            observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        })
        .unwrap();
    context
        .versions
        .as_mut()
        .unwrap()
        .remove_submission_writer_root_for_test_v1(submission.id);
    assert!(matches!(
        context.wait(&mut submission, Duration::ZERO),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::InvalidBackendDescription
        ))
    ));
    assert!(context.is_terminal());
    assert_eq!(
        context.query_submission(&submission).unwrap(),
        RuntimeCompletionStatusV1::Pending
    );
    assert_eq!(state(&context, allocation).content_lineage, 0);
    assert_eq!(called.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert!(!context.cleanup().is_complete());
}

#[test]
fn callback_panic_cannot_undo_settlement_or_repeat_delivery() {
    let (mut context, stream, allocation, kernel) = fixture(1);
    let mut submission = launch(&mut context, stream, allocation, &kernel);
    context
        .on_completion(&submission, |status| {
            assert_eq!(status, RuntimeCompletionStatusV1::Succeeded);
            panic!("scripted callback panic after completion");
        })
        .unwrap();
    context.wait(&mut submission, Duration::ZERO).unwrap();
    assert_eq!(state(&context, allocation).content_lineage, 1);
    assert_eq!(context.version_journal_writer_records_v1(), Some(0));
    assert_eq!(context.completion_callback_panic_count(), 1);
    context.wait(&mut submission, Duration::ZERO).unwrap();
    assert_eq!(context.completion_callback_panic_count(), 1);
    assert!(!context.is_terminal());
    assert!(context.cleanup().is_complete());
}

#[test]
fn prepared_copy_revalidates_original_destination_before_begin_or_backend_entry() {
    let (mut context, stream, source, _) = fixture(1);
    let device = context.devices()[0].id();
    let destination = context
        .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
        .unwrap();
    let action = context
        .prepare_graph_copy_v1(
            stream,
            region(source, RuntimeAccessV1::Read, 0),
            region(destination, RuntimeAccessV1::Write, 0),
        )
        .unwrap();
    context.release_allocation(destination).unwrap();
    let reservation = context.reserve_graph_v1(1).unwrap();
    let next = context.next_identity;
    let backend_next = context.backend.next;
    assert!(matches!(
        context.submit_graph_action_v1(reservation, action),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::UnknownAllocation
        ))
    ));
    assert_eq!(context.next_identity, next);
    assert_eq!(context.backend.next, backend_next);
    assert_eq!(context.version_journal_writer_records_v1(), Some(0));
    context.close_graph_issue_v1(reservation).unwrap();
    context.release_graph_v1(reservation).unwrap();
    assert!(context.cleanup().is_complete());
}

#[test]
fn writer_registry_reuses_capacity_across_dense_settlement_cycles() {
    let (mut context, stream, a, kernel) = fixture(64);
    let device = context.devices()[0].id();
    let mut allocations = vec![a];
    for _ in 1..64 {
        allocations.push(
            context
                .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
                .unwrap(),
        );
    }
    let mut submissions: Vec<_> = allocations
        .iter()
        .map(|&id| launch(&mut context, stream, id, &kernel))
        .collect();
    for cycle in 0..8 {
        for parity in 0..2 {
            for index in (parity..64).step_by(2) {
                context
                    .wait(&mut submissions[index], Duration::ZERO)
                    .unwrap();
                context
                    .release_submission_ref(&submissions[index], None)
                    .unwrap();
            }
            for index in (parity..64).step_by(2) {
                assert_eq!(
                    state(&context, allocations[index]).content_lineage,
                    cycle + 1
                );
                submissions[index] = launch(&mut context, stream, allocations[index], &kernel);
            }
        }
    }
    for submission in &mut submissions {
        context.wait(submission, Duration::ZERO).unwrap();
    }
    assert_eq!(context.version_journal_writer_records_v1(), Some(0));
    assert!(context.cleanup().is_complete());
}
