use super::*;

type Copy = RuntimeSubmissionV1<RuntimeCopyV1>;
type Calls = Arc<Mutex<Vec<(RuntimeSubmissionIdV1, RuntimeCompletionStatusV1)>>>;

struct Prefix {
    context: Context,
    allocations: [RuntimeAllocationIdV1; 5],
    streams: [RuntimeStreamIdV1; 4],
    incoming: Peer,
    compute: RuntimeSubmissionV1<MixedArguments>,
    events: [RuntimeEventIdV1; 2],
}

impl Prefix {
    fn new() -> Self {
        let backend = MockBackend {
            next: 100,
            deferred_copies: true,
            deferred_kernel_reads: true,
            pending_compute_peer: true,
            pending_peer_readback: true,
            ..MockBackend::default()
        };
        let mut context =
            Context::open_with_version_journal_members_v1(backend, 32, 16, 64).unwrap();
        let devices = [context.devices()[0].id(), context.devices()[1].id()];
        let allocations = [0, 1, 1, 0].map(|index| {
            context
                .allocate(devices[index], RuntimeMemoryKindV1::DeviceLocal, 64, 16)
                .unwrap()
        });
        let host = context
            .allocate(devices[0], RuntimeMemoryKindV1::HostVisible, 64, 16)
            .unwrap();
        let allocations = [
            allocations[0],
            allocations[1],
            allocations[2],
            allocations[3],
            host,
        ];
        for (index, allocation) in allocations.into_iter().enumerate() {
            context
                .write_allocation(allocation, 0, &[0x31 + index as u8; 64])
                .unwrap();
        }
        let streams = [1, 1, 0, 0].map(|index| context.create_stream(devices[index]).unwrap());
        let module = context.load_module(devices[1], b"deferred-chain").unwrap();
        let kernel = context
            .resolve_kernel::<MixedArguments>(module, "mixed")
            .unwrap();
        let incoming = context
            .peer_copy(
                streams[0],
                span(allocations[0], RuntimeAccessV1::Read, 0, 64),
                span(allocations[1], RuntimeAccessV1::Write, 0, 64),
                &[],
            )
            .unwrap();
        let incoming_event = context.record_event(&incoming).unwrap();
        let compute = context
            .launch_producer_aware_v1(
                streams[1],
                &kernel,
                &MixedArguments(vec![
                    span(allocations[1], RuntimeAccessV1::Read, 0, 64),
                    span(allocations[2], RuntimeAccessV1::Write, 0, 64),
                ]),
                geometry(),
                &[incoming_event],
            )
            .unwrap();
        let compute_event = context.record_event(&compute).unwrap();
        Self {
            context,
            allocations,
            streams,
            incoming,
            compute,
            events: [incoming_event, compute_event],
        }
    }

    fn outgoing(&mut self, events: &[RuntimeEventIdV1]) -> Result<Peer, RuntimeErrorV1<MockError>> {
        self.context.peer_copy(
            self.streams[2],
            span(self.allocations[2], RuntimeAccessV1::Read, 0, 64),
            span(self.allocations[3], RuntimeAccessV1::Write, 0, 64),
            events,
        )
    }

    fn chain(&mut self) -> (Peer, Copy) {
        let outgoing = self.outgoing(&[self.events[1]]).unwrap();
        let event = self.context.record_event(&outgoing).unwrap();
        let readback = self
            .context
            .copy_async(
                self.streams[3],
                span(self.allocations[3], RuntimeAccessV1::Read, 0, 64),
                span(self.allocations[4], RuntimeAccessV1::Write, 0, 64),
                &[event],
            )
            .unwrap();
        for event in [self.events[0], self.events[1], event] {
            self.context.release_event(event).unwrap();
        }
        (outgoing, readback)
    }

