use super::*;

#[test]
fn progress_stream_default_delegates_once_to_legacy_flush() {
    let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
    let stream = context.create_stream(context.devices()[0].id()).unwrap();
    let backend_stream = context.streams[&stream].backend_stream;
    context.progress_stream_v1(stream).unwrap();
    assert_eq!(context.backend.flush_call_count, 1);
    assert_eq!(context.backend.last_flushed_stream, Some(backend_stream));
    context.flush_stream(stream).unwrap();
    assert_eq!(context.backend.flush_call_count, 2);
    assert!(context.cleanup().is_complete());
}

#[test]
fn progress_stream_rejects_unknown_foreign_and_reserved_before_backend() {
    let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
    let stream = context.create_stream(context.devices()[0].id()).unwrap();
    let mut other = RuntimeContextV1::open(MockBackend::default()).unwrap();
    let foreign = other.create_stream(other.devices()[0].id()).unwrap();
    let unknown = RuntimeStreamIdV1::new(context.context_generation, stream.get() + 1);
    for stream in [foreign, unknown] {
        assert!(matches!(
            context.progress_stream_v1(stream),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::UnknownStream
            ))
        ));
    }
    let hold = context.hold_unpublished_stream_v1(stream).unwrap();
    assert!(matches!(
        context.progress_stream_v1(stream),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::ContextReserved
        ))
    ));
    context.release_unpublished_hold_v1(&hold).unwrap();
    let graph = context.reserve_graph_v1(1).unwrap();
    assert!(matches!(
        context.progress_stream_v1(stream),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::ContextReserved
        ))
    ));
    assert_eq!(context.backend.flush_call_count, 0);
    context
        .flush_with_graph_access_v1(stream, Some(graph))
        .unwrap();
    assert_eq!(context.backend.flush_call_count, 1);
    context.close_graph_issue_v1(graph).unwrap();
    context.release_graph_v1(graph).unwrap();
    context.progress_stream_v1(stream).unwrap();
    assert_eq!(context.backend.flush_call_count, 2);
    assert!(context.cleanup().is_complete());
    assert!(other.cleanup().is_complete());
}

#[test]
fn progress_stream_preserves_rejected_quiescent_and_terminal_dispositions() {
    for failure in [
        MockFlushFailure::RejectOnce,
        MockFlushFailure::Quiescent,
        MockFlushFailure::Terminal,
    ] {
        let mut context = RuntimeContextV1::open(MockBackend {
            flush_failure: failure,
            ..MockBackend::default()
        })
        .unwrap();
        let stream = context.create_stream(context.devices()[0].id()).unwrap();
        let error = context.progress_stream_v1(stream).unwrap_err();
        match failure {
            MockFlushFailure::RejectOnce => assert!(matches!(
                error,
                RuntimeErrorV1::BackendRejected(MockError("flush rejected"))
            )),
            MockFlushFailure::Quiescent => assert!(matches!(
                error,
                RuntimeErrorV1::BackendQuiescent(MockError("flush quiescent"))
            )),
            MockFlushFailure::Terminal => assert!(matches!(
                error,
                RuntimeErrorV1::BackendTerminal(MockError("flush terminal"))
            )),
            _ => unreachable!(),
        }
        assert_eq!(context.backend.flush_call_count, 1);
        if matches!(failure, MockFlushFailure::Terminal) {
            assert!(context.is_terminal());
            assert!(matches!(
                context.progress_stream_v1(stream),
                Err(RuntimeErrorV1::Validation(
                    RuntimeValidationErrorV1::ContextTerminal
                ))
            ));
            assert!(matches!(
                context.flush_stream(stream),
                Err(RuntimeErrorV1::Validation(
                    RuntimeValidationErrorV1::ContextTerminal
                ))
            ));
            assert_eq!(context.backend.flush_call_count, 1);
        } else {
            assert!(!context.is_terminal());
            context.backend.flush_failure = MockFlushFailure::None;
            context.progress_stream_v1(stream).unwrap();
            assert_eq!(context.backend.flush_call_count, 2);
            assert!(context.cleanup().is_complete());
        }
    }
}

#[test]
fn progress_stream_journal_unwind_preserves_roots_and_seals_context() {
    let mut context =
        RuntimeContextV1::open_with_version_journal_v1(MockBackend::default(), 4, 4).unwrap();
    let device = context.devices()[0].id();
    let stream = context.create_stream(device).unwrap();
    let allocation = context
        .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 8)
        .unwrap();
    let module = context.load_module(device, b"object").unwrap();
    let kernel = context
        .resolve_kernel::<AddArguments>(module, "add")
        .unwrap();
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
    let event = context.record_event(&submission).unwrap();
    context.backend.flush_failure = MockFlushFailure::Panic;
    let error = catch_unwind(AssertUnwindSafe(|| context.progress_stream_v1(stream))).unwrap_err();
    assert_eq!(error.downcast_ref::<&str>(), Some(&"flush adapter panic"));
    assert!(context.is_terminal());
    assert_eq!(context.backend.flush_call_count, 1);
    assert!(context.allocations.contains_key(&allocation));
    assert!(context.submissions.contains_key(&submission.id()));
    assert!(context.events.contains_key(&event));
    assert_eq!(context.backend.poll_call_count, 0);
    assert!(matches!(
        context.progress_stream_v1(stream),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::ContextTerminal
        ))
    ));
    assert_eq!(context.backend.flush_call_count, 1);
    assert!(!context.cleanup().is_complete());
}
