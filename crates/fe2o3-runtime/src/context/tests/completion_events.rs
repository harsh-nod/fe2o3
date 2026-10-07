use super::*;

#[test]
fn cancellation_distinguishes_prepublication_quiescence_from_too_late() {
    for (cancel_before_publication, expected) in [
        (true, RuntimeCancellationV1::Cancelled),
        (false, RuntimeCancellationV1::TooLate),
    ] {
        let mut context = RuntimeContextV1::open(MockBackend {
            cancel_before_publication,
            ..MockBackend::default()
        })
        .unwrap();
        let device = context.devices()[0].id();
        let stream = context.create_stream(device).unwrap();
        let source = context
            .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 16, 8)
            .unwrap();
        let destination = context
            .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 16, 8)
            .unwrap();
        let mut submission = context
            .copy_async(
                stream,
                RuntimeMemoryRegionV1 {
                    allocation: source,
                    access: RuntimeAccessV1::Read,
                    byte_offset: 0,
                    byte_len: 16,
                },
                RuntimeMemoryRegionV1 {
                    allocation: destination,
                    access: RuntimeAccessV1::Write,
                    byte_offset: 0,
                    byte_len: 16,
                },
                &[],
            )
            .unwrap();
        assert_eq!(context.cancel(&mut submission).unwrap(), expected);
        if expected == RuntimeCancellationV1::Cancelled {
            assert_eq!(
                context.poll(&mut submission).unwrap(),
                RuntimePollV1::Failed {
                    code: RUNTIME_CANCELLED_CODE_V1
                }
            );
        } else {
            assert_eq!(
                context
                    .drain(&mut submission, Instant::now() + Duration::from_secs(1))
                    .unwrap(),
                RuntimePollV1::Succeeded
            );
        }
        context.release_submission(submission).unwrap();
    }
}

#[test]
fn launch_rejects_invalid_or_nonzero_pointer_patches() {
    let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
    let device = context.devices()[0].id();
    let stream = context.create_stream(device).unwrap();
    let allocation = context
        .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
        .unwrap();
    let module = context.load_module(device, b"object").unwrap();
    let kernel = context
        .resolve_kernel::<PatchArguments>(module, "patches")
        .unwrap();

    for arguments in [
        PatchArguments {
            allocation,
            offsets: [0, 16],
            kernarg_len: 16,
            patch_fill: 0,
        },
        PatchArguments {
            allocation,
            offsets: [0, 4],
            kernarg_len: 16,
            patch_fill: 0,
        },
        PatchArguments {
            allocation,
            offsets: [0, 0],
            kernarg_len: 16,
            patch_fill: 0,
        },
        PatchArguments {
            allocation,
            offsets: [0, 8],
            kernarg_len: 16,
            patch_fill: 1,
        },
    ] {
        assert!(matches!(
            context.launch(stream, &kernel, &arguments, geometry(), &[]),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::InvalidKernargPatch
            ))
        ));
    }
}

#[test]
fn event_and_submission_observation_share_one_exact_once_completion() {
    use std::sync::{Arc, Mutex};

    let (mut context, stream, allocation, kernel) = context_with_launch_prerequisites();
    let mut submission = context
        .launch(
            stream,
            &kernel,
            &AddArguments {
                allocation,
                scalar: 1,
            },
            geometry(),
            &[],
        )
        .unwrap();
    let observed = Arc::new(Mutex::new(Vec::new()));
    let callback_observed = Arc::clone(&observed);
    context
        .on_completion(&submission, move |status| {
            callback_observed.lock().unwrap().push(status);
        })
        .unwrap();
    let event = context.record_event(&submission).unwrap();

    assert_eq!(
        context.query_event(event),
        Ok(RuntimeCompletionStatusV1::Pending)
    );
    assert_eq!(
        context.poll_event(event).unwrap(),
        RuntimeCompletionStatusV1::Pending
    );
    assert_eq!(context.backend().poll_call_count, 1);
    assert_eq!(
        context.wait_event(event, Duration::from_secs(1)).unwrap(),
        RuntimeCompletionStatusV1::Succeeded
    );
    assert_eq!(context.backend().wait_call_count, 1);
    assert_eq!(
        context.query_submission(&submission),
        Ok(RuntimeCompletionStatusV1::Succeeded)
    );
    assert_eq!(
        context.poll(&mut submission).unwrap(),
        RuntimePollV1::Succeeded
    );
    assert_eq!(context.backend().poll_call_count, 1);
    assert_eq!(
        observed.lock().unwrap().as_slice(),
        [RuntimeCompletionStatusV1::Succeeded]
    );

    let after_completion = Arc::new(Mutex::new(Vec::new()));
    let callback_observed = Arc::clone(&after_completion);
    context
        .on_completion(&submission, move |status| {
            callback_observed.lock().unwrap().push(status);
        })
        .unwrap();
    assert_eq!(
        after_completion.lock().unwrap().as_slice(),
        [RuntimeCompletionStatusV1::Succeeded]
    );
    context.release_event(event).unwrap();
    context.release_submission(submission).unwrap();
    assert_eq!(observed.lock().unwrap().len(), 1);
}

