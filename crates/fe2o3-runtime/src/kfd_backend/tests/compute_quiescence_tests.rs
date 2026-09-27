//! Public native SPI over scripted persistent owners, not GPU evidence.

use super::*;
use std::mem::ManuallyDrop;

mod sdma;
mod waw;

fn dependency(
    f: &mut ScriptedActiveProducerFixtureV1,
    stream: u64,
    id: u64,
) -> BackendLaunchProducerV1 {
    BackendLaunchProducerV1 {
        event: f.backend.record_event_v1(stream, id).unwrap(),
        producer_submission: id,
    }
}

fn submit(
    f: &mut ScriptedActiveProducerFixtureV1,
    stream: u64,
    deps: &[BackendLaunchProducerV1],
) -> Result<u64, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
    f.backend
        .submit_producer_aware_launch_v1(BackendProducerAwareLaunchV1 {
            stream,
            kernel: f.launch.kernel,
            explicit_kernarg: &f.launch.explicit_kernarg,
            bindings: &f.launch.bindings,
            dependencies: deps,
            geometry: f.launch.geometry,
        })
}

fn finish_submission(f: &mut ScriptedActiveProducerFixtureV1, stream: u64, id: u64) {
    f.backend.flush_stream_v1(stream).unwrap();
    assert_eq!(
        f.backend
            .wait_v1(id, Instant::now() + Duration::from_secs(1))
            .unwrap(),
        BackendPollV1::Succeeded
    );
    assert_eq!(
        f.backend
            .last_launch_performance_v1()
            .unwrap()
            .user_data_materializations(),
        0
    );
}

#[test]
fn native_quiescence_shared_inputs_follow_native_intermediate_without_extra_success_edges() {
    let mut f = ManuallyDrop::new(ScriptedActiveProducerFixtureV1::new(true));
    let b_stream = f.launch.stream;
    let b = f.submit().unwrap();
    let b_dep = dependency(&mut f, b_stream, b);
    let stream = f.backend.create_stream_v1(7).unwrap();
    let before = f.backend.scripted_sdma.as_ref().unwrap().remaining_steps();
    let c = submit(&mut f, stream, &[b_dep]).unwrap();
    let pending = &f.backend.pending_compute[&c];
    assert_eq!(&*pending.explicit_success_dependencies, &[b]);
    assert_eq!(&*pending.quiescence_dependencies, &[f.producer]);
    assert_eq!(pending.quiescence_cursor, 0);
    assert_eq!(pending.dependency_depth, 3);
    assert_eq!(f.backend.compute_dependency_retain_counts[&f.producer], 2);
    assert_eq!(f.backend.active.as_ref().unwrap().id, f.producer);
    assert_eq!(
        f.backend.scripted_sdma.as_ref().unwrap().remaining_steps(),
        before
    );
    let event = f.event;
    f.backend.release_event_v1(event).unwrap();
    f.backend.release_event_v1(b_dep.event).unwrap();
    assert_eq!(f.backend.poll_v1(c).unwrap(), BackendPollV1::Pending);
    assert!(f.backend.pending_compute.contains_key(&b));
    finish_submission(&mut f, b_stream, b);
    finish_submission(&mut f, stream, c);
    f.backend.release_submission_v1(c).unwrap();
    f.backend.release_submission_v1(b).unwrap();
    f.backend.destroy_stream_v1(stream).unwrap();
    ManuallyDrop::into_inner(f).finish(None);
}

