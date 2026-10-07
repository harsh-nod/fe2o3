use super::*;

fn retain_physical_success(context: &mut Context, submissions: &[Submission]) {
    for submission in submissions {
        context
            .backend
            .finish_submission(submission.backend_submission, true);
        assert!(
            context
                .retain_directed_observation_v1(submission.id, BackendPollV1::Succeeded)
                .unwrap()
        );
        assert_eq!(
            context.query_submission(submission).unwrap(),
            RuntimeCompletionStatusV1::Pending
        );
    }
}

fn assert_no_observation(context: &Context) {
    assert!(context.backend.directed_calls.is_empty());
    assert_eq!(context.backend.poll_call_count, 0);
    assert_eq!(context.backend.wait_call_count, 0);
    assert_eq!(context.backend.flush_call_count, 0);
}

#[test]
fn directed_reconciliation_yields_at_exact_fuel_and_resumes_retained_prefix() {
    for journal in [false, true] {
        let mut fixture = Fixture::new(journal, 257);
        let mut submissions = Vec::new();
        let mut event = None;
        let callbacks = Arc::new(Mutex::new(Vec::new()));
        for index in 0..MAX_RUNTIME_DEPENDENCIES_V1 {
            let submission = fixture.copy(index, index + 1, event.as_slice()).unwrap();
            if let Some(previous) = event.take() {
                fixture.context.release_event(previous).unwrap();
            }
            event = Some(fixture.context.record_event(&submission).unwrap());
            let output = callbacks.clone();
            let id = submission.id;
            fixture
                .context
                .on_completion(&submission, move |status| {
                    assert_eq!(status, RuntimeCompletionStatusV1::Succeeded);
                    output.lock().unwrap().push(id);
                })
                .unwrap();
            submissions.push(submission);
        }
        fixture.context.release_event(event.unwrap()).unwrap();
        retain_physical_success(&mut fixture.context, &submissions);
        let requested = submissions.last().unwrap().id;

        // All physical facts are retained, but the first two bounded passes must yield.
        for (pass, completed, steps) in [(0, 86, 513), (1, 201, 513), (2, 256, 219)] {
            let (result, observed_steps) = fixture
                .context
                .reconcile_directed_counted_for_test_v1(requested);
            assert_eq!(observed_steps, steps);
            assert_eq!(
                result.unwrap(),
                if completed == submissions.len() {
                    RuntimeCompletionStatusV1::Succeeded
                } else {
                    RuntimeCompletionStatusV1::Pending
                }
            );
            for (index, submission) in submissions.iter().enumerate() {
                let record = &fixture.context.submissions[&submission.id];
                let state = fixture.context.scalar_peer_copies[&submission.id]
                    .directed
                    .as_ref()
                    .unwrap();
                assert_eq!(
                    record.status,
                    if index < completed {
                        RuntimeCompletionStatusV1::Succeeded
                    } else {
                        RuntimeCompletionStatusV1::Pending
                    }
                );
                assert_eq!(
                    state.cursor,
                    usize::from(
                        index > 0 && (index < completed || pass == 0 && index == completed)
                    )
                );
                assert_eq!(state.terminal, Some(BackendPollV1::Succeeded));
                assert_eq!(
                    record.dependency_retains,
                    usize::from(index + 1 < submissions.len() && index + 1 >= completed)
                );
                assert_eq!(
                    fixture.context.scalar_peer_copies[&submission.id].dependencies_held,
                    index >= completed
                );
            }
            assert_eq!(
                *callbacks.lock().unwrap(),
                submissions[..completed]
                    .iter()
                    .map(|submission| submission.id)
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                fixture.context.version_journal_read_records_v1(),
                journal.then_some(submissions.len() - completed)
            );
            assert_no_observation(&fixture.context);
        }
        let mut bytes = [0; 64];
        fixture
            .context
            .read_allocation(fixture.allocations[256], 0, &mut bytes)
            .unwrap();
        let mut expected = [0; 64];
        expected[8..24].fill(0x51);
        assert_eq!(bytes, expected);
        assert!(fixture.context.cleanup().is_complete());
    }
}

