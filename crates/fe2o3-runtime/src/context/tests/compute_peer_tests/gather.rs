use super::*;
use fe2o3_runtime_model::ContextQueuedWriterStatusV1;

pub(super) type Producer = RuntimeSubmissionV1<MixedArguments>;

pub(super) struct Gather {
    pub(super) context: Context,
    pub(super) peer_stream: RuntimeStreamIdV1,
    pub(super) readback_stream: RuntimeStreamIdV1,
    pub(super) sources: Vec<RuntimeAllocationIdV1>,
    pub(super) producers: Vec<Producer>,
    pub(super) events: Vec<RuntimeEventIdV1>,
    pub(super) destination: RuntimeAllocationIdV1,
    pub(super) host: RuntimeAllocationIdV1,
    pub(super) peers: Vec<Peer>,
    pub(super) peer_events: Vec<RuntimeEventIdV1>,
    pub(super) expected: Vec<u8>,
}

impl Gather {
    pub(super) fn new(count: usize) -> Self {
        let Fixture {
            mut context,
            compute_stream,
            peer_stream,
            readback_stream,
            source,
            destination,
            host,
            kernel,
            producer,
            event,
        } = Fixture::with_sizes([73, 257, 257], |source| {
            vec![span(source, RuntimeAccessV1::Write, 0, 73)]
        });
        context.backend.ordered_compute_peer = true;
        let device = context.devices()[0].id();
        let mut sources = vec![source];
        let mut producers = vec![producer];
        let mut events = vec![event];
        for index in 1..count {
            let length = 73 + index as u64 * 7;
            let source = context
                .allocate(device, RuntimeMemoryKindV1::DeviceLocal, length, 16)
                .unwrap();
            context
                .write_allocation(source, 0, &vec![3; length as usize])
                .unwrap();
            let producer = context
                .launch_producer_aware_v1(
                    compute_stream,
                    &kernel,
                    &MixedArguments(vec![span(source, RuntimeAccessV1::Write, 0, length)]),
                    geometry(),
                    &[],
                )
                .unwrap();
            events.push(context.record_event(&producer).unwrap());
            producers.push(producer);
            sources.push(source);
        }
        Self {
            context,
            peer_stream,
            readback_stream,
            sources,
            producers,
            events,
            destination,
            host,
            peers: Vec::new(),
            peer_events: Vec::new(),
            expected: vec![5; 257],
        }
    }

    pub(super) fn enqueue(&mut self, index: usize, offset: u64, bytes: u64) {
        let mut dependencies = vec![self.events[index]];
        dependencies.extend(self.peer_events.last().copied());
        let peer = self
            .context
            .peer_copy(
                self.peer_stream,
                span(self.sources[index], RuntimeAccessV1::Read, 3, bytes),
                span(self.destination, RuntimeAccessV1::Write, offset, bytes),
                &dependencies,
            )
            .unwrap();
        let value = (self.producers[index].backend_submission as u8).wrapping_add(37);
        self.expected[offset as usize..(offset + bytes) as usize].fill(value);
        self.peer_events
            .push(self.context.record_event(&peer).unwrap());
        self.peers.push(peer);
    }

    pub(super) fn release_events(&mut self) {
        for event in self.events.drain(..).chain(self.peer_events.drain(..)) {
            self.context.release_event(event).unwrap();
        }
    }

    pub(super) fn finish_backend(&mut self) {
        // Execute the existing mock operations in dependency order. Only normal
        // Context observations may commit their journal state and callbacks.
        for id in self
            .producers
            .iter()
            .map(|value| value.backend_submission)
            .chain(self.peers.iter().map(|value| value.backend_submission))
        {
            self.context.backend.finish_submission(id, true);
            self.context.backend.polls.insert(id, 1);
        }
    }

    pub(super) fn release_results(&mut self) {
        for peer in self.peers.drain(..).rev() {
            self.context.release_submission(peer).unwrap();
        }
        for producer in self.producers.drain(..).rev() {
            self.context.release_submission(producer).unwrap();
        }
    }

    fn cancel_all(mut self) {
        self.release_events();
        self.context.backend.cancel_before_publication = true;
        for peer in self.peers.iter_mut().rev() {
            self.context.cancel(peer).unwrap();
        }
        for producer in self.producers.iter_mut().rev() {
            self.context.cancel(producer).unwrap();
        }
        self.release_results();
        assert!(self.context.cleanup().is_complete());
    }
}

