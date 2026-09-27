use super::*;
use fe2o3_runtime_model::ContextQueuedWriterStatusV1;

fn output(f: &Fixture, index: usize) -> RuntimeMemoryRegionV1 {
    span(f.allocations[index], RuntimeAccessV1::Write, 0, 64)
}

fn queued_status(f: &Fixture, submission: &Submission) -> Option<ContextQueuedWriterStatusV1> {
    let writer = f.context.submissions[&submission.id]
        .journal_writer
        .unwrap();
    f.context
        .versions
        .as_ref()
        .unwrap()
        .journal_for_test()
        .queued_writer_status(writer)
        .unwrap()
}

#[test]
fn queued_outputs_chain_reconciles_latest_writer_after_public_events_are_released() {
    for streams in [[0, 0, 0], [0, 1, 2]] {
        let mut f = Fixture::new(8);
        let before = state(&f.context, f.allocations[2]);
        let a = f.launch(streams[0], vec![output(&f, 2)], &[]).unwrap();
        let ae = f.context.record_event(&a).unwrap();
        let b = f.launch(streams[1], vec![output(&f, 2)], &[ae]).unwrap();
        let be = f.context.record_event(&b).unwrap();
        let mut c = f.launch(streams[2], vec![output(&f, 2)], &[be]).unwrap();
        assert_eq!(
            queued_status(&f, &b),
            Some(ContextQueuedWriterStatusV1::Waiting)
        );
        assert_eq!(
            queued_status(&f, &c),
            Some(ContextQueuedWriterStatusV1::Waiting)
        );
        f.context.release_event(ae).unwrap();
        f.context.release_event(be).unwrap();
        // Physical completion cannot release the logical queued reservation.
        assert!(
            f.context
                .backend
                .finish_producer_launch_test_v1(c.backend_submission, true)
        );
        let snapshot = f.snapshot();
        validation(
            f.context.read_allocation(f.allocations[2], 0, &mut [0; 64]),
            RuntimeValidationErrorV1::ContextReserved,
        );
        assert_eq!(f.snapshot(), snapshot);
        f.complete(&mut c);
        for id in [a.id, b.id, c.id] {
            assert_eq!(
                f.context.submissions[&id].status,
                RuntimeCompletionStatusV1::Succeeded
            );
        }
        let after = state(&f.context, f.allocations[2]);
        assert_eq!(after.attempt_epoch, before.attempt_epoch + 3);
        assert_eq!(after.content_lineage, before.content_lineage + 3);
        assert_eq!(after.pending_writer, None);
        assert_eq!(f.context.version_journal_writer_records_v1(), Some(0));
        let mut bytes = [0; 64];
        f.context
            .read_allocation(f.allocations[2], 0, &mut bytes)
            .unwrap();
        assert_eq!(bytes, [output_byte(c.backend_submission); 64]);
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn queued_outputs_require_exact_latest_explicit_dependency_and_full_write_aliases() {
    let mut f = Fixture::new(8);
    let a = f.launch(0, vec![output(&f, 2)], &[]).unwrap();
    let ae = f.context.record_event(&a).unwrap();
    let mut b = f.launch(1, vec![output(&f, 2)], &[ae]).unwrap();
    let be = f.context.record_event(&b).unwrap();
    for (regions, events) in [
        (vec![output(&f, 2)], vec![]),
        (vec![output(&f, 2)], vec![ae]),
        (
            vec![span(f.allocations[2], RuntimeAccessV1::Write, 8, 16)],
            vec![be],
        ),
        (
            vec![span(f.allocations[2], RuntimeAccessV1::ReadWrite, 0, 64)],
            vec![be],
        ),
        (
            vec![
                output(&f, 2),
                span(f.allocations[2], RuntimeAccessV1::Read, 0, 64),
            ],
            vec![be],
        ),
        (
            vec![
                output(&f, 2),
                span(f.allocations[3], RuntimeAccessV1::Write, 8, 16),
            ],
            vec![be],
        ),
    ] {
        let before = f.snapshot();
        validation(
            f.launch(2, regions, &events),
            RuntimeValidationErrorV1::ContextReserved,
        );
        assert_eq!(f.snapshot(), before);
        assert!(!f.context.is_terminal());
    }
    f.context.release_event(ae).unwrap();
    f.context.release_event(be).unwrap();
    f.complete(&mut b);
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn queued_outputs_exclude_host_and_launch_access_after_head_reconciliation() {
    let mut f = Fixture::new(8);
    let mut a = f.launch(0, vec![output(&f, 2)], &[]).unwrap();
    let ae = f.context.record_event(&a).unwrap();
    let mut b = f
        .launch(1, vec![output(&f, 2), output(&f, 3)], &[ae])
        .unwrap();
    let be = f.context.record_event(&b).unwrap();
    f.complete(&mut a);
    assert_eq!(
        queued_status(&f, &b),
        Some(ContextQueuedWriterStatusV1::Ready)
    );
    for index in [2, 3] {
        assert_eq!(state(&f.context, f.allocations[index]).pending_writer, None);
        let before = f.snapshot();
        validation(
            f.context
                .read_allocation(f.allocations[index], 0, &mut [0; 64]),
            RuntimeValidationErrorV1::ContextReserved,
        );
        validation(
            f.context
                .write_allocation(f.allocations[index], 0, &[1; 64]),
            RuntimeValidationErrorV1::ContextReserved,
        );
        validation(
            f.context.release_allocation(f.allocations[index]),
            RuntimeValidationErrorV1::ContextReserved,
        );
        let args = MixedArguments(vec![output(&f, index)]);
        validation(
            f.context
                .launch(f.streams[2], &f.kernel, &args, geometry(), &[]),
            RuntimeValidationErrorV1::ContextReserved,
        );
        let reads = vec![span(f.allocations[index], RuntimeAccessV1::Read, 0, 64)];
        validation(
            f.launch(2, reads.clone(), &[]),
            RuntimeValidationErrorV1::ContextReserved,
        );
        validation(
            f.launch(2, reads, &[be]),
            RuntimeValidationErrorV1::ContextReserved,
        );
        assert_eq!(f.snapshot(), before);
        assert!(!f.context.is_terminal());
    }
    f.complete(&mut b);
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn queued_outputs_multi_parent_and_idle_destination_settle_as_one_roster() {
    let mut f = Fixture::new(8);
    let a = f.launch(0, vec![output(&f, 0)], &[]).unwrap();
    let b = f.launch(1, vec![output(&f, 1)], &[]).unwrap();
    let ae = f.context.record_event(&a).unwrap();
    let be = f.context.record_event(&b).unwrap();
    let before = f.allocations.map(|id| state(&f.context, id));
    let mut c = f
        .launch(
            2,
            vec![output(&f, 2), output(&f, 1), output(&f, 0)],
            &[be, ae],
        )
        .unwrap();
    assert_eq!(f.allocations.map(|id| state(&f.context, id)), before);
    f.complete(&mut c);
    for (index, prior) in before.iter().enumerate().take(3) {
        let after = state(&f.context, f.allocations[index]);
        assert_eq!(after.pending_writer, None);
        assert_eq!(
            after.content_lineage,
            prior.content_lineage + if index == 2 { 1 } else { 2 }
        );
    }
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn queued_outputs_cancel_middle_never_reparents_descendant() {
    for descendant_fails in [false, true] {
        let mut f = Fixture::new(8);
        let mut a = f.launch(0, vec![output(&f, 2)], &[]).unwrap();
        let ae = f.context.record_event(&a).unwrap();
        let mut b = f.launch(1, vec![output(&f, 2)], &[ae]).unwrap();
        let be = f.context.record_event(&b).unwrap();
        let mut c = f.launch(2, vec![output(&f, 2)], &[be]).unwrap();
        let before = state(&f.context, f.allocations[2]);
        f.context.backend.cancel_before_publication = true;
        assert_eq!(
            f.context.cancel(&mut b).unwrap(),
            RuntimeCancellationV1::Cancelled
        );
        assert_eq!(state(&f.context, f.allocations[2]), before);
        assert_eq!(
            queued_status(&f, &c),
            Some(ContextQueuedWriterStatusV1::Blocked)
        );
        f.complete(&mut a);
        assert_eq!(
            queued_status(&f, &c),
            Some(ContextQueuedWriterStatusV1::Blocked)
        );
        if descendant_fails {
            f.context
                .backend
                .producer_launch
                .observations
                .insert(c.backend_submission, Observation::Failed);
            assert!(matches!(
                f.context.poll(&mut c).unwrap(),
                RuntimePollV1::Failed { .. }
            ));
            assert_eq!(
                queued_status(&f, &c),
                Some(ContextQueuedWriterStatusV1::Unknown)
            );
            assert_eq!(f.context.version_journal_writer_records_v1(), Some(1));
            f.context.release_allocation(f.allocations[2]).unwrap();
        } else {
            assert_eq!(
                f.context.cancel(&mut c).unwrap(),
                RuntimeCancellationV1::Cancelled
            );
            assert_eq!(f.context.version_journal_writer_records_v1(), Some(0));
        }
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn queued_outputs_no_handle_rejection_and_quiescence_preserve_exact_custody() {
    for failure in [MockMemoryFailure::Rejected, MockMemoryFailure::Quiescent] {
        let mut f = Fixture::new(8);
        let mut a = f.launch(0, vec![output(&f, 2)], &[]).unwrap();
        let ae = f.context.record_event(&a).unwrap();
        let before = [
            state(&f.context, f.allocations[2]),
            state(&f.context, f.allocations[3]),
        ];
        f.context.backend.launch_failure = failure;
        let result = f.launch(1, vec![output(&f, 2), output(&f, 3)], &[ae]);
        assert!(match failure {
            MockMemoryFailure::Rejected =>
                matches!(result, Err(RuntimeErrorV1::BackendRejected(_))),
            _ => matches!(result, Err(RuntimeErrorV1::BackendQuiescent(_))),
        });
        assert_eq!(
            [
                state(&f.context, f.allocations[2]),
                state(&f.context, f.allocations[3])
            ],
            before
        );
        f.context.backend.launch_failure = MockMemoryFailure::None;
        if failure == MockMemoryFailure::Quiescent {
            assert_eq!(f.context.version_journal_writer_records_v1(), Some(2));
            validation(
                f.context.release_allocation(f.allocations[3]),
                RuntimeValidationErrorV1::ContextReserved,
            );
            f.complete(&mut a);
            f.context.release_allocation(f.allocations[3]).unwrap();
            assert_eq!(f.context.version_journal_writer_records_v1(), Some(1));
            f.context.release_allocation(f.allocations[2]).unwrap();
            assert_eq!(f.context.version_journal_writer_records_v1(), Some(0));
        } else {
            assert_eq!(f.context.version_journal_writer_records_v1(), Some(1));
            f.context
                .write_allocation(f.allocations[3], 0, &[9; 64])
                .unwrap();
            f.complete(&mut a);
        }
        assert!(!f.context.is_terminal());
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn queued_outputs_unknown_group_releases_each_unique_allocation_once() {
    let mut f = Fixture::new(8);
    let mut a = f
        .launch(0, vec![output(&f, 2), output(&f, 4)], &[])
        .unwrap();
    let ae = f.context.record_event(&a).unwrap();
    let mut b = f
        .launch(1, vec![output(&f, 2), output(&f, 3)], &[ae])
        .unwrap();
    for submission in [&mut a, &mut b] {
        f.context
            .backend
            .producer_launch
            .observations
            .insert(submission.backend_submission, Observation::Failed);
        assert!(matches!(
            f.context.poll(submission).unwrap(),
            RuntimePollV1::Failed { .. }
        ));
    }
    let usage = f
        .context
        .allocation_admission_usage_v1(f.context.devices()[0].id())
        .unwrap();
    for index in [3, 4] {
        f.context.release_allocation(f.allocations[index]).unwrap();
        assert_eq!(
            f.context
                .allocation_admission_usage_v1(f.context.devices()[0].id())
                .unwrap(),
            usage
        );
        assert_eq!(f.context.version_journal_writer_records_v1(), Some(2));
    }
    f.context.release_allocation(f.allocations[2]).unwrap();
    assert_eq!(f.context.version_journal_writer_records_v1(), Some(0));
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn queued_outputs_member_exhaustion_rejects_before_ids_and_backend_entry() {
    for member_capacity in [6, 9] {
        let mut f = Fixture::with_capacities(16, true, 6, member_capacity);
        let mut tail = f.launch(0, vec![output(&f, 2)], &[]).unwrap();
        for index in 1..member_capacity {
            let event = f.context.record_event(&tail).unwrap();
            tail = f.launch(index % 3, vec![output(&f, 2)], &[event]).unwrap();
            f.context.release_event(event).unwrap();
        }
        let event = f.context.record_event(&tail).unwrap();
        let before = f.snapshot();
        validation(
            f.launch(0, vec![output(&f, 2)], &[event]),
            RuntimeValidationErrorV1::Capacity,
        );
        validation(
            f.context.write_allocation(f.allocations[3], 0, &[1; 64]),
            RuntimeValidationErrorV1::Capacity,
        );
        assert_eq!(f.snapshot(), before);
        assert!(!f.context.is_terminal());
        f.complete(&mut tail);
        f.context
            .write_allocation(f.allocations[3], 0, &[1; 64])
            .unwrap();
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn queued_outputs_native_success_retains_custody_until_reconciliation_and_is_too_late_to_cancel() {
    let mut f = Fixture::new(8);
    let a = f.launch(0, vec![output(&f, 2)], &[]).unwrap();
    let ae = f.context.record_event(&a).unwrap();
    let mut b = f.launch(1, vec![output(&f, 2)], &[ae]).unwrap();
    assert_eq!(f.context.poll(&mut b).unwrap(), RuntimePollV1::Pending);
    assert_eq!(
        queued_status(&f, &b),
        Some(ContextQueuedWriterStatusV1::Waiting)
    );
    let before = f.snapshot();
    f.context.backend.cancel_before_publication = true;
    let calls = f.context.backend.cancel_call_count;
    assert_eq!(
        f.context.cancel(&mut b).unwrap(),
        RuntimeCancellationV1::TooLate
    );
    assert_eq!(f.context.backend.cancel_call_count, calls);
    assert_eq!(f.snapshot(), before);
    f.complete(&mut b);
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn queued_outputs_completion_faults_retain_custody_after_committed_prefix() {
    use crate::context::versions::completion_faults::{
        CompletionJournalFailureV1 as Failure, CompletionJournalPointV1 as Point,
        CompletionJournalStageV1 as Stage,
    };
    for point in [Point::BeforeEffect, Point::AfterEffect] {
        for panic in [false, true] {
            if point == Point::AfterEffect && !panic {
                continue;
            }
            let mut f = Fixture::new(8);
            let a = f.launch(0, vec![output(&f, 2)], &[]).unwrap();
            let ae = f.context.record_event(&a).unwrap();
            let mut b = f.launch(1, vec![output(&f, 2)], &[ae]).unwrap();
            let be = f.context.record_event(&b).unwrap();
            let c = f.launch(2, vec![output(&f, 2)], &[be]).unwrap();
            f.context
                .versions
                .as_mut()
                .unwrap()
                .inject_completion_fault_for_test_v1(
                    b.id,
                    Stage::Writer,
                    point,
                    if panic {
                        Failure::Panic(Box::new("queued completion fault"))
                    } else {
                        Failure::Error
                    },
                );
            let mut failed = false;
            for _ in 0..16 {
                match f.context.poll(&mut b) {
                    Ok(RuntimePollV1::Pending) => {}
                    Err(RuntimeErrorV1::Validation(
                        RuntimeValidationErrorV1::InvalidBackendDescription,
                    )) => {
                        failed = true;
                        break;
                    }
                    _ => panic!("unexpected completion at injected boundary"),
                }
            }
            assert!(failed);
            assert!(f.context.is_terminal());
            assert_eq!(f.context.version_journal_writer_records_v1(), Some(2));
            assert!(f.context.submissions[&b.id].journal_writer.is_some());
            assert!(f.context.submissions[&c.id].journal_writer.is_some());
            let state = state(&f.context, f.allocations[2]);
            assert_eq!(
                state.content_lineage,
                if point == Point::AfterEffect { 3 } else { 2 }
            );
            assert!(!f.context.cleanup().is_complete());
            assert_eq!(f.context.version_journal_writer_records_v1(), Some(2));
        }
    }
}

#[test]
fn queued_outputs_terminal_submission_failure_and_panic_retain_original_roster() {
    for failure in [MockMemoryFailure::Terminal, MockMemoryFailure::Panic] {
        let mut f = Fixture::new(8);
        let a = f.launch(0, vec![output(&f, 2)], &[]).unwrap();
        let ae = f.context.record_event(&a).unwrap();
        let id = RuntimeSubmissionIdV1::new(f.context.context_generation, f.context.next_identity);
        f.context.backend.launch_failure = failure;
        let result = catch_unwind(AssertUnwindSafe(|| {
            f.launch(1, vec![output(&f, 2), output(&f, 3)], &[ae])
        }));
        match failure {
            MockMemoryFailure::Terminal => assert!(matches!(
                result,
                Ok(Err(RuntimeErrorV1::BackendTerminal(_)))
            )),
            _ => assert!(result.is_err()),
        }
        assert!(f.context.is_terminal());
        assert_eq!(f.context.version_journal_writer_records_v1(), Some(2));
        assert!(f.context.producer_launches.contains_key(&id));
        assert_eq!(f.context.submissions[&a.id].dependency_retains, 1);
        assert!(!f.context.cleanup().is_complete());
        assert_eq!(f.context.version_journal_writer_records_v1(), Some(2));
    }
}

#[test]
fn queued_outputs_inconsistent_native_parent_observation_quarantines() {
    for observation in [Observation::Pending, Observation::Failed] {
        let mut f = Fixture::new(8);
        let a = f.launch(0, vec![output(&f, 2)], &[]).unwrap();
        let ae = f.context.record_event(&a).unwrap();
        let mut b = f.launch(1, vec![output(&f, 2)], &[ae]).unwrap();
        assert_eq!(f.context.poll(&mut b).unwrap(), RuntimePollV1::Pending);
        // B's native Success proves A succeeded. Contradictory native observations
        // are a backend contract violation, not ordinary delayed reconciliation.
        f.context
            .backend
            .producer_launch
            .observations
            .insert(a.backend_submission, observation);
        validation(
            f.context.poll(&mut b),
            RuntimeValidationErrorV1::InvalidBackendDescription,
        );
        assert!(f.context.is_terminal());
        assert_eq!(f.context.version_journal_writer_records_v1(), Some(2));
        assert!(!f.context.cleanup().is_complete());
    }
}
