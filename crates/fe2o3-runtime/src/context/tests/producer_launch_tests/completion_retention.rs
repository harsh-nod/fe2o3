use super::*;

#[test]
fn producer_launch_public_event_release_and_fanout_keep_exact_producer_custody() {
    let mut f = Fixture::new(4);
    let producer = f
        .launch(
            0,
            vec![span(f.allocations[0], RuntimeAccessV1::Write, 8, 16)],
            &[],
        )
        .unwrap();
    let event = f.context.record_event(&producer).unwrap();
    let mut first = f
        .launch(
            1,
            vec![region(f.allocations[0], RuntimeAccessV1::Read, 12)],
            &[event],
        )
        .unwrap();
    let mut second = f
        .launch(
            2,
            vec![region(f.allocations[0], RuntimeAccessV1::Read, 12)],
            &[event],
        )
        .unwrap();
    f.context.release_event(event).unwrap();
    assert!(
        !f.context
            .backend
            .producer_launch
            .events
            .values()
            .any(|id| *id == producer.backend_submission)
    );
    assert_eq!(f.context.submissions[&producer.id].dependency_retains, 2);
    assert!(f.context.unload_module(f.module).is_err());
    f.complete(&mut first);
    assert_eq!(f.context.submissions[&producer.id].dependency_retains, 1);
    assert!(f.context.release_submission_ref(&producer, None).is_err());
    f.context.poll(&mut first).unwrap();
    assert_eq!(f.context.submissions[&producer.id].dependency_retains, 1);
    f.complete(&mut second);
    assert_eq!(f.context.submissions[&producer.id].dependency_retains, 0);
    f.context.release_submission(producer).unwrap();
    assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn producer_launch_initial_failures_preserve_diagnostics_and_uncertain_mixed_roots() {
    for failure in [
        MockMemoryFailure::Rejected,
        MockMemoryFailure::Quiescent,
        MockMemoryFailure::Terminal,
        MockMemoryFailure::Panic,
    ] {
        let mut f = Fixture::new(4);
        let (producers, events) = f.producers();
        f.context.backend.launch_failure = failure;
        let result = catch_unwind(AssertUnwindSafe(|| f.launch(2, f.mixed(true), &events)));
        match failure {
            MockMemoryFailure::Rejected => assert!(matches!(
                result.unwrap(),
                Err(RuntimeErrorV1::BackendRejected(MockError(
                    "allocation rejected"
                )))
            )),
            MockMemoryFailure::Quiescent => assert!(matches!(
                result.unwrap(),
                Err(RuntimeErrorV1::BackendQuiescent(MockError(
                    "allocation quiescent failure"
                )))
            )),
            MockMemoryFailure::Terminal => assert!(matches!(
                result.unwrap(),
                Err(RuntimeErrorV1::BackendTerminal(MockError(
                    "allocation terminal failure"
                )))
            )),
            MockMemoryFailure::Panic => assert!(result.is_err()),
            MockMemoryFailure::None => unreachable!(),
        }
        let retained = matches!(
            failure,
            MockMemoryFailure::Terminal | MockMemoryFailure::Panic
        );
        assert_eq!(
            f.context.version_journal_read_records_v1(),
            Some(if retained { 3 } else { 0 })
        );
        for producer in &producers {
            assert_eq!(
                f.context.submissions[&producer.id].dependency_retains,
                usize::from(retained)
            );
        }
        assert!(f.context.backend.observed_kernel_reads.is_empty());
        assert_eq!(f.context.terminal, retained);
        if !retained {
            assert!(f.context.cleanup().is_complete());
        }
    }
}

#[test]
fn producer_launch_non_success_observations_release_only_quiescent_mixed_inputs() {
    for observation in [
        Observation::Pending,
        Observation::Failed,
        Observation::Rejected,
        Observation::Quiescent,
        Observation::Terminal,
        Observation::Panic,
    ] {
        let mut f = Fixture::new(4);
        let (producers, events) = f.producers();
        let mut consumer = f.launch(2, f.mixed(true), &events).unwrap();
        f.context
            .backend
            .producer_launch
            .observations
            .insert(consumer.backend_submission, observation);
        let result = catch_unwind(AssertUnwindSafe(|| f.context.poll(&mut consumer)));
        match observation {
            Observation::Pending => assert!(matches!(result.unwrap(), Ok(RuntimePollV1::Pending))),
            Observation::Failed => assert!(matches!(
                result.unwrap(),
                Ok(RuntimePollV1::Failed { code: 7 })
            )),
            Observation::Rejected => assert!(matches!(
                result.unwrap(),
                Err(RuntimeErrorV1::BackendRejected(MockError(
                    "producer launch rejected"
                )))
            )),
            Observation::Quiescent => assert!(matches!(
                result.unwrap(),
                Err(RuntimeErrorV1::BackendQuiescent(MockError(
                    "producer launch quiescent"
                )))
            )),
            Observation::Terminal => assert!(matches!(
                result.unwrap(),
                Err(RuntimeErrorV1::BackendTerminal(MockError(
                    "producer launch terminal"
                )))
            )),
            Observation::Panic => assert!(result.is_err()),
        }
        let released = matches!(observation, Observation::Failed | Observation::Quiescent);
        assert_eq!(
            f.context.version_journal_read_records_v1(),
            Some(if released { 0 } else { 3 })
        );
        for producer in &producers {
            assert_eq!(
                f.context.submissions[&producer.id].dependency_retains,
                usize::from(!released)
            );
        }
        assert_eq!(f.context.backend.producer_launch.calls.len(), 1);
        assert!(f.context.backend.observed_kernel_reads.is_empty());
        if !matches!(observation, Observation::Terminal | Observation::Panic) {
            assert!(f.context.cleanup().is_complete());
        }
    }
}

#[test]
fn producer_launch_all_completion_ingresses_share_bounded_producer_first_reconciliation() {
    for ingress in 0..7 {
        let mut f = Fixture::new(4);
        let (producers, events) = f.producers();
        let mut consumer = f.launch(2, f.mixed(true), &events).unwrap();
        let completion = f.context.record_event(&consumer).unwrap();
        for event in events {
            f.context.release_event(event).unwrap();
        }
        let callbacks = Arc::new(Mutex::new(Vec::new()));
        for submission in [&producers[0], &producers[1], &consumer] {
            let calls = callbacks.clone();
            let id = submission.id;
            f.context
                .on_completion(submission, move |status| {
                    calls.lock().unwrap().push((id, status))
                })
                .unwrap();
        }
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
            assert_eq!(f.context.backend.producer_launch.calls.len(), step + 1);
            assert_eq!(
                f.context.backend.producer_launch.calls[step].1,
                if step == 0 {
                    consumer.backend_submission
                } else {
                    producers[step - 1].backend_submission
                }
            );
            assert_eq!(
                f.context.query_submission(&consumer).unwrap(),
                if step == 2 {
                    RuntimeCompletionStatusV1::Succeeded
                } else {
                    RuntimeCompletionStatusV1::Pending
                }
            );
        }
        assert_eq!(
            callbacks
                .lock()
                .unwrap()
                .iter()
                .map(|entry| entry.0)
                .collect::<Vec<_>>(),
            vec![producers[0].id, producers[1].id, consumer.id]
        );
        f.assert_mixed_bytes(consumer.backend_submission, &producers);
        assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
        assert_eq!(f.context.backend.flush_call_count, 0);
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn producer_launch_retained_consumer_success_cannot_override_producer_results() {
    for observation in [
        Observation::Pending,
        Observation::Failed,
        Observation::Rejected,
        Observation::Terminal,
        Observation::Panic,
    ] {
        let mut f = Fixture::new(4);
        let (producers, events) = f.producers();
        let mut consumer = f.launch(2, f.mixed(false), &events).unwrap();
        assert_eq!(
            f.context.poll(&mut consumer).unwrap(),
            RuntimePollV1::Pending
        );
        f.context
            .backend
            .producer_launch
            .observations
            .insert(producers[0].backend_submission, observation);
        let result = catch_unwind(AssertUnwindSafe(|| f.context.poll(&mut consumer)));
        if matches!(observation, Observation::Panic) {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err());
        }
        assert!(f.context.terminal);
        assert_eq!(
            f.context.query_submission(&consumer).unwrap(),
            RuntimeCompletionStatusV1::Pending
        );
        assert_eq!(f.context.version_journal_read_records_v1(), Some(3));
        for producer in &producers {
            assert_eq!(f.context.submissions[&producer.id].dependency_retains, 1);
        }
    }
}

#[test]
fn producer_launch_discarded_producer_result_preserves_quiescence_without_success() {
    for writable in [false, true] {
        for discarded in 0..2 {
            let mut f = Fixture::new(4);
            let (mut producers, events) = f.producers();
            let mut consumer = f.launch(2, f.mixed(writable), &events).unwrap();
            for event in events {
                f.context.release_event(event).unwrap();
            }
            let callbacks = Arc::new(Mutex::new(Vec::new()));
            for submission in [&producers[0], &producers[1], &consumer] {
                let calls = callbacks.clone();
                let id = submission.id;
                f.context
                    .on_completion(submission, move |status| {
                        calls.lock().unwrap().push((id, status))
                    })
                    .unwrap();
            }
            assert_eq!(
                f.context.poll(&mut consumer).unwrap(),
                RuntimePollV1::Pending
            );
            for submission in [&producers[0], &producers[1], &consumer] {
                assert_eq!(
                    f.context.backend.producer_launch.completed[&submission.backend_submission],
                    BackendPollV1::Succeeded
                );
            }
            f.assert_mixed_bytes(consumer.backend_submission, &producers);
            for _ in 0..discarded {
                assert_eq!(
                    f.context.poll(&mut consumer).unwrap(),
                    RuntimePollV1::Pending
                );
            }
            f.context.backend.producer_launch.observations.insert(
                producers[discarded].backend_submission,
                Observation::Quiescent,
            );
            assert!(matches!(
                f.context.poll(&mut consumer),
                Err(RuntimeErrorV1::BackendQuiescent(MockError(
                    "producer launch quiescent"
                )))
            ));
            let unknown = RuntimeCompletionStatusV1::QuiescentWithoutResult;
            assert_eq!(f.context.query_submission(&consumer).unwrap(), unknown);
            assert!(f.context.retained_directed_success_v1(consumer.id));
            assert_eq!(
                f.context.query_submission(&producers[discarded]).unwrap(),
                unknown
            );
            assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
            assert!(!f.context.terminal);
            for producer in &producers {
                assert_eq!(f.context.submissions[&producer.id].dependency_retains, 0);
            }
            assert_eq!(
                writer_state(&f.context, f.allocations[discarded]),
                ContextWriterStateV1::Unknown { member_count: 1 }
            );
            if writable {
                assert_eq!(
                    writer_state(&f.context, f.allocations[3]),
                    ContextWriterStateV1::Unknown { member_count: 1 }
                );
            }
            let mut expected: Vec<_> = producers[..discarded]
                .iter()
                .map(|producer| (producer.id, RuntimeCompletionStatusV1::Succeeded))
                .collect();
            expected.extend([(producers[discarded].id, unknown), (consumer.id, unknown)]);
            assert_eq!(*callbacks.lock().unwrap(), expected);
            let calls = f.context.backend.producer_launch.calls.len();
            for submission in [&mut producers[discarded], &mut consumer] {
                assert_eq!(
                    f.context.poll(submission).unwrap(),
                    RuntimePollV1::Failed {
                        code: RUNTIME_QUIESCENT_WITHOUT_RESULT_CODE_V1
                    }
                );
            }
            assert_eq!(f.context.backend.producer_launch.calls.len(), calls);
            assert_eq!(*callbacks.lock().unwrap(), expected);
            for producer in &mut producers[discarded + 1..] {
                assert_eq!(
                    f.context.query_submission(producer).unwrap(),
                    RuntimeCompletionStatusV1::Pending
                );
                f.complete(producer);
                expected.push((producer.id, RuntimeCompletionStatusV1::Succeeded));
            }
            assert_eq!(*callbacks.lock().unwrap(), expected);
            assert_eq!(
                f.context.version_journal_writer_records_v1(),
                Some(1 + usize::from(writable))
            );
            assert!(f.context.cleanup().is_complete());
        }
    }
}

#[test]
fn producer_launch_resolved_reservation_survives_writer_slot_reuse() {
    for cancel in [false, true] {
        let mut f = Fixture::new(3);
        let mut producer = f
            .launch(
                0,
                vec![span(f.allocations[0], RuntimeAccessV1::Write, 8, 16)],
                &[],
            )
            .unwrap();
        let event = f.context.record_event(&producer).unwrap();
        let mut consumer = f
            .launch(
                1,
                vec![region(f.allocations[0], RuntimeAccessV1::Read, 12)],
                &[event],
            )
            .unwrap();
        let old = f.context.submissions[&producer.id].journal_writer.unwrap();
        let marker = f.context.submissions[&consumer.id]
            .journal_producer_read
            .unwrap();
        if cancel {
            f.context.backend.cancel_before_publication = true;
            f.context.cancel(&mut producer).unwrap();
        } else {
            f.complete(&mut producer);
        }
        let unrelated = f
            .launch(
                2,
                vec![region(f.allocations[4], RuntimeAccessV1::Write, 0)],
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
                RuntimePollV1::Failed { code: 7 }
            ));
            assert!(
                f.context
                    .backend
                    .observed_kernel_reads
                    .iter()
                    .all(|read| read.submission != consumer.backend_submission)
            );
        } else {
            f.complete(&mut consumer);
        }
        assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn producer_launch_invalid_handles_retain_new_roots_and_every_mixed_input() {
    for duplicate in [false, true] {
        let mut f = Fixture::new(4);
        let (producers, events) = f.producers();
        for producer in &producers {
            f.context
                .on_completion(producer, |_| panic!("ambiguous handle must not notify"))
                .unwrap();
        }
        let handle = if duplicate {
            producers[0].backend_submission
        } else {
            0
        };
        let local = f.context.next_identity;
        f.context.backend.handle_override = Some((MockHandleKind::Submission, handle));
        assert!(matches!(
            f.launch(2, f.mixed(true), &events),
            Err(RuntimeErrorV1::BackendProtocol(_))
        ));
        let id = RuntimeSubmissionIdV1::new(f.context.context_generation, local);
        assert!(f.context.terminal);
        assert!(f.context.producer_launches.contains_key(&id));
        let record = f.context.submissions[&id];
        assert!(record.producer_launch);
        assert_eq!(record.journal_read.unwrap().count, 1);
        assert_eq!(record.journal_producer_read.unwrap().count, 2);
        assert!(record.journal_writer.is_some());
        assert_eq!(f.context.version_journal_read_records_v1(), Some(3));
        for producer in &producers {
            assert_eq!(f.context.submissions[&producer.id].dependency_retains, 1);
        }
        assert!(f.context.backend.observed_kernel_reads.is_empty());
        let pending = f.context.backend.pending_kernel_reads.clone();
        for _ in 0..2 {
            let report = f.context.cleanup();
            assert!(!report.is_complete());
            assert_eq!(report.reader_journal_records_v1(), 3);
            assert_eq!(f.context.backend.pending_kernel_reads, pending);
        }
        assert_eq!(f.context.completion_callback_panic_count(), 0);
        assert!(f.context.backend.cleanup_log.is_empty());
    }
}

#[test]
fn producer_launch_cancellation_keeps_or_releases_the_complete_mixed_roster() {
    for writable in [false, true] {
        for case in 0..5 {
            let mut f = Fixture::new(4);
            let (producers, events) = f.producers();
            let mut consumer = f.launch(2, f.mixed(writable), &events).unwrap();
            assert_eq!(
                f.context.submissions[&consumer.id].journal_writer.is_some(),
                writable
            );
            if case == 0 {
                f.context
                    .backend
                    .producer_launch
                    .observations
                    .insert(consumer.backend_submission, Observation::Pending);
                assert_eq!(
                    f.context.poll(&mut consumer).unwrap(),
                    RuntimePollV1::Pending
                );
            }
            if case == 3 {
                assert_eq!(
                    f.context.poll(&mut consumer).unwrap(),
                    RuntimePollV1::Pending
                );
            }
            f.context.backend.cancel_before_publication = case == 2 || case == 3;
            if case == 1 {
                f.context.backend.cancel_failure = MockMemoryFailure::Rejected;
            }
            if case == 4 {
                f.context.backend.cancel_failure = MockMemoryFailure::Quiescent;
            }
            let result = f.context.cancel(&mut consumer);
            match case {
                0 | 3 => assert!(matches!(result, Ok(RuntimeCancellationV1::TooLate))),
                1 => assert!(matches!(
                    result,
                    Err(RuntimeErrorV1::BackendRejected(MockError(
                        "allocation rejected"
                    )))
                )),
                2 => assert!(matches!(result, Ok(RuntimeCancellationV1::Cancelled))),
                4 => assert!(matches!(
                    result,
                    Err(RuntimeErrorV1::BackendQuiescent(MockError(
                        "allocation quiescent failure"
                    )))
                )),
                _ => unreachable!(),
            }
            let released = case == 2 || case == 4;
            assert_eq!(f.context.backend.cancel_call_count, usize::from(case != 3));
            assert_eq!(
                f.context.version_journal_read_records_v1(),
                Some(if released { 0 } else { 3 })
            );
            for producer in &producers {
                assert_eq!(
                    f.context.submissions[&producer.id].dependency_retains,
                    usize::from(!released)
                );
            }
            if case != 3 {
                assert!(f.context.backend.observed_kernel_reads.is_empty());
            }
            assert!(!f.context.terminal);
            assert!(f.context.cleanup().is_complete());
        }
    }
}

#[test]
fn producer_launch_root_flag_dependency_and_late_marker_loss_seal_before_observation() {
    for late in [false, true] {
        for corruption in 0..4 {
            let mut f = Fixture::new(4);
            let (producers, events) = f.producers();
            let mut consumer = f.launch(2, f.mixed(true), &events).unwrap();
            let callbacks = Arc::new(Mutex::new(Vec::new()));
            for submission in [&producers[0], &producers[1], &consumer] {
                let calls = callbacks.clone();
                f.context
                    .on_completion(submission, move |status| calls.lock().unwrap().push(status))
                    .unwrap();
            }
            if late {
                assert_eq!(
                    f.context.poll(&mut consumer).unwrap(),
                    RuntimePollV1::Pending
                );
            }
            match corruption {
                0 => {
                    f.context.producer_launches.remove(&consumer.id);
                }
                1 => {
                    f.context
                        .submissions
                        .get_mut(&consumer.id)
                        .unwrap()
                        .producer_launch = false;
                }
                2 => {
                    f.context
                        .producer_launches
                        .get_mut(&consumer.id)
                        .unwrap()
                        .dependencies
                        .pop();
                }
                3 => {
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
            assert!(f.context.terminal);
            assert_eq!(f.context.version_journal_read_records_v1(), Some(3));
            assert!(f.context.submissions[&consumer.id].journal_read.is_some());
            assert!(
                f.context.submissions[&consumer.id]
                    .journal_producer_read
                    .is_some()
            );
            for producer in &producers {
                assert_eq!(f.context.submissions[&producer.id].dependency_retains, 1);
            }
            assert!(callbacks.lock().unwrap().is_empty());
        }
    }
}

#[test]
fn producer_launch_discarded_observer_keeps_inputs_until_consumer_stream_quiesces() {
    for writable in [false, true] {
        for quiescent_error in [false, true] {
            let mut f = Fixture::new(4);
            let (mut producers, events) = f.producers();
            let callbacks = Arc::new(Mutex::new(Vec::new()));
            let (id, backend_id) = {
                let consumer = f.launch(2, f.mixed(writable), &events).unwrap();
                let calls = callbacks.clone();
                f.context
                    .on_completion(&consumer, move |status| calls.lock().unwrap().push(status))
                    .unwrap();
                (consumer.id, consumer.backend_submission)
            };
            for event in events {
                f.context.release_event(event).unwrap();
            }
            assert_eq!(f.context.version_journal_read_records_v1(), Some(3));
            for producer in &producers {
                assert_eq!(f.context.submissions[&producer.id].dependency_retains, 1);
            }
            assert!(f.context.unload_module(f.module).is_err());
            assert!(f.context.backend.observed_kernel_reads.is_empty());
            if quiescent_error {
                f.context.backend.cleanup_failure = MockCleanupFailure::QuiescentStreamOnce;
            }
            let result = f.context.destroy_stream(f.streams[2]);
            if quiescent_error {
                assert!(matches!(
                    result,
                    Err(RuntimeErrorV1::BackendQuiescent(MockError(
                        "destroy failed after quiescence"
                    )))
                ));
            } else {
                result.unwrap();
            }
            let record = f.context.submissions[&id];
            assert_eq!(
                record.status,
                RuntimeCompletionStatusV1::QuiescentWithoutResult
            );
            assert!(record.quiescent);
            assert!(record.journal_read.is_none());
            assert!(record.journal_producer_read.is_none());
            assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
            assert!(!f.context.producer_launches[&id].dependencies_held);
            assert_eq!(
                callbacks.lock().unwrap().as_slice(),
                &[RuntimeCompletionStatusV1::QuiescentWithoutResult]
            );
            assert_eq!(
                f.context.backend.producer_launch.completed[&backend_id],
                BackendPollV1::Succeeded
            );
            f.assert_mixed_bytes(backend_id, &producers);
            for producer in &mut producers {
                assert_eq!(f.context.submissions[&producer.id].dependency_retains, 0);
                f.complete(producer);
            }
            assert_eq!(
                f.context.version_journal_writer_records_v1(),
                Some(usize::from(writable))
            );
            f.context.unload_module(f.module).unwrap();
            assert!(!f.context.modules.contains_key(&f.module));
            assert!(f.context.cleanup().is_complete());
            assert_eq!(callbacks.lock().unwrap().len(), 1);
            assert!(!f.context.terminal);
        }
    }
}
