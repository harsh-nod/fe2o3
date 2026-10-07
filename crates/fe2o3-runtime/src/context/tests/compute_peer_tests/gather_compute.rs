//! Context/journal composition over existing mock operations, not native kernel evidence.

use super::gather::Gather;
use super::*;
use fe2o3_runtime_model::ContextProducerReadStatusV1;

type Consumer = RuntimeSubmissionV1<MixedArguments>;

struct ConsumerFixture {
    gather: Gather,
    stream: RuntimeStreamIdV1,
    kernel: TypedRuntimeKernelV1<MixedArguments>,
    stable: RuntimeAllocationIdV1,
    output: RuntimeAllocationIdV1,
}

impl ConsumerFixture {
    fn new(count: usize) -> Self {
        let mut gather = Gather::new(count);
        let device = gather.context.devices()[1].id();
        let stream = gather.context.create_stream(device).unwrap();
        let module = gather
            .context
            .load_module(device, b"gather-compute-tests")
            .unwrap();
        let kernel = gather
            .context
            .resolve_kernel::<MixedArguments>(module, "mixed")
            .unwrap();
        let stable = gather
            .context
            .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 47, 16)
            .unwrap();
        let output = gather
            .context
            .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 61, 16)
            .unwrap();
        gather
            .context
            .write_allocation(stable, 0, &[19; 47])
            .unwrap();
        gather
            .context
            .write_allocation(output, 0, &[23; 61])
            .unwrap();
        Self {
            gather,
            stream,
            kernel,
            stable,
            output,
        }
    }

    fn enqueue_gather(&mut self, overlap: bool) {
        for index in 0..self.gather.producers.len() {
            self.gather
                .enqueue(index, 11 + index as u64 * if overlap { 7 } else { 43 }, 31);
        }
    }

    fn launch(
        &mut self,
        bindings: Vec<RuntimeMemoryRegionV1>,
        dependencies: &[RuntimeEventIdV1],
    ) -> Result<Consumer, RuntimeErrorV1<MockError>> {
        self.gather.context.launch_producer_aware_v1(
            self.stream,
            &self.kernel,
            &MixedArguments(bindings),
            geometry(),
            dependencies,
        )
    }

    fn bindings(&self, variant: usize) -> Vec<RuntimeMemoryRegionV1> {
        let allocation = self.gather.destination;
        match variant {
            0 => vec![span(allocation, RuntimeAccessV1::Read, 0, 257)],
            1 => vec![span(allocation, RuntimeAccessV1::Read, 1, 255)],
            2 => vec![
                span(allocation, RuntimeAccessV1::Read, 0, 13),
                span(allocation, RuntimeAccessV1::Read, 29, 95),
                span(allocation, RuntimeAccessV1::Read, 250, 7),
            ],
            _ => unreachable!(),
        }
    }

    fn finish_physical(&mut self, consumer: &Consumer) {
        self.gather.finish_backend();
        assert!(
            self.gather
                .context
                .backend
                .finish_producer_launch_test_v1(consumer.backend_submission, true)
        );
    }

    fn reconcile(&mut self, consumer: &mut Consumer) {
        for _ in 0..self.gather.peers.len() * 4 + 16 {
            if self.gather.context.poll(consumer).unwrap() == RuntimePollV1::Succeeded {
                return;
            }
        }
        panic!("gathered compute did not reconcile");
    }

    fn cancel_gather(&mut self) {
        self.gather.release_events();
        self.gather.context.backend.cancel_before_publication = true;
        for peer in self.gather.peers.iter_mut().rev() {
            self.gather.context.cancel(peer).unwrap();
        }
        for producer in self.gather.producers.iter_mut().rev() {
            self.gather.context.cancel(producer).unwrap();
        }
    }

    fn clean(mut self) {
        self.gather.release_results();
        assert!(self.gather.context.cleanup().is_complete());
    }
}