    fn finish_backend(&mut self, outgoing: &Peer, readback: &Copy) {
        // The existing mock launch executes its retained input copy and real
        // mock read/write operation. Only final Context observation settles roots.
        self.context
            .backend
            .finish_submission(self.compute.backend_submission, true);
        for id in [outgoing.backend_submission, readback.backend_submission] {
            self.context.backend.finish_submission(id, true);
            self.context.backend.polls.insert(id, 1);
        }
    }

    fn ids(&self, outgoing: &Peer, readback: &Copy) -> [RuntimeSubmissionIdV1; 4] {
        [self.incoming.id, self.compute.id, outgoing.id, readback.id]
    }

    fn callbacks(&mut self, outgoing: &Peer, readback: &Copy) -> Calls {
        fn attach<A>(context: &mut Context, submission: &RuntimeSubmissionV1<A>, calls: &Calls) {
            let id = submission.id;
            let calls = Arc::clone(calls);
            context
                .on_completion(submission, move |status| {
                    calls.lock().unwrap().push((id, status))
                })
                .unwrap();
        }
        let calls = Arc::new(Mutex::new(Vec::new()));
        attach(&mut self.context, &self.incoming, &calls);
        attach(&mut self.context, &self.compute, &calls);
        attach(&mut self.context, outgoing, &calls);
        attach(&mut self.context, readback, &calls);
        calls
    }

    fn held(&self, ids: [RuntimeSubmissionIdV1; 4]) {
        assert!(self.context.events.is_empty());
        assert_eq!(self.context.scalar_peer_copies.len(), 2);
        assert_eq!(self.context.producer_launches.len(), 1);
        assert_eq!(self.context.same_device_copies.len(), 1);
        assert_eq!(self.context.version_journal_read_records_v1(), Some(4));
        for (index, id) in ids.into_iter().enumerate() {
            let record = &self.context.submissions[&id];
            assert_eq!(record.status, RuntimeCompletionStatusV1::Pending);
            assert!(!record.quiescent);
            assert_eq!(record.dependency_retains, usize::from(index < 3));
        }
    }

    fn release(mut self, outgoing: Peer, readback: Copy) {
        self.context.release_submission(readback).unwrap();
        self.context.release_submission(outgoing).unwrap();
        self.context.release_submission(self.compute).unwrap();
        self.context.release_submission(self.incoming).unwrap();
        assert!(self.context.submissions.is_empty());
        assert!(self.context.events.is_empty());
        assert!(self.context.scalar_peer_copies.is_empty());
        assert!(self.context.producer_launches.is_empty());
        assert!(self.context.same_device_copies.is_empty());
        assert_eq!(self.context.version_journal_read_records_v1(), Some(0));
        assert_eq!(self.context.version_journal_writer_records_v1(), Some(0));
        assert_eq!(self.context.completion_callback_count, 0);
        assert!(self.context.cleanup().is_complete());
    }
}

