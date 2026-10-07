use super::*;

#[test]
fn terminal_backend_failure_marks_the_context_lost() {
    let mut context = RuntimeContextV1::open(MockBackend {
        terminal_on_submit: true,
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
    assert!(matches!(result, Err(RuntimeErrorV1::BackendTerminal(_))));
    assert!(context.is_terminal());
    assert!(matches!(
        context.release_allocation(allocation),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::ContextTerminal
        ))
    ));
}

#[test]
fn facade_rejects_oversized_backend_descriptions_and_kernel_symbols() {
    for backend in [
        MockBackend {
            device_name_len: MAX_RUNTIME_DEVICE_NAME_BYTES_V1 + 1,
            ..MockBackend::default()
        },
        MockBackend {
            device_target_len: MAX_RUNTIME_DEVICE_TARGET_BYTES_V1 + 1,
            ..MockBackend::default()
        },
    ] {
        assert!(matches!(
            RuntimeContextV1::open(backend),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::InvalidBackendDescription
            ))
        ));
    }

    let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
    let device = context.devices()[0].id();
    let module = context.load_module(device, b"object").unwrap();
    assert!(matches!(
        context.resolve_kernel::<AddArguments>(
            module,
            &"k".repeat(MAX_RUNTIME_KERNEL_NAME_BYTES_V1 + 1),
        ),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::KernelNameTooLong
        ))
    ));
    assert!(matches!(
        context.resolve_kernel::<AddArguments>(module, "bad\0symbol"),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::InvalidKernelName
        ))
    ));
}

#[test]
fn module_image_limit_matches_hsaco_and_accepts_its_exact_boundary() {
    assert_eq!(
        MAX_RUNTIME_MODULE_IMAGE_BYTES_V1,
        fe2o3_hsaco::MAX_HSACO_BYTES
    );
    let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
    let device = context.devices()[0].id();
    let exact = vec![1; MAX_RUNTIME_MODULE_IMAGE_BYTES_V1];
    assert!(context.load_module(device, &exact).is_ok());
    drop(exact);
    let oversized = vec![1; MAX_RUNTIME_MODULE_IMAGE_BYTES_V1 + 1];
    assert!(matches!(
        context.load_module(device, &oversized),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::ModuleTooLarge
        ))
    ));
}

#[test]
fn argument_encoder_outputs_are_bounded_before_binding_validation() {
    let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
    let device = context.devices()[0].id();
    let stream = context.create_stream(device).unwrap();
    let allocation = context
        .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
        .unwrap();
    let module = context.load_module(device, b"object").unwrap();
    let kernel = context
        .resolve_kernel::<HostileArguments>(module, "hostile")
        .unwrap();

    assert!(matches!(
        context.launch(
            stream,
            &kernel,
            &HostileArguments {
                allocation,
                kernarg_len: MAX_RUNTIME_EXPLICIT_KERNARG_BYTES_V1 + 1,
                binding_count: 0,
            },
            geometry(),
            &[],
        ),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::KernargTooLarge
        ))
    ));
    assert!(matches!(
        context.launch(
            stream,
            &kernel,
            &HostileArguments {
                allocation,
                kernarg_len: 8,
                binding_count: fe2o3_host_api::MAX_DISPATCH_BINDINGS_V1 + 1,
            },
            geometry(),
            &[],
        ),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::TooManyBindings
        ))
    ));
}

#[test]
fn shutdown_quiesces_streams_before_releasing_dependent_resources() {
    let context = context_with_cleanup_resources(MockBackend::default());
    let backend = context.shutdown().unwrap();
    assert_eq!(
        backend
            .cleanup_log
            .iter()
            .map(|(kind, _)| *kind)
            .collect::<Vec<_>>(),
        [
            MockCleanupKind::Stream,
            MockCleanupKind::Event,
            MockCleanupKind::Submission,
            MockCleanupKind::Module,
            MockCleanupKind::Allocation,
        ]
    );
    assert!(backend.memory.is_empty());
}