#[test]
fn gathered_compute_full_contained_and_aliased_reads_reconcile_only_from_final_consumer() {
    for count in [1, 2, 3, 5] {
        for overlap in [false, true] {
            for variant in 0..3 {
                let mut f = ConsumerFixture::new(count);
                let before = state(&f.gather.context, f.gather.destination);
                f.enqueue_gather(overlap);
                let event = *f.gather.peer_events.last().unwrap();
                let bindings = f.bindings(variant);
                let mut consumer = f.launch(bindings.clone(), &[event]).unwrap();
                let tail = f.gather.peers.last().unwrap().id;
                let marker = f.gather.context.submissions[&consumer.id]
                    .journal_producer_read
                    .unwrap();
                assert_eq!(marker.count, 1);
                assert_eq!(marker.active_first_for_test().is_some(), count == 1);
                assert_eq!(
                    f.gather
                        .context
                        .directed_input_status_v1(consumer.id)
                        .unwrap(),
                    Some(ContextProducerReadStatusV1::Pending)
                );
                assert_eq!(
                    f.gather
                        .context
                        .completion_parent_depth_v1(consumer.id)
                        .unwrap(),
                    count + 2
                );
                let mut expected_ids = Vec::new();
                let callbacks = Arc::new(Mutex::new(Vec::new()));
                for producer in &f.gather.producers {
                    let calls = Arc::clone(&callbacks);
                    let id = producer.id;
                    expected_ids.push(id);
                    f.gather
                        .context
                        .on_completion(producer, move |status| {
                            calls.lock().unwrap().push((id, status));
                        })
                        .unwrap();
                }
                for peer in &f.gather.peers {
                    let calls = Arc::clone(&callbacks);
                    let id = peer.id;
                    expected_ids.push(id);
                    f.gather
                        .context
                        .on_completion(peer, move |status| {
                            calls.lock().unwrap().push((id, status));
                        })
                        .unwrap();
                }
                let calls = Arc::clone(&callbacks);
                let id = consumer.id;
                expected_ids.push(id);
                f.gather
                    .context
                    .on_completion(&consumer, move |status| {
                        calls.lock().unwrap().push((id, status));
                    })
                    .unwrap();
                f.gather.release_events();
                assert!(f.gather.context.events.is_empty());
                assert_eq!(f.gather.context.submissions[&tail].dependency_retains, 1);
                assert!(
                    f.gather
                        .context
                        .release_allocation(f.gather.destination)
                        .is_err()
                );
                assert!(
                    f.gather
                        .context
                        .write_allocation(f.gather.destination, 0, &[0])
                        .is_err()
                );
                let pending = state(&f.gather.context, f.gather.destination);
                f.finish_physical(&consumer);
                assert_eq!(
                    f.gather.context.poll(&mut consumer).unwrap(),
                    RuntimePollV1::Pending
                );
                assert_eq!(state(&f.gather.context, f.gather.destination), pending);
                assert!(callbacks.lock().unwrap().is_empty());
                f.reconcile(&mut consumer);
                let actual_reads: Vec<_> = f
                    .gather
                    .context
                    .backend
                    .observed_kernel_reads
                    .iter()
                    .filter(|read| read.submission == consumer.backend_submission)
                    .collect();
                assert_eq!(actual_reads.len(), bindings.len());
                for (read, binding) in actual_reads.into_iter().zip(&bindings) {
                    assert_eq!(read.binding.region.byte_offset, binding.byte_offset);
                    assert_eq!(read.binding.region.byte_len, binding.byte_len);
                    let start = binding.byte_offset as usize;
                    assert_eq!(
                        read.bytes,
                        f.gather.expected[start..start + binding.byte_len as usize]
                    );
                }
                let delivered = callbacks.lock().unwrap();
                assert_eq!(delivered.len(), expected_ids.len());
                assert!(
                    delivered
                        .iter()
                        .all(|(_, status)| *status == RuntimeCompletionStatusV1::Succeeded)
                );
                assert_eq!(delivered.last().unwrap().0, consumer.id);
                let mut actual_ids: Vec<_> = delivered.iter().map(|(id, _)| *id).collect();
                expected_ids.sort_unstable();
                actual_ids.sort_unstable();
                assert_eq!(actual_ids, expected_ids);
                drop(delivered);
                assert_eq!(
                    state(&f.gather.context, f.gather.destination).content_lineage,
                    before.content_lineage + count as u64
                );
                assert_eq!(f.gather.context.version_journal_read_records_v1(), Some(0));
                assert_eq!(
                    f.gather.context.version_journal_writer_records_v1(),
                    Some(0)
                );
                f.gather.context.release_submission(consumer).unwrap();
                f.clean();
            }
        }
    }
}

