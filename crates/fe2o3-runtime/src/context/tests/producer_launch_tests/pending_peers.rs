use super::*;

type Peer = RuntimeSubmissionV1<RuntimeDirectedScalarPeerCopyV1>;

fn pending_peer(f: &mut Fixture, destination: usize) -> (Peer, RuntimeEventIdV1, Vec<u8>) {
    f.context.backend.deferred_copies = true;
    let remote = f.context.devices()[1].id();
    let source = f
        .context
        .allocate(remote, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
        .unwrap();
    let bytes = (0..64).map(|n| (n * 3 + 1) as u8).collect::<Vec<_>>();
    f.context.write_allocation(source, 0, &bytes).unwrap();
    let copy = f
        .context
        .directed_peer_copy_v1(
            f.streams[1],
            span(source, RuntimeAccessV1::Read, 8, 16),
            span(f.allocations[destination], RuntimeAccessV1::Write, 16, 16),
            &[],
        )
        .unwrap();
    let event = f.context.record_event(&copy).unwrap();
    (copy, event, bytes)
}

fn callback<A>(
    f: &mut Fixture,
    submission: &RuntimeSubmissionV1<A>,
    calls: &Arc<Mutex<Vec<RuntimeSubmissionIdV1>>>,
) {
    let calls = calls.clone();
    let id = submission.id;
    f.context
        .on_completion(submission, move |status| {
            assert_eq!(status, RuntimeCompletionStatusV1::Succeeded);
            calls.lock().unwrap().push(id);
        })
        .unwrap();
}

#[test]
fn pending_peer_launch_mixed_inputs_reconcile_through_every_completion_ingress() {
    for ingress in 0..7 {
        let mut f = Fixture::new(8);
        let producer = f
            .launch(
                0,
                vec![span(f.allocations[0], RuntimeAccessV1::Write, 8, 16)],
                &[],
            )
            .unwrap();
        let native_event = f.context.record_event(&producer).unwrap();
        let (peer, peer_event, bytes) = pending_peer(&mut f, 1);
        let mut consumer = f
            .launch(2, f.mixed(true), &[peer_event, native_event])
            .unwrap();
        let backend_order = f.context.backend.producer_launch.requests
            [&consumer.backend_submission]
            .dependencies
            .iter()
            .map(|dep| dep.producer_submission)
            .collect::<Vec<_>>();
        assert_eq!(
            backend_order,
            [peer.backend_submission, producer.backend_submission]
        );
        assert_eq!(
            f.context.producer_launches[&consumer.id]
                .dependencies
                .iter()
                .map(|dep| dep.submission)
                .collect::<Vec<_>>(),
            [producer.id, peer.id]
        );
        assert_eq!(f.context.version_journal_read_records_v1(), Some(4));
        // Three consumer allocations plus the peer's still-held source read.
        let completion = f.context.record_event(&consumer).unwrap();
        for event in [native_event, peer_event] {
            f.context.release_event(event).unwrap();
        }
        let calls = Arc::new(Mutex::new(Vec::new()));
        callback(&mut f, &producer, &calls);
        callback(&mut f, &peer, &calls);
        callback(&mut f, &consumer, &calls);
        for step in 0..3 {
            match ingress {
                0 => {
                    f.context.poll(&mut consumer).unwrap();
                }
                1 => {
                    f.context.wait(&mut consumer, Duration::ZERO).unwrap();
                }
                2 => {
                    f.context.poll_event(completion).unwrap();
                }
                3 => {
                    f.context.wait_event(completion, Duration::ZERO).unwrap();
                }
                4 => {
                    f.context
                        .synchronize_stream(f.streams[2], Duration::ZERO)
                        .unwrap();
                }
                5 => {
                    f.context
                        .drain(&mut consumer, Instant::now() + Duration::from_secs(1))
                        .unwrap();
                }
                6 => {
                    f.context.poll_async_drain_v1(consumer.id).unwrap();
                }
                _ => unreachable!(),
            }
            assert_eq!(
                f.context.backend.producer_launch.calls.len(),
                (step + 1).min(2)
            );
            assert_eq!(
                f.context.backend.directed_calls.len(),
                usize::from(step == 2)
            );
            assert_eq!(
                f.context.query_submission(&consumer).unwrap(),
                if step == 2 {
                    RuntimeCompletionStatusV1::Succeeded
                } else {
                    RuntimeCompletionStatusV1::Pending
                }
            );
            if step == 0 {
                assert_eq!(
                    f.context.backend.producer_launch.completed[&consumer.backend_submission],
                    BackendPollV1::Succeeded
                );
                assert_eq!(
                    f.context.query_submission(&peer).unwrap(),
                    RuntimeCompletionStatusV1::Pending
                );
                assert!(calls.lock().unwrap().is_empty());
                assert_eq!(f.context.version_journal_read_records_v1(), Some(4));
                assert!(state(&f.context, f.allocations[3]).pending_writer.is_some());
            }
        }
        assert_eq!(*calls.lock().unwrap(), [producer.id, peer.id, consumer.id]);
        let reads = f
            .context
            .backend
            .observed_kernel_reads
            .iter()
            .filter(|read| read.submission == consumer.backend_submission)
            .collect::<Vec<_>>();
        assert_eq!(reads.len(), 4);
        assert_eq!(reads[0].bytes, bytes[12..20]);
        assert_eq!(reads[1].bytes, initial_bytes(2)[3..11]);
        assert_eq!(
            reads[2].bytes,
            [output_byte(producer.backend_submission); 8]
        );
        assert_eq!(reads[3].bytes, bytes[12..20]);
        assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
        assert_eq!(f.context.backend.flush_call_count, 0);
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn pending_peer_launch_checks_each_alias_not_the_whole_allocation_lease() {
    let mut f = Fixture::new(8);
    let (peer, event, bytes) = pending_peer(&mut f, 0);
    let before = f.snapshot();
    for ranges in [
        vec![span(f.allocations[0], RuntimeAccessV1::Read, 15, 1)],
        vec![span(f.allocations[0], RuntimeAccessV1::Read, 31, 2)],
        vec![
            span(f.allocations[0], RuntimeAccessV1::Read, 20, 4),
            span(f.allocations[0], RuntimeAccessV1::Read, 30, 3),
        ],
        vec![span(f.allocations[0], RuntimeAccessV1::ReadWrite, 20, 4)],
        vec![
            span(f.allocations[0], RuntimeAccessV1::Read, 20, 4),
            span(f.allocations[0], RuntimeAccessV1::Write, 24, 4),
        ],
    ] {
        validation(
            f.launch(2, ranges, &[event]),
            RuntimeValidationErrorV1::ContextReserved,
        );
        assert_eq!(f.snapshot(), before);
    }
    validation(
        f.launch(
            2,
            vec![span(f.allocations[0], RuntimeAccessV1::Read, 20, 4)],
            &[],
        ),
        RuntimeValidationErrorV1::ContextReserved,
    );
    assert_eq!(f.snapshot(), before);
    let mut consumer = f
        .launch(
            2,
            vec![
                span(f.allocations[0], RuntimeAccessV1::Read, 16, 8),
                span(f.allocations[0], RuntimeAccessV1::Read, 24, 8),
            ],
            &[event],
        )
        .unwrap();
    let marker = f.context.submissions[&consumer.id]
        .journal_producer_read
        .unwrap();
    let input = f
        .context
        .versions
        .as_mut()
        .unwrap()
        .read_leases_for_test_v1()
        .lookup_producer_read(marker.active_first_for_test().unwrap())
        .unwrap()
        .read;
    assert_eq!((input.byte_offset, input.byte_len), (0, 64));
    assert_eq!(f.context.submissions[&peer.id].dependency_retains, 1);
    f.context.release_event(event).unwrap();
    let pending = f.context.release_submission(peer).unwrap_err();
    assert!(matches!(
        pending.error,
        RuntimeErrorV1::Validation(RuntimeValidationErrorV1::SubmissionPending)
    ));
    let mut peer = pending.submission;
    assert_eq!(
        f.context.wait(&mut peer, Duration::from_secs(1)).unwrap(),
        RuntimePollV1::Succeeded
    );
    let retained = f.context.release_submission(peer).unwrap_err();
    assert!(matches!(
        retained.error,
        RuntimeErrorV1::Validation(RuntimeValidationErrorV1::SubmissionRetainedByDependency)
    ));
    f.complete(&mut consumer);
    let reads = &f.context.backend.observed_kernel_reads;
    assert_eq!(reads[0].bytes, bytes[8..16]);
    assert_eq!(reads[1].bytes, bytes[16..24]);
    f.context.release_submission(retained.submission).unwrap();
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn pending_peer_launch_parent_failure_after_admission_preserves_valid_custody() {
    for outcome in 0..3 {
        let mut f = Fixture::new(8);
        let (mut peer, event, _) = pending_peer(&mut f, 0);
        let mut consumer = f
            .launch(
                2,
                vec![span(f.allocations[0], RuntimeAccessV1::Read, 20, 4)],
                &[event],
            )
            .unwrap();
        f.context.release_event(event).unwrap();
        if outcome == 0 {
            f.context
                .backend
                .directed_observations
                .insert(peer.backend_submission, Observation::Failed);
            f.context.poll(&mut peer).unwrap();
        } else if outcome == 1 {
            f.context.backend.cancel_before_publication = true;
            f.context.cancel(&mut peer).unwrap();
        } else {
            f.context
                .backend
                .directed_observations
                .insert(peer.backend_submission, Observation::Quiescent);
            assert!(matches!(
                f.context.poll(&mut peer),
                Err(RuntimeErrorV1::BackendQuiescent(_))
            ));
        }
        assert!(
            f.context
                .validate_producer_launch_custody_v1(consumer.id)
                .is_ok()
        );
        assert!(matches!(
            f.context.poll(&mut consumer).unwrap(),
            RuntimePollV1::Failed { .. }
        ));
        assert!(!f.context.is_terminal());
        assert_eq!(f.context.submissions[&peer.id].dependency_retains, 0);
        assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
        assert!(f.context.backend.observed_kernel_reads.is_empty());
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn pending_peer_launch_physical_success_cannot_override_parent_observations() {
    for observation in [Observation::Pending, Observation::Failed] {
        let mut f = Fixture::new(8);
        let (peer, event, _) = pending_peer(&mut f, 0);
        let mut consumer = f
            .launch(
                2,
                vec![span(f.allocations[0], RuntimeAccessV1::Read, 20, 4)],
                &[event],
            )
            .unwrap();
        f.context
            .backend
            .directed_observations
            .insert(peer.backend_submission, observation);
        assert_eq!(
            f.context.poll(&mut consumer).unwrap(),
            RuntimePollV1::Pending
        );
        assert!(
            f.context
                .backend
                .directed_observations
                .contains_key(&peer.backend_submission)
        );
        assert!(f.context.backend.directed_calls.is_empty());
        assert!(f.context.poll(&mut consumer).is_err());
        assert!(f.context.is_terminal());
        assert!(f.context.producer_launches[&consumer.id].dependencies_held);
        assert!(f.context.version_journal_read_records_v1().unwrap() > 0);
    }
}

#[test]
fn pending_peer_launch_consumer_cancellation_releases_only_its_own_inputs() {
    let mut f = Fixture::new(8);
    let (mut peer, event, _) = pending_peer(&mut f, 0);
    let mut consumer = f
        .launch(
            2,
            vec![
                span(f.allocations[0], RuntimeAccessV1::Read, 20, 4),
                span(f.allocations[2], RuntimeAccessV1::Read, 0, 8),
            ],
            &[event],
        )
        .unwrap();
    f.context.backend.cancel_before_publication = true;
    f.context.cancel(&mut consumer).unwrap();
    assert_eq!(f.context.submissions[&peer.id].dependency_retains, 0);
    assert_eq!(f.context.version_journal_read_records_v1(), Some(1));
    assert_eq!(
        f.context.query_submission(&peer).unwrap(),
        RuntimeCompletionStatusV1::Pending
    );
    assert!(
        f.context
            .backend
            .pending_copies
            .contains_key(&peer.backend_submission)
    );
    f.context.cancel(&mut peer).unwrap();
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn pending_peer_launch_resolved_input_survives_writer_slot_reuse() {
    for cancel in [false, true] {
        let mut f = Fixture::new(3);
        let (mut peer, event, bytes) = pending_peer(&mut f, 0);
        let mut consumer = f
            .launch(
                2,
                vec![span(f.allocations[0], RuntimeAccessV1::Read, 20, 4)],
                &[event],
            )
            .unwrap();
        let old = f.context.submissions[&peer.id].journal_writer.unwrap();
        let marker = f.context.submissions[&consumer.id]
            .journal_producer_read
            .unwrap();
        if cancel {
            f.context.backend.cancel_before_publication = true;
            f.context.cancel(&mut peer).unwrap();
        } else {
            assert_eq!(
                f.context.wait(&mut peer, Duration::from_secs(1)).unwrap(),
                RuntimePollV1::Succeeded
            );
        }
        let unrelated = f
            .launch(
                0,
                vec![span(f.allocations[4], RuntimeAccessV1::Write, 0, 8)],
                &[],
            )
            .unwrap();
        let fresh = f.context.submissions[&unrelated.id].journal_writer.unwrap();
        assert_eq!(old.slot, fresh.slot);
        assert_ne!(old.key, fresh.key);
        assert_eq!(
            f.context
                .versions
                .as_mut()
                .unwrap()
                .read_leases_for_test_v1()
                .producer_read_status(marker.active_first_for_test().unwrap())
                .unwrap(),
            if cancel {
                ContextProducerReadStatusV1::NoEffect
            } else {
                ContextProducerReadStatusV1::Success
            }
        );
        if cancel {
            assert!(matches!(
                f.context.poll(&mut consumer).unwrap(),
                RuntimePollV1::Failed { .. }
            ));
            assert!(f.context.backend.observed_kernel_reads.is_empty());
        } else {
            f.complete(&mut consumer);
            assert_eq!(
                f.context.backend.observed_kernel_reads[0].bytes,
                bytes[12..16]
            );
        }
        assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn pending_peer_launch_custody_corruption_seals_before_backend_observation() {
    for late in [false, true] {
        for corruption in 0..2 {
            let mut f = Fixture::new(8);
            let (peer, event, _) = pending_peer(&mut f, 0);
            let mut consumer = f
                .launch(
                    2,
                    vec![span(f.allocations[0], RuntimeAccessV1::Read, 20, 4)],
                    &[event],
                )
                .unwrap();
            if late {
                assert_eq!(
                    f.context.poll(&mut consumer).unwrap(),
                    RuntimePollV1::Pending
                );
            }
            match corruption {
                0 => {
                    f.context.scalar_peer_copies.remove(&peer.id);
                }
                1 => {
                    f.context
                        .submissions
                        .get_mut(&consumer.id)
                        .unwrap()
                        .journal_producer_read
                        .as_mut()
                        .unwrap()
                        .count += 1;
                }
                _ => unreachable!(),
            }
            let calls = f.context.backend.producer_launch.calls.len();
            assert!(f.context.poll(&mut consumer).is_err());
            assert_eq!(f.context.backend.producer_launch.calls.len(), calls);
            assert!(f.context.backend.directed_calls.is_empty());
            assert!(f.context.is_terminal());
            assert!(f.context.producer_launches[&consumer.id].dependencies_held);
            assert_eq!(f.context.submissions[&peer.id].dependency_retains, 1);
            assert_eq!(f.context.version_journal_read_records_v1(), Some(2));
        }
    }
}

#[test]
fn pending_peer_launch_ordering_only_dependency_still_reconciles() {
    let mut f = Fixture::new(8);
    let (peer, event, _) = pending_peer(&mut f, 0);
    let mut consumer = f
        .launch(
            2,
            vec![span(f.allocations[2], RuntimeAccessV1::Read, 0, 8)],
            &[event],
        )
        .unwrap();
    assert!(
        f.context.submissions[&consumer.id]
            .journal_producer_read
            .is_none()
    );
    assert_eq!(f.context.submissions[&peer.id].dependency_retains, 1);
    f.context.release_event(event).unwrap();
    assert_eq!(
        f.context.poll(&mut consumer).unwrap(),
        RuntimePollV1::Pending
    );
    assert_eq!(
        f.context.query_submission(&peer).unwrap(),
        RuntimeCompletionStatusV1::Pending
    );
    assert_eq!(
        f.context.poll(&mut consumer).unwrap(),
        RuntimePollV1::Succeeded
    );
    assert_eq!(
        f.context.query_submission(&peer).unwrap(),
        RuntimeCompletionStatusV1::Succeeded
    );
    assert_eq!(
        f.context.backend.observed_kernel_reads[0].bytes,
        initial_bytes(2)[..8]
    );
    assert_eq!(f.context.submissions[&peer.id].dependency_retains, 0);
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn pending_peer_launch_discarded_parent_result_does_not_publish_success() {
    for writable in [false, true] {
        let mut f = Fixture::new(8);
        let (peer, event, _) = pending_peer(&mut f, 0);
        let mut bindings = vec![span(f.allocations[0], RuntimeAccessV1::Read, 20, 4)];
        if writable {
            bindings.push(span(f.allocations[3], RuntimeAccessV1::Write, 0, 8));
        }
        let mut consumer = f.launch(2, bindings, &[event]).unwrap();
        f.context
            .backend
            .directed_observations
            .insert(peer.backend_submission, Observation::Quiescent);
        assert_eq!(
            f.context.poll(&mut consumer).unwrap(),
            RuntimePollV1::Pending
        );
        assert!(matches!(
            f.context.poll(&mut consumer),
            Err(RuntimeErrorV1::BackendQuiescent(_))
        ));
        for id in [peer.id, consumer.id] {
            assert_eq!(
                f.context.submissions[&id].status,
                RuntimeCompletionStatusV1::QuiescentWithoutResult
            );
        }
        assert!(!f.context.is_terminal());
        assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
        assert_eq!(
            writer_state(&f.context, f.allocations[0]),
            ContextWriterStateV1::Unknown { member_count: 1 }
        );
        if writable {
            assert_eq!(
                writer_state(&f.context, f.allocations[3]),
                ContextWriterStateV1::Unknown { member_count: 1 }
            );
        }
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn pending_peer_launch_depth_limit_includes_settled_peer_history() {
    for depth in [255, 256] {
        let mut f = Fixture::new(8);
        f.context.backend.deferred_copies = true;
        let remote_device = f.context.devices()[1].id();
        let remote = f
            .context
            .allocate(remote_device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
            .unwrap();
        let remote_stream = f.context.create_stream(remote_device).unwrap();
        let mut previous: Option<(Peer, RuntimeEventIdV1)> = None;
        let mut predecessor = None;
        for index in 0..depth {
            let (source, destination, stream) = if (depth - index) % 2 == 1 {
                (remote, f.allocations[0], f.streams[0])
            } else {
                (f.allocations[0], remote, remote_stream)
            };
            let events = previous
                .as_ref()
                .map(|(_, event)| *event)
                .into_iter()
                .collect::<Vec<_>>();
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
            if index + 1 < depth {
                assert_eq!(
                    f.context.wait(&mut copy, Duration::from_secs(1)).unwrap(),
                    RuntimePollV1::Succeeded
                );
            }
            if let Some((prior, event)) = previous.take() {
                f.context.release_event(event).unwrap();
                if index + 1 < depth {
                    f.context.release_submission(prior).unwrap();
                } else {
                    predecessor = Some(prior);
                }
            }
            previous = Some((copy, event));
        }
        let (mut peer, event) = previous.unwrap();
        assert_eq!(
            f.context.query_submission(&peer).unwrap(),
            RuntimeCompletionStatusV1::Pending
        );
        let before = f.snapshot();
        let result = f.launch(
            2,
            vec![span(f.allocations[0], RuntimeAccessV1::Read, 0, 8)],
            &[event],
        );
        if depth == 256 {
            validation(result, RuntimeValidationErrorV1::TooManyDependencies);
            assert_eq!(f.snapshot(), before);
            assert_eq!(
                f.context.wait(&mut peer, Duration::from_secs(1)).unwrap(),
                RuntimePollV1::Succeeded
            );
        } else {
            let mut consumer = result.unwrap();
            assert_eq!(
                f.context.producer_launches[&consumer.id].state.depth,
                depth + 1
            );
            f.context.release_event(event).unwrap();
            f.complete(&mut consumer);
        }
        f.context.release_submission(predecessor.unwrap()).unwrap();
        assert_eq!(f.context.submissions[&peer.id].dependency_retains, 0);
        assert!(f.context.cleanup().is_complete());
    }
}
