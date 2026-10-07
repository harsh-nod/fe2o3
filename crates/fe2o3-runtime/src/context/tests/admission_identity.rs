use super::*;

#[test]
fn generated_scope_epoch_preserves_context_send_for_send_backends() {
    fn assert_send<T: Send>() {}
    assert_send::<RuntimeContextV1<MockBackend>>();
}

#[test]
fn host_capture_registration_is_logical_and_range_checked() {
    let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
    let device = context.devices()[0].id();
    let allocation = context
        .allocate(device, RuntimeMemoryKindV1::HostVisible, 64, 8)
        .unwrap();
    let source = context
        .prepare_host_drain_capture_v1(allocation, 8, 16)
        .unwrap();
    assert_eq!(source.allocation(), allocation);
    assert_eq!(source.device(), device);
    assert_eq!(source.byte_offset(), 8);
    assert_eq!(source.byte_len(), 16);
    assert!(source.belongs_to_context(context.capture_context_generation_v1()));
    assert_eq!(context.backend.allocation_calls, 1);
    assert_eq!(context.backend.poll_call_count, 0);
    assert_eq!(context.backend.wait_call_count, 0);
    assert_eq!(context.backend.flush_call_count, 0);
    for (offset, length) in [(0, 0), (63, 2), (64, 1), (u64::MAX, 1)] {
        assert_eq!(
            context
                .prepare_host_drain_capture_v1(allocation, offset, length)
                .unwrap_err(),
            RuntimeHostCaptureErrorV1::InvalidRange,
        );
    }
    assert_eq!(context.backend.allocation_calls, 1);
    assert!(context.cleanup().is_complete());
}

#[test]
fn host_capture_registration_rejects_foreign_device_local_released_and_terminal() {
    let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
    let device = context.devices()[0].id();
    let allocation = context
        .allocate(device, RuntimeMemoryKindV1::HostVisible, 64, 8)
        .unwrap();
    let local = context
        .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 8)
        .unwrap();
    assert_eq!(
        context
            .prepare_host_drain_capture_v1(local, 0, 8)
            .unwrap_err(),
        RuntimeHostCaptureErrorV1::DeviceLocal,
    );
    let mut other = RuntimeContextV1::open(MockBackend::default()).unwrap();
    assert_eq!(
        other
            .prepare_host_drain_capture_v1(allocation, 0, 8)
            .unwrap_err(),
        RuntimeHostCaptureErrorV1::ForeignContext,
    );
    context.release_allocation(allocation).unwrap();
    assert_eq!(
        context
            .prepare_host_drain_capture_v1(allocation, 0, 8)
            .unwrap_err(),
        RuntimeHostCaptureErrorV1::UnknownAllocation,
    );
    assert!(context.cleanup().is_complete());
    other.quarantine_after_async_command_panic_v1();
    assert_eq!(
        other
            .prepare_host_drain_capture_v1(allocation, 0, 8)
            .unwrap_err(),
        RuntimeHostCaptureErrorV1::ContextTerminal,
    );
}

#[test]
fn r63_private_reservations_require_closed_issue_and_fresh_exact_generation() {
    let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
    let token = context.reserve_graph_v1(1).unwrap();
    let report = context.cleanup();
    assert!(report.retained().is_empty());
    assert!(report.is_graph_reserved());
    assert!(!report.is_complete());
    assert_eq!(
        context.release_graph_v1(token),
        Err(RuntimeValidationErrorV1::SubmissionPending)
    );
    context.close_graph_issue_v1(token).unwrap();
    context.release_graph_v1(token).unwrap();
    let fresh = context.reserve_graph_v1(1).unwrap();
    assert_ne!(token, fresh);
    assert_eq!(
        context.close_graph_issue_v1(token),
        Err(RuntimeValidationErrorV1::ContextReserved)
    );
    assert_eq!(
        context.release_graph_v1(token),
        Err(RuntimeValidationErrorV1::ContextReserved)
    );
    let mut other = RuntimeContextV1::open(MockBackend::default()).unwrap();
    let foreign = other.reserve_graph_v1(1).unwrap();
    assert_eq!(
        context.close_graph_issue_v1(foreign),
        Err(RuntimeValidationErrorV1::ContextReserved)
    );
    context.close_graph_issue_v1(fresh).unwrap();
    context.release_graph_v1(fresh).unwrap();
    other.close_graph_issue_v1(foreign).unwrap();
    other.release_graph_v1(foreign).unwrap();
    context.next_identity = u64::MAX;
    assert_eq!(
        context.reserve_graph_v1(1),
        Err(RuntimeValidationErrorV1::Capacity)
    );
    assert_eq!(context.next_identity, u64::MAX);
    assert!(context.cleanup().is_complete());
}

