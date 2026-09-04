use alloc::{vec, vec::Vec};

use super::*;

fn device() -> R13SchedulerDeviceKeyV1 {
    R13SchedulerDeviceKeyV1 {
        device_id: 13,
        generation: 1,
    }
}

fn stream(stream_id: u64) -> R13LogicalStreamKeyV1 {
    R13LogicalStreamKeyV1 {
        device: device(),
        stream_id,
        generation: 1,
    }
}

fn submission(stream_id: u64, sequence: u64) -> R13ScheduledSubmissionKeyV1 {
    R13ScheduledSubmissionKeyV1 {
        stream: stream(stream_id),
        sequence,
    }
}

fn resource(resource_id: u64) -> R13ScheduledResourceKeyV1 {
    R13ScheduledResourceKeyV1 {
        device: device(),
        resource_id,
        generation: 1,
    }
}

fn model_with_streams(count: u64) -> R13LogicalSchedulerModelV1 {
    let mut model = R13LogicalSchedulerModelV1::new_model_only(device()).unwrap();
    for stream_id in 1..=count {
        model.register_stream_model_only(stream(stream_id)).unwrap();
    }
    model
}

#[test]
fn many_logical_streams_share_exactly_two_physical_lanes() {
    let mut model = model_with_streams(8);
    assert_eq!(model.physical_lane_count(), 2);
    for stream_id in 1..=8 {
        model
            .enqueue_model_only(
                submission(stream_id, 1),
                R13OperationClassV1::Compute,
                &[],
                &[],
            )
            .unwrap();
    }
    assert_eq!(model.publish_head_model_only(submission(1, 1)), Ok(0));
    assert_eq!(model.publish_head_model_only(submission(2, 1)), Ok(1));
    assert_eq!(
        model.publish_head_model_only(submission(3, 1)),
        Err(R13SchedulerModelErrorV1::NoPhysicalLane)
    );
    assert_eq!(model.lane_owner(2), None);
    model.validate_global_invariants().unwrap();
}

#[test]
fn fifo_order_spans_compute_and_copy_classes() {
    let mut model = model_with_streams(1);
    let first = submission(1, 1);
    let second = submission(1, 2);
    model
        .enqueue_model_only(first, R13OperationClassV1::Compute, &[], &[])
        .unwrap();
    model
        .enqueue_model_only(second, R13OperationClassV1::Copy, &[], &[])
        .unwrap();
    assert_eq!(
        model.publish_head_model_only(second),
        Err(R13SchedulerModelErrorV1::NotStreamHead)
    );
    let lane = model.publish_head_model_only(first).unwrap();
    model
        .observe_terminal_model_only(first, lane, R13TerminalStatusV1::Succeeded)
        .unwrap();
    assert_eq!(model.stream(stream(1)).unwrap().head(), Some(second));
    assert_eq!(model.publish_head_model_only(second), Ok(0));
    assert_eq!(
        model.submission(second).unwrap().class(),
        R13OperationClassV1::Copy
    );
    model.validate_global_invariants().unwrap();
}

#[test]
fn publication_requires_successful_bounded_dependencies() {
    let mut model = model_with_streams(2);
    let producer = submission(1, 1);
    let consumer = submission(2, 1);
    model
        .enqueue_model_only(producer, R13OperationClassV1::Compute, &[], &[])
        .unwrap();
    model
        .enqueue_model_only(consumer, R13OperationClassV1::Copy, &[producer], &[])
        .unwrap();
    assert_eq!(model.submission(consumer).unwrap().dependency_depth(), 2);
    assert_eq!(
        model.publish_head_model_only(consumer),
        Err(R13SchedulerModelErrorV1::DependencyNotReady)
    );
    let lane = model.publish_head_model_only(producer).unwrap();
    model
        .observe_terminal_model_only(producer, lane, R13TerminalStatusV1::Succeeded)
        .unwrap();
    assert!(model.publish_head_model_only(consumer).is_ok());
    model.validate_global_invariants().unwrap();
}

