use super::*;

#[test]
fn typed_launch_memory_event_and_dependency_flow_is_address_free() {
    let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
    let device = context.devices()[0].id();
    let stream = context.create_stream(device).unwrap();
    let allocation = context
        .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
        .unwrap();
    context
        .write_allocation(allocation, 4, &[1, 2, 3, 4])
        .unwrap();
    let mut readback = [0; 4];
    context
        .read_allocation(allocation, 4, &mut readback)
        .unwrap();
    assert_eq!(readback, [1, 2, 3, 4]);
    let module = context.load_module(device, b"object").unwrap();
    let kernel = context
        .resolve_kernel::<AddArguments>(module, "add")
        .unwrap();
    assert_ne!(kernel.model_identity().as_bytes(), &[7; 32]);
    let arguments = AddArguments {
        allocation,
        scalar: 9,
    };
    let mut first = context
        .launch(stream, &kernel, &arguments, geometry(), &[])
        .unwrap();
    assert_eq!(context.poll(&mut first).unwrap(), RuntimePollV1::Pending);
    assert_eq!(context.poll(&mut first).unwrap(), RuntimePollV1::Succeeded);
    let event = context.record_event(&first).unwrap();
    let mut second = context
        .launch(stream, &kernel, &arguments, geometry(), &[event])
        .unwrap();
    assert_eq!(context.backend().last_dependency_count, 1);
    assert_eq!(
        context.wait(&mut second, Duration::from_secs(1)).unwrap(),
        RuntimePollV1::Succeeded
    );
    context.release_event(event).unwrap();
}

#[test]
fn typed_kernel_identity_commits_to_module_target_symbol_and_signature() {
    let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
    let device = context.devices()[0].id();
    let first_module = context.load_module(device, b"object-a").unwrap();
    let second_module = context.load_module(device, b"object-b").unwrap();
    let first = context
        .resolve_kernel::<AddArguments>(first_module, "same")
        .unwrap();
    let repeated = context
        .resolve_kernel::<AddArguments>(first_module, "same")
        .unwrap();
    let other_symbol = context
        .resolve_kernel::<AddArguments>(first_module, "other")
        .unwrap();
    let other_module = context
        .resolve_kernel::<AddArguments>(second_module, "same")
        .unwrap();

    assert_eq!(first.model_identity(), repeated.model_identity());
    assert_ne!(first.model_identity(), other_symbol.model_identity());
    assert_ne!(first.model_identity(), other_module.model_identity());
}

#[test]
fn retained_submission_cannot_reach_backend_after_its_stream_is_destroyed() {
    let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
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
    context.destroy_stream(stream).unwrap();

    assert!(matches!(
        context.poll(&mut submission),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::UnknownStream
        ))
    ));
    assert!(matches!(
        context.wait(&mut submission, Duration::from_secs(1)),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::UnknownStream
        ))
    ));
    assert!(matches!(
        context.record_event(&submission),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::UnknownStream
        ))
    ));
}

#[test]
fn wait_rejects_unrepresentable_deadline() {
    let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
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
    assert!(matches!(
        context.wait(&mut submission, Duration::MAX),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::InvalidDeadline
        ))
    ));
}

#[test]
fn submission_release_is_consuming_retryable_and_requires_quiescence() {
    let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
    let device = context.devices()[0].id();
    let stream = context.create_stream(device).unwrap();
    let allocation = context
        .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
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

    let failure = context.release_submission(submission).unwrap_err();
    assert!(matches!(
        failure.error(),
        RuntimeErrorV1::Validation(RuntimeValidationErrorV1::SubmissionPending)
    ));
    let (mut submission, _) = failure.into_parts();
    assert_eq!(
        context
            .wait(&mut submission, Duration::from_secs(1))
            .unwrap(),
        RuntimePollV1::Succeeded
    );
    context.release_submission(submission).unwrap();
    assert_eq!(
        context.backend().cleanup_log.last().map(|(kind, _)| *kind),
        Some(MockCleanupKind::Submission)
    );

    let submission = context
        .launch(
            stream,
            &kernel,
            &AddArguments {
                allocation,
                scalar: 2,
            },
            geometry(),
            &[],
        )
        .unwrap();
    context.destroy_stream(stream).unwrap();
    context.release_submission(submission).unwrap();
}

