use super::*;

#[test]
fn graph_retirement_requires_closed_original_reservation_and_preserves_refusals() {
    let mut f = Fixture::new(2);
    let original = f.context.reserve_graph_v1(1).unwrap();
    assert_eq!(
        f.context.release_graph_v1(original),
        Err(RuntimeValidationErrorV1::SubmissionPending)
    );
    assert_eq!(f.context.graph_reservation, Some(original));
    assert!(!f.context.graph_issue_closed);
    f.context.close_graph_issue_v1(original).unwrap();
    f.context.release_graph_v1(original).unwrap();
    let current = f.context.reserve_graph_v1(1).unwrap();
    assert_ne!(current, original);
    f.context.close_graph_issue_v1(current).unwrap();
    assert_eq!(
        f.context.release_graph_v1(original),
        Err(RuntimeValidationErrorV1::ContextReserved)
    );
    assert_eq!(f.context.graph_reservation, Some(current));
    assert!(f.context.graph_issue_closed);
    f.context.release_graph_v1(current).unwrap();
    f.finish();
}

#[test]
fn graph_retirement_waits_for_original_replica_release_not_successful_poll() {
    let mut f = Fixture::new(3);
    let action = f
        .context
        .prepare_graph_replica_copy_v1(
            f.streams[1],
            region(f.allocations[0], RuntimeAccessV1::Read),
            region(f.allocations[1], RuntimeAccessV1::Write),
        )
        .unwrap();
    let token = f.context.reserve_graph_v1(1).unwrap();
    let mut submission = f.context.submit_graph_action_v1(token, action).unwrap();
    f.context.close_graph_issue_v1(token).unwrap();
    let charged = f.account.usage();
    assert_eq!(f.context.pending_replicas_v1(), 1);
    assert_eq!(
        f.context.release_graph_v1(token),
        Err(RuntimeValidationErrorV1::SubmissionPending)
    );
    let mut observed = false;
    for _ in 0..4 {
        if f.context
            .poll_with_graph_access_v1(&mut submission.original, Some(token))
            .unwrap()
            == RuntimePollV1::Succeeded
        {
            observed = true;
            break;
        }
    }
    assert!(observed);
    assert_eq!(f.context.pending_replicas_v1(), 1);
    assert_eq!(
        f.context.release_graph_v1(token),
        Err(RuntimeValidationErrorV1::SubmissionPending)
    );
    f.context.backend.release_submission_failure = MockMemoryFailure::Rejected;
    assert!(matches!(
        f.context.release_graph_submission_v1(token, &submission),
        Err(RuntimeErrorV1::BackendRejected(_))
    ));
    assert_eq!(f.context.graph_reservation, Some(token));
    assert_eq!(f.context.pending_replicas_v1(), 1);
    assert_eq!(f.context.submissions.len(), 1);
    assert_eq!(f.account.usage(), charged);
    f.context
        .release_graph_submission_v1(token, &submission)
        .unwrap();
    assert_eq!(f.context.pending_replicas_v1(), 0);
    assert_eq!(f.context.replica_registry_usage_v1().unwrap().settled, 1);
    f.context.release_graph_v1(token).unwrap();
    assert_eq!(f.context.graph_reservation, None);
    assert_eq!(f.account.usage(), charged);
    assert_eq!(f.read(1), [0x31; 96]);
    f.finish();
}

#[test]
fn replica_usage_counts_original_pending_and_settled_slots_separately() {
    let mut f = Fixture::new(5);
    let first = f.start(0, 1);
    let reference = f.settle(first);
    let second = f.start(0, 2);
    assert_eq!(
        f.context.replica_registry_usage_v1().unwrap(),
        RuntimeReplicaUsageV1 {
            capacity: 5,
            pending: 1,
            settled: 1,
        }
    );
    f.settle(second);
    assert_eq!(f.context.replica_registry_usage_v1().unwrap().pending, 0);
    assert_eq!(f.context.replica_registry_usage_v1().unwrap().settled, 2);
    f.context.forget_replica_v1(reference).unwrap();
    assert_eq!(f.context.replica_registry_usage_v1().unwrap().settled, 1);
    let token = f.context.reserve_graph_v1(0).unwrap();
    f.context.close_graph_issue_v1(token).unwrap();
    f.context.release_graph_v1(token).unwrap();
    f.finish();
}