#[test]
fn r63_closed_reservation_cannot_issue_prepared_copy() {
    let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
    let device = context.devices()[0].id();
    let stream = context.create_stream(device).unwrap();
    let source = context
        .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 8)
        .unwrap();
    let destination = context
        .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 8)
        .unwrap();
    let action = context
        .prepare_graph_copy_v1(
            stream,
            RuntimeMemoryRegionV1 {
                allocation: source,
                access: RuntimeAccessV1::Read,
                byte_offset: 0,
                byte_len: 64,
            },
            RuntimeMemoryRegionV1 {
                allocation: destination,
                access: RuntimeAccessV1::Write,
                byte_offset: 0,
                byte_len: 64,
            },
        )
        .unwrap();
    let token = context.reserve_graph_v1(1).unwrap();
    context.close_graph_issue_v1(token).unwrap();
    assert!(matches!(
        context.submit_graph_action_v1(token, action),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::ContextReserved
        ))
    ));
    assert!(context.submissions.is_empty());
    context.release_graph_v1(token).unwrap();
    assert!(context.cleanup().is_complete());
}

#[test]
fn one_context_multiplexes_streams_across_devices() {
    let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
    let devices = context.devices().to_vec();
    let first = context.create_stream(devices[0].id()).unwrap();
    let second = context.create_stream(devices[0].id()).unwrap();
    let third = context.create_stream(devices[1].id()).unwrap();
    assert_ne!(first, second);
    assert_ne!(second, third);
    context.destroy_stream(first).unwrap();
    context.destroy_stream(second).unwrap();
    context.destroy_stream(third).unwrap();
}

#[test]
fn exhausted_facade_identity_prevents_backend_resource_creation() {
    let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
    let device = context.devices()[0].id();
    context.next_identity = u64::MAX;
    assert!(matches!(
        context.create_stream(device),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::Capacity
        ))
    ));
    assert_eq!(context.backend().next, 0);
    assert!(context.streams.is_empty());
}

#[test]
fn zero_backend_handles_terminally_seal_every_handle_producing_operation() {
    {
        let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
        context.backend.handle_override = Some((MockHandleKind::Stream, 0));
        let device = context.devices()[0].id();
        assert_protocol_failure(
            context.create_stream(device),
            RuntimeBackendProtocolErrorV1::ZeroHandle(RuntimeBackendResourceKindV1::Stream),
        );
        assert!(context.is_terminal());
        assert_eq!(context.streams.len(), 1);
    }
    {
        let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
        context.backend.handle_override = Some((MockHandleKind::Allocation, 0));
        let device = context.devices()[0].id();
        assert_protocol_failure(
            context.allocate(device, RuntimeMemoryKindV1::HostVisible, 64, 16),
            RuntimeBackendProtocolErrorV1::ZeroHandle(RuntimeBackendResourceKindV1::Allocation),
        );
        assert!(context.is_terminal());
        assert_eq!(context.allocations.len(), 1);
    }
    {
        let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
        context.backend.handle_override = Some((MockHandleKind::Module, 0));
        let device = context.devices()[0].id();
        assert_protocol_failure(
            context.load_module(device, b"object"),
            RuntimeBackendProtocolErrorV1::ZeroHandle(RuntimeBackendResourceKindV1::Module),
        );
        assert!(context.is_terminal());
        assert_eq!(context.modules.len(), 1);
    }
    {
        let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
        let device = context.devices()[0].id();
        let module = context.load_module(device, b"object").unwrap();
        context.backend.handle_override = Some((MockHandleKind::Kernel, 0));
        assert_protocol_failure(
            context.resolve_kernel::<AddArguments>(module, "add"),
            RuntimeBackendProtocolErrorV1::ZeroHandle(RuntimeBackendResourceKindV1::Kernel),
        );
        assert!(context.is_terminal());
    }
    {
        let (mut context, stream, allocation, kernel) = context_with_launch_prerequisites();
        context.backend.handle_override = Some((MockHandleKind::Submission, 0));
        assert_protocol_failure(
            context.launch(
                stream,
                &kernel,
                &AddArguments {
                    allocation,
                    scalar: 1,
                },
                geometry(),
                &[],
            ),
            RuntimeBackendProtocolErrorV1::ZeroHandle(RuntimeBackendResourceKindV1::Submission),
        );
        assert!(context.is_terminal());
        assert_eq!(context.submissions.len(), 1);
    }
    {
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
        context.backend.handle_override = Some((MockHandleKind::Event, 0));
        assert_protocol_failure(
            context.record_event(&submission),
            RuntimeBackendProtocolErrorV1::ZeroHandle(RuntimeBackendResourceKindV1::Event),
        );
        assert!(context.is_terminal());
        assert_eq!(context.events.len(), 1);
    }
    {
        let (mut context, stream, source, destination) = context_with_peer_prerequisites();
        context.backend.handle_override = Some((MockHandleKind::Submission, 0));
        assert_protocol_failure(
            context.peer_copy(stream, source, destination, &[]),
            RuntimeBackendProtocolErrorV1::ZeroHandle(RuntimeBackendResourceKindV1::Submission),
        );
        assert!(context.is_terminal());
        assert_eq!(context.submissions.len(), 1);
    }
    {
        let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
        let device = context.devices()[0].id();
        let stream = context.create_stream(device).unwrap();
        let source = context
            .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 16, 8)
            .unwrap();
        let destination = context
            .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 16, 8)
            .unwrap();
        let source = RuntimeMemoryRegionV1 {
            allocation: source,
            access: RuntimeAccessV1::Read,
            byte_offset: 0,
            byte_len: 8,
        };
        let destination = RuntimeMemoryRegionV1 {
            allocation: destination,
            access: RuntimeAccessV1::Write,
            byte_offset: 0,
            byte_len: 8,
        };
        context.backend.handle_override = Some((MockHandleKind::Submission, 0));
        assert_protocol_failure(
            context.copy_async(stream, source, destination, &[]),
            RuntimeBackendProtocolErrorV1::ZeroHandle(RuntimeBackendResourceKindV1::Submission),
        );
        assert!(context.is_terminal());
        assert_eq!(context.submissions.len(), 1);
    }
}