#[test]
fn cancellation_is_typed_and_callbacks_are_not_repeated_by_drain_or_release() {
    use std::sync::{Arc, Mutex};

    let mut context = RuntimeContextV1::open(MockBackend {
        cancel_before_publication: true,
        ..MockBackend::default()
    })
    .unwrap();
    let device = context.devices()[0].id();
    let stream = context.create_stream(device).unwrap();
    let allocation = context
        .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
        .unwrap();
    let module = context.load_module(device, b"object").unwrap();
    let kernel = context
        .resolve_kernel::<AddArguments>(module, "add")
        .unwrap();
    let mut submission = context
        .launch(
            stream,
            &kernel,
            &AddArguments {
                allocation,
                scalar: 1,
            },
            geometry(),
            &[],
        )
        .unwrap();
    let observed = Arc::new(Mutex::new(Vec::new()));
    let callback_observed = Arc::clone(&observed);
    context
        .on_completion(&submission, move |status| {
            callback_observed.lock().unwrap().push(status);
        })
        .unwrap();
    let event = context.record_event(&submission).unwrap();

    assert_eq!(
        context.cancel(&mut submission).unwrap(),
        RuntimeCancellationV1::Cancelled
    );
    let cancelled = RuntimeCompletionStatusV1::Failed(RuntimeCompletionFailureV1::Cancelled);
    assert_eq!(context.query_event(event), Ok(cancelled));
    assert_eq!(context.poll_event(event).unwrap(), cancelled);
    assert_eq!(
        context
            .drain(&mut submission, Instant::now() + Duration::from_secs(1))
            .unwrap(),
        RuntimePollV1::Failed {
            code: RUNTIME_CANCELLED_CODE_V1
        }
    );
    context.release_event(event).unwrap();
    context.release_submission(submission).unwrap();
    assert_eq!(observed.lock().unwrap().as_slice(), [cancelled]);
    assert_eq!(context.backend().poll_call_count, 0);
    assert_eq!(context.backend().wait_call_count, 0);
}

#[test]
fn callback_panics_are_contained_and_stream_quiescence_has_typed_status() {
    use std::sync::{Arc, Mutex};

    struct PanickingDropPayload;

    impl Drop for PanickingDropPayload {
        fn drop(&mut self) {
            panic!("completion callback panic payload destructor");
        }
    }

    let (mut context, stream, allocation, kernel) = context_with_launch_prerequisites();
    let submission = context
        .launch(
            stream,
            &kernel,
            &AddArguments {
                allocation,
                scalar: 1,
            },
            geometry(),
            &[],
        )
        .unwrap();
    let observed = Arc::new(Mutex::new(Vec::new()));
    let callback_observed = Arc::clone(&observed);
    context
        .on_completion(&submission, move |status| {
            callback_observed.lock().unwrap().push(status);
        })
        .unwrap();
    context
        .on_completion(&submission, |_| std::panic::panic_any(PanickingDropPayload))
        .unwrap();

    context.destroy_stream(stream).unwrap();
    assert_eq!(
        context.query_submission(&submission),
        Ok(RuntimeCompletionStatusV1::QuiescentWithoutResult)
    );
    assert_eq!(
        observed.lock().unwrap().as_slice(),
        [RuntimeCompletionStatusV1::QuiescentWithoutResult]
    );
    assert_eq!(context.completion_callback_panic_count(), 1);
    context
        .on_completion(&submission, |_| std::panic::panic_any(PanickingDropPayload))
        .unwrap();
    assert_eq!(context.completion_callback_panic_count(), 2);
    context.release_submission(submission).unwrap();
}

#[test]
fn typed_completion_distinguishes_backend_codes_from_cancellation() {
    let mut context = RuntimeContextV1::open(MockBackend {
        wait_observation: Some(BackendPollV1::Failed {
            code: RUNTIME_CANCELLED_CODE_V1,
        }),
        ..MockBackend::default()
    })
    .unwrap();
    let device = context.devices()[0].id();
    let stream = context.create_stream(device).unwrap();
    let allocation = context
        .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
        .unwrap();
    let module = context.load_module(device, b"object").unwrap();
    let kernel = context
        .resolve_kernel::<AddArguments>(module, "add")
        .unwrap();
    let mut submission = context
        .launch(
            stream,
            &kernel,
            &AddArguments {
                allocation,
                scalar: 1,
            },
            geometry(),
            &[],
        )
        .unwrap();

    context
        .wait(&mut submission, Duration::from_secs(1))
        .unwrap();
    assert_eq!(
        context.query_submission(&submission),
        Ok(RuntimeCompletionStatusV1::Failed(
            RuntimeCompletionFailureV1::BackendCode(RUNTIME_CANCELLED_CODE_V1)
        ))
    );
}