#[test]
fn compute_peer_gather_queued_windows_settle_exact_order_and_preserve_guards() {
    for count in [2, 3, 5] {
        for overlap in [false, true] {
            let mut f = Gather::new(count);
            let before = state(&f.context, f.destination);
            let calls = Arc::new(Mutex::new(Vec::new()));
            for index in 0..count {
                f.enqueue(index, 11 + index as u64 * if overlap { 7 } else { 43 }, 31);
                let peer = f.peers.last().unwrap();
                let root = &f.context.scalar_peer_copies[&peer.id];
                assert_eq!(root.compute.as_ref().unwrap().state.depth, index + 2);
                assert_eq!(
                    root.compute_predecessor_v1().map(|value| value.submission),
                    index.checked_sub(1).map(|previous| f.peers[previous].id)
                );
                assert!(
                    f.context
                        .validate_pending_peer_copy_roots_v1(peer.id)
                        .is_ok()
                );
                let writer = f.context.submissions[&peer.id].journal_writer.unwrap();
                assert_eq!(
                    f.context
                        .versions
                        .as_ref()
                        .unwrap()
                        .journal_for_test()
                        .queued_writer_status(writer)
                        .unwrap(),
                    (index != 0).then_some(ContextQueuedWriterStatusV1::Waiting)
                );
                let output = Arc::clone(&calls);
                let id = peer.id;
                f.context
                    .on_completion(peer, move |status| {
                        output.lock().unwrap().push((id, status))
                    })
                    .unwrap();
            }
            assert_eq!(
                state(&f.context, f.destination).attempt_epoch,
                before.attempt_epoch + 1
            );
            assert_eq!(
                state(&f.context, f.destination).content_lineage,
                before.content_lineage
            );
            assert_eq!(
                f.context.version_journal_writer_records_v1(),
                Some(count * 2)
            );
            f.release_events();
            validation(
                f.context.write_allocation(f.destination, 0, &[0; 257]),
                RuntimeValidationErrorV1::ContextReserved,
            );
            assert!(f.context.release_allocation(f.destination).is_err());
            let expected_ids: Vec<_> = f
                .peers
                .iter()
                .map(|peer| (peer.id, RuntimeCompletionStatusV1::Succeeded))
                .collect();
            f.finish_backend();
            for _ in 0..count * 4 + 4 {
                if f.context.poll(f.peers.last_mut().unwrap()).unwrap() == RuntimePollV1::Succeeded
                {
                    break;
                }
            }
            assert_eq!(
                f.context.query_submission(f.peers.last().unwrap()).unwrap(),
                RuntimeCompletionStatusV1::Succeeded
            );
            assert_eq!(*calls.lock().unwrap(), expected_ids);
            for result in &f.peers {
                assert!(f.context.submissions[&result.id].quiescent);
                assert_eq!(f.context.submissions[&result.id].dependency_retains, 0);
            }
            let after = state(&f.context, f.destination);
            assert_eq!(after.attempt_epoch, before.attempt_epoch + count as u64);
            assert_eq!(after.content_lineage, before.content_lineage + count as u64);
            let mut actual = vec![0; 257];
            f.context
                .read_allocation(f.destination, 0, &mut actual)
                .unwrap();
            assert_eq!(actual, f.expected);
            assert_eq!(f.context.version_journal_writer_records_v1(), Some(0));
            assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
            f.release_results();
            assert!(f.context.cleanup().is_complete());
        }
    }
}