#[test]
fn duplicate_backend_handles_terminally_seal_every_handle_producing_operation() {
    {
        let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
        let device = context.devices()[0].id();
        let first = context.create_stream(device).unwrap();
        let duplicate = context.streams[&first].backend_stream;
        context.backend.handle_override = Some((MockHandleKind::Stream, duplicate));
        assert_protocol_failure(
            context.create_stream(device),
            RuntimeBackendProtocolErrorV1::DuplicateHandle(RuntimeBackendResourceKindV1::Stream),
        );
        assert!(context.is_terminal());
        assert_eq!(context.streams.len(), 2);
    }
    {
        let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
        let device = context.devices()[0].id();
        let first = context
            .allocate(device, RuntimeMemoryKindV1::HostVisible, 64, 16)
            .unwrap();
        let duplicate = context.allocations[&first].backend_allocation;
        context.backend.handle_override = Some((MockHandleKind::Allocation, duplicate));
        assert_protocol_failure(
            context.allocate(device, RuntimeMemoryKindV1::HostVisible, 64, 16),
            RuntimeBackendProtocolErrorV1::DuplicateHandle(
                RuntimeBackendResourceKindV1::Allocation,
            ),
        );
        assert!(context.is_terminal());
        assert_eq!(context.allocations.len(), 2);
    }
    {
        let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
        let device = context.devices()[0].id();
        let first = context.load_module(device, b"object-a").unwrap();
        let duplicate = context.modules[&first].backend_module;
        context.backend.handle_override = Some((MockHandleKind::Module, duplicate));
        assert_protocol_failure(
            context.load_module(device, b"object-b"),
            RuntimeBackendProtocolErrorV1::DuplicateHandle(RuntimeBackendResourceKindV1::Module),
        );
        assert!(context.is_terminal());
        assert_eq!(context.modules.len(), 2);
    }
    {
        let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
        let device = context.devices()[0].id();
        let module = context.load_module(device, b"object").unwrap();
        let first = context
            .resolve_kernel::<AddArguments>(module, "add-a")
            .unwrap();
        context.backend.handle_override = Some((MockHandleKind::Kernel, first.backend_kernel));
        assert_protocol_failure(
            context.resolve_kernel::<AddArguments>(module, "add-b"),
            RuntimeBackendProtocolErrorV1::DuplicateHandle(RuntimeBackendResourceKindV1::Kernel),
        );
        assert!(context.is_terminal());
    }
    {
        let (mut context, stream, allocation, kernel) = context_with_launch_prerequisites();
        let first = context
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
        context.backend.handle_override =
            Some((MockHandleKind::Submission, first.backend_submission));
        assert_protocol_failure(
            context.launch(
                stream,
                &kernel,
                &AddArguments {
                    allocation,
                    scalar: 2,
                },
                geometry(),
                &[],
            ),
            RuntimeBackendProtocolErrorV1::DuplicateHandle(
                RuntimeBackendResourceKindV1::Submission,
            ),
        );
        assert!(context.is_terminal());
        assert_eq!(context.submissions.len(), 2);
    }
    {
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
        let first = context.record_event(&submission).unwrap();
        let duplicate = context.events[&first].backend_event;
        context.backend.handle_override = Some((MockHandleKind::Event, duplicate));
        assert_protocol_failure(
            context.record_event(&submission),
            RuntimeBackendProtocolErrorV1::DuplicateHandle(RuntimeBackendResourceKindV1::Event),
        );
        assert!(context.is_terminal());
        assert_eq!(context.events.len(), 2);
    }
    {
        let (mut context, stream, source, destination) = context_with_peer_prerequisites();
        let first = context.peer_copy(stream, source, destination, &[]).unwrap();
        context.backend.handle_override =
            Some((MockHandleKind::Submission, first.backend_submission));
        assert_protocol_failure(
            context.peer_copy(stream, source, destination, &[]),
            RuntimeBackendProtocolErrorV1::DuplicateHandle(
                RuntimeBackendResourceKindV1::Submission,
            ),
        );
        assert!(context.is_terminal());
        assert_eq!(context.submissions.len(), 2);
    }
    {
        let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
        let device = context.devices()[0].id();
        let stream = context.create_stream(device).unwrap();
        let source_allocation = context
            .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 16, 8)
            .unwrap();
        let destination_allocation = context
            .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 16, 8)
            .unwrap();
        let source = RuntimeMemoryRegionV1 {
            allocation: source_allocation,
            access: RuntimeAccessV1::Read,
            byte_offset: 0,
            byte_len: 8,
        };
        let destination = RuntimeMemoryRegionV1 {
            allocation: destination_allocation,
            access: RuntimeAccessV1::Write,
            byte_offset: 0,
            byte_len: 8,
        };
        let first = context
            .copy_async(stream, source, destination, &[])
            .unwrap();
        context.backend.handle_override =
            Some((MockHandleKind::Submission, first.backend_submission));
        assert_protocol_failure(
            context.copy_async(stream, source, destination, &[]),
            RuntimeBackendProtocolErrorV1::DuplicateHandle(
                RuntimeBackendResourceKindV1::Submission,
            ),
        );
        assert!(context.is_terminal());
        assert_eq!(context.submissions.len(), 2);
    }
}

