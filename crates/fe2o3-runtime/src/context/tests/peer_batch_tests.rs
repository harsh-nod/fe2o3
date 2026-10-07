use super::async_journal_tests::{region, state, writer_state};
use super::*;
use fe2o3_runtime_model::ContextWriterStateV1;
use std::sync::{Arc, Mutex};

type Context = RuntimeContextV1<MockBackend>;
type Submission = RuntimeSubmissionV1<RuntimePeerCopyV1>;

impl RuntimePeerCopyBatchBackendV1 for MockBackend {
    fn progress_peer_copy_batch_v1(
        &mut self,
        submissions: &[u64],
        deadline: Instant,
    ) -> Result<RuntimePeerCopyBatchPollV1, RuntimeBackendFailureV1<Self::Error>> {
        self.batch_calls.push((submissions.to_vec(), deadline));
        let failure = core::mem::take(&mut self.batch_failure);
        match failure {
            MockMemoryFailure::None => {}
            MockMemoryFailure::Rejected => {
                return Err(RuntimeBackendFailureV1::Rejected(MockError(
                    "batch rejected",
                )));
            }
            MockMemoryFailure::Quiescent => {
                for submission in submissions {
                    self.finish_submission(*submission, false);
                }
                return Err(RuntimeBackendFailureV1::Quiescent(MockError(
                    "batch quiescent",
                )));
            }
            MockMemoryFailure::Terminal => {
                return Err(RuntimeBackendFailureV1::Terminal(MockError(
                    "batch terminal",
                )));
            }
            MockMemoryFailure::Panic => panic!("scripted peer-copy batch panic"),
        }
        if self.batch_pending {
            return Ok(RuntimePeerCopyBatchPollV1::Pending);
        }
        for submission in submissions {
            self.finish_submission(*submission, true);
        }
        Ok(RuntimePeerCopyBatchPollV1::Succeeded)
    }
}

struct Fixture {
    context: Context,
    sources: Vec<RuntimeAllocationIdV1>,
    destinations: Vec<RuntimeAllocationIdV1>,
    submissions: Vec<Submission>,
}

