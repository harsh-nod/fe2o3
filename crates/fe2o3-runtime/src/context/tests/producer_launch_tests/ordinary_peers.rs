use super::*;

type Peer = RuntimeSubmissionV1<RuntimePeerCopyV1>;

fn pending_peer(
    f: &mut Fixture,
    destination: usize,
    dependencies: &[RuntimeEventIdV1],
) -> (Peer, RuntimeEventIdV1, RuntimeAllocationIdV1, Vec<u8>) {
    f.context.backend.deferred_copies = true;
    let source = f
        .context
        .allocate(
            f.context.devices()[1].id(),
            RuntimeMemoryKindV1::DeviceLocal,
            64,
            16,
        )
        .unwrap();
    let bytes = (0..64).map(|n| (n * 3 + 1) as u8).collect::<Vec<_>>();
    f.context.write_allocation(source, 0, &bytes).unwrap();
    let peer = f
        .context
        .peer_copy(
            f.streams[0],
            span(source, RuntimeAccessV1::Read, 8, 16),
            span(f.allocations[destination], RuntimeAccessV1::Write, 16, 16),
            dependencies,
        )
        .unwrap();
    let event = f.context.record_event(&peer).unwrap();
    assert!(f.context.scalar_peer_copies[&peer.id].directed.is_none());
    (peer, event, source, bytes)
}

fn read(f: &Fixture, destination: usize) -> Vec<RuntimeMemoryRegionV1> {
    vec![span(
        f.allocations[destination],
        RuntimeAccessV1::Read,
        20,
        8,
    )]
}

fn callback<A>(
    f: &mut Fixture,
    submission: &RuntimeSubmissionV1<A>,
    calls: &Arc<Mutex<Vec<RuntimeSubmissionIdV1>>>,
) {
    let calls = Arc::clone(calls);
    let id = submission.id;
    f.context
        .on_completion(submission, move |status| {
            assert_eq!(status, RuntimeCompletionStatusV1::Succeeded);
            calls.lock().unwrap().push(id);
        })
        .unwrap();
}

#[test]
fn ordinary_peer_consumer_preserves_exact_pending_version_and_reconciles_every_ingress() {
    for already_complete in [false, true] {
        for ingress in 0..7 {
            let mut f = Fixture::new(8);
            let (peer, event, _, bytes) = pending_peer(&mut f, 0, &[]);
            let before = state(&f.context, f.allocations[0]);
            let writer = f.context.submissions[&peer.id].journal_writer.unwrap();
            if already_complete {
                assert_eq!(
                    f.context
                        .backend
                        .wait_v1(peer.backend_submission, Instant::now())
                        .unwrap(),
                    BackendPollV1::Succeeded
                );
            }
            let mut consumer = f
                .launch(
                    2,
                    vec![
                        span(f.allocations[0], RuntimeAccessV1::Read, 20, 8),
                        span(f.allocations[2], RuntimeAccessV1::Read, 3, 8),
                        span(f.allocations[3], RuntimeAccessV1::Write, 4, 8),
                    ],
                    &[event],
                )
                .unwrap();
            assert_eq!(
                f.context.query_submission(&peer).unwrap(),
                RuntimeCompletionStatusV1::Pending
            );
            assert_eq!(state(&f.context, f.allocations[0]), before);
            let marker = f.context.submissions[&consumer.id]
                .journal_producer_read
                .unwrap();
            let reservation = f
                .context
                .versions
                .as_mut()
                .unwrap()
                .read_leases_for_test_v1()
                .lookup_producer_read(marker.active_first_for_test().unwrap())
                .unwrap();
            assert_eq!(reservation.producer, writer);
            assert_eq!(reservation.read.attempt_epoch, before.attempt_epoch);
            assert_eq!(reservation.read.content_lineage, before.content_lineage);
            assert_eq!(
                (reservation.read.byte_offset, reservation.read.byte_len),
                (0, 64)
            );
            let completion = f.context.record_event(&consumer).unwrap();
            f.context.release_event(event).unwrap();
            let calls = Arc::new(Mutex::new(Vec::new()));
            callback(&mut f, &peer, &calls);
            callback(&mut f, &consumer, &calls);
            for step in 0..2 {
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
                    f.context.query_submission(&consumer).unwrap(),
                    if step == 0 {
                        RuntimeCompletionStatusV1::Pending
                    } else {
                        RuntimeCompletionStatusV1::Succeeded
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
                    assert_eq!(state(&f.context, f.allocations[0]), before);
                    assert!(calls.lock().unwrap().is_empty());
                    assert_eq!(f.context.version_journal_read_records_v1(), Some(3));
                }
            }
            assert_eq!(*calls.lock().unwrap(), [peer.id, consumer.id]);
            assert!(f.context.backend.directed_calls.is_empty());
            assert_eq!(f.context.backend.producer_launch.calls.len(), 1);
            assert_eq!(
                f.context.backend.observed_kernel_reads[0].bytes,
                bytes[12..20]
            );
            assert_eq!(
                f.context.backend.observed_kernel_reads[1].bytes,
                initial_bytes(2)[3..11]
            );
            assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
            assert!(f.context.cleanup().is_complete());
        }
    }
}