#[test]
fn opaque_handles_with_equal_local_ids_never_cross_contexts() {
    let mut first = RuntimeContextV1::open(MockBackend::default()).unwrap();
    let mut second = RuntimeContextV1::open(MockBackend::default()).unwrap();
    let first_device = first.devices()[0].id();
    let second_device = second.devices()[0].id();
    let first_stream = first.create_stream(first_device).unwrap();
    let second_stream = second.create_stream(second_device).unwrap();
    let first_allocation = first
        .allocate(first_device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
        .unwrap();
    let second_allocation = second
        .allocate(second_device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
        .unwrap();
    let first_module = first.load_module(first_device, b"object").unwrap();
    let second_module = second.load_module(second_device, b"object").unwrap();
    let first_kernel = first
        .resolve_kernel::<AddArguments>(first_module, "add")
        .unwrap();
    let second_kernel = second
        .resolve_kernel::<AddArguments>(second_module, "add")
        .unwrap();
    let mut first_submission = first
        .launch(
            first_stream,
            &first_kernel,
            &AddArguments {
                allocation: first_allocation,
                scalar: 1,
            },
            geometry(),
            &[],
        )
        .unwrap();
    let second_submission = second
        .launch(
            second_stream,
            &second_kernel,
            &AddArguments {
                allocation: second_allocation,
                scalar: 1,
            },
            geometry(),
            &[],
        )
        .unwrap();
    let first_event = first.record_event(&first_submission).unwrap();
    let second_event = second.record_event(&second_submission).unwrap();

    assert_eq!(first_device.get(), second_device.get());
    assert_eq!(first_stream.get(), second_stream.get());
    assert_eq!(first_allocation.get(), second_allocation.get());
    assert_eq!(first_module.get(), second_module.get());
    assert_eq!(first_event.get(), second_event.get());
    assert_eq!(first_submission.id().get(), second_submission.id().get());
    assert_ne!(first_device, second_device);
    assert_ne!(first_stream, second_stream);

    assert!(matches!(
        second.create_stream(first_device),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::UnknownDevice
        ))
    ));
    assert!(matches!(
        second.destroy_stream(first_stream),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::UnknownStream
        ))
    ));
    assert!(matches!(
        second.write_allocation(first_allocation, 0, &[1]),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::UnknownAllocation
        ))
    ));
    assert!(matches!(
        second.unload_module(first_module),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::UnknownModule
        ))
    ));
    assert!(matches!(
        second.release_event(first_event),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::UnknownEvent
        ))
    ));
    assert!(matches!(
        second.poll(&mut first_submission),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::UnknownSubmission
        ))
    ));
    assert!(matches!(
        second.record_event(&first_submission),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::UnknownSubmission
        ))
    ));
    let failure = second.release_submission(first_submission).unwrap_err();
    assert!(matches!(
        failure.error(),
        RuntimeErrorV1::Validation(RuntimeValidationErrorV1::UnknownSubmission)
    ));
}
