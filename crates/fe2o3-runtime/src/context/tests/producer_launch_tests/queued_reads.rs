use super::*;
use crate::context::versions::completion_faults::{
    CompletionJournalFailureV1 as Failure, CompletionJournalPointV1 as Point,
    CompletionJournalStageV1 as Stage,
};

#[test]
fn queued_reads_producer_quiescence_and_terminal_do_not_publish_retained_success() {
    for terminal in [false, true] {
        let mut f = Fixture::new(8);
        let (mut a, b, events) = chain(&mut f);
        let mut c = f.launch(2, reads(&f), &[events[1]]).unwrap();
        for event in events {
            f.context.release_event(event).unwrap();
        }
        f.complete(&mut a);
        assert_eq!(f.context.poll(&mut c).unwrap(), RuntimePollV1::Pending);
        f.context.backend.producer_launch.observations.insert(
            b.backend_submission,
            if terminal {
                Observation::Terminal
            } else {
                Observation::Quiescent
            },
        );
        let result = f.context.poll(&mut c);
        if terminal {
            assert!(matches!(result, Err(RuntimeErrorV1::BackendTerminal(_))));
            assert!(f.context.terminal);
            assert_eq!(f.context.version_journal_read_records_v1(), Some(1));
            assert_eq!(
                f.context.query_submission(&c).unwrap(),
                RuntimeCompletionStatusV1::Pending
            );
            assert_eq!(f.context.submissions[&b.id].dependency_retains, 1);
            assert!(!f.context.cleanup().is_complete());
        } else {
            assert!(matches!(result, Err(RuntimeErrorV1::BackendQuiescent(_))));
            assert!(!f.context.terminal);
            for submission in [&b, &c] {
                assert_eq!(
                    f.context.query_submission(submission).unwrap(),
                    RuntimeCompletionStatusV1::QuiescentWithoutResult
                );
            }
            assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
            assert_eq!(f.context.submissions[&b.id].dependency_retains, 0);
            assert!(f.context.cleanup().is_complete());
        }
    }
}

#[test]
fn queued_reads_corruption_rejects_before_backend_before_and_after_producer_slot_reuse() {
    for resolved in [false, true] {
        for corruption in 0..4 {
            let mut f = Fixture::new(8);
            let (_, mut b, events) = chain(&mut f);
            let producer = f.context.submissions[&b.id].journal_writer.unwrap();
            let mut c = f.launch(2, reads(&f), &[events[1]]).unwrap();
            if resolved {
                f.complete(&mut b);
                let d = f.launch(0, vec![output(&f, 4)], &[]).unwrap();
                let reused = f.context.submissions[&d.id].journal_writer.unwrap();
                assert_eq!(producer.slot, reused.slot);
                assert_ne!(producer, reused);
            }
            let before = f.allocations.map(|id| state(&f.context, id));
            let calls = f.context.backend.producer_launch.calls.len();
            f.context
                .versions
                .as_mut()
                .unwrap()
                .corrupt_queued_read_for_test_v1(c.id, corruption);
            validation(
                f.context.poll(&mut c),
                RuntimeValidationErrorV1::InvalidBackendDescription,
            );
            assert!(f.context.terminal);
            assert_eq!(f.context.backend.producer_launch.calls.len(), calls);
            assert_eq!(f.allocations.map(|id| state(&f.context, id)), before);
            assert_eq!(f.context.version_journal_read_records_v1(), Some(1));
            assert_eq!(
                f.context.query_submission(&c).unwrap(),
                RuntimeCompletionStatusV1::Pending
            );
            assert_eq!(f.context.submissions[&b.id].dependency_retains, 1);
        }
    }
}