#[test]
fn compute_peer_gather_requires_exact_latest_event_and_explicit_opt_in() {
    for mode in 0..6 {
        let mut f = Gather::new(3);
        if mode == 3 {
            f.context.backend.ordered_compute_peer = false;
        }
        f.enqueue(0, 7, 31);
        if mode == 2 {
            f.enqueue(1, 17, 31);
        }
        let producer_index = if mode == 2 { 2 } else { 1 };
        let mut dependencies = vec![f.events[producer_index], f.peer_events[0]];
        let mut stream = f.peer_stream;
        match mode {
            0 => {
                dependencies.pop();
            }
            1 => {
                dependencies.push(f.peer_events[0]);
            }
            2 => {}
            3 => {}
            4 => {
                stream = f
                    .context
                    .create_stream(f.context.devices()[1].id())
                    .unwrap();
            }
            _ => {
                let event = f.peer_events.pop().unwrap();
                f.context.release_event(event).unwrap();
            }
        }
        let calls = f.context.backend.copy_call_count;
        let writers = f.context.version_journal_writer_records_v1();
        let reads = f.context.version_journal_read_records_v1();
        let before = state(&f.context, f.destination);
        let result = f.context.peer_copy(
            stream,
            span(f.sources[producer_index], RuntimeAccessV1::Read, 3, 31),
            span(f.destination, RuntimeAccessV1::Write, 27, 31),
            &dependencies,
        );
        validation(
            result,
            match mode {
                1 => RuntimeValidationErrorV1::DuplicateDependency,
                5 => RuntimeValidationErrorV1::UnknownEvent,
                _ => RuntimeValidationErrorV1::ContextReserved,
            },
        );
        assert_eq!(f.context.backend.copy_call_count, calls);
        assert_eq!(f.context.version_journal_writer_records_v1(), writers);
        assert_eq!(f.context.version_journal_read_records_v1(), reads);
        assert_eq!(state(&f.context, f.destination), before);
        f.context.backend.ordered_compute_peer = true;
        f.cancel_all();
    }
}

#[test]
fn compute_peer_gather_tail_cancellation_preserves_predecessor_and_frame() {
    let mut f = Gather::new(3);
    for index in 0..3 {
        f.enqueue(index, 5 + index as u64 * 17, 31);
    }
    f.release_events();
    f.context.backend.cancel_before_publication = true;
    let before = state(&f.context, f.destination);
    assert_eq!(
        f.context.cancel(&mut f.peers[2]).unwrap(),
        RuntimeCancellationV1::Cancelled
    );
    assert_eq!(state(&f.context, f.destination), before);
    assert_eq!(
        f.context.query_submission(&f.peers[1]).unwrap(),
        RuntimeCompletionStatusV1::Pending
    );
    assert_eq!(f.context.submissions[&f.peers[1].id].dependency_retains, 0);
    assert_eq!(f.context.version_journal_writer_records_v1(), Some(5));
    assert_eq!(f.context.version_journal_read_records_v1(), Some(2));
    assert!(f.context.release_allocation(f.destination).is_err());
    f.cancel_all();
}

#[test]
fn compute_peer_gather_predecessor_drift_fails_before_backend_observation() {
    for mode in 0..4 {
        let mut f = Gather::new(3);
        for index in 0..3 {
            f.enqueue(index, 11 + index as u64 * 17, 31);
        }
        f.release_events();
        let id = f.peers[2].id;
        let previous = f.peers[1].id;
        let root = f.context.scalar_peer_copies.get_mut(&id).unwrap();
        let dependency = root
            .dependencies
            .iter_mut()
            .find(|value| value.submission == previous)
            .unwrap();
        match mode {
            0 => dependency.submission = id,
            1 => dependency.backend_submission += 1,
            2 => dependency.event.context_generation += 1,
            _ => {
                let previous = f.context.scalar_peer_copies.get_mut(&previous).unwrap();
                previous.destination.region.byte_offset += 1;
            }
        }
        let calls = f.context.backend.poll_call_count;
        assert!(f.context.poll(&mut f.peers[2]).is_err());
        assert_eq!(f.context.backend.poll_call_count, calls);
        assert!(f.context.is_terminal());
        assert_eq!(f.context.submissions[&previous].dependency_retains, 1);
        assert!(!f.context.cleanup().is_complete());
    }
}

#[test]
fn compute_peer_gather_cancelled_middle_blocks_tail_without_promoting_lineage() {
    let mut f = Gather::new(3);
    for index in 0..3 {
        f.enqueue(index, 11 + index as u64 * 7, 31);
    }
    f.release_events();
    let before = state(&f.context, f.destination);
    f.context.backend.cancel_before_publication = true;
    assert_eq!(
        f.context.cancel(&mut f.peers[1]).unwrap(),
        RuntimeCancellationV1::Cancelled
    );
    let writer = f.context.submissions[&f.peers[2].id]
        .journal_writer
        .unwrap();
    assert_eq!(
        f.context
            .versions
            .as_ref()
            .unwrap()
            .journal_for_test()
            .queued_writer_status(writer)
            .unwrap(),
        Some(ContextQueuedWriterStatusV1::Blocked)
    );
    f.context
        .backend
        .producer_launch
        .observations
        .insert(f.peers[2].backend_submission, Observation::Pending);
    assert_eq!(
        f.context.poll(&mut f.peers[2]).unwrap(),
        RuntimePollV1::Pending
    );
    assert_eq!(state(&f.context, f.destination), before);
    assert_eq!(f.context.submissions[&f.peers[1].id].dependency_retains, 1);
    assert!(f.context.release_allocation(f.destination).is_err());
    f.cancel_all();
}