#[test]
fn peer_copy_moves_between_distinct_devices() {
    let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
    let source_device = context.devices()[0].id();
    let destination_device = context.devices()[1].id();
    let stream = context.create_stream(destination_device).unwrap();
    let source = context
        .allocate(source_device, RuntimeMemoryKindV1::DeviceLocal, 16, 8)
        .unwrap();
    let destination = context
        .allocate(destination_device, RuntimeMemoryKindV1::DeviceLocal, 16, 8)
        .unwrap();
    context.write_allocation(source, 0, &[3, 1, 4, 1]).unwrap();
    let submission = context
        .peer_copy(
            stream,
            RuntimeMemoryRegionV1 {
                allocation: source,
                access: RuntimeAccessV1::Read,
                byte_offset: 0,
                byte_len: 4,
            },
            RuntimeMemoryRegionV1 {
                allocation: destination,
                access: RuntimeAccessV1::Write,
                byte_offset: 4,
                byte_len: 4,
            },
            &[],
        )
        .unwrap();
    assert!(matches!(
        submission.peer_transfer_mechanism(),
        Some(PeerTransferMechanismV1::DeclaredPeerCopy { .. })
    ));
    let mut bytes = [0; 4];
    context.read_allocation(destination, 4, &mut bytes).unwrap();
    assert_eq!(bytes, [3, 1, 4, 1]);

    for (source_access, destination_access) in [
        (RuntimeAccessV1::Write, RuntimeAccessV1::Write),
        (RuntimeAccessV1::Read, RuntimeAccessV1::Read),
    ] {
        assert!(matches!(
            context.peer_copy(
                stream,
                RuntimeMemoryRegionV1 {
                    allocation: source,
                    access: source_access,
                    byte_offset: 0,
                    byte_len: 4,
                },
                RuntimeMemoryRegionV1 {
                    allocation: destination,
                    access: destination_access,
                    byte_offset: 4,
                    byte_len: 4,
                },
                &[],
            ),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::InvalidAccess
            ))
        ));
    }
}

#[test]
fn async_copy_is_typed_validated_and_same_device() {
    let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
    let device = context.devices()[0].id();
    let other_device = context.devices()[1].id();
    let stream = context.create_stream(device).unwrap();
    let source = context
        .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 16, 8)
        .unwrap();
    let destination = context
        .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 16, 8)
        .unwrap();
    let foreign = context
        .allocate(other_device, RuntimeMemoryKindV1::DeviceLocal, 16, 8)
        .unwrap();
    context.write_allocation(source, 2, &[4, 3, 2, 1]).unwrap();
    let region = |allocation, access, byte_offset| RuntimeMemoryRegionV1 {
        allocation,
        access,
        byte_offset,
        byte_len: 4,
    };
    let mut submission = context
        .copy_async(
            stream,
            region(source, RuntimeAccessV1::Read, 2),
            region(destination, RuntimeAccessV1::Write, 8),
            &[],
        )
        .unwrap();
    assert_eq!(submission.peer_transfer_mechanism(), None);
    assert_eq!(
        context.poll(&mut submission).unwrap(),
        RuntimePollV1::Pending
    );
    assert_eq!(
        context
            .wait(&mut submission, Duration::from_secs(1))
            .unwrap(),
        RuntimePollV1::Succeeded
    );
    let mut observed = [0_u8; 4];
    context
        .read_allocation(destination, 8, &mut observed)
        .unwrap();
    assert_eq!(observed, [4, 3, 2, 1]);
    assert!(matches!(
        context.copy_async(
            stream,
            region(source, RuntimeAccessV1::Read, 2),
            region(foreign, RuntimeAccessV1::Write, 0),
            &[],
        ),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::WrongDevice
        ))
    ));
    assert!(matches!(
        context.copy_async(
            stream,
            region(source, RuntimeAccessV1::Write, 2),
            region(destination, RuntimeAccessV1::Write, 8),
            &[],
        ),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::InvalidAccess
        ))
    ));
    assert!(matches!(
        context.copy_async(
            stream,
            region(source, RuntimeAccessV1::Read, 2),
            RuntimeMemoryRegionV1 {
                allocation: destination,
                access: RuntimeAccessV1::Write,
                byte_offset: 8,
                byte_len: 5,
            },
            &[],
        ),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::InvalidRange
        ))
    ));
    assert!(matches!(
        context.copy_async(
            stream,
            region(source, RuntimeAccessV1::Read, 2),
            region(destination, RuntimeAccessV1::Write, 14),
            &[],
        ),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::InvalidRange
        ))
    ));
    assert!(matches!(
        context.copy_async(
            stream,
            region(source, RuntimeAccessV1::Read, 0),
            region(source, RuntimeAccessV1::Write, 8),
            &[],
        ),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::InvalidRange
        ))
    ));
    let event = context.record_event(&submission).unwrap();
    assert!(matches!(
        context.copy_async(
            stream,
            region(source, RuntimeAccessV1::Read, 2),
            region(destination, RuntimeAccessV1::Write, 8),
            &[event, event],
        ),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::DuplicateDependency
        ))
    ));
    context.release_event(event).unwrap();
    context.release_submission(submission).unwrap();
}