#[test]
fn stream_query_and_synchronize_preserve_aggregate_completion() {
    let (mut context, stream, allocation, kernel) = context_with_launch_prerequisites();
    let arguments = AddArguments {
        allocation,
        scalar: 1,
    };
    let first = context
        .launch(stream, &kernel, &arguments, geometry(), &[])
        .unwrap();
    let second = context
        .launch(stream, &kernel, &arguments, geometry(), &[])
        .unwrap();

    assert_eq!(
        context.query_stream(stream),
        Ok(RuntimeStreamObservationV1 {
            total_submissions: 2,
            pending: 2,
            ..RuntimeStreamObservationV1::default()
        })
    );
    assert_eq!(
        context
            .synchronize_stream(stream, Duration::from_secs(1))
            .unwrap(),
        RuntimeStreamObservationV1 {
            total_submissions: 2,
            succeeded: 2,
            ..RuntimeStreamObservationV1::default()
        }
    );
    assert_eq!(context.backend().wait_call_count, 2);
    assert_eq!(
        context.backend().wait_deadlines[0],
        context.backend().wait_deadlines[1]
    );
    context.release_submission(first).unwrap();
    context.release_submission(second).unwrap();
    assert_eq!(
        context.query_stream(stream),
        Ok(RuntimeStreamObservationV1::default())
    );
}

#[test]
fn stream_observation_retains_failure_while_other_work_completes() {
    let mut context = RuntimeContextV1::open(MockBackend {
        cancel_before_publication: true,
        ..MockBackend::default()
    })
    .unwrap();
    let device = context.devices()[0].id();
    let stream = context.create_stream(device).unwrap();
    let allocation = context
        .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
        .unwrap();
    let module = context.load_module(device, b"object").unwrap();
    let kernel = context
        .resolve_kernel::<AddArguments>(module, "add")
        .unwrap();
    let arguments = AddArguments {
        allocation,
        scalar: 1,
    };
    let mut cancelled = context
        .launch(stream, &kernel, &arguments, geometry(), &[])
        .unwrap();
    let completed = context
        .launch(stream, &kernel, &arguments, geometry(), &[])
        .unwrap();

    assert_eq!(
        context.cancel(&mut cancelled).unwrap(),
        RuntimeCancellationV1::Cancelled
    );
    assert_eq!(
        context
            .synchronize_stream(stream, Duration::from_secs(1))
            .unwrap(),
        RuntimeStreamObservationV1 {
            total_submissions: 2,
            succeeded: 1,
            failed: 1,
            first_failure: Some(RuntimeCompletionFailureV1::Cancelled),
            ..RuntimeStreamObservationV1::default()
        }
    );
    assert_eq!(context.backend().wait_call_count, 1);
    context.release_submission(cancelled).unwrap();
    context.release_submission(completed).unwrap();
}

#[test]
fn stream_synchronize_continues_after_rejected_wait_and_discharges_later_callbacks() {
    use std::sync::{Arc, Mutex};

    let (mut context, stream, allocation, kernel) =
        context_with_launch_prerequisites_using(MockBackend {
            first_wait_failure: MockWaitFailure::RejectFirst,
            ..MockBackend::default()
        });
    let arguments = AddArguments {
        allocation,
        scalar: 1,
    };
    let first = context
        .launch(stream, &kernel, &arguments, geometry(), &[])
        .unwrap();
    let second = context
        .launch(stream, &kernel, &arguments, geometry(), &[])
        .unwrap();
    let observed = Arc::new(Mutex::new(Vec::new()));
    let callback_observed = Arc::clone(&observed);
    context
        .on_completion(&second, move |status| {
            callback_observed.lock().unwrap().push(status);
        })
        .unwrap();

    assert!(matches!(
        context.synchronize_stream(stream, Duration::from_secs(1)),
        Err(RuntimeErrorV1::BackendRejected(_))
    ));
    assert_eq!(context.backend().wait_call_count, 2);
    assert_eq!(
        context.query_submission(&first),
        Ok(RuntimeCompletionStatusV1::Pending)
    );
    assert_eq!(
        context.query_submission(&second),
        Ok(RuntimeCompletionStatusV1::Succeeded)
    );
    assert_eq!(
        observed.lock().unwrap().as_slice(),
        [RuntimeCompletionStatusV1::Succeeded]
    );

    context.destroy_stream(stream).unwrap();
    assert_eq!(
        context.query_submission(&first),
        Ok(RuntimeCompletionStatusV1::QuiescentWithoutResult)
    );
    context.release_submission(first).unwrap();
    context.release_submission(second).unwrap();
}