#[test]
fn failed_dependency_never_becomes_publishable() {
    let mut model = model_with_streams(2);
    let producer = submission(1, 1);
    let consumer = submission(2, 1);
    model
        .enqueue_model_only(producer, R13OperationClassV1::Compute, &[], &[])
        .unwrap();
    model
        .enqueue_model_only(consumer, R13OperationClassV1::Compute, &[producer], &[])
        .unwrap();
    let lane = model.publish_head_model_only(producer).unwrap();
    model
        .observe_terminal_model_only(producer, lane, R13TerminalStatusV1::Failed { code: -5 })
        .unwrap();
    assert_eq!(
        model.publish_head_model_only(consumer),
        Err(R13SchedulerModelErrorV1::DependencyNotReady)
    );
}

#[test]
fn already_failed_dependency_is_rejected_before_mutation() {
    let mut model = model_with_streams(2);
    let producer = submission(1, 1);
    let consumer = submission(2, 1);
    model
        .enqueue_model_only(producer, R13OperationClassV1::Compute, &[], &[])
        .unwrap();
    let lane = model.publish_head_model_only(producer).unwrap();
    model
        .observe_terminal_model_only(producer, lane, R13TerminalStatusV1::Failed { code: -5 })
        .unwrap();
    assert_eq!(
        model.enqueue_model_only(consumer, R13OperationClassV1::Compute, &[producer], &[],),
        Err(R13SchedulerModelErrorV1::DependencyNotReady)
    );
    assert!(model.submission(consumer).is_none());
    assert_eq!(model.stream(stream(2)).unwrap().head(), None);
    assert_eq!(model.stream(stream(2)).unwrap().tail(), None);
    model.validate_global_invariants().unwrap();
}

#[test]
fn failed_implicit_stream_predecessor_never_becomes_publishable() {
    let mut model = model_with_streams(1);
    let producer = submission(1, 1);
    let consumer = submission(1, 2);
    model
        .enqueue_model_only(producer, R13OperationClassV1::Compute, &[], &[])
        .unwrap();
    model
        .enqueue_model_only(consumer, R13OperationClassV1::Copy, &[], &[])
        .unwrap();
    assert_eq!(
        model.submission(consumer).unwrap().dependencies(),
        &[producer]
    );
    let lane = model.publish_head_model_only(producer).unwrap();
    model
        .observe_terminal_model_only(producer, lane, R13TerminalStatusV1::Failed { code: -5 })
        .unwrap();
    assert_eq!(
        model.publish_head_model_only(consumer),
        Err(R13SchedulerModelErrorV1::DependencyNotReady)
    );
    model.validate_global_invariants().unwrap();
}

#[test]
fn dependency_count_and_depth_are_bounded() {
    let mut model = model_with_streams(2);
    let unknown_dependencies = vec![submission(1, 1); MAX_R13_DEPENDENCIES_V1 + 1];
    assert_eq!(
        model.enqueue_model_only(
            submission(2, 1),
            R13OperationClassV1::Compute,
            &unknown_dependencies,
            &[],
        ),
        Err(R13SchedulerModelErrorV1::CapacityExceeded)
    );

    let first = submission(1, 1);
    model
        .enqueue_model_only(first, R13OperationClassV1::Compute, &[], &[])
        .unwrap();
    assert_eq!(model.submission(first).unwrap().dependency_depth(), 1);
    for sequence in 2..=u64::try_from(MAX_R13_DEPENDENCY_DEPTH_V1).unwrap() {
        let next = submission(1, sequence);
        model
            .enqueue_model_only(next, R13OperationClassV1::Compute, &[], &[])
            .unwrap();
        assert_eq!(
            model.submission(next).unwrap().dependency_depth(),
            usize::try_from(sequence).unwrap()
        );
    }
    assert_eq!(
        model.enqueue_model_only(
            submission(1, u64::try_from(MAX_R13_DEPENDENCY_DEPTH_V1 + 1).unwrap()),
            R13OperationClassV1::Compute,
            &[],
            &[],
        ),
        Err(R13SchedulerModelErrorV1::CapacityExceeded)
    );
    model.validate_global_invariants().unwrap();
}