#[test]
fn queued_reads_release_fault_preserves_exact_active_prefix_and_quarantines_remainder() {
    for panic in [false, true] {
        let mut f = Fixture::new(4);
        let (_, mut b, events) = chain(&mut f);
        let mut p = f.launch(0, vec![output(&f, 0)], &[]).unwrap();
        let pe = f.context.record_event(&p).unwrap();
        let mut regions = reads(&f);
        regions.push(span(f.allocations[0], RuntimeAccessV1::Read, 0, 64));
        regions.push(span(f.allocations[1], RuntimeAccessV1::Read, 0, 64));
        regions.push(output(&f, 3));
        let mut c = f.launch(2, regions, &[pe, events[1]]).unwrap();
        f.complete(&mut b);
        f.complete(&mut p);
        let marker = f.context.submissions[&c.id].journal_producer_read;
        let callbacks = Arc::new(Mutex::new(Vec::new()));
        let calls = callbacks.clone();
        f.context
            .on_completion(&c, move |status| calls.lock().unwrap().push(status))
            .unwrap();
        let payload = Arc::new(());
        f.context
            .versions
            .as_mut()
            .unwrap()
            .inject_completion_fault_for_test_v1(
                c.id,
                Stage::Producer,
                Point::BetweenProducerClasses,
                if panic {
                    Failure::Panic(Box::new(payload.clone()))
                } else {
                    Failure::Error
                },
            );
        f.context.backend.cancel_before_publication = true;
        validation(
            f.context.cancel(&mut c),
            RuntimeValidationErrorV1::InvalidBackendDescription,
        );
        assert!(f.context.terminal);
        assert_eq!(Arc::strong_count(&payload), if panic { 2 } else { 1 });
        assert!(callbacks.lock().unwrap().is_empty());
        assert_eq!(f.context.version_journal_read_records_v1(), Some(1));
        let versions = f.context.versions.as_mut().unwrap();
        assert_eq!(
            versions.completion_roots_for_test_v1(c.id),
            (false, true, true)
        );
        assert!(!versions.completion_fault_pending_for_test_v1());
        let journal = versions.read_leases_for_test_v1();
        for (index, expected) in [(0, 0), (1, 0), (2, 1)] {
            assert_eq!(
                journal.reader_count(
                    f.context.allocations[&f.allocations[index]]
                        .journal
                        .unwrap()
                ),
                Ok(expected)
            );
        }
        assert_eq!(f.context.submissions[&c.id].journal_producer_read, marker);
        assert_eq!(
            f.context.query_submission(&c).unwrap(),
            RuntimeCompletionStatusV1::Pending
        );
        assert_eq!(f.context.submissions[&b.id].dependency_retains, 1);
        assert_eq!(f.context.submissions[&p.id].dependency_retains, 1);
        let before = f.snapshot();
        validation(
            f.context.poll(&mut c),
            RuntimeValidationErrorV1::ContextTerminal,
        );
        assert_eq!(f.snapshot(), before);
    }
}

fn output(f: &Fixture, index: usize) -> RuntimeMemoryRegionV1 {
    span(f.allocations[index], RuntimeAccessV1::Write, 0, 64)
}

fn chain(f: &mut Fixture) -> (Submission, Submission, [RuntimeEventIdV1; 2]) {
    let a = f.launch(0, vec![output(f, 2)], &[]).unwrap();
    let ae = f.context.record_event(&a).unwrap();
    let b = f.launch(1, vec![output(f, 2)], &[ae]).unwrap();
    let be = f.context.record_event(&b).unwrap();
    (a, b, [ae, be])
}

fn reads(f: &Fixture) -> Vec<RuntimeMemoryRegionV1> {
    vec![
        span(f.allocations[2], RuntimeAccessV1::Read, 8, 16),
        span(f.allocations[2], RuntimeAccessV1::Read, 40, 8),
    ]
}

