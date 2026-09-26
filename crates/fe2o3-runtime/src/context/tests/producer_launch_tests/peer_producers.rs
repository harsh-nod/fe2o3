use super::*;

#[test]
fn peer_launch_rejects_unsettled_failed_and_unknown_copy_results_without_effects() {
    for outcome in 0..4 {
        let mut f = Fixture::new(8);
        f.context.backend.deferred_copies = outcome != 3;
        let remote = f.context.devices()[1].id();
        let source = f
            .context
            .allocate(remote, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
            .unwrap();
        let mut copy = f
            .context
            .peer_copy(
                f.streams[0],
                span(source, RuntimeAccessV1::Read, 0, 64),
                span(f.allocations[0], RuntimeAccessV1::Write, 0, 64),
                &[],
            )
            .unwrap();
        let event = f.context.record_event(&copy).unwrap();
        match outcome {
            1 => {
                f.context.backend.wait_observation = Some(BackendPollV1::Failed { code: 7 });
                f.context.wait(&mut copy, Duration::from_secs(1)).unwrap();
            }
            2 => {
                f.context.backend.first_wait_failure = MockWaitFailure::QuiescentFirst;
                assert!(matches!(
                    f.context.wait(&mut copy, Duration::from_secs(1)),
                    Err(RuntimeErrorV1::BackendQuiescent(_))
                ));
            }
            _ => {}
        }
        let before = f.snapshot();
        validation(
            f.launch(
                1,
                vec![span(f.allocations[0], RuntimeAccessV1::Read, 0, 8)],
                &[event],
            ),
            RuntimeValidationErrorV1::Unsupported,
        );
        assert_eq!(f.snapshot(), before);
        f.context.backend.wait_observation = None;
        f.context.backend.first_wait_failure = MockWaitFailure::None;
        if outcome == 0 || outcome == 3 {
            f.context.wait(&mut copy, Duration::from_secs(1)).unwrap();
        }
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn corrupted_settled_peer_custody_quarantines_before_backend_entry() {
    for after_admission in [false, true] {
        let mut f = Fixture::new(8);
        let remote = f.context.devices()[1].id();
        let source = f
            .context
            .allocate(remote, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
            .unwrap();
        let mut copy = f
            .context
            .peer_copy(
                f.streams[0],
                span(source, RuntimeAccessV1::Read, 0, 64),
                span(f.allocations[0], RuntimeAccessV1::Write, 0, 64),
                &[],
            )
            .unwrap();
        f.context.wait(&mut copy, Duration::from_secs(1)).unwrap();
        let event = f.context.record_event(&copy).unwrap();
        let mut consumer = after_admission.then(|| {
            f.launch(
                1,
                vec![span(f.allocations[0], RuntimeAccessV1::Read, 0, 8)],
                &[event],
            )
            .unwrap()
        });
        f.context
            .scalar_peer_copies
            .get_mut(&copy.id)
            .unwrap()
            .backend_submission = None;
        let before = f.context.backend.submit_count;
        let observations = f.context.backend.producer_launch.calls.len();
        if let Some(consumer) = &mut consumer {
            validation(
                f.context.poll(consumer),
                RuntimeValidationErrorV1::InvalidBackendDescription,
            );
            assert_eq!(f.context.submissions[&copy.id].dependency_retains, 1);
        } else {
            validation(
                f.launch(
                    1,
                    vec![span(f.allocations[0], RuntimeAccessV1::Read, 0, 8)],
                    &[event],
                ),
                RuntimeValidationErrorV1::InvalidBackendDescription,
            );
        }
        assert!(f.context.is_terminal());
        assert_eq!(f.context.backend.submit_count, before);
        assert_eq!(f.context.backend.producer_launch.calls.len(), observations);
        assert!(f.context.scalar_peer_copies.contains_key(&copy.id));
    }
}

#[test]
fn settled_directed_peer_preserves_depth_after_ancestors_and_source_are_released() {
    for depth in [2, 255, 256] {
        let mut f = Fixture::new(8);
        f.context.backend.deferred_copies = true;
        let remote_device = f.context.devices()[1].id();
        let remote = f
            .context
            .allocate(remote_device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
            .unwrap();
        let remote_stream = f.context.create_stream(remote_device).unwrap();
        let mut previous: Option<(
            RuntimeSubmissionV1<RuntimeDirectedScalarPeerCopyV1>,
            RuntimeEventIdV1,
        )> = None;
        for index in 0..depth {
            let destination_local = (depth - index) % 2 == 1;
            let (source, destination, stream) = if destination_local {
                (remote, f.allocations[0], f.streams[0])
            } else {
                (f.allocations[0], remote, remote_stream)
            };
            let events: Vec<_> = previous
                .as_ref()
                .map(|(_, event)| *event)
                .into_iter()
                .collect();
            let mut copy = f
                .context
                .directed_peer_copy_v1(
                    stream,
                    span(source, RuntimeAccessV1::Read, 0, 64),
                    span(destination, RuntimeAccessV1::Write, 0, 64),
                    &events,
                )
                .unwrap();
            let event = f.context.record_event(&copy).unwrap();
            assert_eq!(
                f.context.wait(&mut copy, Duration::from_secs(1)).unwrap(),
                RuntimePollV1::Succeeded
            );
            if let Some((prior, event)) = previous.take() {
                f.context.release_event(event).unwrap();
                f.context.release_submission(prior).unwrap();
            }
            previous = Some((copy, event));
        }
        f.context.release_allocation(remote).unwrap();
        let (copy, event) = previous.unwrap();
        let before = f.snapshot();
        let result = f.launch(
            1,
            vec![span(f.allocations[0], RuntimeAccessV1::Read, 0, 8)],
            &[event],
        );
        if depth == 256 {
            validation(result, RuntimeValidationErrorV1::TooManyDependencies);
            assert_eq!(f.snapshot(), before);
        } else {
            let mut consumer = result.unwrap();
            assert_eq!(
                f.context.producer_launches[&consumer.id].state.depth,
                depth + 1
            );
            f.context.release_event(event).unwrap();
            f.complete(&mut consumer);
            f.context.release_submission(consumer).unwrap();
        }
        assert_eq!(f.context.submissions[&copy.id].dependency_retains, 0);
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn settled_peer_launch_rejects_duplicate_event_aliases_and_foreign_device() {
    let mut f = Fixture::new(8);
    let remote = f.context.devices()[1].id();
    let allocation = f
        .context
        .allocate(remote, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
        .unwrap();
    let remote_stream = f.context.create_stream(remote).unwrap();
    let mut copy = f
        .context
        .peer_copy(
            remote_stream,
            span(f.allocations[0], RuntimeAccessV1::Read, 0, 64),
            span(allocation, RuntimeAccessV1::Write, 0, 64),
            &[],
        )
        .unwrap();
    f.context.wait(&mut copy, Duration::from_secs(1)).unwrap();
    let event = f.context.record_event(&copy).unwrap();
    let before = f.snapshot();
    validation(
        f.launch(1, vec![], &[event]),
        RuntimeValidationErrorV1::WrongDevice,
    );
    assert_eq!(f.snapshot(), before);
    let mut local_copy = f
        .context
        .peer_copy(
            f.streams[0],
            span(allocation, RuntimeAccessV1::Read, 0, 64),
            span(f.allocations[0], RuntimeAccessV1::Write, 0, 64),
            &[],
        )
        .unwrap();
    f.context
        .wait(&mut local_copy, Duration::from_secs(1))
        .unwrap();
    let events = [
        f.context.record_event(&local_copy).unwrap(),
        f.context.record_event(&local_copy).unwrap(),
    ];
    let before = f.snapshot();
    validation(
        f.launch(1, vec![], &events),
        RuntimeValidationErrorV1::DuplicateDependency,
    );
    assert_eq!(f.snapshot(), before);
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn settled_peer_copy_feeds_typed_launch_and_retains_exact_producer() {
    let mut f = Fixture::new(8);
    f.context.backend.deferred_copies = true;
    let remote = f.context.devices()[1].id();
    let source = f
        .context
        .allocate(remote, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
        .unwrap();
    let bytes: Vec<_> = (0..64).map(|n| (n * 3 + 1) as u8).collect();
    f.context.write_allocation(source, 0, &bytes).unwrap();
    let mut copy = f
        .context
        .peer_copy(
            f.streams[0],
            span(source, RuntimeAccessV1::Read, 8, 16),
            span(f.allocations[0], RuntimeAccessV1::Write, 16, 16),
            &[],
        )
        .unwrap();
    let event = f.context.record_event(&copy).unwrap();
    assert_eq!(
        f.context.wait(&mut copy, Duration::from_secs(1)).unwrap(),
        RuntimePollV1::Succeeded
    );
    f.context.release_allocation(source).unwrap();
    let mut consumer = f
        .launch(
            1,
            vec![span(f.allocations[0], RuntimeAccessV1::Read, 20, 8)],
            &[event],
        )
        .unwrap();
    f.context.release_event(event).unwrap();
    assert_eq!(f.context.submissions[&copy.id].dependency_retains, 1);
    let failure = f.context.release_submission(copy).unwrap_err();
    assert!(matches!(
        failure.error,
        RuntimeErrorV1::Validation(RuntimeValidationErrorV1::SubmissionRetainedByDependency)
    ));
    let copy = failure.submission;
    f.complete(&mut consumer);
    let reads = &f.context.backend.observed_kernel_reads;
    assert_eq!(reads.last().unwrap().bytes, bytes[12..20]);
    assert_eq!(
        reads.last().unwrap().submission,
        consumer.backend_submission
    );
    assert_eq!(f.context.submissions[&copy.id].dependency_retains, 0);
    f.context.release_submission(copy).unwrap();
    f.context.release_submission(consumer).unwrap();
    assert!(f.context.cleanup().is_complete());
}