#[test]
fn native_quiescence_cancelled_explicit_intermediate_fails_without_waiting_for_ancestor() {
    let mut f = ManuallyDrop::new(ScriptedActiveProducerFixtureV1::new(true));
    let b_stream = f.launch.stream;
    let b = f.submit().unwrap();
    let b_dep = dependency(&mut f, b_stream, b);
    let stream = f.backend.create_stream_v1(7).unwrap();
    let c = submit(&mut f, stream, &[b_dep]).unwrap();
    assert_eq!(
        f.backend.cancel_v1(b).unwrap(),
        crate::BackendCancellationV1::Cancelled
    );
    assert_eq!(f.backend.compute_dependency_retain_counts[&f.producer], 1);
    f.backend.scripted_persistent_poll_pending_observations = 1;
    assert!(matches!(
        f.backend.poll_v1(c).unwrap(),
        BackendPollV1::Failed { .. }
    ));
    assert_eq!(f.backend.scripted_persistent_poll_pending_observations, 0);
    assert!(!f.backend.exact_submission_quiescent_v1(f.producer));
    assert!(f.backend.active_compute_submission_v1(f.producer).is_some());
    assert!(
        !f.backend
            .compute_dependency_retain_counts
            .contains_key(&f.producer)
    );
    f.backend.release_event_v1(b_dep.event).unwrap();
    f.backend.release_submission_v1(c).unwrap();
    f.backend.release_submission_v1(b).unwrap();
    f.backend.destroy_stream_v1(stream).unwrap();
    ManuallyDrop::into_inner(f).finish(None);
}

#[test]
fn native_quiescence_failed_fifo_intermediate_keeps_ancestor_until_real_completion() {
    let mut f = ManuallyDrop::new(ScriptedActiveProducerFixtureV1::new(true));
    let failed_stream = f.launch.stream;
    let failed = f.submit().unwrap();
    let failed_dep = dependency(&mut f, failed_stream, failed);
    let stream = f.backend.create_stream_v1(7).unwrap();
    let a_dep = BackendLaunchProducerV1 {
        event: f.event,
        producer_submission: f.producer,
    };
    let b = submit(&mut f, stream, &[failed_dep, a_dep]).unwrap();
    let c = submit(&mut f, stream, &[]).unwrap();
    assert_eq!(
        &*f.backend.pending_compute[&c].quiescence_dependencies,
        &[f.producer, failed]
    );
    assert!(
        f.backend.pending_compute[&c]
            .explicit_success_dependencies
            .is_empty()
    );
    assert_eq!(f.backend.pending_compute[&c].ordered_predecessor, Some(b));
    assert_eq!(f.backend.pending_compute[&c].dependency_depth, 1);
    assert_eq!(
        f.backend.cancel_v1(b).unwrap(),
        crate::BackendCancellationV1::TooLate
    );
    assert_eq!(
        f.backend.cancel_v1(failed).unwrap(),
        crate::BackendCancellationV1::Cancelled
    );
    for event in [failed_dep.event, a_dep.event] {
        f.backend.release_event_v1(event).unwrap();
    }
    f.backend.scripted_persistent_poll_pending_observations = 1;
    assert!(matches!(
        f.backend.poll_v1(b).unwrap(),
        BackendPollV1::Failed { .. }
    ));
    assert_eq!(f.backend.scripted_persistent_poll_pending_observations, 0);
    assert!(!f.backend.exact_submission_quiescent_v1(f.producer));
    assert!(f.backend.active_compute_submission_v1(f.producer).is_some());
    assert_eq!(f.backend.compute_dependency_retain_counts[&f.producer], 1);
    assert_eq!(f.backend.compute_dependency_retain_counts[&failed], 1);
    assert!(
        matches!(f.backend.release_submission_v1(failed), Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Busy)
    );
    let producer = f.producer;
    assert!(
        matches!(f.backend.release_submission_v1(producer), Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Busy)
    );
    assert!(!f.backend.pending_compute_can_publish_under_deadline_v1(c));
    f.backend.scripted_persistent_poll_pending_observations = 1;
    f.backend.flush_stream_v1(stream).unwrap();
    assert_eq!(f.backend.scripted_persistent_poll_pending_observations, 0);
    assert_eq!(f.backend.pending_compute[&c].quiescence_cursor, 0);
    assert!(!f.backend.exact_submission_quiescent_v1(producer));
    assert!(f.backend.active_compute_lane_v1(c).is_none());
    assert_eq!(f.backend.poll_v1(c).unwrap(), BackendPollV1::Pending);
    assert_eq!(
        f.backend.submissions[&f.producer].status,
        BackendPollV1::Succeeded
    );
    assert!(f.backend.pending_compute.contains_key(&c));
    assert!(f.backend.active_compute_lane_v1(c).is_none());
    finish_submission(&mut f, stream, c);
    for id in [c, b, failed] {
        f.backend.release_submission_v1(id).unwrap();
    }
    f.backend.destroy_stream_v1(stream).unwrap();
    ManuallyDrop::into_inner(f).finish(None);
}