#[test]
fn gathered_compute_exact_latest_event_and_every_read_alias_are_checked_before_effects() {
    for mode in 0..11 {
        let mut f = ConsumerFixture::new(2);
        f.enqueue_gather(false);
        let tail = *f.gather.peer_events.last().unwrap();
        let alias = f
            .gather
            .context
            .record_event(f.gather.peers.last().unwrap())
            .unwrap();
        let mut dependencies = vec![tail];
        let mut bindings = f.bindings(0);
        let expected = match mode {
            0 => {
                dependencies.clear();
                RuntimeValidationErrorV1::ContextReserved
            }
            1 => {
                dependencies[0] = f.gather.peer_events[0];
                RuntimeValidationErrorV1::ContextReserved
            }
            2 => {
                dependencies.push(tail);
                RuntimeValidationErrorV1::DuplicateDependency
            }
            3 => {
                dependencies.push(alias);
                RuntimeValidationErrorV1::DuplicateDependency
            }
            4 => {
                dependencies[0] = f.gather.events[0];
                RuntimeValidationErrorV1::WrongDevice
            }
            5 => {
                bindings[0].access = RuntimeAccessV1::ReadWrite;
                RuntimeValidationErrorV1::ContextReserved
            }
            6 => {
                bindings.push(span(f.gather.destination, RuntimeAccessV1::Write, 19, 1));
                RuntimeValidationErrorV1::ContextReserved
            }
            7 => {
                bindings[0].byte_offset = 1;
                RuntimeValidationErrorV1::InvalidRange
            }
            8 => {
                bindings[0].byte_offset = u64::MAX;
                bindings[0].byte_len = 2;
                RuntimeValidationErrorV1::InvalidRange
            }
            9 => {
                bindings[0].byte_len = 0;
                RuntimeValidationErrorV1::InvalidRange
            }
            10 => {
                f.gather.context.release_event(tail).unwrap();
                f.gather.peer_events.pop();
                RuntimeValidationErrorV1::UnknownEvent
            }
            _ => unreachable!(),
        };
        let calls = f.gather.context.backend.submit_count;
        let reads = f.gather.context.version_journal_read_records_v1();
        let writers = f.gather.context.version_journal_writer_records_v1();
        let destination = state(&f.gather.context, f.gather.destination);
        validation(f.launch(bindings, &dependencies), expected);
        assert_eq!(f.gather.context.backend.submit_count, calls);
        assert_eq!(f.gather.context.version_journal_read_records_v1(), reads);
        assert_eq!(
            f.gather.context.version_journal_writer_records_v1(),
            writers
        );
        assert_eq!(state(&f.gather.context, f.gather.destination), destination);
        f.gather.context.release_event(alias).unwrap();
        f.cancel_gather();
        f.clean();
    }
}

#[test]
fn gathered_compute_requires_frame_opt_in_without_weakening_copied_range_coverage() {
    let mut f = ConsumerFixture::new(1);
    f.gather.context.backend.ordered_compute_peer = false;
    f.enqueue_gather(false);
    let event = f.gather.peer_events[0];
    let calls = f.gather.context.backend.submit_count;
    validation(
        f.launch(f.bindings(0), &[event]),
        RuntimeValidationErrorV1::ContextReserved,
    );
    assert_eq!(f.gather.context.backend.submit_count, calls);
    let mut consumer = f
        .launch(
            vec![span(f.gather.destination, RuntimeAccessV1::Read, 11, 31)],
            &[event],
        )
        .unwrap();
    f.gather.context.backend.cancel_before_publication = true;
    assert_eq!(
        f.gather.context.cancel(&mut consumer).unwrap(),
        RuntimeCancellationV1::Cancelled
    );
    f.gather.context.release_submission(consumer).unwrap();
    f.cancel_gather();
    f.clean();
}