#[test]
fn compute_peer_deferred_chain_observes_four_original_roots_and_actual_mock_bytes() {
    for already_executed in [false, true] {
        let mut f = Prefix::new();
        if already_executed {
            f.context
                .backend
                .finish_submission(f.compute.backend_submission, true);
        }
        let (outgoing, mut readback) = f.chain();
        let ids = f.ids(&outgoing, &readback);
        let calls = f.callbacks(&outgoing, &readback);
        f.held(ids);
        assert_eq!(
            ids.map(|id| f.context.completion_parent_depth_v1(id).unwrap()),
            [1, 2, 3, 4]
        );
        let before = f.allocations.map(|id| state(&f.context, id));
        for allocation in f.allocations {
            validation(
                f.context.write_allocation(allocation, 0, &[9; 64]),
                RuntimeValidationErrorV1::ContextReserved,
            );
            validation(
                f.context.release_allocation(allocation),
                RuntimeValidationErrorV1::ContextReserved,
            );
        }
        let failure = f.context.release_submission(f.compute).unwrap_err();
        assert!(matches!(
            failure.error(),
            RuntimeErrorV1::Validation(RuntimeValidationErrorV1::SubmissionPending)
        ));
        (f.compute, _) = failure.into_parts();
        let polls = f.context.backend.poll_call_count;
        validation(
            f.context.drain(&mut readback, Instant::now()),
            RuntimeValidationErrorV1::InvalidDeadline,
        );
        assert_eq!(f.context.backend.poll_call_count, polls);
        assert_eq!(f.context.backend.flush_call_count, 0);
        f.finish_backend(&outgoing, &readback);
        for _ in 0..3 {
            assert_eq!(
                f.context.poll(&mut readback).unwrap(),
                RuntimePollV1::Pending
            );
            assert!(calls.lock().unwrap().is_empty());
            assert_eq!(f.allocations.map(|id| state(&f.context, id)), before);
        }
        assert_eq!(
            f.context.poll(&mut readback).unwrap(),
            RuntimePollV1::Succeeded
        );
        assert_eq!(
            *calls.lock().unwrap(),
            ids.map(|id| (id, RuntimeCompletionStatusV1::Succeeded))
        );
        assert_eq!(f.context.backend.poll_call_count - polls, 4);
        assert_eq!(
            f.context.poll(&mut readback).unwrap(),
            RuntimePollV1::Succeeded
        );
        assert_eq!(calls.lock().unwrap().len(), 4);
        let reads = &f.context.backend.observed_kernel_reads;
        assert_eq!(reads.len(), 1);
        assert_eq!(reads[0].submission, f.compute.backend_submission);
        assert_eq!(
            reads[0].binding.region.allocation,
            f.context.allocations[&f.allocations[1]].backend_allocation
        );
        assert_eq!(reads[0].bytes, [0x31; 64]);
        let mut bytes = [0; 64];
        f.context
            .read_allocation(f.allocations[4], 0, &mut bytes)
            .unwrap();
        assert_eq!(
            bytes,
            [(f.compute.backend_submission as u8).wrapping_add(37); 64]
        );
        for (index, (allocation, before)) in f.allocations.into_iter().zip(before).enumerate() {
            let after = state(&f.context, allocation);
            assert_eq!(after.attempt_epoch, before.attempt_epoch);
            assert_eq!(
                after.content_lineage,
                before.content_lineage + u64::from(index != 0)
            );
        }
        assert_eq!(f.context.backend.flush_call_count, 0);
        f.release(outgoing, readback);
    }
}

#[test]
fn compute_peer_deferred_chain_cancel_releases_only_each_exact_consumer() {
    let mut f = Prefix::new();
    let (mut outgoing, mut readback) = f.chain();
    let ids = f.ids(&outgoing, &readback);
    let calls = f.callbacks(&outgoing, &readback);
    let before = f
        .allocations
        .map(|id| state(&f.context, id).content_lineage);
    f.context.backend.cancel_before_publication = true;
    assert_eq!(
        f.context.cancel(&mut readback).unwrap(),
        RuntimeCancellationV1::Cancelled
    );
    assert_eq!(f.context.submissions[&outgoing.id].dependency_retains, 0);
    assert_eq!(f.context.version_journal_read_records_v1(), Some(3));
    assert_eq!(
        f.context.query_submission(&outgoing).unwrap(),
        RuntimeCompletionStatusV1::Pending
    );
    assert_eq!(
        f.context.cancel(&mut outgoing).unwrap(),
        RuntimeCancellationV1::Cancelled
    );
    assert_eq!(f.context.submissions[&f.compute.id].dependency_retains, 0);
    assert_eq!(f.context.version_journal_read_records_v1(), Some(2));
    assert_eq!(
        f.context.query_submission(&f.compute).unwrap(),
        RuntimeCompletionStatusV1::Pending
    );
    assert_eq!(
        f.context.cancel(&mut f.compute).unwrap(),
        RuntimeCancellationV1::Cancelled
    );
    assert_eq!(f.context.submissions[&f.incoming.id].dependency_retains, 0);
    assert_eq!(f.context.version_journal_read_records_v1(), Some(1));
    assert_eq!(
        f.context.query_submission(&f.incoming).unwrap(),
        RuntimeCompletionStatusV1::Pending
    );
    assert_eq!(
        f.context.cancel(&mut f.incoming).unwrap(),
        RuntimeCancellationV1::Cancelled
    );
    assert_eq!(
        f.allocations
            .map(|id| state(&f.context, id).content_lineage),
        before
    );
    let calls = calls.lock().unwrap();
    assert_eq!(
        calls.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
        ids.into_iter().rev().collect::<Vec<_>>()
    );
    assert!(calls.iter().all(|(_, status)| matches!(
        status,
        RuntimeCompletionStatusV1::Failed(RuntimeCompletionFailureV1::Cancelled)
    )));
    drop(calls);
    f.release(outgoing, readback);
}