#[test]
fn compute_peer_gather_rejected_queue_admission_refunds_only_new_reservations() {
    let mut f = Gather::new(2);
    f.enqueue(0, 11, 31);
    let before = state(&f.context, f.destination);
    let writers = f.context.version_journal_writer_records_v1();
    let readers = f.context.version_journal_read_records_v1();
    f.context.backend.copy_failure = MockMemoryFailure::Rejected;
    assert!(matches!(
        f.context.peer_copy(
            f.peer_stream,
            span(f.sources[1], RuntimeAccessV1::Read, 3, 31),
            span(f.destination, RuntimeAccessV1::Write, 17, 31),
            &[f.events[1], f.peer_events[0]]
        ),
        Err(RuntimeErrorV1::BackendRejected(_))
    ));
    assert_eq!(state(&f.context, f.destination), before);
    assert_eq!(f.context.version_journal_writer_records_v1(), writers);
    assert_eq!(f.context.version_journal_read_records_v1(), readers);
    assert_eq!(f.context.scalar_peer_copies.len(), 1);
    assert_eq!(f.context.submissions[&f.peers[0].id].dependency_retains, 0);
    assert_eq!(
        f.context.submissions[&f.producers[1].id].dependency_retains,
        0
    );
    assert!(!f.context.is_terminal());
    f.cancel_all();
}

#[test]
fn compute_peer_gather_uncertain_predecessor_retains_entire_ordered_chain() {
    for fault in [Observation::Terminal, Observation::Panic] {
        let mut f = Gather::new(3);
        for index in 0..3 {
            f.enqueue(index, 11 + index as u64 * 7, 31);
        }
        f.release_events();
        let before = state(&f.context, f.destination);
        f.context
            .backend
            .producer_launch
            .observations
            .insert(f.peers[0].backend_submission, fault);
        let result = catch_unwind(AssertUnwindSafe(|| f.context.poll(&mut f.peers[0])));
        match (fault, result) {
            (Observation::Terminal, Ok(Err(RuntimeErrorV1::BackendTerminal(_))))
            | (Observation::Panic, Err(_)) => {}
            _ => panic!("uncertain predecessor diagnostic was lost"),
        }
        assert!(f.context.is_terminal());
        assert_eq!(
            state(&f.context, f.destination).content_lineage,
            before.content_lineage
        );
        assert_eq!(f.context.scalar_peer_copies.len(), 3);
        assert_eq!(f.context.submissions[&f.peers[0].id].dependency_retains, 1);
        assert_eq!(f.context.submissions[&f.peers[1].id].dependency_retains, 1);
        assert_eq!(f.context.version_journal_read_records_v1(), Some(3));
        assert!(!f.context.cleanup().is_complete());
    }
}

#[test]
fn compute_peer_gather_chain_validation_visits_each_predecessor_once() {
    let mut f = Gather::new(7);
    for index in 0..7 {
        f.enqueue(index, 11 + index as u64 * 7, 31);
        let id = f.peers[index].id;
        let (result, visits) = f
            .context
            .count_compute_peer_validations_for_test_v1(|context| {
                context.validate_scalar_peer_custody_v1(id)
            });
        assert!(result.is_ok());
        assert_eq!(visits, index + 1);
        let (result, visits) = f
            .context
            .count_compute_peer_validations_for_test_v1(|context| {
                context.check_operation_custody_v1(id)
            });
        assert!(result.is_ok());
        assert_eq!(visits, index + 1);
        let (result, visits) = f
            .context
            .count_compute_peer_validations_for_test_v1(|context| {
                context.validate_pending_peer_copy_roots_v1(id)
            });
        assert!(result.is_ok());
        assert_eq!(visits, 2 * (index + 1));
    }
    f.cancel_all();
}
