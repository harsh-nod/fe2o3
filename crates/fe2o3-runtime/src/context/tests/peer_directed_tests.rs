use super::*;
use std::sync::{Arc, Mutex};

type Context = RuntimeContextV1<MockBackend>;
type Submission = RuntimeSubmissionV1<RuntimeDirectedScalarPeerCopyV1>;

#[derive(Clone, Copy, Debug)]
pub(super) enum Observation {
    Pending,
    Failed,
    Rejected,
    Quiescent,
    Terminal,
    Panic,
}

impl MockBackend {
    pub(super) fn observe_directed_test_v1(
        &mut self,
        kind: &'static str,
        id: u64,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<MockError>> {
        self.directed_calls.push((kind, id));
        match self.directed_observations.remove(&id) {
            Some(Observation::Pending) => return Ok(BackendPollV1::Pending),
            Some(Observation::Failed) => {
                self.finish_submission(id, false);
                return Ok(BackendPollV1::Failed { code: 7 });
            }
            Some(Observation::Rejected) => {
                return Err(RuntimeBackendFailureV1::Rejected(MockError(
                    "directed rejected",
                )));
            }
            Some(Observation::Quiescent) => {
                self.finish_submission(id, false);
                return Err(RuntimeBackendFailureV1::Quiescent(MockError(
                    "directed quiescent",
                )));
            }
            Some(Observation::Terminal) => {
                return Err(RuntimeBackendFailureV1::Terminal(MockError(
                    "directed terminal",
                )));
            }
            Some(Observation::Panic) => panic!("directed observation panic"),
            None => {}
        }
        // Script physical producer-first completion without synthesizing Context observations.
        let mut stack = vec![(id, false)];
        while let Some((current, visited)) = stack.pop() {
            if visited {
                self.finish_submission(current, true);
                continue;
            }
            stack.push((current, true));
            for dependency in &self.directed_routes[&current].1 {
                if self
                    .pending_copies
                    .contains_key(&dependency.producer_submission)
                {
                    stack.push((dependency.producer_submission, false));
                }
            }
        }
        Ok(BackendPollV1::Succeeded)
    }
}

impl RuntimeDirectedScalarPeerCopyBackendV1 for MockBackend {
    fn submit_directed_scalar_peer_copy_v1(
        &mut self,
        request: BackendDirectedScalarPeerCopyV1<'_>,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        let events: Vec<_> = request
            .dependencies
            .iter()
            .map(|dependency| dependency.event)
            .collect();
        let id = self.submit_copy(
            request.route.stream,
            request.route.source,
            request.route.destination,
            &events,
        )?;
        self.directed_routes
            .insert(id, (request.route, request.dependencies.to_vec()));
        Ok(id)
    }

    fn progress_directed_scalar_peer_copy_v1(
        &mut self,
        request: BackendDirectedScalarProgressV1<'_>,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>> {
        let (route, dependencies) = &self.directed_routes[&request.submission];
        assert_eq!(*route, request.route);
        assert_eq!(
            dependencies
                .iter()
                .map(|dep| dep.producer_submission)
                .collect::<Vec<_>>(),
            request.producer_submissions
        );
        self.observe_directed_test_v1("directed", request.submission)
    }
}

struct Fixture {
    context: Context,
    streams: [RuntimeStreamIdV1; 2],
    allocations: Vec<RuntimeAllocationIdV1>,
}

fn region(allocation: RuntimeAllocationIdV1, access: RuntimeAccessV1) -> RuntimeMemoryRegionV1 {
    RuntimeMemoryRegionV1 {
        allocation,
        access,
        byte_offset: 8,
        byte_len: 16,
    }
}

impl Fixture {
    fn new(journal: bool, count: usize) -> Self {
        Self::with_budget(journal, count, count + 4)
    }

    fn with_budget(journal: bool, count: usize, writers: usize) -> Self {
        let backend = MockBackend {
            next: 100,
            deferred_copies: true,
            ..MockBackend::default()
        };
        let mut context = if journal {
            Context::open_with_version_journal_v1(backend, count + 4, writers).unwrap()
        } else {
            Context::open(backend).unwrap()
        };
        let devices = [context.devices()[0].id(), context.devices()[1].id()];
        let streams = [
            context.create_stream(devices[0]).unwrap(),
            context.create_stream(devices[1]).unwrap(),
        ];
        let allocations = (0..count)
            .map(|index| {
                context
                    .allocate(devices[index % 2], RuntimeMemoryKindV1::DeviceLocal, 64, 16)
                    .unwrap()
            })
            .collect::<Vec<_>>();
        context
            .write_allocation(allocations[0], 0, &[0x51; 64])
            .unwrap();
        Self {
            context,
            streams,
            allocations,
        }
    }