#[test]
fn gathered_compute_mixed_stable_active_and_queued_leases_are_atomic_and_exact() {
    let mut f = ConsumerFixture::new(3);
    f.enqueue_gather(true);
    let tail = *f.gather.peer_events.last().unwrap();
    let device = f.gather.context.devices()[1].id();
    let active = f
        .gather
        .context
        .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 37, 16)
        .unwrap();
    f.gather
        .context
        .write_allocation(active, 0, &[3; 37])
        .unwrap();
    let producer = f
        .launch(vec![span(active, RuntimeAccessV1::Write, 0, 37)], &[])
        .unwrap();
    let event = f.gather.context.record_event(&producer).unwrap();
    let mut consumer = f
        .launch(
            vec![
                span(f.gather.destination, RuntimeAccessV1::Read, 0, 257),
                span(f.stable, RuntimeAccessV1::Read, 3, 41),
                span(active, RuntimeAccessV1::Read, 2, 31),
                span(f.output, RuntimeAccessV1::Write, 0, 61),
            ],
            &[tail, event],
        )
        .unwrap();
    let record = f.gather.context.submissions[&consumer.id];
    assert_eq!(record.journal_read.unwrap().count, 1);
    assert_eq!(record.journal_producer_read.unwrap().count, 2);
    assert_eq!(f.gather.context.version_journal_read_records_v1(), Some(6));
    f.gather.release_events();
    f.gather.context.release_event(event).unwrap();
    f.finish_physical(&consumer);
    f.reconcile(&mut consumer);
    let reads: Vec<_> = f
        .gather
        .context
        .backend
        .observed_kernel_reads
        .iter()
        .filter(|read| read.submission == consumer.backend_submission)
        .collect();
    assert_eq!(reads.len(), 3);
    assert_eq!(reads[0].bytes, f.gather.expected);
    assert_eq!(reads[1].bytes, vec![19; 41]);
    assert_eq!(
        reads[2].bytes,
        vec![(producer.backend_submission as u8).wrapping_add(37); 31]
    );
    let mut output = [0; 61];
    f.gather
        .context
        .read_allocation(f.output, 0, &mut output)
        .unwrap();
    assert_eq!(
        output,
        [(consumer.backend_submission as u8).wrapping_add(37); 61]
    );
    assert_eq!(f.gather.context.version_journal_read_records_v1(), Some(0));
    f.gather.context.release_submission(consumer).unwrap();
    f.gather.context.release_submission(producer).unwrap();
    f.clean();
}

#[test]
fn gathered_compute_rejection_and_cancellation_refund_only_consumer_custody() {
    for reject in [false, true] {
        let mut f = ConsumerFixture::new(3);
        f.enqueue_gather(false);
        let tail = *f.gather.peer_events.last().unwrap();
        let destination = state(&f.gather.context, f.gather.destination);
        let output_lineage = state(&f.gather.context, f.output).content_lineage;
        let mut bindings = f.bindings(0);
        bindings.push(span(f.output, RuntimeAccessV1::Write, 0, 61));
        if reject {
            f.gather.context.backend.launch_failure = MockMemoryFailure::Rejected;
            assert!(matches!(
                f.launch(bindings, &[tail]),
                Err(RuntimeErrorV1::BackendRejected(_))
            ));
        } else {
            let mut consumer = f.launch(bindings, &[tail]).unwrap();
            f.gather.context.backend.cancel_before_publication = true;
            assert_eq!(
                f.gather.context.cancel(&mut consumer).unwrap(),
                RuntimeCancellationV1::Cancelled
            );
            f.gather.context.release_submission(consumer).unwrap();
        }
        assert_eq!(state(&f.gather.context, f.gather.destination), destination);
        assert_eq!(
            state(&f.gather.context, f.output).content_lineage,
            output_lineage
        );
        assert_eq!(f.gather.context.version_journal_read_records_v1(), Some(3));
        assert_eq!(
            f.gather.context.submissions[&f.gather.peers[2].id].dependency_retains,
            0
        );
        assert_eq!(
            f.gather
                .context
                .query_submission(&f.gather.peers[2])
                .unwrap(),
            RuntimeCompletionStatusV1::Pending
        );
        f.cancel_gather();
        f.clean();
    }
}