#[test]
fn native_quiescence_unrelated_owner_is_rejected_and_consumer_cancellation_refunds_once() {
    let mut f = ManuallyDrop::new(ScriptedActiveProducerFixtureV1::new(true));
    let b_stream = f.launch.stream;
    let b = f.submit().unwrap();
    let b_dep = dependency(&mut f, b_stream, b);
    let stream = f.backend.create_stream_v1(7).unwrap();
    let before = f.backend.compute_dependency_retain_counts.clone();
    assert!(
        matches!(submit(&mut f, stream, &[]), Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Busy)
    );
    assert_eq!(f.backend.compute_dependency_retain_counts, before);
    let c = submit(&mut f, stream, &[b_dep]).unwrap();
    assert_eq!(
        f.backend.cancel_v1(c).unwrap(),
        crate::BackendCancellationV1::Cancelled
    );
    assert_eq!(f.backend.compute_dependency_retain_counts, before);
    assert_eq!(f.backend.active.as_ref().unwrap().id, f.producer);
    f.backend.release_event_v1(b_dep.event).unwrap();
    f.backend.release_submission_v1(c).unwrap();
    assert_eq!(
        f.backend.cancel_v1(b).unwrap(),
        crate::BackendCancellationV1::Cancelled
    );
    f.backend.release_submission_v1(b).unwrap();
    f.backend.destroy_stream_v1(stream).unwrap();
    ManuallyDrop::into_inner(f).finish(None);
}

#[test]
fn native_quiescence_same_stream_input_owner_survives_an_intermediate() {
    let mut f = ManuallyDrop::new(ScriptedActiveProducerFixtureV1::new(false));
    let stream = f.launch.stream;
    let b = f.submit().unwrap();
    let c = submit(&mut f, stream, &[]).unwrap();
    assert_eq!(f.backend.pending_compute[&c].ordered_predecessor, Some(b));
    assert_eq!(
        &*f.backend.pending_compute[&c].quiescence_dependencies,
        &[f.producer]
    );
    assert!(
        f.backend.pending_compute[&c]
            .explicit_success_dependencies
            .is_empty()
    );
    assert_eq!(f.backend.compute_dependency_retain_counts[&f.producer], 2);
    let producer = f.producer;
    assert!(
        matches!(f.backend.release_submission_v1(producer), Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Busy)
    );
    assert_eq!(f.backend.poll_v1(c).unwrap(), BackendPollV1::Pending);
    assert!(f.backend.pending_compute.contains_key(&b));
    finish_submission(&mut f, stream, b);
    finish_submission(&mut f, stream, c);
    f.backend.release_submission_v1(c).unwrap();
    f.backend.release_submission_v1(b).unwrap();
    ManuallyDrop::into_inner(f).finish(None);
}

#[test]
fn native_quiescence_fifo_chain_does_not_copy_pending_owner_history() {
    let mut f = ManuallyDrop::new(ScriptedActiveProducerFixtureV1::new(false));
    let stream = f.launch.stream;
    let mut submissions = vec![f.submit().unwrap()];
    for _ in 0..7 {
        let id = submit(&mut f, stream, &[]).unwrap();
        let pending = &f.backend.pending_compute[&id];
        assert_eq!(pending.ordered_predecessor, submissions.last().copied());
        assert_eq!(&*pending.quiescence_dependencies, &[f.producer]);
        assert!(pending.explicit_success_dependencies.is_empty());
        assert_eq!(pending.dependency_depth, 1);
        submissions.push(id);
    }
    assert_eq!(
        f.backend.compute_dependency_retain_counts[&f.producer],
        submissions.len()
    );
    for id in submissions.into_iter().rev() {
        assert_eq!(
            f.backend.cancel_v1(id).unwrap(),
            crate::BackendCancellationV1::Cancelled
        );
        f.backend.release_submission_v1(id).unwrap();
    }
    assert!(f.backend.compute_dependency_retain_counts.is_empty());
    assert!(!f.backend.exact_submission_quiescent_v1(f.producer));
    ManuallyDrop::into_inner(f).finish(None);
}