    fn copy(
        &mut self,
        source: usize,
        destination: usize,
        dependencies: &[RuntimeEventIdV1],
    ) -> Result<Submission, RuntimeErrorV1<MockError>> {
        self.context.directed_peer_copy_v1(
            self.streams[destination % 2],
            region(self.allocations[source], RuntimeAccessV1::Read),
            region(self.allocations[destination], RuntimeAccessV1::Write),
            dependencies,
        )
    }

    fn pair(&mut self) -> (Submission, Submission, RuntimeEventIdV1) {
        let producer = self.copy(0, 1, &[]).unwrap();
        let event = self.context.record_event(&producer).unwrap();
        let consumer = self.copy(1, 2, &[event]).unwrap();
        (producer, consumer, event)
    }
}

#[test]
fn directed_all_observation_ingresses_reconcile_exact_cross_stream_producer() {
    for journal in [false, true] {
        for ingress in 0..8 {
            let mut f = Fixture::new(journal, 3);
            let (producer, mut consumer, event) = f.pair();
            let completion = f.context.record_event(&consumer).unwrap();
            f.context.release_event(event).unwrap();
            let callbacks = Arc::new(Mutex::new(Vec::new()));
            for submission in [&producer, &consumer] {
                let calls = callbacks.clone();
                let id = submission.id;
                f.context
                    .on_completion(submission, move |status| {
                        calls.lock().unwrap().push((id, status))
                    })
                    .unwrap();
            }
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
                            .synchronize_stream(f.streams[0], Duration::ZERO)
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
                    7 => {
                        f.context
                            .progress_directed_peer_copy_v1(&mut consumer)
                            .unwrap();
                    }
                    _ => unreachable!(),
                }
                assert_eq!(f.context.backend.directed_calls.len(), step + 1);
                assert_eq!(
                    f.context.backend.directed_calls[step].1,
                    if step == 0 {
                        consumer.backend_submission
                    } else {
                        producer.backend_submission
                    }
                );
                assert_eq!(
                    f.context.query_submission(&consumer).unwrap(),
                    if step == 0 {
                        RuntimeCompletionStatusV1::Pending
                    } else {
                        RuntimeCompletionStatusV1::Succeeded
                    }
                );
                assert_eq!(
                    callbacks.lock().unwrap().len(),
                    if step == 0 { 0 } else { 2 }
                );
            }
            assert_eq!(callbacks.lock().unwrap()[0].0, producer.id);
            assert_eq!(
                f.context.version_journal_read_records_v1(),
                journal.then_some(0)
            );
            let mut bytes = [0; 16];
            f.context
                .read_allocation(f.allocations[2], 8, &mut bytes)
                .unwrap();
            assert_eq!(bytes, [0x51; 16]);
            f.context.poll(&mut consumer).unwrap();
            assert_eq!(f.context.backend.directed_calls.len(), 2);
            assert_eq!(f.context.backend.flush_call_count, 0);
            assert!(f.context.cleanup().is_complete());
        }
    }
}