#[test]
fn rejected_stream_cleanup_retains_dependencies_and_can_retry() {
    let context = context_with_cleanup_resources(MockBackend {
        cleanup_failure: MockCleanupFailure::RejectStreamOnce,
        ..MockBackend::default()
    });
    let failure = context.shutdown().unwrap_err();
    assert_eq!(
        failure.report().retained(),
        RuntimeRetainedResourcesV1 {
            streams: 1,
            events: 1,
            submissions: 1,
            modules: 1,
            allocations: 1,
        }
    );
    assert_eq!(failure.report().failures().len(), 1);
    assert!(matches!(
        failure.report().failures()[0].failure(),
        RuntimeBackendFailureV1::Rejected(_)
    ));
    let mut context = failure.into_context();
    let retry = context.cleanup();
    assert!(retry.is_complete());
    assert_eq!(
        context
            .backend()
            .cleanup_log
            .iter()
            .map(|(kind, _)| *kind)
            .collect::<Vec<_>>(),
        [
            MockCleanupKind::Stream,
            MockCleanupKind::Stream,
            MockCleanupKind::Event,
            MockCleanupKind::Submission,
            MockCleanupKind::Module,
            MockCleanupKind::Allocation,
        ]
    );
}

#[test]
fn quiescent_stream_failure_allows_dependent_cleanup_and_retains_stream() {
    let context = context_with_cleanup_resources(MockBackend {
        cleanup_failure: MockCleanupFailure::QuiescentStreamOnce,
        ..MockBackend::default()
    });
    let failure = context.shutdown().unwrap_err();
    assert_eq!(
        failure.report().retained(),
        RuntimeRetainedResourcesV1 {
            streams: 1,
            events: 0,
            submissions: 0,
            modules: 0,
            allocations: 0,
        }
    );
    assert!(matches!(
        failure.report().failures()[0].failure(),
        RuntimeBackendFailureV1::Quiescent(_)
    ));
    let mut context = failure.into_context();
    assert!(context.cleanup().is_complete());
}

#[test]
fn rejected_event_cleanup_blocks_module_and_allocation_release() {
    let context = context_with_cleanup_resources(MockBackend {
        cleanup_failure: MockCleanupFailure::RejectEventOnce,
        ..MockBackend::default()
    });
    let failure = context.shutdown().unwrap_err();
    assert_eq!(
        failure.report().retained(),
        RuntimeRetainedResourcesV1 {
            streams: 0,
            events: 1,
            submissions: 1,
            modules: 1,
            allocations: 1,
        }
    );
    assert_eq!(
        failure
            .context()
            .backend()
            .cleanup_log
            .iter()
            .map(|(kind, _)| *kind)
            .collect::<Vec<_>>(),
        [MockCleanupKind::Stream, MockCleanupKind::Event]
    );
    let mut context = failure.into_context();
    assert!(context.cleanup().is_complete());
}

#[test]
fn terminal_cleanup_stops_calls_and_retains_unprocessed_resources() {
    let context = context_with_cleanup_resources(MockBackend {
        cleanup_failure: MockCleanupFailure::TerminalEvent,
        ..MockBackend::default()
    });
    let failure = context.shutdown().unwrap_err();
    assert!(failure.report().is_terminal());
    assert_eq!(
        failure.report().retained(),
        RuntimeRetainedResourcesV1 {
            streams: 0,
            events: 1,
            submissions: 1,
            modules: 1,
            allocations: 1,
        }
    );
    let mut context = failure.into_context();
    assert_eq!(
        context
            .backend()
            .cleanup_log
            .iter()
            .map(|(kind, _)| *kind)
            .collect::<Vec<_>>(),
        [MockCleanupKind::Stream, MockCleanupKind::Event]
    );
    let second = context.cleanup();
    assert!(second.is_terminal());
    assert_eq!(context.backend().cleanup_log.len(), 2);
}

#[test]
fn execution_capability_detail_defaults_closed_and_reports_opt_in_bits() {
    let context = RuntimeContextV1::open(MockBackend::default()).unwrap();
    let device = context.devices()[0].id();
    assert_eq!(
        context.execution_capabilities(device).unwrap(),
        RuntimeExecutionCapabilitiesV1::default()
    );
    let backend = context.shutdown().unwrap();
    let expected = RuntimeExecutionCapabilitiesV1 {
        native_async_copy: true,
        native_peer_copy: true,
        concurrent_compute: true,
        compute_copy_overlap: true,
        memory_pool: true,
        profiling: true,
        cancellation: true,
        atomics: true,
        collectives: true,
    };
    let context = RuntimeContextV1::open(MockBackend {
        execution_capabilities: expected,
        ..backend
    })
    .unwrap();
    assert_eq!(
        context.execution_capabilities(context.devices()[0].id()),
        Ok(expected)
    );
}