impl Fixture {
    fn new(count: usize, journal: bool) -> Self {
        let backend = MockBackend {
            next: 100,
            deferred_copies: true,
            ..MockBackend::default()
        };
        let mut context = if journal {
            Context::open_with_version_journal_v1(backend, count * 2 + 2, count + 1).unwrap()
        } else {
            Context::open(backend).unwrap()
        };
        let source_device = context.devices()[0].id();
        let destination_device = context.devices()[1].id();
        for device in [source_device, destination_device] {
            context
                .configure_allocation_admission_v1(device, count as u64 * 64 + 64, count + 1)
                .unwrap();
        }
        let stream = context.create_stream(destination_device).unwrap();
        let mut sources = Vec::new();
        let mut destinations = Vec::new();
        let mut submissions = Vec::new();
        sources.try_reserve_exact(count).unwrap();
        destinations.try_reserve_exact(count).unwrap();
        submissions.try_reserve_exact(count).unwrap();
        for index in 0..count {
            let source = context
                .allocate(source_device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
                .unwrap();
            let destination = context
                .allocate(destination_device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
                .unwrap();
            let value = u8::try_from(index + 1).unwrap();
            context.write_allocation(source, 0, &[value; 8]).unwrap();
            context
                .write_allocation(destination, 0, &[0xa5; 8])
                .unwrap();
            submissions.push(
                context
                    .peer_copy(
                        stream,
                        region(source, RuntimeAccessV1::Read, 0),
                        region(destination, RuntimeAccessV1::Write, 0),
                        &[],
                    )
                    .unwrap(),
            );
            sources.push(source);
            destinations.push(destination);
        }
        Self {
            context,
            sources,
            destinations,
            submissions,
        }
    }

    fn progress(
        &mut self,
        reverse: bool,
        timeout: Duration,
    ) -> Result<RuntimePeerCopyBatchPollV1, RuntimeErrorV1<MockError>> {
        let mut submissions: Vec<_> = self.submissions.iter_mut().collect();
        if reverse {
            submissions.reverse();
        }
        self.context.wait_peer_copy_batch(&mut submissions, timeout)
    }

    fn expected_backend_ids(&self, reverse: bool) -> Vec<u64> {
        let mut ids: Vec<_> = self
            .submissions
            .iter()
            .map(|submission| submission.backend_submission)
            .collect();
        if reverse {
            ids.reverse();
        }
        ids
    }

    fn assert_pending_journal(&self, journal: bool) {
        if journal {
            assert_eq!(
                self.context.version_journal_read_records_v1(),
                Some(self.submissions.len())
            );
            assert_eq!(
                self.context.version_journal_writer_records_v1(),
                Some(self.submissions.len())
            );
            for destination in &self.destinations {
                assert_eq!(
                    writer_state(&self.context, *destination),
                    ContextWriterStateV1::Pending { member_count: 1 }
                );
            }
        } else {
            assert_eq!(self.context.version_journal_read_records_v1(), None);
            assert_eq!(self.context.version_journal_writer_records_v1(), None);
        }
    }
}

fn copy_token(token: &Submission) -> Submission {
    RuntimeSubmissionV1 {
        id: token.id,
        backend_submission: token.backend_submission,
        stream: token.stream,
        device: token.device,
        completion: token.completion,
        peer_transfer: token.peer_transfer,
        marker: PhantomData,
    }
}

fn validation(
    error: RuntimeValidationErrorV1,
    result: Result<RuntimePeerCopyBatchPollV1, RuntimeErrorV1<MockError>>,
) {
    assert!(matches!(result, Err(RuntimeErrorV1::Validation(actual)) if actual == error));
}

#[test]
fn exact_batches_pending_retry_and_complete_in_caller_order() {
    for journal in [false, true] {
        for count in [1, 2, MAX_RUNTIME_PEER_COPY_BATCH_SUBMISSIONS_V1] {
            let mut fixture = Fixture::new(count, journal);
            let observed = Arc::new(Mutex::new(Vec::new()));
            let mut events = Vec::new();
            events.try_reserve_exact(count).unwrap();
            for (index, submission) in fixture.submissions.iter().enumerate() {
                let observed = Arc::clone(&observed);
                fixture
                    .context
                    .on_completion(submission, move |status| {
                        observed.lock().unwrap().push((index, status));
                    })
                    .unwrap();
                events.push(fixture.context.record_event(submission).unwrap());
            }
            let expected_ids = fixture.expected_backend_ids(true);
            fixture.context.backend.batch_pending = true;
            assert_eq!(
                fixture.progress(true, Duration::ZERO).unwrap(),
                RuntimePeerCopyBatchPollV1::Pending
            );
            assert_eq!(fixture.context.backend.batch_calls.len(), 1);
            assert_eq!(fixture.context.backend.batch_calls[0].0, expected_ids);
            assert_eq!(fixture.context.backend.pending_copies.len(), count);
            assert_eq!(fixture.context.backend.poll_call_count, 0);
            assert_eq!(fixture.context.backend.wait_call_count, 0);
            assert!(observed.lock().unwrap().is_empty());
            for event in &events {
                assert_eq!(
                    fixture.context.query_event(*event).unwrap(),
                    RuntimeCompletionStatusV1::Pending
                );
            }
            fixture.assert_pending_journal(journal);

            fixture.context.backend.batch_pending = false;
            assert_eq!(
                fixture.progress(true, Duration::ZERO).unwrap(),
                RuntimePeerCopyBatchPollV1::Succeeded
            );
            assert_eq!(fixture.context.backend.batch_calls.len(), 2);
            assert_eq!(fixture.context.backend.batch_calls[1].0, expected_ids);
            assert!(fixture.context.backend.pending_copies.is_empty());
            assert_eq!(fixture.context.backend.poll_call_count, 0);
            assert_eq!(fixture.context.backend.wait_call_count, 0);
            let expected_callbacks: Vec<_> = (0..count)
                .rev()
                .map(|index| (index, RuntimeCompletionStatusV1::Succeeded))
                .collect();
            assert_eq!(*observed.lock().unwrap(), expected_callbacks);
            for event in &events {
                assert_eq!(
                    fixture.context.wait_event(*event, Duration::ZERO).unwrap(),
                    RuntimeCompletionStatusV1::Succeeded
                );
            }
            assert_eq!(fixture.context.backend.poll_call_count, 0);
            assert_eq!(fixture.context.backend.wait_call_count, 0);
            for (index, (source, destination)) in fixture
                .sources
                .iter()
                .zip(&fixture.destinations)
                .enumerate()
            {
                let source = fixture.context.allocations[source].backend_allocation;
                let destination = fixture.context.allocations[destination].backend_allocation;
                assert_eq!(
                    &fixture.context.backend.memory[&source][..8],
                    &[index as u8 + 1; 8]
                );
                assert_eq!(
                    &fixture.context.backend.memory[&destination][..8],
                    &[index as u8 + 1; 8]
                );
            }
            if journal {
                assert_eq!(fixture.context.version_journal_read_records_v1(), Some(0));
                assert_eq!(fixture.context.version_journal_writer_records_v1(), Some(0));
                for destination in &fixture.destinations {
                    assert_eq!(state(&fixture.context, *destination).content_lineage, 2);
                }
            }
            let calls = fixture.context.backend.batch_calls.len();
            validation(
                RuntimeValidationErrorV1::InvalidPeerCopyBatch,
                fixture.progress(false, Duration::ZERO),
            );
            assert_eq!(fixture.context.backend.batch_calls.len(), calls);
            assert_eq!(*observed.lock().unwrap(), expected_callbacks);
            assert!(fixture.context.cleanup().is_complete());
        }
    }
}

#[test]
fn rejected_quiescent_and_terminal_batches_preserve_exact_class_and_custody() {
    for failure in [
        MockMemoryFailure::Rejected,
        MockMemoryFailure::Quiescent,
        MockMemoryFailure::Terminal,
    ] {
        let mut fixture = Fixture::new(2, true);
        let observed = Arc::new(Mutex::new(Vec::new()));
        for (index, submission) in fixture.submissions.iter().enumerate() {
            let observed = Arc::clone(&observed);
            fixture
                .context
                .on_completion(submission, move |status| {
                    observed.lock().unwrap().push((index, status));
                })
                .unwrap();
        }
        fixture.context.backend.batch_failure = failure;
        let result = fixture.progress(false, Duration::ZERO);
        match (failure, result) {
            (MockMemoryFailure::Rejected, Err(RuntimeErrorV1::BackendRejected(error))) => {
                assert_eq!(error.0, "batch rejected");
                assert!(!fixture.context.is_terminal());
                assert_eq!(fixture.context.backend.pending_copies.len(), 2);
                fixture.assert_pending_journal(true);
                assert!(observed.lock().unwrap().is_empty());
                fixture.progress(false, Duration::ZERO).unwrap();
                assert!(fixture.context.cleanup().is_complete());
            }
            (MockMemoryFailure::Quiescent, Err(RuntimeErrorV1::BackendQuiescent(error))) => {
                assert_eq!(error.0, "batch quiescent");
                assert!(!fixture.context.is_terminal());
                assert!(fixture.context.backend.pending_copies.is_empty());
                assert_eq!(fixture.context.version_journal_read_records_v1(), Some(0));
                assert_eq!(fixture.context.version_journal_writer_records_v1(), Some(2));
                for destination in &fixture.destinations {
                    assert_eq!(
                        writer_state(&fixture.context, *destination),
                        ContextWriterStateV1::Unknown { member_count: 1 }
                    );
                    assert_eq!(state(&fixture.context, *destination).content_lineage, 1);
                }
                assert_eq!(
                    *observed.lock().unwrap(),
                    vec![
                        (0, RuntimeCompletionStatusV1::QuiescentWithoutResult),
                        (1, RuntimeCompletionStatusV1::QuiescentWithoutResult),
                    ]
                );
                assert!(fixture.context.cleanup().is_complete());
            }
            (MockMemoryFailure::Terminal, Err(RuntimeErrorV1::BackendTerminal(error))) => {
                assert_eq!(error.0, "batch terminal");
                assert!(fixture.context.is_terminal());
                assert_eq!(fixture.context.backend.pending_copies.len(), 2);
                assert_eq!(fixture.context.version_journal_read_records_v1(), Some(2));
                assert_eq!(fixture.context.version_journal_writer_records_v1(), Some(2));
                assert!(observed.lock().unwrap().is_empty());
                for submission in &fixture.submissions {
                    assert_eq!(
                        fixture.context.query_submission(submission).unwrap(),
                        RuntimeCompletionStatusV1::Pending
                    );
                }
                assert!(!fixture.context.cleanup().is_complete());
            }
            outcome => panic!("unexpected batch failure outcome: {outcome:?}"),
        }
        assert_eq!(
            fixture.context.backend.batch_calls.len(),
            if failure == MockMemoryFailure::Rejected {
                2
            } else {
                1
            }
        );
        assert_eq!(fixture.context.backend.poll_call_count, 0);
        assert_eq!(fixture.context.backend.wait_call_count, 0);
    }
}

#[test]
fn every_corrupt_reader_or_writer_marker_blocks_the_whole_roster_before_backend_entry() {
    for reader in [false, true] {
        for index in 0..3 {
            let mut fixture = Fixture::new(3, true);
            fixture
                .context
                .on_completion(&fixture.submissions[0], |_| {
                    panic!("corrupt batch callback")
                })
                .unwrap();
            let id = fixture.submissions[index].id;
            let record = fixture.context.submissions.get_mut(&id).unwrap();
            if reader {
                record.journal_read.as_mut().unwrap().first.incarnation += 1;
            } else {
                record.journal_writer.as_mut().unwrap().key.local += 1;
            }
            assert!(matches!(
                fixture.progress(false, Duration::ZERO),
                Err(RuntimeErrorV1::Validation(
                    RuntimeValidationErrorV1::InvalidBackendDescription
                ))
            ));
            assert!(fixture.context.is_terminal());
            assert!(fixture.context.backend.batch_calls.is_empty());
            assert_eq!(fixture.context.backend.pending_copies.len(), 3);
            assert_eq!(fixture.context.version_journal_read_records_v1(), Some(3));
            assert_eq!(fixture.context.version_journal_writer_records_v1(), Some(3));
            assert_eq!(fixture.context.completion_callback_panic_count(), 0);
            for submission in &fixture.submissions {
                assert_eq!(
                    fixture.context.query_submission(submission).unwrap(),
                    RuntimeCompletionStatusV1::Pending
                );
            }
            assert!(!fixture.context.cleanup().is_complete());
        }
    }
}

#[test]
fn corruption_after_backend_entry_preserves_quiescent_diagnostic_without_partial_callbacks() {
    for quiescent in [false, true] {
        for index in 0..3 {
            let mut fixture = Fixture::new(3, true);
            let observed = Arc::new(Mutex::new(Vec::new()));
            for submission in &fixture.submissions {
                let observed = Arc::clone(&observed);
                fixture
                    .context
                    .on_completion(submission, move |status| {
                        observed.lock().unwrap().push(status);
                    })
                    .unwrap();
            }
            let ids = fixture.expected_backend_ids(false);
            if quiescent {
                fixture.context.backend.batch_failure = MockMemoryFailure::Quiescent;
            }
            let result = fixture
                .context
                .backend
                .progress_peer_copy_batch_v1(&ids, Instant::now());
            // Split backend return from Context observation to inject corruption
            // at the genuine settlement boundary, not an unsafe public race.
            fixture
                .context
                .submissions
                .get_mut(&fixture.submissions[index].id)
                .unwrap()
                .journal_writer
                .as_mut()
                .unwrap()
                .key
                .local += 1;
            let mut refs: Vec<_> = fixture.submissions.iter_mut().collect();
            let observed_result = fixture
                .context
                .observe_peer_copy_batch_result_v1(&mut refs, result);
            if quiescent {
                assert!(matches!(
                    observed_result,
                    Err(RuntimeErrorV1::BackendQuiescent(MockError(
                        "batch quiescent"
                    )))
                ));
            } else {
                validation(
                    RuntimeValidationErrorV1::InvalidBackendDescription,
                    observed_result,
                );
            }
            assert!(fixture.context.is_terminal());
            assert!(observed.lock().unwrap().is_empty());
            assert_eq!(fixture.context.version_journal_read_records_v1(), Some(3));
            for submission in &fixture.submissions {
                assert_eq!(
                    fixture.context.query_submission(submission).unwrap(),
                    RuntimeCompletionStatusV1::Pending
                );
            }
            assert!(!fixture.context.cleanup().is_complete());
            assert_eq!(fixture.context.backend.batch_calls.len(), 1);
        }
    }
}

#[test]
fn backend_unwind_quarantines_all_roots_and_preserves_the_payload() {
    let mut fixture = Fixture::new(3, true);
    fixture.context.backend.batch_failure = MockMemoryFailure::Panic;
    let panic =
        catch_unwind(AssertUnwindSafe(|| fixture.progress(false, Duration::ZERO))).unwrap_err();
    assert_eq!(
        panic.downcast_ref::<&str>(),
        Some(&"scripted peer-copy batch panic")
    );
    assert!(fixture.context.is_terminal());
    assert_eq!(fixture.context.version_journal_read_records_v1(), Some(3));
    assert_eq!(fixture.context.backend.pending_copies.len(), 3);
    validation(
        RuntimeValidationErrorV1::ContextTerminal,
        fixture.progress(false, Duration::ZERO),
    );
    assert!(!fixture.context.cleanup().is_complete());
    assert_eq!(fixture.context.backend.batch_calls.len(), 1);
}

#[test]
fn invalid_rosters_never_enter_the_backend() {
    let mut empty = Context::open(MockBackend::default()).unwrap();
    let mut none: [&mut Submission; 0] = [];
    validation(
        RuntimeValidationErrorV1::InvalidPeerCopyBatch,
        empty.wait_peer_copy_batch(&mut none, Duration::ZERO),
    );
    assert!(empty.backend.batch_calls.is_empty());
    assert!(empty.cleanup().is_complete());

    let mut oversized = Fixture::new(1, false);
    let mut aliases: Vec<_> = (0..=MAX_RUNTIME_PEER_COPY_BATCH_SUBMISSIONS_V1)
        .map(|_| copy_token(&oversized.submissions[0]))
        .collect();
    let mut alias_refs: Vec<_> = aliases.iter_mut().collect();
    validation(
        RuntimeValidationErrorV1::InvalidPeerCopyBatch,
        oversized
            .context
            .wait_peer_copy_batch(&mut alias_refs, Duration::ZERO),
    );
    assert!(oversized.context.backend.batch_calls.is_empty());

    let mut duplicate = copy_token(&oversized.submissions[0]);
    let mut duplicate_refs = [&mut oversized.submissions[0], &mut duplicate];
    validation(
        RuntimeValidationErrorV1::InvalidPeerCopyBatch,
        oversized
            .context
            .wait_peer_copy_batch(&mut duplicate_refs, Duration::ZERO),
    );
    validation(
        RuntimeValidationErrorV1::InvalidDeadline,
        oversized.progress(false, Duration::MAX),
    );
    assert!(oversized.context.backend.batch_calls.is_empty());
    oversized.progress(false, Duration::ZERO).unwrap();
    assert!(oversized.context.cleanup().is_complete());

    let mut local = Fixture::new(1, false);
    let mut foreign = Fixture::new(1, false);
    let mut foreign_refs = [&mut foreign.submissions[0]];
    assert!(matches!(
        local
            .context
            .wait_peer_copy_batch(&mut foreign_refs, Duration::ZERO),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::UnknownSubmission
        ))
    ));
    assert!(local.context.backend.batch_calls.is_empty());
    local.progress(false, Duration::ZERO).unwrap();
    foreign.progress(false, Duration::ZERO).unwrap();
    assert!(local.context.cleanup().is_complete());
    assert!(foreign.context.cleanup().is_complete());
}