#[test]
fn stream_synchronize_continues_after_quiescent_wait_but_stops_on_terminal_wait() {
    use std::sync::{Arc, Mutex};

    let (mut context, stream, allocation, kernel) =
        context_with_launch_prerequisites_using(MockBackend {
            first_wait_failure: MockWaitFailure::QuiescentFirst,
            ..MockBackend::default()
        });
    let arguments = AddArguments {
        allocation,
        scalar: 1,
    };
    let first = context
        .launch(stream, &kernel, &arguments, geometry(), &[])
        .unwrap();
    let second = context
        .launch(stream, &kernel, &arguments, geometry(), &[])
        .unwrap();
    let observed = Arc::new(Mutex::new(Vec::new()));
    for submission in [&first, &second] {
        let callback_observed = Arc::clone(&observed);
        context
            .on_completion(submission, move |status| {
                callback_observed.lock().unwrap().push(status);
            })
            .unwrap();
    }

    assert!(matches!(
        context.synchronize_stream(stream, Duration::from_secs(1)),
        Err(RuntimeErrorV1::BackendQuiescent(_))
    ));
    assert_eq!(context.backend().wait_call_count, 2);
    assert_eq!(
        context.query_stream(stream),
        Ok(RuntimeStreamObservationV1 {
            total_submissions: 2,
            succeeded: 1,
            quiescent_without_result: 1,
            ..RuntimeStreamObservationV1::default()
        })
    );
    assert_eq!(
        observed.lock().unwrap().as_slice(),
        [
            RuntimeCompletionStatusV1::QuiescentWithoutResult,
            RuntimeCompletionStatusV1::Succeeded,
        ]
    );
    context.release_submission(first).unwrap();
    context.release_submission(second).unwrap();

    let (mut terminal, stream, allocation, kernel) =
        context_with_launch_prerequisites_using(MockBackend {
            first_wait_failure: MockWaitFailure::TerminalFirst,
            ..MockBackend::default()
        });
    let arguments = AddArguments {
        allocation,
        scalar: 1,
    };
    let first = terminal
        .launch(stream, &kernel, &arguments, geometry(), &[])
        .unwrap();
    let _second = terminal
        .launch(stream, &kernel, &arguments, geometry(), &[])
        .unwrap();
    let terminal_callbacks = Arc::new(Mutex::new(Vec::new()));
    let callback_observed = Arc::clone(&terminal_callbacks);
    terminal
        .on_completion(&first, move |status| {
            callback_observed.lock().unwrap().push(status);
        })
        .unwrap();
    assert!(matches!(
        terminal.synchronize_stream(stream, Duration::from_secs(1)),
        Err(RuntimeErrorV1::BackendTerminal(_))
    ));
    assert!(terminal.is_terminal());
    assert_eq!(terminal.backend().wait_call_count, 1);
    assert!(terminal_callbacks.lock().unwrap().is_empty());
}

#[test]
fn flush_stream_validates_before_backend_and_preserves_failure_class() {
    let mut context = RuntimeContextV1::open(MockBackend {
        flush_failure: MockFlushFailure::RejectOnce,
        ..MockBackend::default()
    })
    .unwrap();
    let device = context.devices()[0].id();
    let stream = context.create_stream(device).unwrap();
    let backend_stream = context.streams[&stream].backend_stream;
    let unknown = RuntimeStreamIdV1::new(context.context_generation, stream.get() + 1);

    assert!(matches!(
        context.flush_stream(unknown),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::UnknownStream
        ))
    ));
    assert_eq!(context.backend().flush_call_count, 0);
    assert!(matches!(
        context.flush_stream(stream),
        Err(RuntimeErrorV1::BackendRejected(_))
    ));
    assert!(!context.is_terminal());
    assert_eq!(context.backend().flush_call_count, 1);
    assert_eq!(context.backend().last_flushed_stream, Some(backend_stream));
    context.flush_stream(stream).unwrap();
    assert_eq!(context.backend().flush_call_count, 2);
}

#[test]
fn terminal_flush_seals_context_without_a_second_backend_call() {
    let mut context = RuntimeContextV1::open(MockBackend {
        flush_failure: MockFlushFailure::Terminal,
        ..MockBackend::default()
    })
    .unwrap();
    let device = context.devices()[0].id();
    let stream = context.create_stream(device).unwrap();

    assert!(matches!(
        context.flush_stream(stream),
        Err(RuntimeErrorV1::BackendTerminal(_))
    ));
    assert!(context.is_terminal());
    assert_eq!(context.backend().flush_call_count, 1);
    assert!(matches!(
        context.flush_stream(stream),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::ContextTerminal
        ))
    ));
    assert_eq!(context.backend().flush_call_count, 1);
}
