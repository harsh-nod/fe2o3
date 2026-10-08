use super::*;

#[test]
fn repeated_cached_poll_frames_context_callbacks_and_neighbor() {
    let mut fixture = Fixture::new(true);
    let before = snapshot(&fixture.context, &fixture.probes);
    let neighbor = token_snapshot(&fixture.neighbor);
    let mut expected = token_snapshot(&fixture.target);
    expected.completion = Some(RuntimePollV1::Succeeded);
    for _ in 0..3 {
        assert_eq!(
            fixture.context.poll(&mut fixture.target).unwrap(),
            RuntimePollV1::Succeeded
        );
        assert_eq!(token_snapshot(&fixture.target), expected);
        assert_eq!(token_snapshot(&fixture.neighbor), neighbor);
        assert_eq!(snapshot(&fixture.context, &fixture.probes), before);
    }
    fixture.cleanup();
}

#[test]
fn repeated_cached_poll_does_not_commit_the_original_writer_again() {
    use super::super::async_journal_tests::{fixture, state};

    let (mut context, stream, allocation, kernel) = fixture(2);
    let mut submission = context
        .launch(
            stream,
            &kernel,
            &AddArguments {
                allocation,
                scalar: 7,
            },
            geometry(),
            &[],
        )
        .unwrap();
    assert!(context.submissions[&submission.id].journal_writer.is_some());
    let event = context.record_event(&submission).unwrap();
    assert_eq!(
        context.wait_event(event, Duration::ZERO).unwrap(),
        RuntimeCompletionStatusV1::Succeeded
    );
    assert_eq!(submission.completion, None);
    assert!(context.submissions[&submission.id].journal_writer.is_none());
    let committed = state(&context, allocation);
    assert_eq!(committed.content_lineage, 1);
    assert!(committed.pending_writer.is_none());
    let usage = context.version_journal_usage_v1();
    let before = snapshot(&context, &[]);
    for _ in 0..3 {
        assert_eq!(
            context.poll(&mut submission).unwrap(),
            RuntimePollV1::Succeeded
        );
        assert_eq!(state(&context, allocation), committed);
        assert_eq!(context.version_journal_writer_records_v1(), Some(0));
        assert_eq!(context.version_journal_usage_v1(), usage);
        assert_eq!(snapshot(&context, &[]), before);
    }
    context.release_event(event).unwrap();
    context.release_submission(submission).unwrap();
    assert!(context.cleanup().is_complete());
}

#[test]
fn pending_record_ignores_a_scripted_cached_token_completion() {
    let mut fixture = Fixture::new(false);
    // A private fixture substitution must not turn token metadata into authority.
    fixture.target.completion = Some(RuntimePollV1::Succeeded);
    let before = snapshot(&fixture.context, &fixture.probes);
    let token = token_snapshot(&fixture.target);
    assert_eq!(
        fixture.context.poll_cached_status_v1(&mut fixture.target),
        Ok(None)
    );
    assert_eq!(snapshot(&fixture.context, &fixture.probes), before);
    assert_eq!(token_snapshot(&fixture.target), token);
    assert_eq!(
        fixture.context.poll(&mut fixture.target).unwrap(),
        RuntimePollV1::Pending
    );
    assert_eq!(
        fixture.context.backend.poll_call_count,
        before.backend.counts[2] + 1
    );
    assert_eq!(
        fixture.context.submissions[&fixture.target.id].status,
        RuntimeCompletionStatusV1::Pending
    );
    fixture.cleanup();
}