#[test]
fn peer_copy_identity_includes_the_private_context_brand() {
    fn transfer(context: &mut RuntimeContextV1<MockBackend>) -> PeerTransferMechanismV1 {
        let source_device = context.devices()[0].id();
        let destination_device = context.devices()[1].id();
        let stream = context.create_stream(destination_device).unwrap();
        let source = context
            .allocate(source_device, RuntimeMemoryKindV1::DeviceLocal, 16, 8)
            .unwrap();
        let destination = context
            .allocate(destination_device, RuntimeMemoryKindV1::DeviceLocal, 16, 8)
            .unwrap();
        context
            .peer_copy(
                stream,
                RuntimeMemoryRegionV1 {
                    allocation: source,
                    access: RuntimeAccessV1::Read,
                    byte_offset: 0,
                    byte_len: 4,
                },
                RuntimeMemoryRegionV1 {
                    allocation: destination,
                    access: RuntimeAccessV1::Write,
                    byte_offset: 0,
                    byte_len: 4,
                },
                &[],
            )
            .unwrap()
            .peer_transfer_mechanism()
            .unwrap()
    }

    let mut first = RuntimeContextV1::open(MockBackend::default()).unwrap();
    let mut second = RuntimeContextV1::open(MockBackend::default()).unwrap();
    assert_ne!(transfer(&mut first), transfer(&mut second));
}

#[test]
fn peer_copy_bounds_dependencies_and_admits_source_device_events() {
    let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
    let source_device = context.devices()[0].id();
    let destination_device = context.devices()[1].id();
    let source_stream = context.create_stream(source_device).unwrap();
    let destination_stream = context.create_stream(destination_device).unwrap();
    let source = context
        .allocate(source_device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
        .unwrap();
    let destination = context
        .allocate(destination_device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
        .unwrap();
    let module = context.load_module(source_device, b"object").unwrap();
    let kernel = context
        .resolve_kernel::<AddArguments>(module, "add")
        .unwrap();
    let submission = context
        .launch(
            source_stream,
            &kernel,
            &AddArguments {
                allocation: source,
                scalar: 1,
            },
            geometry(),
            &[],
        )
        .unwrap();
    let source_event = context.record_event(&submission).unwrap();
    let source_region = RuntimeMemoryRegionV1 {
        allocation: source,
        access: RuntimeAccessV1::Read,
        byte_offset: 0,
        byte_len: 4,
    };
    let destination_region = RuntimeMemoryRegionV1 {
        allocation: destination,
        access: RuntimeAccessV1::Write,
        byte_offset: 0,
        byte_len: 4,
    };

    context
        .peer_copy(
            destination_stream,
            source_region,
            destination_region,
            &[source_event],
        )
        .unwrap();
    assert_eq!(context.backend().last_dependency_count, 1);
    assert!(matches!(
        context.peer_copy(
            destination_stream,
            source_region,
            destination_region,
            &[source_event, source_event],
        ),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::DuplicateDependency
        ))
    ));
    let excessive = vec![source_event; MAX_RUNTIME_DEPENDENCIES_V1 + 1];
    assert!(matches!(
        context.peer_copy(
            destination_stream,
            source_region,
            destination_region,
            &excessive,
        ),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::TooManyDependencies
        ))
    ));
}