#[test]
fn native_quiescence_cancelled_sdma_intermediate_cannot_discard_pending_compute_owner() {
    let steps = (0..6).flat_map(|_| {
        [
            ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
        ]
    });
    let (backend, stream, [a, b, c, d, source, destination]) =
        scripted_persistent_backend_with_steps_v1::<6>(64, steps);
    let mut f = ManuallyDrop::new(ScriptedActiveProducerFixtureV1::with_backend(
        backend,
        stream,
        [a, b, c, d],
        false,
    ));
    let producer = f.producer;
    let pending_owner = f.submit().unwrap();
    let (source_region, destination_region) =
        scripted_same_device_copy_regions_v1(source, destination, 64);
    let copy = f
        .backend
        .copy_async_v1(stream, source_region, destination_region, &[])
        .unwrap();
    assert!(matches!(
        f.backend.active_sdma[&copy].phase,
        ActiveSdmaPhaseV1::Ready
    ));
    let consumer = submit(&mut f, stream, &[]).unwrap();
    let pending = &f.backend.pending_compute[&consumer];
    assert_eq!(pending.ordered_predecessor, Some(copy));
    assert_eq!(
        &*pending.quiescence_dependencies,
        &[producer, pending_owner]
    );
    assert_eq!(
        f.backend.cancel_v1(copy).unwrap(),
        crate::BackendCancellationV1::Cancelled
    );
    assert_eq!(
        f.backend.compute_dependency_retain_counts[&pending_owner],
        1
    );
    assert_eq!(
        f.backend.poll_v1(pending_owner).unwrap(),
        BackendPollV1::Pending
    );
    f.backend.flush_stream_v1(stream).unwrap();
    assert!(
        f.backend
            .active_compute_submission_v1(pending_owner)
            .is_some()
    );
    assert!(!f.backend.exact_submission_quiescent_v1(pending_owner));
    f.backend.scripted_persistent_poll_pending_observations = 1;
    f.backend.flush_stream_v1(stream).unwrap();
    assert_eq!(f.backend.scripted_persistent_poll_pending_observations, 0);
    assert_eq!(f.backend.pending_compute[&consumer].quiescence_cursor, 1);
    assert!(f.backend.active_compute_lane_v1(consumer).is_none());
    assert!(!f.backend.exact_submission_quiescent_v1(pending_owner));
    assert_eq!(f.backend.poll_v1(consumer).unwrap(), BackendPollV1::Pending);
    finish_submission(&mut f, stream, consumer);
    for id in [consumer, copy, pending_owner] {
        f.backend.release_submission_v1(id).unwrap();
    }
    for allocation in [source, destination] {
        f.backend
            .allocations
            .get_mut(&allocation)
            .unwrap()
            .sdma_backed = false;
        f.backend.release_allocation_v1(allocation).unwrap();
    }
    ManuallyDrop::into_inner(f).finish(None);
}

