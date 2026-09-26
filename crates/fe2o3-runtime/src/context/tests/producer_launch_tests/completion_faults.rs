use super::*;
use crate::context::versions::completion_faults::{
    CompletionJournalFailureV1 as Failure, CompletionJournalPointV1 as Point,
    CompletionJournalStageV1 as Stage,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Completion {
    Success,
    NoEffect,
    Failed,
    Quiescent,
}

fn assert_quarantine(f: &Fixture, mut expected: crate::RuntimeResourceCreditUsageV1) {
    let device = f.context.devices()[0].id();
    expected.retained_records = 0;
    expected.quarantined_records = 6;
    assert_eq!(
        f.context.allocation_admission_usage_v1(device),
        Ok(Some(expected))
    );
    for allocation in &f.allocations {
        assert!(!f.context.allocation_admission.has_expected_credit(
            *allocation,
            device,
            f.context.allocations[allocation].byte_len
        ),);
    }
}

fn fault_case(completion: Completion, stage: Stage, point: Point, panic: bool) {
    let mut f = Fixture::new(4);
    let (mut producers, events) = f.producers();
    let mut consumer = f.launch(2, f.mixed(true), &events).unwrap();
    for event in events {
        f.context.release_event(event).unwrap();
    }
    // Keep resolved producer reservations live until the consumer releases them.
    for producer in &mut producers {
        f.complete(producer);
    }
    let callbacks = Arc::new(Mutex::new(Vec::new()));
    let calls = Arc::clone(&callbacks);
    f.context
        .on_completion(&consumer, move |status| calls.lock().unwrap().push(status))
        .unwrap();
    let before_record = f.context.submissions[&consumer.id];
    let before_allocations = f.allocations.map(|id| state(&f.context, id));
    let before_retained = f.context.cleanup_report(Vec::new()).retained;
    let before_credits = f
        .context
        .allocation_admission_usage_v1(f.context.devices()[0].id())
        .unwrap()
        .unwrap();
    let payload = Arc::new(());
    f.context
        .versions
        .as_mut()
        .unwrap()
        .inject_completion_fault_for_test_v1(
            consumer.id,
            stage,
            point,
            if panic {
                Failure::Panic(Box::new(Arc::clone(&payload)))
            } else {
                Failure::Error
            },
        );
    let result = catch_unwind(AssertUnwindSafe(|| match completion {
        Completion::NoEffect => {
            f.context.backend.cancel_before_publication = true;
            f.context.cancel(&mut consumer).map(|_| ())
        }
        _ => {
            let observation = match completion {
                Completion::Failed => Some(Observation::Failed),
                Completion::Quiescent => Some(Observation::Quiescent),
                _ => None,
            };
            if let Some(observation) = observation {
                f.context
                    .backend
                    .producer_launch
                    .observations
                    .insert(consumer.backend_submission, observation);
            }
            for _ in 0..16 {
                match f.context.poll(&mut consumer) {
                    Ok(RuntimePollV1::Pending) => continue,
                    result => return result.map(|_| ()),
                }
            }
            panic!("completion fault was not reached");
        }
    }))
    .expect("journal adapter catches the injected panic");
    if completion == Completion::Quiescent {
        assert!(matches!(
            result,
            Err(RuntimeErrorV1::BackendQuiescent(MockError(
                "producer launch quiescent"
            )))
        ));
    } else {
        validation(result, RuntimeValidationErrorV1::InvalidBackendDescription);
    }
    assert!(f.context.terminal);
    assert!(
        !f.context
            .versions
            .as_ref()
            .unwrap()
            .completion_fault_pending_for_test_v1()
    );
    assert_eq!(Arc::strong_count(&payload), if panic { 2 } else { 1 });

    // Root/marker retirement follows a successful adapter return, not just the
    // journal commit: a post-effect unwind keeps that Context root/marker for
    // fail-stop diagnosis and quarantine, even though its journal effect committed.
    let stable_root = stage == Stage::Stable;
    let producer_root = stage != Stage::Writer;
    let stable_released = !stable_root || point == Point::AfterEffect;
    let producer_released =
        stage == Stage::Writer || stage == Stage::Producer && point == Point::AfterEffect;
    let writer_settled = stage == Stage::Writer
        && point == Point::AfterEffect
        && matches!(completion, Completion::Success | Completion::NoEffect);
    let versions = f.context.versions.as_mut().unwrap();
    assert_eq!(
        versions.completion_roots_for_test_v1(consumer.id),
        (stable_root, producer_root, true)
    );
    let journal = versions.read_leases_for_test_v1();
    assert_eq!(
        journal.retained_read_count(),
        usize::from(!stable_released) + 2 * usize::from(!producer_released)
    );
    for (index, released) in [producer_released, producer_released, stable_released]
        .into_iter()
        .enumerate()
    {
        let reference = f.context.allocations[&f.allocations[index]]
            .journal
            .unwrap();
        assert_eq!(journal.reader_count(reference), Ok(usize::from(!released)));
    }
    let writer = before_record.journal_writer.unwrap();
    if writer_settled {
        assert_eq!(
            journal.lookup_writer(writer),
            Err(fe2o3_runtime_model::ContextVersionJournalErrorV1::InvalidReference)
        );
    } else {
        assert_eq!(
            journal.lookup_writer(writer),
            Ok(ContextWriterStateV1::Unknown { member_count: 1 })
        );
    }
    let record = &f.context.submissions[&consumer.id];
    assert_eq!(record.status, RuntimeCompletionStatusV1::Pending);
    assert!(!record.quiescent);
    assert_eq!(
        record.journal_read,
        stable_root.then_some(before_record.journal_read.unwrap())
    );
    assert_eq!(
        record.journal_producer_read,
        producer_root.then_some(before_record.journal_producer_read.unwrap())
    );
    assert_eq!(record.journal_writer, before_record.journal_writer);
    assert!(f.context.producer_launches[&consumer.id].dependencies_held);
    for producer in &producers {
        let record = &f.context.submissions[&producer.id];
        assert_eq!(record.status, RuntimeCompletionStatusV1::Succeeded);
        assert_eq!(record.dependency_retains, 1);
    }
    assert!(callbacks.lock().unwrap().is_empty());
    assert_eq!(f.context.completion_callback_count, 1);
    assert_eq!(f.context.completion_callbacks[&consumer.id].len(), 1);
    assert_eq!(f.context.completion_callback_panic_count(), 0);
    let mut expected_allocations = before_allocations;
    if writer_settled {
        expected_allocations[3].pending_writer = None;
        expected_allocations[3].content_lineage += u64::from(completion == Completion::Success);
    }
    assert_eq!(
        f.allocations.map(|id| state(&f.context, id)),
        expected_allocations
    );
    assert_eq!(
        f.context.cleanup_report(Vec::new()).retained,
        before_retained
    );
    assert_quarantine(&f, before_credits);

    // A failed completion cannot become a retry or disposal authority.
    let before_retry = f.snapshot();
    let native_calls = f.context.backend.producer_launch.calls.len();
    validation(
        f.context.poll(&mut consumer),
        RuntimeValidationErrorV1::ContextTerminal,
    );
    validation(
        f.context.cancel(&mut consumer),
        RuntimeValidationErrorV1::ContextTerminal,
    );
    assert!(!f.context.cleanup().is_complete());
    assert_eq!(f.snapshot(), before_retry);
    assert_eq!(f.context.backend.producer_launch.calls.len(), native_calls);
    assert!(callbacks.lock().unwrap().is_empty());
}

fn fault_matrix(point: Point, panic: bool) {
    for completion in [
        Completion::Success,
        Completion::NoEffect,
        Completion::Failed,
        Completion::Quiescent,
    ] {
        for stage in [Stage::Stable, Stage::Producer, Stage::Writer] {
            fault_case(completion, stage, point, panic);
        }
    }
}

#[test]
fn completion_context_boundary_errors_preserve_exact_committed_prefix() {
    fault_matrix(Point::BeforeEffect, false);
}

#[test]
fn completion_context_before_effect_panics_preserve_exact_committed_prefix() {
    fault_matrix(Point::BeforeEffect, true);
}

#[test]
fn completion_context_after_effect_panics_preserve_commit_without_retiring_roots() {
    fault_matrix(Point::AfterEffect, true);
}

#[test]
fn completion_context_prevalidation_rejects_before_the_first_effect_boundary() {
    for corruption in 0..3 {
        let mut f = Fixture::new(4);
        let (producers, events) = f.producers();
        let mut consumer = f.launch(2, f.mixed(true), &events).unwrap();
        let before_allocations = f.allocations.map(|id| state(&f.context, id));
        let before_credits = f
            .context
            .allocation_admission_usage_v1(f.context.devices()[0].id())
            .unwrap()
            .unwrap();
        let callbacks = Arc::new(Mutex::new(Vec::new()));
        let calls = Arc::clone(&callbacks);
        f.context
            .on_completion(&consumer, move |status| calls.lock().unwrap().push(status))
            .unwrap();
        match corruption {
            0 => f
                .context
                .versions
                .as_mut()
                .unwrap()
                .corrupt_submission_read_reference_for_test_v1(consumer.id, 0),
            1 => f
                .context
                .versions
                .as_mut()
                .unwrap()
                .corrupt_producer_read_reference_for_test_v1(consumer.id, 1),
            2 => {
                f.context
                    .submissions
                    .get_mut(&consumer.id)
                    .unwrap()
                    .journal_writer
                    .as_mut()
                    .unwrap()
                    .key
                    .local += 1
            }
            _ => unreachable!(),
        }
        f.context
            .versions
            .as_mut()
            .unwrap()
            .inject_completion_fault_for_test_v1(
                consumer.id,
                Stage::Stable,
                Point::BeforeEffect,
                Failure::Error,
            );
        let native_calls = f.context.backend.producer_launch.calls.len();
        validation(
            f.context.poll(&mut consumer),
            RuntimeValidationErrorV1::InvalidBackendDescription,
        );
        assert!(f.context.terminal);
        let versions = f.context.versions.as_ref().unwrap();
        assert!(versions.completion_fault_pending_for_test_v1());
        assert_eq!(
            versions.completion_roots_for_test_v1(consumer.id),
            (true, true, true)
        );
        assert_eq!(f.context.version_journal_read_records_v1(), Some(3));
        assert_eq!(
            f.allocations.map(|id| state(&f.context, id)),
            before_allocations
        );
        assert_eq!(f.context.backend.producer_launch.calls.len(), native_calls);
        assert_eq!(
            f.context.submissions[&consumer.id].status,
            RuntimeCompletionStatusV1::Pending
        );
        for producer in &producers {
            assert_eq!(f.context.submissions[&producer.id].dependency_retains, 1);
        }
        assert!(callbacks.lock().unwrap().is_empty());
        assert_eq!(f.context.completion_callback_count, 1);
        assert_quarantine(&f, before_credits);
    }
}

#[test]
fn completion_context_late_writer_identity_error_preserves_released_inputs() {
    let mut f = Fixture::new(4);
    let (producers, events) = f.producers();
    let consumer = f.launch(2, f.mixed(true), &events).unwrap();
    let writer = f.context.submissions[&consumer.id].journal_writer.unwrap();
    let before_allocations = f.allocations.map(|id| state(&f.context, id));
    let before_credits = f
        .context
        .allocation_admission_usage_v1(f.context.devices()[0].id())
        .unwrap()
        .unwrap();
    let callbacks = Arc::new(Mutex::new(Vec::new()));
    let calls = Arc::clone(&callbacks);
    f.context
        .on_completion(&consumer, move |status| calls.lock().unwrap().push(status))
        .unwrap();
    assert!(
        f.context
            .backend
            .finish_producer_launch_test_v1(consumer.backend_submission, false)
    );
    // Deliberately exercise the real adapters separately: this is a late
    // cross-map validation error, not a concurrent mutation of a Context call.
    f.context.release_submission_inputs_v1(consumer.id).unwrap();
    f.context
        .submissions
        .get_mut(&consumer.id)
        .unwrap()
        .journal_writer
        .as_mut()
        .unwrap()
        .key
        .local += 1;
    assert_eq!(
        f.context
            .settle_submission_writer_v1(consumer.id, SubmissionWriterOutcomeV1::Unknown),
        Err(RuntimeValidationErrorV1::InvalidBackendDescription)
    );
    assert!(f.context.terminal);
    let versions = f.context.versions.as_mut().unwrap();
    assert_eq!(
        versions.completion_roots_for_test_v1(consumer.id),
        (false, false, true)
    );
    assert_eq!(versions.read_leases_for_test_v1().retained_read_count(), 0);
    assert_eq!(
        versions.journal_for_test().lookup_writer(writer),
        Ok(ContextWriterStateV1::Unknown { member_count: 1 })
    );
    let record = &f.context.submissions[&consumer.id];
    assert!(record.journal_read.is_none());
    assert!(record.journal_producer_read.is_none());
    assert!(record.journal_writer.is_some());
    assert_eq!(record.status, RuntimeCompletionStatusV1::Pending);
    assert!(!record.quiescent);
    assert_eq!(
        f.allocations.map(|id| state(&f.context, id)),
        before_allocations
    );
    for producer in &producers {
        assert_eq!(f.context.submissions[&producer.id].dependency_retains, 1);
        assert_eq!(
            f.context.submissions[&producer.id].status,
            RuntimeCompletionStatusV1::Pending
        );
    }
    assert!(f.context.producer_launches[&consumer.id].dependencies_held);
    assert!(callbacks.lock().unwrap().is_empty());
    assert_eq!(f.context.completion_callback_count, 1);
    assert_quarantine(&f, before_credits);
}