#[test]
fn directed_pending_source_requires_exact_producer_event_and_covered_range() {
    let mut f = Fixture::new(true, 5);
    let producer = f.copy(0, 1, &[]).unwrap();
    let event = f.context.record_event(&producer).unwrap();
    let other = f.copy(0, 3, &[]).unwrap();
    let wrong = f.context.record_event(&other).unwrap();
    let before = f.context.backend.copy_call_count;
    for dependencies in [&[][..], &[wrong][..]] {
        assert!(matches!(
            f.copy(1, 2, dependencies),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::ContextReserved
            ))
        ));
    }
    let mut source = region(f.allocations[1], RuntimeAccessV1::Read);
    source.byte_offset = 0;
    assert!(matches!(
        f.context.directed_peer_copy_v1(
            f.streams[0],
            source,
            region(f.allocations[2], RuntimeAccessV1::Write),
            &[event]
        ),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::ContextReserved
        ))
    ));
    assert_eq!(f.context.backend.copy_call_count, before);
    let mut consumer = f.copy(1, 2, &[wrong, event]).unwrap();
    while f.context.query_submission(&consumer).unwrap() == RuntimeCompletionStatusV1::Pending {
        f.context
            .progress_directed_peer_copy_v1(&mut consumer)
            .unwrap();
    }
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn directed_aliases_and_legacy_dependencies_reject_before_backend() {
    let mut f = Fixture::new(true, 3);
    let producer = f.copy(0, 1, &[]).unwrap();
    let a = f.context.record_event(&producer).unwrap();
    let b = f.context.record_event(&producer).unwrap();
    assert!(matches!(
        f.copy(1, 2, &[a, b]),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::DuplicateDependency
        ))
    ));
    assert_eq!(f.context.backend.copy_call_count, 1);
    let mut f = Fixture::new(true, 3);
    let legacy = f
        .context
        .peer_copy(
            f.streams[1],
            region(f.allocations[0], RuntimeAccessV1::Read),
            region(f.allocations[1], RuntimeAccessV1::Write),
            &[],
        )
        .unwrap();
    let event = f.context.record_event(&legacy).unwrap();
    assert!(matches!(
        f.copy(1, 2, &[event]),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::Unsupported
        ))
    ));
    assert_eq!(f.context.backend.copy_call_count, 1);
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn directed_three_link_chain_resolves_inputs_and_full_rosters_once() {
    let mut f = Fixture::new(true, 5);
    let (first, second, a) = f.pair();
    let b = f.context.record_event(&second).unwrap();
    let mut third = f.copy(2, 3, &[b, a]).unwrap();
    f.context.release_event(a).unwrap();
    f.context.release_event(b).unwrap();
    assert_eq!(f.context.version_journal_read_records_v1(), Some(3));
    for _ in 0..3 {
        f.context
            .progress_directed_peer_copy_v1(&mut third)
            .unwrap();
    }
    assert_eq!(
        f.context.query_submission(&third).unwrap(),
        RuntimeCompletionStatusV1::Succeeded
    );
    assert_eq!(f.context.submissions[&first.id].dependency_retains, 0);
    assert_eq!(f.context.submissions[&second.id].dependency_retains, 0);
    assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn directed_non_success_does_not_wait_for_pending_producer() {
    for observation in [Observation::Failed, Observation::Quiescent] {
        let mut f = Fixture::new(true, 3);
        let (producer, mut consumer, _) = f.pair();
        f.context
            .backend
            .directed_observations
            .insert(consumer.backend_submission, observation);
        let result = f.context.progress_directed_peer_copy_v1(&mut consumer);
        if matches!(observation, Observation::Quiescent) {
            assert!(matches!(result, Err(RuntimeErrorV1::BackendQuiescent(_))));
        } else {
            result.unwrap();
        }
        assert!(f.context.query_submission(&consumer).unwrap().is_terminal());
        assert_eq!(
            f.context.query_submission(&producer).unwrap(),
            RuntimeCompletionStatusV1::Pending
        );
        assert_eq!(f.context.submissions[&producer.id].dependency_retains, 0);
        assert_eq!(f.context.version_journal_read_records_v1(), Some(1));
        assert_eq!(f.context.backend.directed_calls.len(), 1);
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn directed_cancel_releases_pending_input_but_retained_success_is_too_late() {
    for early_success in [false, true] {
        let mut f = Fixture::new(true, 3);
        let (producer, mut consumer, _) = f.pair();
        if early_success {
            f.context
                .progress_directed_peer_copy_v1(&mut consumer)
                .unwrap();
        }
        f.context.backend.cancel_before_publication = true;
        assert_eq!(
            f.context.cancel(&mut consumer).unwrap(),
            if early_success {
                RuntimeCancellationV1::TooLate
            } else {
                RuntimeCancellationV1::Cancelled
            }
        );
        assert_eq!(
            f.context.backend.cancel_call_count,
            usize::from(!early_success)
        );
        assert_eq!(
            f.context.submissions[&producer.id].dependency_retains,
            usize::from(early_success)
        );
        assert_eq!(
            f.context.version_journal_read_records_v1(),
            Some(if early_success { 2 } else { 1 })
        );
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn directed_parent_success_contradictions_seal_and_retain() {
    for observation in [
        Observation::Pending,
        Observation::Failed,
        Observation::Rejected,
        Observation::Terminal,
        Observation::Panic,
    ] {
        let mut f = Fixture::new(true, 3);
        let (producer, mut consumer, _) = f.pair();
        f.context
            .progress_directed_peer_copy_v1(&mut consumer)
            .unwrap();
        f.context
            .backend
            .directed_observations
            .insert(producer.backend_submission, observation);
        let result = catch_unwind(AssertUnwindSafe(|| {
            f.context.progress_directed_peer_copy_v1(&mut consumer)
        }));
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
        assert!(f.context.retained_directed_success_v1(consumer.id));
        assert_eq!(f.context.submissions[&producer.id].dependency_retains, 1);
        assert_eq!(f.context.version_journal_read_records_v1(), Some(2));
    }
}

#[test]
fn directed_discarded_producer_result_never_becomes_success() {
    for destroy in [false, true] {
        let mut f = Fixture::new(true, 3);
        let (producer, mut consumer, _) = f.pair();
        if destroy {
            f.context.destroy_stream(f.streams[1]).unwrap();
        }
        f.context
            .progress_directed_peer_copy_v1(&mut consumer)
            .unwrap();
        if !destroy {
            f.context
                .backend
                .directed_observations
                .insert(producer.backend_submission, Observation::Quiescent);
            assert!(matches!(
                f.context.progress_directed_peer_copy_v1(&mut consumer),
                Err(RuntimeErrorV1::BackendQuiescent(_))
            ));
        }
        assert_eq!(
            f.context.query_submission(&consumer).unwrap(),
            RuntimeCompletionStatusV1::QuiescentWithoutResult
        );
        assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
        assert!(!f.context.terminal);
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn directed_initial_failure_releases_only_definitely_quiescent_inputs() {
    for failure in [
        MockMemoryFailure::Rejected,
        MockMemoryFailure::Quiescent,
        MockMemoryFailure::Terminal,
        MockMemoryFailure::Panic,
    ] {
        let mut f = Fixture::new(true, 3);
        let producer = f.copy(0, 1, &[]).unwrap();
        let event = f.context.record_event(&producer).unwrap();
        f.context.backend.copy_failure = failure;
        let result = catch_unwind(AssertUnwindSafe(|| f.copy(1, 2, &[event])));
        if failure == MockMemoryFailure::Panic {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err());
        }
        let retained = matches!(
            failure,
            MockMemoryFailure::Terminal | MockMemoryFailure::Panic
        );
        assert_eq!(
            f.context.submissions[&producer.id].dependency_retains,
            usize::from(retained)
        );
        assert_eq!(
            f.context.version_journal_read_records_v1(),
            Some(if retained { 2 } else { 1 })
        );
        if !retained {
            assert!(f.context.cleanup().is_complete());
        }
    }
}

#[test]
fn directed_success_cannot_bypass_reconciliation_via_generic_transition() {
    let mut f = Fixture::new(true, 3);
    let (producer, consumer, _) = f.pair();
    assert_eq!(
        f.context
            .transition_submission_status(consumer.id, RuntimeCompletionStatusV1::Succeeded),
        Err(RuntimeValidationErrorV1::InvalidBackendDescription)
    );
    assert!(f.context.terminal);
    assert_eq!(f.context.submissions[&producer.id].dependency_retains, 1);
    assert_eq!(f.context.version_journal_read_records_v1(), Some(2));
}

#[test]
fn directed_depth_limit_and_long_chain_make_bounded_progress() {
    let mut f = Fixture::new(true, 258);
    let mut event = None;
    let mut submissions = Vec::new();
    for index in 0..256 {
        let submission = f.copy(index, index + 1, event.as_slice()).unwrap();
        event = Some(f.context.record_event(&submission).unwrap());
        submissions.push(submission);
    }
    assert!(matches!(
        f.copy(256, 257, event.as_slice()),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::TooManyDependencies
        ))
    ));
    let last = submissions.last_mut().unwrap();
    for _ in 0..260 {
        let before = f.context.backend.directed_calls.len();
        f.context.progress_directed_peer_copy_v1(last).unwrap();
        assert!(f.context.backend.directed_calls.len() <= before + 1);
        if f.context.query_submission(last).unwrap().is_terminal() {
            break;
        }
    }
    assert_eq!(
        f.context.query_submission(last).unwrap(),
        RuntimeCompletionStatusV1::Succeeded
    );
    assert_eq!(f.context.backend.directed_calls.len(), 256);
    assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn directed_unknown_source_rejects_without_poisoning_context() {
    for observation in [Observation::Failed, Observation::Quiescent] {
        let mut f = Fixture::new(true, 3);
        let mut producer = f.copy(0, 1, &[]).unwrap();
        let event = f.context.record_event(&producer).unwrap();
        f.context
            .backend
            .directed_observations
            .insert(producer.backend_submission, observation);
        let _ = f.context.progress_directed_peer_copy_v1(&mut producer);
        assert!(matches!(
            f.copy(1, 2, &[event]),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::ContextReserved
            ))
        ));
        assert!(!f.context.terminal);
        assert_eq!(f.context.backend.copy_call_count, 1);
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn directed_corrupt_root_cursor_and_depth_reject_before_observation_or_cancel() {
    for mutation in 0..5 {
        for cancel in [false, true] {
            let mut f = Fixture::new(true, 3);
            let (producer, mut consumer, _) = f.pair();
            f.context
                .progress_directed_peer_copy_v1(&mut consumer)
                .unwrap();
            match mutation {
                0 => {
                    f.context
                        .scalar_peer_copies
                        .get_mut(&consumer.id)
                        .unwrap()
                        .directed
                        .as_mut()
                        .unwrap()
                        .cursor = 1;
                }
                1 => {
                    f.context
                        .scalar_peer_copies
                        .get_mut(&consumer.id)
                        .unwrap()
                        .directed
                        .as_mut()
                        .unwrap()
                        .depth += 1;
                }
                2 => {
                    f.context
                        .submissions
                        .get_mut(&consumer.id)
                        .unwrap()
                        .directed_peer_copy = false;
                }
                3 => {
                    f.context.scalar_peer_copies.remove(&consumer.id);
                }
                4 => {
                    f.context
                        .scalar_peer_copies
                        .get_mut(&consumer.id)
                        .unwrap()
                        .directed = None;
                }
                _ => unreachable!(),
            }
            if cancel {
                assert!(f.context.cancel(&mut consumer).is_err());
            } else {
                assert!(
                    f.context
                        .progress_directed_peer_copy_v1(&mut consumer)
                        .is_err()
                );
            }
            assert!(f.context.terminal);
            assert_eq!(f.context.backend.directed_calls.len(), 1);
            assert_eq!(f.context.backend.cancel_call_count, 0);
            assert_eq!(f.context.submissions[&producer.id].dependency_retains, 1);
            assert_eq!(f.context.version_journal_read_records_v1(), Some(2));
        }
    }
}

#[test]
fn directed_missing_reservation_root_or_marker_preserves_retained_success() {
    for remove_root in [false, true] {
        for retained_success in [false, true] {
            for cancel in [false, true] {
                let mut f = Fixture::new(true, 3);
                let (producer, mut consumer, _) = f.pair();
                if retained_success {
                    f.context
                        .progress_directed_peer_copy_v1(&mut consumer)
                        .unwrap();
                }
                if remove_root {
                    f.context
                        .versions
                        .as_mut()
                        .unwrap()
                        .remove_producer_read_root_for_test_v1(consumer.id);
                } else {
                    f.context
                        .submissions
                        .get_mut(&consumer.id)
                        .unwrap()
                        .journal_producer_read = None;
                }
                if cancel {
                    assert!(f.context.cancel(&mut consumer).is_err());
                } else {
                    assert!(
                        f.context
                            .progress_directed_peer_copy_v1(&mut consumer)
                            .is_err()
                    );
                }
                assert!(f.context.terminal);
                assert_eq!(
                    f.context.retained_directed_success_v1(consumer.id),
                    retained_success
                );
                assert_eq!(
                    f.context.backend.directed_calls.len(),
                    usize::from(retained_success)
                );
                assert_eq!(f.context.backend.cancel_call_count, 0);
                assert_eq!(
                    f.context.query_submission(&consumer).unwrap(),
                    RuntimeCompletionStatusV1::Pending
                );
                assert_eq!(f.context.submissions[&producer.id].dependency_retains, 1);
                assert_eq!(f.context.version_journal_read_records_v1(), Some(2));
            }
        }
    }
}

#[test]
fn directed_reservation_survives_success_and_no_effect_writer_slot_reuse() {
    use fe2o3_runtime_model::ContextProducerReadStatusV1;
    for cancel in [false, true] {
        let mut f = Fixture::new(true, 5);
        let (mut producer, mut consumer, _) = f.pair();
        let old = f.context.submissions[&producer.id].journal_writer.unwrap();
        let reservation = f.context.submissions[&consumer.id]
            .journal_producer_read
            .unwrap();
        if cancel {
            f.context.backend.cancel_before_publication = true;
            f.context.cancel(&mut producer).unwrap();
        } else {
            f.context
                .progress_directed_peer_copy_v1(&mut producer)
                .unwrap();
        }
        let unrelated = f.copy(0, 3, &[]).unwrap();
        let new = f.context.submissions[&unrelated.id].journal_writer.unwrap();
        assert_eq!(old.slot, new.slot);
        assert_ne!(old.key, new.key);
        assert_eq!(
            f.context
                .versions
                .as_mut()
                .unwrap()
                .read_leases_for_test_v1()
                .producer_read_status(reservation)
                .unwrap(),
            if cancel {
                ContextProducerReadStatusV1::NoEffect
            } else {
                ContextProducerReadStatusV1::Success
            }
        );
        if cancel {
            f.context
                .backend
                .directed_observations
                .insert(consumer.backend_submission, Observation::Failed);
        }
        f.context
            .progress_directed_peer_copy_v1(&mut consumer)
            .unwrap();
        assert_eq!(
            f.context.query_submission(&consumer).unwrap(),
            if cancel {
                RuntimeCompletionStatusV1::Failed(RuntimeCompletionFailureV1::BackendCode(7))
            } else {
                RuntimeCompletionStatusV1::Succeeded
            }
        );
        assert_eq!(f.context.version_journal_read_records_v1(), Some(1));
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn directed_stable_and_producer_inputs_share_one_read_budget() {
    let mut f = Fixture::with_budget(true, 5, 3);
    let extra_sources: Vec<_> = [2, 4]
        .map(|index| ContextReadSourceV1 {
            region: region(f.allocations[index], RuntimeAccessV1::Read),
            record: f.context.allocations[&f.allocations[index]],
        })
        .into();
    let extra = f
        .context
        .prepare_submission_readers_v1(&extra_sources)
        .unwrap();
    let extra_id =
        RuntimeSubmissionIdV1::new(f.context.context_generation, f.context.next_id().unwrap());
    f.context
        .begin_submission_readers_v1(
            extra_id,
            extra,
            versions::SubmissionWriterDomainV1::Ordinary,
        )
        .unwrap();
    let producer = f.copy(0, 1, &[]).unwrap();
    let event = f.context.record_event(&producer).unwrap();
    // Destination 2 is also read-reserved, so first remove that independent hazard
    // by targeting a new device-0 allocation outside the extra source roster.
    let device = f.context.devices()[0].id();
    let destination = f
        .context
        .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
        .unwrap();
    let before = (f.context.next_identity, f.context.backend.copy_call_count);
    assert!(matches!(
        f.context.directed_peer_copy_v1(
            f.streams[0],
            region(f.allocations[1], RuntimeAccessV1::Read),
            region(destination, RuntimeAccessV1::Write),
            &[event]
        ),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::Capacity
        ))
    ));
    assert_eq!(f.context.next_identity, before.0);
    assert_eq!(f.context.backend.copy_call_count, before.1);
    assert_eq!(f.context.version_journal_read_records_v1(), Some(3));
    assert!(!f.context.terminal);
    f.context.release_submission_inputs_v1(extra_id).unwrap();
    let mut consumer = f.copy(1, 2, &[event]).unwrap();
    assert_eq!(f.context.version_journal_read_records_v1(), Some(2));
    f.context
        .progress_directed_peer_copy_v1(&mut consumer)
        .unwrap();
    f.context
        .progress_directed_peer_copy_v1(&mut consumer)
        .unwrap();
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn directed_diamond_settles_shared_producer_and_callbacks_once() {
    for panic_callback in [false, true] {
        let mut f = Fixture::new(true, 5);
        let root = f.copy(0, 1, &[]).unwrap();
        let root_event = f.context.record_event(&root).unwrap();
        let left = f.copy(1, 2, &[root_event]).unwrap();
        let right = f.copy(1, 4, &[root_event]).unwrap();
        let left_event = f.context.record_event(&left).unwrap();
        let right_event = f.context.record_event(&right).unwrap();
        let mut join = f.copy(2, 3, &[right_event, left_event]).unwrap();
        let expected = [root.id, left.id, right.id, join.id];
        let callbacks = Arc::new(Mutex::new(Vec::new()));
        for submission in [&root, &left, &right, &join] {
            let output = callbacks.clone();
            let id = submission.id;
            f.context
                .on_completion(submission, move |_| output.lock().unwrap().push(id))
                .unwrap();
        }
        if panic_callback {
            f.context
                .on_completion(&root, |_| panic!("callback payload"))
                .unwrap();
        }
        for event in [root_event, left_event, right_event] {
            f.context.release_event(event).unwrap();
        }
        assert_eq!(f.context.version_journal_read_records_v1(), Some(4));
        for _ in 0..4 {
            f.context.progress_directed_peer_copy_v1(&mut join).unwrap();
        }
        assert_eq!(
            f.context.query_submission(&join).unwrap(),
            RuntimeCompletionStatusV1::Succeeded
        );
        assert_eq!(*callbacks.lock().unwrap(), expected);
        assert_eq!(
            f.context.completion_callback_panic_count(),
            u64::from(panic_callback)
        );
        assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
        for id in expected {
            assert_eq!(f.context.submissions[&id].dependency_retains, 0);
        }
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn directed_context_success_cannot_replace_reservation_result() {
    for state in 0..3 {
        let mut f = Fixture::new(true, 3);
        let (mut producer, mut consumer, _) = f.pair();
        match state {
            0 => {}
            1 => {
                f.context.backend.cancel_before_publication = true;
                f.context.cancel(&mut producer).unwrap();
            }
            2 => {
                f.context
                    .backend
                    .directed_observations
                    .insert(producer.backend_submission, Observation::Failed);
                f.context
                    .progress_directed_peer_copy_v1(&mut producer)
                    .unwrap();
            }
            _ => unreachable!(),
        }
        // Deliberately forge the descriptive result without changing the reservation.
        f.context.submissions.get_mut(&producer.id).unwrap().status =
            RuntimeCompletionStatusV1::Succeeded;
        let result = f.context.progress_directed_peer_copy_v1(&mut consumer);
        if state == 2 {
            result.unwrap();
            assert_eq!(
                f.context.query_submission(&consumer).unwrap(),
                RuntimeCompletionStatusV1::QuiescentWithoutResult
            );
            assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
            assert!(!f.context.terminal);
        } else {
            assert!(result.is_err());
            assert!(f.context.terminal);
            assert_eq!(
                f.context.query_submission(&consumer).unwrap(),
                RuntimeCompletionStatusV1::Pending
            );
            assert_eq!(f.context.submissions[&producer.id].dependency_retains, 1);
        }
        assert!(f.context.retained_directed_success_v1(consumer.id));
    }
}

#[test]
fn directed_marker_substitution_cannot_enter_legacy_batch() {
    let mut f = Fixture::new(true, 3);
    let (producer, consumer, _) = f.pair();
    let mut forged = RuntimeSubmissionV1::<RuntimePeerCopyV1> {
        id: consumer.id,
        backend_submission: consumer.backend_submission,
        stream: consumer.stream,
        device: consumer.device,
        completion: None,
        peer_transfer: consumer.peer_transfer,
        marker: PhantomData,
    };
    assert!(
        f.context
            .wait_peer_copy_batch(&mut [&mut forged], Duration::ZERO)
            .is_err()
    );
    assert!(f.context.terminal);
    assert!(f.context.backend.batch_calls.is_empty());
    assert_eq!(f.context.submissions[&producer.id].dependency_retains, 1);
    assert_eq!(f.context.version_journal_read_records_v1(), Some(2));
}