#[test]
fn native_quiescence_same_stream_optimization_authenticates_owner_before_skipping() {
    let mut f = ManuallyDrop::new(ScriptedActiveProducerFixtureV1::new(false));
    let stream = f.launch.stream;
    let b = f.submit().unwrap();
    let allocation = f.allocations[3];
    assert_eq!(
        f.backend.allocation_custody[&allocation].owners[0].submission,
        b
    );
    f.backend
        .allocation_custody
        .get_mut(&allocation)
        .unwrap()
        .owners[0]
        .submission = b + 100;
    let next = f.backend.next_handle;
    let reservations = f.backend.compute_completion_reservations;
    assert!(matches!(
        submit(&mut f, stream, &[]),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    assert_eq!(f.backend.next_handle, next);
    assert_eq!(f.backend.compute_completion_reservations, reservations);
    assert_eq!(f.backend.pending_compute.len(), 1);
    f.backend
        .allocation_custody
        .get_mut(&allocation)
        .unwrap()
        .owners[0]
        .submission = b;
    f.backend.terminal = false;
    assert_eq!(
        f.backend.cancel_v1(b).unwrap(),
        crate::BackendCancellationV1::Cancelled
    );
    f.backend.release_submission_v1(b).unwrap();
    ManuallyDrop::into_inner(f).finish(None);
}

#[test]
fn native_quiescence_observer_error_and_unwind_preserve_exact_custody() {
    for failure in 0..3 {
        let mut f = ManuallyDrop::new(ScriptedActiveProducerFixtureV1::new(true));
        let b_stream = f.launch.stream;
        let b = f.submit().unwrap();
        let b_dep = dependency(&mut f, b_stream, b);
        let stream = f.backend.create_stream_v1(7).unwrap();
        let c = submit(&mut f, stream, &[b_dep]).unwrap();
        let pending = f.backend.pending_compute.remove(&c).unwrap();
        let launch = Arc::clone(&pending.launch);
        let retains = f.backend.compute_dependency_retain_counts.clone();
        let fifo = f.backend.pending_compute_streams.clone();
        let reservations = f.backend.compute_completion_reservations;
        let owners = f
            .allocations
            .map(|id| f.backend.allocation_custody[&id].owners.clone());
        let native_owners = f.backend.scripted_sdma.as_ref().unwrap().live_owner_count();
        let producer = f.producer;
        let mut polls = 0;
        let observed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            f.backend
                .observe_compute_quiescence_with_v1(pending, |backend, id| {
                    polls += 1;
                    assert_eq!(id, producer);
                    match failure {
                        0 => Err(KfdRuntimeBackendV1::rejected(
                            KfdRuntimeBackendErrorKindV1::UnknownHandle,
                            "injected retained-owner rejection",
                        )),
                        1 => {
                            Err(backend.terminal_error("injected retained-owner terminal failure"))
                        }
                        _ => std::panic::panic_any("injected retained-owner unwind"),
                    }
                })
        }));
        match observed {
            Ok(Err(RuntimeBackendFailureV1::Terminal(_))) => assert!(failure < 2),
            Err(payload) => {
                assert_eq!(failure, 2);
                assert_eq!(
                    payload.downcast_ref::<&str>(),
                    Some(&"injected retained-owner unwind")
                );
            }
            _ => panic!("unexpected quiescence observation result"),
        }
        assert_eq!(polls, 1);
        assert!(f.backend.terminal);
        let pending = &f.backend.pending_compute[&c];
        assert!(Arc::ptr_eq(&pending.launch, &launch));
        assert_eq!(&*pending.quiescence_dependencies, &[producer]);
        assert_eq!(pending.quiescence_cursor, 0);
        assert_eq!(f.backend.compute_dependency_retain_counts, retains);
        assert_eq!(f.backend.pending_compute_streams, fifo);
        assert_eq!(f.backend.compute_completion_reservations, reservations);
        assert_eq!(
            f.allocations
                .map(|id| f.backend.allocation_custody[&id].owners.clone()),
            owners
        );
        assert_eq!(
            f.backend.scripted_sdma.as_ref().unwrap().live_owner_count(),
            native_owners
        );
        assert!(!f.backend.exact_submission_quiescent_v1(producer));
        // The injected callback never entered the native owner; this is fixture cleanup, not terminal recovery.
        f.backend.terminal = false;
        f.backend.release_event_v1(b_dep.event).unwrap();
        for id in [c, b] {
            assert_eq!(
                f.backend.cancel_v1(id).unwrap(),
                crate::BackendCancellationV1::Cancelled
            );
            f.backend.release_submission_v1(id).unwrap();
        }
        f.backend.destroy_stream_v1(stream).unwrap();
        ManuallyDrop::into_inner(f).finish(None);
    }
}