#[test]
fn compute_peer_deferred_chain_nested_launch_journal_drift_precedes_observation() {
    for corruption in 0..7 {
        let mut f = Prefix::new();
        let (outgoing, mut readback) = f.chain();
        let ids = f.ids(&outgoing, &readback);
        f.finish_backend(&outgoing, &readback);
        for _ in 0..2 {
            assert_eq!(
                f.context.poll(&mut readback).unwrap(),
                RuntimePollV1::Pending
            );
        }
        match corruption {
            0 => {
                f.context
                    .submissions
                    .get_mut(&f.compute.id)
                    .unwrap()
                    .journal_producer_read = None
            }
            1 => {
                f.context
                    .submissions
                    .get_mut(&f.compute.id)
                    .unwrap()
                    .journal_writer = None
            }
            2..=6 => f
                .context
                .versions
                .as_mut()
                .unwrap()
                .corrupt_active_input_request_for_test_v1(f.compute.id, corruption - 2),
            _ => unreachable!(),
        }
        let polls = f.context.backend.poll_call_count;
        assert!(f.context.poll(&mut readback).is_err());
        assert_eq!(f.context.backend.poll_call_count, polls);
        assert!(f.context.is_terminal());
        f.held(ids);
        assert!(!f.context.cleanup().is_complete());
    }
}

#[test]
fn compute_peer_deferred_chain_bad_parent_rank_rejects_before_recursive_validation() {
    for corruption in 0..6 {
        let mut f = Prefix::new();
        let (outgoing, mut readback) = f.chain();
        let ids = f.ids(&outgoing, &readback);
        let root = f.context.producer_launches.get_mut(&f.compute.id).unwrap();
        match corruption {
            0 => root.dependencies[0].submission = f.compute.id,
            1 => root.dependencies[0].submission = outgoing.id,
            2 => root.dependencies[0].submission.context_generation += 1,
            3 => root.state.depth = 0,
            4 => root.state.depth = 3,
            5 => root.state.depth = MAX_RUNTIME_DEPENDENCIES_V1,
            _ => unreachable!(),
        }
        let polls = f.context.backend.poll_call_count;
        validation(
            f.context.poll(&mut readback),
            RuntimeValidationErrorV1::InvalidBackendDescription,
        );
        assert_eq!(f.context.backend.poll_call_count, polls);
        assert!(f.context.is_terminal());
        f.held(ids);
        assert!(!f.context.cleanup().is_complete());
    }
}