#[test]
fn implicit_predecessor_counts_toward_dependency_capacity() {
    let dependency_stream_count = u64::try_from(MAX_R13_DEPENDENCIES_V1).unwrap();
    let mut model = model_with_streams(dependency_stream_count + 1);
    let tail = submission(1, 1);
    model
        .enqueue_model_only(tail, R13OperationClassV1::Compute, &[], &[])
        .unwrap();
    let mut explicit = Vec::new();
    for stream_id in 2..=dependency_stream_count + 1 {
        let dependency = submission(stream_id, 1);
        model
            .enqueue_model_only(dependency, R13OperationClassV1::Compute, &[], &[])
            .unwrap();
        explicit.push(dependency);
    }
    assert_eq!(explicit.len(), MAX_R13_DEPENDENCIES_V1);
    assert!(!explicit.contains(&tail));
    assert_eq!(
        model.enqueue_model_only(submission(1, 2), R13OperationClassV1::Copy, &explicit, &[],),
        Err(R13SchedulerModelErrorV1::CapacityExceeded)
    );
    assert_eq!(model.stream(stream(1)).unwrap().tail(), Some(tail));
    assert!(model.submission(submission(1, 2)).is_none());
    model.validate_global_invariants().unwrap();
}

#[test]
fn active_conflict_blocks_publication_but_terminal_retention_does_not() {
    let mut model = model_with_streams(2);
    let shared = resource(1);
    model.register_resource_model_only(shared).unwrap();
    let first = submission(1, 1);
    let second = submission(2, 1);
    model
        .enqueue_model_only(first, R13OperationClassV1::Compute, &[], &[shared])
        .unwrap();
    model
        .enqueue_model_only(second, R13OperationClassV1::Copy, &[], &[shared])
        .unwrap();
    let lane = model.publish_head_model_only(first).unwrap();
    assert_eq!(
        model.publish_head_model_only(second),
        Err(R13SchedulerModelErrorV1::ResourceBusy)
    );
    model
        .observe_terminal_model_only(first, lane, R13TerminalStatusV1::Succeeded)
        .unwrap();
    assert_eq!(model.lane_owner(lane), Some(None));
    assert!(model.publish_head_model_only(second).is_ok());
    model.release_terminal_model_only(first).unwrap();
    model.validate_global_invariants().unwrap();
}

#[test]
fn terminal_observation_is_bound_to_unique_lane_lease() {
    let mut model = model_with_streams(2);
    let first = submission(1, 1);
    let second = submission(2, 1);
    model
        .enqueue_model_only(first, R13OperationClassV1::Compute, &[], &[])
        .unwrap();
    model
        .enqueue_model_only(second, R13OperationClassV1::Compute, &[], &[])
        .unwrap();
    let first_lane = model.publish_head_model_only(first).unwrap();
    let second_lane = model.publish_head_model_only(second).unwrap();
    assert_ne!(first_lane, second_lane);
    assert_eq!(
        model.observe_terminal_model_only(first, second_lane, R13TerminalStatusV1::Succeeded),
        Err(R13SchedulerModelErrorV1::IllegalTransition)
    );
    assert_eq!(model.lane_owner(first_lane), Some(Some(first)));
    model.validate_global_invariants().unwrap();
}

#[test]
fn only_unpublished_tail_cancels_and_prior_tail_is_restored() {
    let mut model = model_with_streams(1);
    let first = submission(1, 1);
    let second = submission(1, 2);
    model
        .enqueue_model_only(first, R13OperationClassV1::Compute, &[], &[])
        .unwrap();
    model
        .enqueue_model_only(second, R13OperationClassV1::Copy, &[], &[])
        .unwrap();
    assert_eq!(
        model.cancel_tail_model_only(first),
        Err(R13SchedulerModelErrorV1::TooLate)
    );
    model.cancel_tail_model_only(second).unwrap();
    let stream = model.stream(stream(1)).unwrap();
    assert_eq!(stream.head(), Some(first));
    assert_eq!(stream.tail(), Some(first));
    assert_eq!(model.submission(first).unwrap().successor(), None);
    assert_eq!(
        model.submission(second).unwrap().phase(),
        R13ScheduledSubmissionPhaseV1::CancelledBeforePublication
    );
    let lane = model.publish_head_model_only(first).unwrap();
    assert_eq!(
        model.cancel_tail_model_only(first),
        Err(R13SchedulerModelErrorV1::TooLate)
    );
    assert_eq!(model.lane_owner(lane), Some(Some(first)));
    model.validate_global_invariants().unwrap();
}