#[test]
fn ordinary_peer_consumer_checks_exact_event_ranges_and_read_only_aliases_before_issue() {
    let mut f = Fixture::new(8);
    let (peer, event, source, bytes) = pending_peer(&mut f, 0, &[]);
    let (_, wrong_event, _, _) = pending_peer(&mut f, 1, &[]);
    let alias = f.context.record_event(&peer).unwrap();
    let before = f.snapshot();
    for ranges in [
        vec![span(f.allocations[0], RuntimeAccessV1::Read, 15, 1)],
        vec![span(f.allocations[0], RuntimeAccessV1::Read, 31, 2)],
        vec![
            span(f.allocations[0], RuntimeAccessV1::Read, 20, 4),
            span(f.allocations[0], RuntimeAccessV1::Read, 30, 3),
        ],
        vec![span(f.allocations[0], RuntimeAccessV1::ReadWrite, 20, 4)],
        vec![span(f.allocations[0], RuntimeAccessV1::Write, 0, 64)],
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
    for events in [&[][..], &[wrong_event][..]] {
        validation(
            f.launch(2, read(&f, 0), events),
            RuntimeValidationErrorV1::ContextReserved,
        );
        assert_eq!(f.snapshot(), before);
    }
    validation(
        f.launch(2, read(&f, 0), &[event, alias]),
        RuntimeValidationErrorV1::DuplicateDependency,
    );
    validation(
        f.launch(2, vec![span(source, RuntimeAccessV1::Read, 8, 8)], &[event]),
        RuntimeValidationErrorV1::WrongDevice,
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
    f.complete(&mut consumer);
    assert_eq!(
        f.context.backend.observed_kernel_reads[0].bytes,
        bytes[8..16]
    );
    assert_eq!(
        f.context.backend.observed_kernel_reads[1].bytes,
        bytes[16..24]
    );
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn ordinary_peer_consumer_keeps_event_independent_result_and_allocation_custody() {
    let mut f = Fixture::new(8);
    let (mut peer, event, source, _) = pending_peer(&mut f, 0, &[]);
    let mut consumer = f.launch(2, read(&f, 0), &[event]).unwrap();
    f.context.release_event(event).unwrap();
    assert_eq!(f.context.submissions[&peer.id].dependency_retains, 1);
    let failed = f.context.release_submission(peer).unwrap_err();
    assert!(matches!(
        failed.error(),
        RuntimeErrorV1::Validation(RuntimeValidationErrorV1::SubmissionPending)
    ));
    (peer, _) = failed.into_parts();
    assert!(f.context.release_allocation(source).is_err());
    assert!(f.context.release_allocation(f.allocations[0]).is_err());
    assert!(f.context.unload_module(f.module).is_err());
    assert!(
        f.context
            .write_allocation(f.allocations[0], 0, &[0; 64])
            .is_err()
    );
    assert_eq!(
        f.context.wait(&mut peer, Duration::ZERO).unwrap(),
        RuntimePollV1::Succeeded
    );
    let failed = f.context.release_submission(peer).unwrap_err();
    assert!(matches!(
        failed.error(),
        RuntimeErrorV1::Validation(RuntimeValidationErrorV1::SubmissionRetainedByDependency)
    ));
    f.context.release_allocation(source).unwrap();
    assert!(
        f.context
            .write_allocation(f.allocations[0], 0, &[0; 64])
            .is_err()
    );
    f.complete(&mut consumer);
    f.context.release_submission(failed.into_parts().0).unwrap();
    f.context.release_submission(consumer).unwrap();
    f.context
        .write_allocation(f.allocations[0], 0, &[0; 64])
        .unwrap();
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn ordinary_peer_failure_cancellation_and_unknown_after_admission_keep_valid_consumer_custody() {
    for outcome in 0..3 {
        let mut f = Fixture::new(8);
        let (mut peer, event, _, _) = pending_peer(&mut f, 0, &[]);
        let mut consumer = f.launch(2, read(&f, 0), &[event]).unwrap();
        f.context.release_event(event).unwrap();
        match outcome {
            0 => {
                f.context.backend.wait_observation = Some(BackendPollV1::Failed { code: 7 });
                f.context.wait(&mut peer, Duration::ZERO).unwrap();
            }
            1 => {
                f.context.backend.cancel_before_publication = true;
                assert_eq!(
                    f.context.cancel(&mut peer).unwrap(),
                    RuntimeCancellationV1::Cancelled
                );
            }
            2 => {
                f.context.backend.first_wait_failure = MockWaitFailure::QuiescentFirst;
                assert!(matches!(
                    f.context.wait(&mut peer, Duration::ZERO),
                    Err(RuntimeErrorV1::BackendQuiescent(_))
                ));
            }
            _ => unreachable!(),
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
fn ordinary_peer_consumer_cancel_releases_only_its_reservations() {
    let mut f = Fixture::new(8);
    let (mut peer, event, _, _) = pending_peer(&mut f, 0, &[]);
    let before = state(&f.context, f.allocations[0]);
    let mut consumer = f.launch(2, read(&f, 0), &[event]).unwrap();
    f.context.backend.cancel_before_publication = true;
    assert_eq!(
        f.context.cancel(&mut consumer).unwrap(),
        RuntimeCancellationV1::Cancelled
    );
    assert_eq!(f.context.submissions[&peer.id].dependency_retains, 0);
    assert_eq!(state(&f.context, f.allocations[0]), before);
    assert_eq!(f.context.version_journal_read_records_v1(), Some(1));
    assert!(
        f.context
            .backend
            .pending_copies
            .contains_key(&peer.backend_submission)
    );
    assert_eq!(
        f.context.query_submission(&peer).unwrap(),
        RuntimeCompletionStatusV1::Pending
    );
    assert_eq!(
        f.context.cancel(&mut peer).unwrap(),
        RuntimeCancellationV1::Cancelled
    );
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn ordinary_peer_observation_leaf_does_not_invent_success_or_promote_ancestry() {
    for contradiction in 0..3 {
        let mut f = Fixture::new(8);
        let (mut peer, event, _, _) = pending_peer(&mut f, 0, &[]);
        let mut consumer = f.launch(2, read(&f, 0), &[event]).unwrap();
        assert_eq!(
            f.context.poll(&mut consumer).unwrap(),
            RuntimePollV1::Pending
        );
        match contradiction {
            0 => {
                f.context.backend.polls.insert(peer.backend_submission, 0);
            }
            1 => {
                f.context.backend.wait_observation = Some(BackendPollV1::Failed { code: 7 });
                f.context.wait(&mut peer, Duration::ZERO).unwrap();
            }
            2 => {
                f.context.backend.first_wait_failure = MockWaitFailure::QuiescentFirst;
                assert!(matches!(
                    f.context.wait(&mut peer, Duration::ZERO),
                    Err(RuntimeErrorV1::BackendQuiescent(_))
                ));
            }
            _ => unreachable!(),
        }
        if contradiction == 2 {
            f.context.poll(&mut consumer).unwrap();
            assert_eq!(
                f.context.query_submission(&consumer).unwrap(),
                RuntimeCompletionStatusV1::QuiescentWithoutResult
            );
            assert!(!f.context.is_terminal());
            assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
            assert!(f.context.cleanup().is_complete());
        } else {
            assert!(f.context.poll(&mut consumer).is_err());
            assert!(f.context.is_terminal());
            assert!(f.context.producer_launches[&consumer.id].dependencies_held);
            assert!(f.context.version_journal_read_records_v1().unwrap() > 0);
            assert!(!f.context.cleanup().is_complete());
        }
    }
    let mut f = Fixture::new(8);
    let mut unrelated = f.launch(1, vec![], &[]).unwrap();
    let unrelated_event = f.context.record_event(&unrelated).unwrap();
    f.context
        .backend
        .producer_launch
        .observations
        .insert(unrelated.backend_submission, Observation::Failed);
    f.context.poll(&mut unrelated).unwrap();
    let (peer, event, _, _) = pending_peer(&mut f, 0, &[unrelated_event]);
    let mut consumer = f.launch(2, read(&f, 0), &[event]).unwrap();
    f.complete(&mut consumer);
    assert_eq!(
        f.context.query_submission(&peer).unwrap(),
        RuntimeCompletionStatusV1::Succeeded
    );
    assert_eq!(
        f.context.query_submission(&unrelated).unwrap(),
        RuntimeCompletionStatusV1::Failed(RuntimeCompletionFailureV1::BackendCode(7))
    );
    assert!(f.context.scalar_peer_copies[&peer.id].directed.is_none());
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn ordinary_peer_version_and_custody_mismatches_reject_without_consumer_issue() {
    for corruption in 0..4 {
        let mut f = Fixture::new(8);
        let (peer, event, _, _) = pending_peer(&mut f, 0, &[]);
        match corruption {
            0 => {
                f.context
                    .submissions
                    .get_mut(&peer.id)
                    .unwrap()
                    .journal_writer = None;
            }
            1 => {
                f.context
                    .scalar_peer_copies
                    .get_mut(&peer.id)
                    .unwrap()
                    .backend_submission = Some(9999);
            }
            2 => {
                // A successful nonquiescent ordinary producer is malformed, not eligible.
                f.context.submissions.get_mut(&peer.id).unwrap().status =
                    RuntimeCompletionStatusV1::Succeeded;
            }
            3 => {
                f.context
                    .submissions
                    .get_mut(&peer.id)
                    .unwrap()
                    .journal_writer
                    .as_mut()
                    .unwrap()
                    .key
                    .local += 1;
            }
            _ => unreachable!(),
        }
        let issued = f.context.backend.submit_count;
        validation(
            f.launch(2, read(&f, 0), &[event]),
            RuntimeValidationErrorV1::InvalidBackendDescription,
        );
        assert_eq!(f.context.backend.submit_count, issued);
        assert!(
            !f.context
                .producer_launches
                .values()
                .any(|root| root.dependencies.iter().any(|d| d.submission == peer.id))
        );
    }
}

#[test]
fn ordinary_peer_consumer_requires_the_latest_writer_not_an_older_successful_event() {
    let mut f = Fixture::new(8);
    let (mut previous, previous_event, _, _) = pending_peer(&mut f, 0, &[]);
    assert_eq!(
        f.context.wait(&mut previous, Duration::ZERO).unwrap(),
        RuntimePollV1::Succeeded
    );
    let (current, current_event, _, _) = pending_peer(&mut f, 0, &[]);
    let before = f.snapshot();
    validation(
        f.launch(2, read(&f, 0), &[previous_event]),
        RuntimeValidationErrorV1::ContextReserved,
    );
    assert_eq!(f.snapshot(), before);
    let mut consumer = f.launch(2, read(&f, 0), &[current_event]).unwrap();
    let marker = f.context.submissions[&consumer.id]
        .journal_producer_read
        .unwrap();
    let read = f
        .context
        .versions
        .as_mut()
        .unwrap()
        .read_leases_for_test_v1()
        .lookup_producer_read(marker.active_first_for_test().unwrap())
        .unwrap();
    assert_eq!(
        Some(read.producer),
        f.context.submissions[&current.id].journal_writer
    );
    f.complete(&mut consumer);
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn ordinary_peer_terminal_observation_keeps_all_consumer_roots_retained() {
    let mut f = Fixture::new(8);
    let (mut peer, event, _, _) = pending_peer(&mut f, 0, &[]);
    let consumer = f.launch(2, read(&f, 0), &[event]).unwrap();
    f.context.release_event(event).unwrap();
    f.context.backend.first_wait_failure = MockWaitFailure::TerminalFirst;
    assert!(matches!(
        f.context.wait(&mut peer, Duration::ZERO),
        Err(RuntimeErrorV1::BackendTerminal(_))
    ));
    assert!(f.context.is_terminal());
    assert_eq!(f.context.submissions[&peer.id].dependency_retains, 1);
    assert!(f.context.producer_launches[&consumer.id].dependencies_held);
    assert_eq!(f.context.version_journal_read_records_v1(), Some(2));
    let cleanup = f.context.cleanup();
    assert!(cleanup.is_terminal() && !cleanup.is_complete());
    assert_eq!(cleanup.retained().submissions, 2);
}

#[test]
fn ordinary_peer_descended_poll_fault_preserves_diagnostic_and_completion_custody() {
    for observation in [
        Observation::Quiescent,
        Observation::Rejected,
        Observation::Terminal,
        Observation::Panic,
    ] {
        let mut f = Fixture::new(8);
        let (peer, event, _, bytes) = pending_peer(&mut f, 0, &[]);
        let mut consumer = f
            .launch(
                2,
                vec![
                    span(f.allocations[0], RuntimeAccessV1::Read, 20, 8),
                    span(f.allocations[3], RuntimeAccessV1::Write, 4, 8),
                ],
                &[event],
            )
            .unwrap();
        f.context.release_event(event).unwrap();
        let callbacks = Arc::new(Mutex::new(Vec::new()));
        let calls = Arc::clone(&callbacks);
        let peer_id = peer.id;
        f.context
            .on_completion(&peer, move |status| {
                calls.lock().unwrap().push((peer_id, status))
            })
            .unwrap();
        let calls = Arc::clone(&callbacks);
        let consumer_id = consumer.id;
        f.context
            .on_completion(&consumer, move |status| {
                calls.lock().unwrap().push((consumer_id, status))
            })
            .unwrap();
        let before_peer = state(&f.context, f.allocations[0]);
        let before_output = state(&f.context, f.allocations[3]);

        // Execute the mock's real copy and consumer before faulting only the
        // ordinary leaf observation selected by the retained consumer success.
        assert_eq!(
            f.context.poll(&mut consumer).unwrap(),
            RuntimePollV1::Pending
        );
        assert_eq!(
            f.context.backend.producer_launch.completed_copies[&peer.backend_submission],
            BackendPollV1::Succeeded
        );
        assert_eq!(
            f.context.backend.producer_launch.completed[&consumer.backend_submission],
            BackendPollV1::Succeeded
        );
        assert_eq!(
            f.context.producer_launches[&consumer.id].state.terminal,
            Some(BackendPollV1::Succeeded)
        );
        assert_eq!(
            f.context.backend.observed_kernel_reads[0].bytes,
            bytes[12..20]
        );
        assert!(callbacks.lock().unwrap().is_empty());
        for id in [peer.id, consumer.id] {
            assert_eq!(
                f.context.submissions[&id].status,
                RuntimeCompletionStatusV1::Pending
            );
        }
        let retained = f.context.cleanup_report(Vec::new()).retained;
        let native_polls = f.context.backend.poll_call_count;
        f.context
            .backend
            .producer_launch
            .observations
            .insert(peer.backend_submission, observation);
        let result = catch_unwind(AssertUnwindSafe(|| f.context.poll(&mut consumer)));
        match (observation, result) {
            (Observation::Quiescent, Ok(Err(RuntimeErrorV1::BackendQuiescent(error)))) => {
                assert_eq!(error.0, "ordinary copy poll quiescent");
            }
            (Observation::Rejected, Ok(Err(RuntimeErrorV1::BackendRejected(error)))) => {
                assert_eq!(error.0, "ordinary copy poll rejected");
            }
            (Observation::Terminal, Ok(Err(RuntimeErrorV1::BackendTerminal(error)))) => {
                assert_eq!(error.0, "ordinary copy poll terminal");
            }
            (Observation::Panic, Err(payload)) => {
                assert_eq!(
                    payload.downcast_ref::<&str>(),
                    Some(&"ordinary copy poll panic")
                );
            }
            (_, result) => panic!("descended observation diagnostic replaced: {result:?}"),
        }
        assert_eq!(f.context.backend.poll_call_count, native_polls + 1);
        assert_eq!(
            f.context.backend.producer_launch.calls,
            [
                ("poll", consumer.backend_submission),
                ("ordinary-poll", peer.backend_submission),
            ]
        );
        assert_eq!(state(&f.context, f.allocations[0]), before_peer);
        assert_eq!(state(&f.context, f.allocations[3]), before_output);
        for allocation in [f.allocations[0], f.allocations[3]] {
            assert_eq!(
                writer_state(&f.context, allocation),
                ContextWriterStateV1::Unknown { member_count: 1 }
            );
        }
        if matches!(observation, Observation::Quiescent) {
            assert!(!f.context.is_terminal());
            assert_eq!(
                *callbacks.lock().unwrap(),
                [
                    (peer.id, RuntimeCompletionStatusV1::QuiescentWithoutResult),
                    (
                        consumer.id,
                        RuntimeCompletionStatusV1::QuiescentWithoutResult
                    ),
                ]
            );
            assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
            assert_eq!(f.context.submissions[&peer.id].dependency_retains, 0);
            assert!(!f.context.producer_launches[&consumer.id].dependencies_held);
            for id in [peer.id, consumer.id] {
                assert_eq!(
                    f.context.submissions[&id].status,
                    RuntimeCompletionStatusV1::QuiescentWithoutResult
                );
                assert!(f.context.submissions[&id].quiescent);
            }
            f.context.release_submission(peer).unwrap();
            f.context.release_submission(consumer).unwrap();
            assert!(f.context.cleanup().is_complete());
        } else {
            assert!(f.context.is_terminal());
            assert!(callbacks.lock().unwrap().is_empty());
            assert_eq!(f.context.completion_callback_count, 2);
            assert_eq!(f.context.version_journal_read_records_v1(), Some(2));
            assert_eq!(f.context.submissions[&peer.id].dependency_retains, 1);
            assert!(f.context.producer_launches[&consumer.id].dependencies_held);
            for id in [peer.id, consumer.id] {
                assert_eq!(
                    f.context.submissions[&id].status,
                    RuntimeCompletionStatusV1::Pending
                );
                assert!(!f.context.submissions[&id].quiescent);
            }
            let cleanup = f.context.cleanup();
            assert!(cleanup.is_terminal() && !cleanup.is_complete());
            assert_eq!(cleanup.retained(), retained);
            assert_eq!(f.context.backend.poll_call_count, native_polls + 1);
            assert_eq!(f.context.backend.producer_launch.requests.len(), 1);
            assert!(
                f.context
                    .backend
                    .producer_launch
                    .completed_copies
                    .contains_key(&peer.backend_submission)
            );
        }
    }
}