#[test]
fn gathered_compute_retained_lease_and_frame_corruption_reject_before_observation() {
    for queued in [false, true] {
        for corruption in 0..7 {
            let mut f = ConsumerFixture::new(if queued { 2 } else { 1 });
            f.enqueue_gather(false);
            let event = *f.gather.peer_events.last().unwrap();
            let mut consumer = f.launch(f.bindings(0), &[event]).unwrap();
            f.gather.release_events();
            let tail = f.gather.peers.last().unwrap().id;
            match corruption {
                0..=3 => {
                    let versions = f.gather.context.versions.as_mut().unwrap();
                    if queued {
                        versions.corrupt_queued_read_for_test_v1(consumer.id, corruption);
                    } else {
                        versions.corrupt_active_input_request_for_test_v1(consumer.id, corruption);
                    }
                }
                4 => {
                    f.gather
                        .context
                        .scalar_peer_copies
                        .get_mut(&tail)
                        .unwrap()
                        .destination
                        .region
                        .byte_offset += 1;
                }
                5 => {
                    f.gather
                        .context
                        .producer_launches
                        .get_mut(&consumer.id)
                        .unwrap()
                        .dependencies[0]
                        .submission = consumer.id;
                }
                6 => {
                    f.gather
                        .context
                        .producer_launches
                        .get_mut(&consumer.id)
                        .unwrap()
                        .state
                        .depth = 1;
                }
                _ => unreachable!(),
            }
            let calls = f
                .gather
                .context
                .backend
                .producer_launch_observation_count_for_test_v1();
            let polls = f.gather.context.backend.poll_call_count;
            let lineage = state(&f.gather.context, f.gather.destination).content_lineage;
            validation(
                f.gather.context.poll(&mut consumer),
                RuntimeValidationErrorV1::InvalidBackendDescription,
            );
            assert_eq!(
                f.gather
                    .context
                    .backend
                    .producer_launch_observation_count_for_test_v1(),
                calls
            );
            assert_eq!(f.gather.context.backend.poll_call_count, polls);
            assert_eq!(
                state(&f.gather.context, f.gather.destination).content_lineage,
                lineage
            );
            assert!(f.gather.context.is_terminal());
            assert_eq!(f.gather.context.submissions[&tail].dependency_retains, 1);
            assert!(!f.gather.context.cleanup().is_complete());
        }
    }
}

#[test]
fn gathered_compute_cancelled_middle_never_reparents_or_reads_the_frame() {
    let mut f = ConsumerFixture::new(3);
    f.enqueue_gather(false);
    let event = *f.gather.peer_events.last().unwrap();
    let mut consumer = f.launch(f.bindings(0), &[event]).unwrap();
    f.gather.release_events();
    let lineage = state(&f.gather.context, f.gather.destination).content_lineage;
    f.gather.context.backend.cancel_before_publication = true;
    f.gather.context.cancel(&mut f.gather.peers[1]).unwrap();
    let tail = &mut f.gather.peers[2];
    f.gather
        .context
        .backend
        .producer_launch
        .observations
        .insert(tail.backend_submission, Observation::Failed);
    assert_eq!(
        f.gather.context.poll(tail).unwrap(),
        RuntimePollV1::Failed { code: 7 }
    );
    f.gather
        .context
        .backend
        .producer_launch
        .observations
        .insert(consumer.backend_submission, Observation::Failed);
    assert_eq!(
        f.gather.context.poll(&mut consumer).unwrap(),
        RuntimePollV1::Failed { code: 7 }
    );
    assert!(
        f.gather
            .context
            .backend
            .observed_kernel_reads
            .iter()
            .all(|read| read.submission != consumer.backend_submission)
    );
    assert_eq!(
        state(&f.gather.context, f.gather.destination).content_lineage,
        lineage
    );
    assert_eq!(
        f.gather.context.submissions[&f.gather.peers[2].id].dependency_retains,
        0
    );
    f.gather.context.release_submission(consumer).unwrap();
    f.cancel_gather();
    f.clean();
}