#[test]
fn native_quiescence_invalid_cursor_is_terminal_without_polling_or_refunding() {
    let mut f = ManuallyDrop::new(ScriptedActiveProducerFixtureV1::new(true));
    let b_stream = f.launch.stream;
    let b = f.submit().unwrap();
    let b_dep = dependency(&mut f, b_stream, b);
    let stream = f.backend.create_stream_v1(7).unwrap();
    let c = submit(&mut f, stream, &[b_dep]).unwrap();
    let mut pending = f.backend.pending_compute.remove(&c).unwrap();
    let launch = Arc::clone(&pending.launch);
    pending.quiescence_cursor = pending.quiescence_dependencies.len() + 1;
    let invalid_cursor = pending.quiescence_cursor;
    assert!(!pending.quiescence_complete_v1());
    let retains = f.backend.compute_dependency_retain_counts.clone();
    let fifo = f.backend.pending_compute_streams.clone();
    let reservations = f.backend.compute_completion_reservations;
    let owners = f.allocations.map(|id| {
        f.backend
            .allocation_custody
            .get(&id)
            .map(|row| row.owners.clone())
    });
    let steps = f.backend.scripted_sdma.as_ref().unwrap().remaining_steps();
    assert!(matches!(
        f.backend
            .observe_compute_quiescence_with_v1(pending, |_, _| {
                panic!("invalid cursor must not observe any owner")
            }),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    assert!(f.backend.terminal);
    let pending = &f.backend.pending_compute[&c];
    assert_eq!(pending.quiescence_cursor, invalid_cursor);
    assert!(Arc::ptr_eq(&pending.launch, &launch));
    assert_eq!(f.backend.compute_dependency_retain_counts, retains);
    assert_eq!(f.backend.pending_compute_streams, fifo);
    assert_eq!(f.backend.compute_completion_reservations, reservations);
    assert_eq!(
        f.allocations.map(|id| f
            .backend
            .allocation_custody
            .get(&id)
            .map(|row| row.owners.clone())),
        owners
    );
    assert_eq!(
        f.backend.scripted_sdma.as_ref().unwrap().remaining_steps(),
        steps
    );
    // No native observer ran; restore only the injected metadata for fixture teardown.
    f.backend
        .pending_compute
        .get_mut(&c)
        .unwrap()
        .quiescence_cursor = 0;
    f.backend.terminal = false;
    f.backend.release_event_v1(b_dep.event).unwrap();
    for id in [c, b] {
        assert_eq!(
            f.backend.cancel_v1(id).unwrap(),
            crate::BackendCancellationV1::Cancelled
        );
        f.backend.release_submission_v1(id).unwrap();
    }
    f.backend.destroy_stream_v1(stream).unwrap();
    ManuallyDrop::into_inner(f).finish(None);
}

#[test]
fn native_quiescence_missing_prefix_retain_is_terminal_before_admission() {
    let mut f = ManuallyDrop::new(ScriptedActiveProducerFixtureV1::new(true));
    let b_stream = f.launch.stream;
    let b = f.submit().unwrap();
    let b_dep = dependency(&mut f, b_stream, b);
    let stream = f.backend.create_stream_v1(7).unwrap();
    let producer = f.producer;
    let count = f
        .backend
        .compute_dependency_retain_counts
        .remove(&producer)
        .unwrap();
    let pending_count = f.backend.pending_compute.len();
    let reservations = f.backend.compute_completion_reservations;
    let before = f.backend.scripted_sdma.as_ref().unwrap().remaining_steps();
    assert!(matches!(
        submit(&mut f, stream, &[b_dep]),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    assert_eq!(f.backend.pending_compute.len(), pending_count);
    assert_eq!(f.backend.compute_completion_reservations, reservations);
    assert_eq!(
        f.backend.scripted_sdma.as_ref().unwrap().remaining_steps(),
        before
    );
    assert!(!f.backend.exact_submission_quiescent_v1(producer));
    f.backend
        .compute_dependency_retain_counts
        .insert(producer, count);
    f.backend.terminal = false;
    f.backend.release_event_v1(b_dep.event).unwrap();
    assert_eq!(
        f.backend.cancel_v1(b).unwrap(),
        crate::BackendCancellationV1::Cancelled
    );
    f.backend.release_submission_v1(b).unwrap();
    f.backend.destroy_stream_v1(stream).unwrap();
    ManuallyDrop::into_inner(f).finish(None);
}