#[test]
fn compute_peer_deferred_chain_exact_event_and_retain_capacity_reject_before_issue() {
    let mut f = Prefix::new();
    let before = state(&f.context, f.allocations[3]);
    let copies = f.context.backend.copy_call_count;
    let alias = f.context.record_event(&f.compute).unwrap();
    for (events, error) in [
        (vec![], RuntimeValidationErrorV1::ContextReserved),
        (vec![f.events[0]], RuntimeValidationErrorV1::ContextReserved),
        (
            vec![f.events[1], alias],
            RuntimeValidationErrorV1::DuplicateDependency,
        ),
        (
            vec![f.events[1]; MAX_RUNTIME_DEPENDENCIES_V1 + 1],
            RuntimeValidationErrorV1::TooManyDependencies,
        ),
    ] {
        validation(f.outgoing(&events), error);
        assert_eq!(f.context.backend.copy_call_count, copies);
        assert_eq!(f.context.submissions[&f.compute.id].dependency_retains, 0);
        assert_eq!(f.context.version_journal_read_records_v1(), Some(2));
        assert_eq!(state(&f.context, f.allocations[3]), before);
    }
    f.context.release_event(alias).unwrap();
    f.context
        .submissions
        .get_mut(&f.compute.id)
        .unwrap()
        .dependency_retains = usize::MAX;
    validation(
        f.outgoing(&[f.events[1]]),
        RuntimeValidationErrorV1::Capacity,
    );
    f.context
        .submissions
        .get_mut(&f.compute.id)
        .unwrap()
        .dependency_retains = 0;
    assert_eq!(f.context.backend.copy_call_count, copies);
    assert_eq!(state(&f.context, f.allocations[3]), before);
    let (outgoing, mut readback) = f.chain();
    f.finish_backend(&outgoing, &readback);
    for _ in 0..8 {
        if f.context.poll(&mut readback).unwrap() == RuntimePollV1::Succeeded {
            break;
        }
    }
    assert_eq!(
        f.context.query_submission(&readback).unwrap(),
        RuntimeCompletionStatusV1::Succeeded
    );
    f.release(outgoing, readback);
}

#[test]
fn compute_peer_deferred_chain_ancestor_faults_preserve_disposition_and_custody() {
    for ancestor in 0..3 {
        for fault in [
            Observation::Quiescent,
            Observation::Rejected,
            Observation::Terminal,
            Observation::Panic,
        ] {
            let mut f = Prefix::new();
            let (outgoing, mut readback) = f.chain();
            let ids = f.ids(&outgoing, &readback);
            let calls = f.callbacks(&outgoing, &readback);
            let before = f
                .allocations
                .map(|id| state(&f.context, id).content_lineage);
            f.finish_backend(&outgoing, &readback);
            for _ in 0..3 - ancestor {
                assert_eq!(
                    f.context.poll(&mut readback).unwrap(),
                    RuntimePollV1::Pending
                );
            }
            let backend = f.context.submissions[&ids[ancestor]].backend_submission;
            f.context
                .backend
                .producer_launch
                .observations
                .insert(backend, fault);
            let result = catch_unwind(AssertUnwindSafe(|| f.context.poll(&mut readback)));
            match (fault, result) {
                (Observation::Quiescent, Ok(Err(RuntimeErrorV1::BackendQuiescent(_)))) => {
                    assert!(!f.context.is_terminal());
                    for id in &ids[ancestor..] {
                        assert_eq!(
                            f.context.submissions[id].status,
                            RuntimeCompletionStatusV1::QuiescentWithoutResult
                        );
                        assert!(f.context.submissions[id].quiescent);
                    }
                    assert_eq!(calls.lock().unwrap().len(), 4 - ancestor);
                    assert!(
                        calls.lock().unwrap().iter().all(|(_, status)| *status
                            == RuntimeCompletionStatusV1::QuiescentWithoutResult)
                    );
                    assert_eq!(
                        f.allocations
                            .map(|id| state(&f.context, id).content_lineage),
                        before
                    );
                    assert!(f.context.cleanup().is_complete());
                }
                (Observation::Rejected, Ok(Err(RuntimeErrorV1::BackendRejected(_))))
                | (Observation::Terminal, Ok(Err(RuntimeErrorV1::BackendTerminal(_))))
                | (Observation::Panic, Err(_)) => {
                    assert!(f.context.is_terminal());
                    f.held(ids);
                    assert!(calls.lock().unwrap().is_empty());
                    assert_eq!(
                        f.allocations
                            .map(|id| state(&f.context, id).content_lineage),
                        before
                    );
                    assert!(!f.context.cleanup().is_complete());
                }
                _ => panic!("descended chain fault lost classification"),
            }
        }
    }
}