#[test]
fn gathered_compute_descended_unknown_terminal_and_unwind_never_promote_success() {
    for fault in [
        Observation::Quiescent,
        Observation::Terminal,
        Observation::Panic,
    ] {
        let mut f = ConsumerFixture::new(2);
        f.enqueue_gather(true);
        let event = *f.gather.peer_events.last().unwrap();
        let mut bindings = f.bindings(0);
        bindings.push(span(f.output, RuntimeAccessV1::Write, 0, 61));
        let mut consumer = f.launch(bindings, &[event]).unwrap();
        f.gather.release_events();
        let lineage = state(&f.gather.context, f.output).content_lineage;
        f.finish_physical(&consumer);
        f.gather
            .context
            .backend
            .producer_launch
            .observations
            .insert(f.gather.peers[0].backend_submission, fault);
        let mut reached = false;
        for _ in 0..24 {
            let result = catch_unwind(AssertUnwindSafe(|| f.gather.context.poll(&mut consumer)));
            match (fault, result) {
                (_, Ok(Ok(RuntimePollV1::Pending))) => continue,
                (Observation::Quiescent, Ok(Err(RuntimeErrorV1::BackendQuiescent(_)))) => {
                    assert_eq!(
                        f.gather.context.query_submission(&consumer).unwrap(),
                        RuntimeCompletionStatusV1::QuiescentWithoutResult
                    );
                    assert_eq!(
                        writer_state(&f.gather.context, f.output),
                        ContextWriterStateV1::Unknown { member_count: 1 }
                    );
                    assert!(!f.gather.context.is_terminal());
                }
                (Observation::Terminal, Ok(Err(RuntimeErrorV1::BackendTerminal(_))))
                | (Observation::Panic, Err(_)) => {
                    assert!(f.gather.context.is_terminal());
                    assert_eq!(
                        f.gather.context.submissions[&f.gather.peers[1].id].dependency_retains,
                        1
                    );
                    assert!(!f.gather.context.cleanup().is_complete());
                }
                _ => panic!("gathered compute uncertainty lost its classification"),
            }
            reached = true;
            break;
        }
        assert!(reached);
        assert_eq!(state(&f.gather.context, f.output).content_lineage, lineage);
        assert_ne!(
            f.gather.context.submissions[&consumer.id].status,
            RuntimeCompletionStatusV1::Succeeded
        );
    }
}

#[test]
fn gathered_compute_resolved_frame_survives_writer_slot_reuse_and_disposed_oldest_source() {
    let mut f = ConsumerFixture::new(2);
    f.enqueue_gather(false);
    let event = *f.gather.peer_events.last().unwrap();
    let writer = f.gather.context.submissions[&f.gather.peers[1].id]
        .journal_writer
        .unwrap();
    let mut consumer = f.launch(f.bindings(0), &[event]).unwrap();
    f.gather.release_events();
    f.gather.finish_backend();
    for _ in 0..16 {
        if f.gather.context.poll(&mut f.gather.peers[1]).unwrap() == RuntimePollV1::Succeeded {
            break;
        }
    }
    assert_eq!(
        f.gather
            .context
            .query_submission(&f.gather.peers[1])
            .unwrap(),
        RuntimeCompletionStatusV1::Succeeded
    );
    assert_eq!(
        f.gather
            .context
            .directed_input_status_v1(consumer.id)
            .unwrap(),
        Some(ContextProducerReadStatusV1::Success)
    );
    assert!(
        f.gather
            .context
            .write_allocation(f.gather.destination, 0, &[0])
            .is_err()
    );
    let oldest_peer = f.gather.peers.remove(0);
    let oldest_producer = f.gather.producers.remove(0);
    let oldest_source = f.gather.sources.remove(0);
    f.gather.context.release_submission(oldest_peer).unwrap();
    f.gather
        .context
        .release_submission(oldest_producer)
        .unwrap();
    f.gather.context.release_allocation(oldest_source).unwrap();
    let device = f.gather.context.devices()[1].id();
    let mut replacements = Vec::new();
    let mut reused = false;
    for _ in 0..4 {
        let allocation = f
            .gather
            .context
            .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 17, 16)
            .unwrap();
        let replacement = f
            .launch(vec![span(allocation, RuntimeAccessV1::Write, 0, 17)], &[])
            .unwrap();
        let current = f.gather.context.submissions[&replacement.id]
            .journal_writer
            .unwrap();
        if current.slot == writer.slot {
            assert_ne!(current, writer);
            reused = true;
        }
        replacements.push(replacement);
    }
    assert!(reused, "the old tail writer slot must actually be reused");
    assert!(
        f.gather
            .context
            .backend
            .finish_producer_launch_test_v1(consumer.backend_submission, true)
    );
    f.reconcile(&mut consumer);
    let read = f
        .gather
        .context
        .backend
        .observed_kernel_reads
        .iter()
        .find(|read| read.submission == consumer.backend_submission)
        .unwrap();
    assert_eq!(read.bytes, f.gather.expected);
    f.gather.context.release_submission(consumer).unwrap();
    f.gather.context.backend.cancel_before_publication = true;
    for mut replacement in replacements.into_iter().rev() {
        f.gather.context.cancel(&mut replacement).unwrap();
        f.gather.context.release_submission(replacement).unwrap();
    }
    f.clean();
}