#[test]
fn mixed_destination_devices_are_rejected_before_backend_entry() {
    let backend = MockBackend {
        next: 100,
        deferred_copies: true,
        ..MockBackend::default()
    };
    let mut context = Context::open(backend).unwrap();
    let devices = [context.devices()[0].id(), context.devices()[1].id()];
    for device in devices {
        context
            .configure_allocation_admission_v1(device, 256, 4)
            .unwrap();
    }
    let streams = [
        context.create_stream(devices[0]).unwrap(),
        context.create_stream(devices[1]).unwrap(),
    ];
    let allocations = [
        context
            .allocate(devices[0], RuntimeMemoryKindV1::DeviceLocal, 64, 16)
            .unwrap(),
        context
            .allocate(devices[1], RuntimeMemoryKindV1::DeviceLocal, 64, 16)
            .unwrap(),
        context
            .allocate(devices[0], RuntimeMemoryKindV1::DeviceLocal, 64, 16)
            .unwrap(),
        context
            .allocate(devices[1], RuntimeMemoryKindV1::DeviceLocal, 64, 16)
            .unwrap(),
    ];
    let mut toward_one = context
        .peer_copy(
            streams[1],
            region(allocations[0], RuntimeAccessV1::Read, 0),
            region(allocations[3], RuntimeAccessV1::Write, 0),
            &[],
        )
        .unwrap();
    let mut toward_zero = context
        .peer_copy(
            streams[0],
            region(allocations[1], RuntimeAccessV1::Read, 0),
            region(allocations[2], RuntimeAccessV1::Write, 0),
            &[],
        )
        .unwrap();
    let mut mixed = [&mut toward_one, &mut toward_zero];
    validation(
        RuntimeValidationErrorV1::WrongDevice,
        context.wait_peer_copy_batch(&mut mixed, Duration::ZERO),
    );
    assert!(context.backend.batch_calls.is_empty());
    let mut first = [&mut toward_one];
    context
        .wait_peer_copy_batch(&mut first, Duration::ZERO)
        .unwrap();
    let mut second = [&mut toward_zero];
    context
        .wait_peer_copy_batch(&mut second, Duration::ZERO)
        .unwrap();
    assert!(context.cleanup().is_complete());
}