#[test]
fn terminal_release_waits_for_queued_dependents() {
    let mut model = model_with_streams(2);
    let producer = submission(1, 1);
    let consumer = submission(2, 1);
    model
        .enqueue_model_only(producer, R13OperationClassV1::Compute, &[], &[])
        .unwrap();
    model
        .enqueue_model_only(consumer, R13OperationClassV1::Copy, &[producer], &[])
        .unwrap();
    let lane = model.publish_head_model_only(producer).unwrap();
    model
        .observe_terminal_model_only(producer, lane, R13TerminalStatusV1::Succeeded)
        .unwrap();
    assert_eq!(
        model.release_terminal_model_only(producer),
        Err(R13SchedulerModelErrorV1::ResourceBusy)
    );
    model.publish_head_model_only(consumer).unwrap();
    model.release_terminal_model_only(producer).unwrap();
    assert_eq!(
        model.submission(producer).unwrap().phase(),
        R13ScheduledSubmissionPhaseV1::Released(R13TerminalStatusV1::Succeeded)
    );
    model.validate_global_invariants().unwrap();
}

#[test]
fn terminal_release_waits_for_implicit_stream_successor() {
    let mut model = model_with_streams(1);
    let retained = resource(1);
    model.register_resource_model_only(retained).unwrap();
    let producer = submission(1, 1);
    let consumer = submission(1, 2);
    model
        .enqueue_model_only(producer, R13OperationClassV1::Compute, &[], &[retained])
        .unwrap();
    model
        .enqueue_model_only(consumer, R13OperationClassV1::Copy, &[], &[])
        .unwrap();
    let lane = model.publish_head_model_only(producer).unwrap();
    model
        .observe_terminal_model_only(producer, lane, R13TerminalStatusV1::Succeeded)
        .unwrap();
    assert_eq!(
        model.release_terminal_model_only(producer),
        Err(R13SchedulerModelErrorV1::ResourceBusy)
    );
    model.publish_head_model_only(consumer).unwrap();
    model.release_terminal_model_only(producer).unwrap();
    assert_eq!(model.resource_active_owner(retained), Some(None));
    assert_eq!(model.resource_retainers(retained), Some([].as_slice()));
    model.validate_global_invariants().unwrap();
}

#[test]
fn three_deep_same_resource_fifo_progresses_with_unreleased_older_terminals() {
    let mut model = model_with_streams(1);
    let shared = resource(1);
    model.register_resource_model_only(shared).unwrap();
    let producer = submission(1, 1);
    let consumer = submission(1, 2);
    let third = submission(1, 3);
    model
        .enqueue_model_only(producer, R13OperationClassV1::Compute, &[], &[shared])
        .unwrap();
    model
        .enqueue_model_only(consumer, R13OperationClassV1::Copy, &[], &[shared])
        .unwrap();
    model
        .enqueue_model_only(third, R13OperationClassV1::Compute, &[], &[shared])
        .unwrap();

    let producer_lane = model.publish_head_model_only(producer).unwrap();
    model
        .observe_terminal_model_only(producer, producer_lane, R13TerminalStatusV1::Succeeded)
        .unwrap();
    assert_eq!(model.resource_active_owner(shared), Some(None));
    assert_eq!(
        model.resource_retainers(shared),
        Some([producer].as_slice())
    );
    assert_eq!(
        model.release_terminal_model_only(producer),
        Err(R13SchedulerModelErrorV1::ResourceBusy)
    );

    let consumer_lane = model.publish_head_model_only(consumer).unwrap();
    assert_eq!(model.resource_active_owner(shared), Some(Some(consumer)));
    assert_eq!(
        model.resource_retainers(shared),
        Some([producer, consumer].as_slice())
    );
    model
        .observe_terminal_model_only(consumer, consumer_lane, R13TerminalStatusV1::Succeeded)
        .unwrap();
    assert_eq!(model.resource_active_owner(shared), Some(None));

    let third_lane = model.publish_head_model_only(third).unwrap();
    assert_eq!(model.resource_active_owner(shared), Some(Some(third)));
    assert_eq!(
        model.resource_retainers(shared),
        Some([producer, consumer, third].as_slice())
    );
    model.release_terminal_model_only(producer).unwrap();
    model.release_terminal_model_only(consumer).unwrap();
    assert_eq!(model.resource_retainers(shared), Some([third].as_slice()));
    model
        .observe_terminal_model_only(third, third_lane, R13TerminalStatusV1::Succeeded)
        .unwrap();
    assert_eq!(model.resource_active_owner(shared), Some(None));
    model.release_terminal_model_only(third).unwrap();
    assert_eq!(model.resource_retainers(shared), Some([].as_slice()));
    model.validate_global_invariants().unwrap();
}