#[test]
fn cached_prefix_rejects_each_identity_before_missing_stream() {
    let mut fixture = Fixture::new(true);
    let original = fixture
        .context
        .streams
        .remove(&fixture.target.stream)
        .unwrap();
    for coordinate in COORDINATES {
        let mut token = fixture.substitute(coordinate);
        let before_token = token_snapshot(&token);
        let before = snapshot(&fixture.context, &fixture.probes);
        assert_eq!(
            fixture.context.poll_cached_status_v1(&mut token),
            Err(RuntimeValidationErrorV1::UnknownSubmission)
        );
        assert_eq!(token_snapshot(&token), before_token);
        assert_eq!(snapshot(&fixture.context, &fixture.probes), before);
    }
    assert_eq!(
        fixture.context.poll_cached_status_v1(&mut fixture.target),
        Err(RuntimeValidationErrorV1::UnknownStream)
    );
    assert!(
        fixture
            .context
            .streams
            .insert(fixture.target.stream, original)
            .is_none()
    );
    fixture.cleanup();
}

#[test]
fn cached_prefix_preserves_wrong_device_before_stream_hold_refusal() {
    let mut fixture = Fixture::new(true);
    let original = fixture.context.streams[&fixture.target.stream];
    let record = fixture
        .context
        .streams
        .get_mut(&fixture.target.stream)
        .unwrap();
    record.device = fixture.neighbor.device;
    record.unpublished = Some(91);
    let before = snapshot(&fixture.context, &fixture.probes);
    let token = token_snapshot(&fixture.target);
    assert_eq!(
        fixture.context.poll_cached_status_v1(&mut fixture.target),
        Err(RuntimeValidationErrorV1::WrongDevice)
    );
    assert_eq!(snapshot(&fixture.context, &fixture.probes), before);
    assert_eq!(token_snapshot(&fixture.target), token);
    fixture
        .context
        .streams
        .get_mut(&fixture.target.stream)
        .unwrap()
        .device = original.device;
    let held = snapshot(&fixture.context, &fixture.probes);
    assert_eq!(
        fixture.context.poll_cached_status_v1(&mut fixture.target),
        Err(RuntimeValidationErrorV1::ContextReserved)
    );
    assert_eq!(snapshot(&fixture.context, &fixture.probes), held);
    assert_eq!(token_snapshot(&fixture.target), token);
    *fixture
        .context
        .streams
        .get_mut(&fixture.target.stream)
        .unwrap() = original;
    fixture.cleanup();
}

#[test]
fn cached_terminal_mapping_is_exact_and_does_not_reenter_backend() {
    let mut fixture = Fixture::new(true);
    let original = fixture.context.submissions[&fixture.target.id];
    // Status-only fixtures exercise the legacy mapping, not native retirement.
    for (status, expected) in [
        (
            RuntimeCompletionStatusV1::Succeeded,
            RuntimePollV1::Succeeded,
        ),
        (
            RuntimeCompletionStatusV1::Failed(RuntimeCompletionFailureV1::BackendCode(17)),
            RuntimePollV1::Failed { code: 17 },
        ),
        (
            RuntimeCompletionStatusV1::Failed(RuntimeCompletionFailureV1::Cancelled),
            RuntimePollV1::Failed {
                code: RUNTIME_CANCELLED_CODE_V1,
            },
        ),
        (
            RuntimeCompletionStatusV1::QuiescentWithoutResult,
            RuntimePollV1::Failed {
                code: RUNTIME_QUIESCENT_WITHOUT_RESULT_CODE_V1,
            },
        ),
    ] {
        fixture
            .context
            .submissions
            .get_mut(&fixture.target.id)
            .unwrap()
            .status = status;
        let before = snapshot(&fixture.context, &fixture.probes);
        let mut token = copy_token(&fixture.target);
        let mut expected_token = token_snapshot(&token);
        expected_token.completion = Some(expected);
        assert_eq!(fixture.context.poll(&mut token).unwrap(), expected);
        assert_eq!(token_snapshot(&token), expected_token);
        assert_eq!(snapshot(&fixture.context, &fixture.probes), before);
    }
    *fixture
        .context
        .submissions
        .get_mut(&fixture.target.id)
        .unwrap() = original;
    fixture.cleanup();
}