#[test]
fn queued_reads_exact_tail_bytes_and_all_completion_ingresses() {
    for ingress in 0..7 {
        let mut f = Fixture::new(8);
        let before = state(&f.context, f.allocations[2]);
        let (a, b, events) = chain(&mut f);
        for bad_events in [vec![], vec![events[0]]] {
            let before = f.snapshot();
            validation(
                f.launch(2, reads(&f), &bad_events),
                RuntimeValidationErrorV1::ContextReserved,
            );
            assert_eq!(f.snapshot(), before);
        }
        let mut c = f.launch(2, reads(&f), &[events[1]]).unwrap();
        let completion = f.context.record_event(&c).unwrap();
        for event in events {
            f.context.release_event(event).unwrap();
        }
        assert_eq!(f.context.version_journal_read_records_v1(), Some(1));
        let callbacks = Arc::new(Mutex::new(Vec::new()));
        for submission in [&a, &b, &c] {
            let calls = callbacks.clone();
            let id = submission.id;
            f.context
                .on_completion(submission, move |_| calls.lock().unwrap().push(id))
                .unwrap();
        }
        assert!(
            f.context
                .backend
                .finish_producer_launch_test_v1(c.backend_submission, true)
        );
        let observed: Vec<_> = f
            .context
            .backend
            .observed_kernel_reads
            .iter()
            .filter(|read| read.submission == c.backend_submission)
            .collect();
        assert_eq!(observed.len(), 2);
        for read in observed {
            assert_eq!(
                read.bytes,
                vec![output_byte(b.backend_submission); read.binding.region.byte_len as usize]
            );
        }
        // Physical completion is not authority to release logical custody.
        validation(
            f.context.read_allocation(f.allocations[2], 0, &mut [0; 64]),
            RuntimeValidationErrorV1::ContextReserved,
        );
        validation(
            f.launch(0, vec![output(&f, 2)], &[completion]),
            RuntimeValidationErrorV1::ContextReserved,
        );
        for _ in 0..16 {
            match ingress {
                0 => {
                    f.context.poll(&mut c).unwrap();
                }
                1 => {
                    f.context.wait(&mut c, Duration::ZERO).unwrap();
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
                        .drain(&mut c, Instant::now() + Duration::from_secs(1))
                        .unwrap();
                }
                6 => {
                    f.context.poll_async_drain_v1(c.id).unwrap();
                }
                _ => unreachable!(),
            }
            if f.context.query_submission(&c).unwrap() == RuntimeCompletionStatusV1::Succeeded {
                break;
            }
        }
        assert_eq!(
            f.context.query_submission(&c).unwrap(),
            RuntimeCompletionStatusV1::Succeeded
        );
        assert_eq!(*callbacks.lock().unwrap(), vec![a.id, b.id, c.id]);
        let after = state(&f.context, f.allocations[2]);
        assert_eq!(after.attempt_epoch, before.attempt_epoch + 2);
        assert_eq!(after.content_lineage, before.content_lineage + 2);
        assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn queued_reads_resolved_custody_blocks_writes_and_retirement_until_consumer_cancel() {
    for resolve in [false, true] {
        let mut f = Fixture::new(8);
        let (a, mut b, events) = chain(&mut f);
        let mut c = f.launch(2, reads(&f), &[events[1]]).unwrap();
        if resolve {
            f.complete(&mut b);
        }
        let before = state(&f.context, f.allocations[2]);
        validation(
            f.context.write_allocation(f.allocations[2], 0, &[1]),
            RuntimeValidationErrorV1::ContextReserved,
        );
        validation(
            f.context.release_allocation(f.allocations[2]),
            RuntimeValidationErrorV1::ContextReserved,
        );
        validation(
            f.launch(0, vec![output(&f, 2)], &[events[1]]),
            RuntimeValidationErrorV1::ContextReserved,
        );
        f.context.backend.cancel_before_publication = true;
        assert_eq!(
            f.context.cancel(&mut c).unwrap(),
            RuntimeCancellationV1::Cancelled
        );
        assert_eq!(state(&f.context, f.allocations[2]), before);
        assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
        assert_eq!(f.context.submissions[&b.id].dependency_retains, 0);
        f.complete(&mut b);
        assert_eq!(
            f.context.submissions[&a.id].status,
            RuntimeCompletionStatusV1::Succeeded
        );
        f.context
            .write_allocation(f.allocations[2], 0, &[1])
            .unwrap();
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn queued_reads_cancelled_producer_never_reparents_to_ancestor() {
    for cancel_consumer in [false, true] {
        let mut f = Fixture::new(8);
        let (mut a, mut b, events) = chain(&mut f);
        let mut c = f.launch(2, reads(&f), &[events[1]]).unwrap();
        f.context.backend.cancel_before_publication = true;
        assert_eq!(
            f.context.cancel(&mut b).unwrap(),
            RuntimeCancellationV1::Cancelled
        );
        f.complete(&mut a);
        assert_eq!(f.context.version_journal_read_records_v1(), Some(1));
        if cancel_consumer {
            assert_eq!(
                f.context.cancel(&mut c).unwrap(),
                RuntimeCancellationV1::Cancelled
            );
        } else {
            assert!(matches!(
                f.context.poll(&mut c).unwrap(),
                RuntimePollV1::Failed { .. }
            ));
        }
        assert!(
            f.context
                .backend
                .observed_kernel_reads
                .iter()
                .all(|read| read.submission != c.backend_submission)
        );
        assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
        assert_eq!(f.context.submissions[&b.id].dependency_retains, 0);
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn queued_reads_mixed_stable_active_and_queued_rosters_release_together() {
    for outcome in 0..4 {
        let mut f = Fixture::new(4);
        let (_, mut b, events) = chain(&mut f);
        let mut p = f.launch(0, vec![output(&f, 0)], &[]).unwrap();
        let pe = f.context.record_event(&p).unwrap();
        let mut regions = reads(&f);
        regions.insert(1, span(f.allocations[1], RuntimeAccessV1::Read, 4, 8));
        regions.insert(2, span(f.allocations[0], RuntimeAccessV1::Read, 8, 16));
        regions.push(output(&f, 3));
        f.context.backend.launch_failure = match outcome {
            2 => MockMemoryFailure::Rejected,
            3 => MockMemoryFailure::Quiescent,
            _ => MockMemoryFailure::None,
        };
        let result = f.launch(2, regions, &[events[1], pe]);
        if outcome < 2 {
            let mut c = result.unwrap();
            let record = f.context.submissions[&c.id];
            assert_eq!(record.journal_read.unwrap().count, 1);
            assert_eq!(record.journal_producer_read.unwrap().count, 2);
            assert_eq!(f.context.version_journal_read_records_v1(), Some(3));
            if outcome == 0 {
                f.complete(&mut c);
                for read in f
                    .context
                    .backend
                    .observed_kernel_reads
                    .iter()
                    .filter(|r| r.submission == c.backend_submission)
                {
                    let expected = match read.ordinal {
                        0 | 3 => vec![output_byte(b.backend_submission); read.bytes.len()],
                        1 => initial_bytes(1)[4..12].to_vec(),
                        2 => vec![output_byte(p.backend_submission); 16],
                        _ => unreachable!(),
                    };
                    assert_eq!(read.bytes, expected);
                }
            } else {
                f.context.backend.cancel_before_publication = true;
                assert_eq!(
                    f.context.cancel(&mut c).unwrap(),
                    RuntimeCancellationV1::Cancelled
                );
            }
        } else if outcome == 2 {
            assert!(matches!(result, Err(RuntimeErrorV1::BackendRejected(_))));
        } else {
            assert!(matches!(result, Err(RuntimeErrorV1::BackendQuiescent(_))));
        }
        assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
        assert_eq!(f.context.submissions[&b.id].dependency_retains, 0);
        assert_eq!(f.context.submissions[&p.id].dependency_retains, 0);
        f.complete(&mut b);
        f.complete(&mut p);
        assert!(f.context.cleanup().is_complete());
    }
}