#[test]
fn currentness_loss_cancels_queued_and_quarantines_retained_custody() {
    let mut model = model_with_streams(3);
    let active_resource = resource(1);
    let terminal_resource = resource(2);
    model.register_resource_model_only(active_resource).unwrap();
    model
        .register_resource_model_only(terminal_resource)
        .unwrap();
    let active = submission(1, 1);
    let terminal = submission(2, 1);
    let queued = submission(3, 1);
    model
        .enqueue_model_only(
            active,
            R13OperationClassV1::Compute,
            &[],
            &[active_resource],
        )
        .unwrap();
    model
        .enqueue_model_only(
            terminal,
            R13OperationClassV1::Copy,
            &[],
            &[terminal_resource],
        )
        .unwrap();
    model
        .enqueue_model_only(
            queued,
            R13OperationClassV1::Compute,
            &[],
            &[active_resource],
        )
        .unwrap();
    let active_lane = model.publish_head_model_only(active).unwrap();
    let terminal_lane = model.publish_head_model_only(terminal).unwrap();
    model
        .observe_terminal_model_only(terminal, terminal_lane, R13TerminalStatusV1::Succeeded)
        .unwrap();

    model.lose_currentness_model_only().unwrap();
    assert!(!model.current());
    assert_eq!(
        model.submission(queued).unwrap().phase(),
        R13ScheduledSubmissionPhaseV1::CancelledBeforePublication
    );
    assert_eq!(
        model.submission(active).unwrap().phase(),
        R13ScheduledSubmissionPhaseV1::Indeterminate {
            lane: Some(active_lane),
            terminal: None,
        }
    );
    assert_eq!(model.resource_quarantined(active_resource), Some(true));
    assert_eq!(
        model.submission(terminal).unwrap().phase(),
        R13ScheduledSubmissionPhaseV1::Indeterminate {
            lane: None,
            terminal: Some(R13TerminalStatusV1::Succeeded),
        }
    );
    assert_eq!(model.resource_quarantined(terminal_resource), Some(true));
    assert_eq!(
        model.release_terminal_model_only(terminal),
        Err(R13SchedulerModelErrorV1::IllegalTransition)
    );
    assert_eq!(
        model.publish_head_model_only(queued),
        Err(R13SchedulerModelErrorV1::NotCurrent)
    );
    model.validate_global_invariants().unwrap();
}

#[test]
fn stream_and_resource_identities_are_device_bound() {
    let mut model = model_with_streams(1);
    let wrong_device = R13SchedulerDeviceKeyV1 {
        generation: 2,
        ..device()
    };
    assert_eq!(
        model.register_stream_model_only(R13LogicalStreamKeyV1 {
            device: wrong_device,
            stream_id: 2,
            generation: 1,
        }),
        Err(R13SchedulerModelErrorV1::InvalidIdentity)
    );
    assert_eq!(
        model.register_resource_model_only(R13ScheduledResourceKeyV1 {
            device: wrong_device,
            resource_id: 1,
            generation: 1,
        }),
        Err(R13SchedulerModelErrorV1::InvalidIdentity)
    );
    model.validate_global_invariants().unwrap();
}

#[test]
fn registered_stream_capacity_exceeds_lane_count_but_remains_bounded() {
    let mut model = R13LogicalSchedulerModelV1::new_model_only(device()).unwrap();
    for stream_id in 1..=u64::try_from(MAX_R13_LOGICAL_STREAMS_V1).unwrap() {
        model.register_stream_model_only(stream(stream_id)).unwrap();
    }
    assert_eq!(
        model.register_stream_model_only(stream(
            u64::try_from(MAX_R13_LOGICAL_STREAMS_V1 + 1).unwrap()
        )),
        Err(R13SchedulerModelErrorV1::CapacityExceeded)
    );
    assert!(model.stream(stream(3)).is_some());
    assert_eq!(model.physical_lane_count(), 2);
    model.validate_global_invariants().unwrap();
}