#[test]
fn directed_reconciliation_retains_successful_cursor_prefix_before_later_failure() {
    let mut fixture = Fixture::new(true, 4);
    let mut first = fixture.copy(0, 1, &[]).unwrap();
    let first_event = fixture.context.record_event(&first).unwrap();
    let second = fixture.copy(0, 3, &[]).unwrap();
    let second_event = fixture.context.record_event(&second).unwrap();
    let consumer = fixture.copy(1, 2, &[second_event, first_event]).unwrap();
    for event in [first_event, second_event] {
        fixture.context.release_event(event).unwrap();
    }
    fixture
        .context
        .progress_directed_peer_copy_v1(&mut first)
        .unwrap();
    fixture
        .context
        .backend
        .finish_submission(second.backend_submission, false);
    fixture
        .context
        .retain_directed_observation_v1(second.id, BackendPollV1::Failed { code: 71 })
        .unwrap();
    fixture
        .context
        .backend
        .finish_submission(consumer.backend_submission, true);
    fixture
        .context
        .retain_directed_observation_v1(consumer.id, BackendPollV1::Succeeded)
        .unwrap();
    let reads = fixture.context.version_journal_read_records_v1();
    let calls = fixture.context.backend.directed_calls.clone();
    let callbacks = Arc::new(Mutex::new(Vec::new()));
    for submission in [&second, &consumer] {
        let output = callbacks.clone();
        let id = submission.id;
        fixture
            .context
            .on_completion(submission, move |status| {
                output.lock().unwrap().push((id, status))
            })
            .unwrap();
    }
    assert_eq!(
        fixture.context.reconcile_directed_success_v1(consumer.id),
        Err(RuntimeValidationErrorV1::InvalidBackendDescription)
    );
    assert!(fixture.context.is_terminal());
    assert_eq!(
        fixture.context.scalar_peer_copies[&consumer.id]
            .directed
            .as_ref()
            .unwrap()
            .cursor,
        1
    );
    assert_eq!(
        fixture.context.scalar_peer_copies[&second.id]
            .directed
            .as_ref()
            .unwrap()
            .cursor,
        0
    );
    assert_eq!(
        fixture.context.submissions[&first.id].status,
        RuntimeCompletionStatusV1::Succeeded
    );
    for submission in [&second, &consumer] {
        assert_eq!(
            fixture.context.submissions[&submission.id].status,
            RuntimeCompletionStatusV1::Pending
        );
    }
    for submission in [&first, &second] {
        assert_eq!(
            fixture.context.submissions[&submission.id].dependency_retains,
            1
        );
    }
    assert!(fixture.context.retained_directed_success_v1(consumer.id));
    assert_eq!(fixture.context.version_journal_read_records_v1(), reads);
    assert_eq!(fixture.context.backend.directed_calls, calls);
    assert_eq!(fixture.context.backend.poll_call_count, 0);
    assert_eq!(fixture.context.backend.wait_call_count, 0);
    assert_eq!(fixture.context.backend.flush_call_count, 0);
    assert!(callbacks.lock().unwrap().is_empty());
    assert_eq!(fixture.context.completion_callback_count, 2);
}

#[test]
fn directed_reconciliation_graph_can_exceed_the_dependency_and_stack_bound() {
    let mut fixture = Fixture::new(true, 604);
    let mut submissions = Vec::new();
    let mut events = Vec::new();
    for index in 0..300 {
        let submission = fixture.copy(0, 2 * index + 1, &[]).unwrap();
        events.push(fixture.context.record_event(&submission).unwrap());
        submissions.push(submission);
    }
    let first = fixture
        .copy(1, 600, &events[..MAX_RUNTIME_DEPENDENCIES_V1])
        .unwrap();
    let second = fixture
        .copy(513, 602, &events[MAX_RUNTIME_DEPENDENCIES_V1..])
        .unwrap();
    let first_event = fixture.context.record_event(&first).unwrap();
    let second_event = fixture.context.record_event(&second).unwrap();
    let root = fixture
        .copy(600, 601, &[second_event, first_event])
        .unwrap();
    let requested = root.id;
    submissions.extend([first, second, root]);
    assert!(submissions.len() > MAX_RUNTIME_DEPENDENCIES_V1);
    events.extend([first_event, second_event]);
    for event in events {
        fixture.context.release_event(event).unwrap();
    }
    retain_physical_success(&mut fixture.context, &submissions);
    let mut complete = false;
    let mut prior = 0;
    for _ in 0..8 {
        let status = fixture
            .context
            .reconcile_directed_success_v1(requested)
            .unwrap();
        let count = submissions
            .iter()
            .filter(|submission| {
                fixture.context.submissions[&submission.id].status
                    == RuntimeCompletionStatusV1::Succeeded
            })
            .count();
        assert!(count > prior);
        prior = count;
        assert_no_observation(&fixture.context);
        if status == RuntimeCompletionStatusV1::Succeeded {
            complete = true;
            break;
        }
        assert_eq!(status, RuntimeCompletionStatusV1::Pending);
    }
    assert!(complete);
    assert_eq!(prior, submissions.len());
    for submission in &submissions {
        assert_eq!(
            fixture.context.submissions[&submission.id].dependency_retains,
            0
        );
    }
    assert_eq!(fixture.context.version_journal_read_records_v1(), Some(0));
    let mut bytes = [0; 64];
    fixture
        .context
        .read_allocation(fixture.allocations[601], 0, &mut bytes)
        .unwrap();
    let mut expected = [0; 64];
    expected[8..24].fill(0x51);
    assert_eq!(bytes, expected);
    assert!(fixture.context.cleanup().is_complete());
}
