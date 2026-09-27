//! Native full-output ordering with independent ready inputs, not Context writer chaining.

use super::*;

mod unwind_tests;

fn fixture(cross_stream: bool) -> (ManuallyDrop<ScriptedActiveProducerFixtureV1>, u64) {
    let steps = (0..5).flat_map(|_| {
        [
            ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
        ]
    });
    let (backend, stream, [a, b, c, d, e]) =
        scripted_persistent_backend_with_steps_v1::<5>(64, steps);
    let mut f =
        ScriptedActiveProducerFixtureV1::with_backend(backend, stream, [a, b, c, d], cross_stream);
    // Only the output overlaps the published producer; neither input can mask it.
    for (binding, allocation) in f.launch.bindings.iter_mut().zip([d, e, c]) {
        binding.region.allocation = allocation;
    }
    (ManuallyDrop::new(f), e)
}

fn finish(mut f: ManuallyDrop<ScriptedActiveProducerFixtureV1>, extra: u64, id: Option<u64>) {
    f.backend.allocations.get_mut(&extra).unwrap().sdma_backed = false;
    f.backend.release_allocation_v1(extra).unwrap();
    ManuallyDrop::into_inner(f).finish(id);
}

fn assert_producer_owns_output(f: &ScriptedActiveProducerFixtureV1) {
    let output = f.launch.bindings[2].region.allocation;
    assert!(matches!(f.backend.allocations[&output].sdma_storage,
        KfdRuntimeSdmaStorageV1::ComputeInFlight(owner) if owner == f.producer));
    assert!(f.backend.allocation_retains_exact_owner_v1(
        output,
        RuntimeAllocationCustodyOwnerV1 {
            submission: f.producer,
            stream: f.producer_stream,
            kind: RuntimeAllocationCustodyKindV1::Compute,
        }
    ));
    assert_eq!(
        f.backend.scripted_sdma.as_ref().unwrap().live_owner_count(),
        5
    );
    assert_eq!(
        f.backend.scripted_sdma.as_ref().unwrap().unexpected_drops(),
        0
    );
}

#[test]
fn native_waw_full_output_waits_for_explicit_or_fifo_owner_without_materialization() {
    for (producer_binding, cross_stream, explicit) in (0..3).flat_map(|binding| {
        [(false, false), (false, true), (true, true)]
            .map(move |(cross_stream, explicit)| (binding, cross_stream, explicit))
    }) {
        let (mut f, extra) = fixture(cross_stream);
        f.launch.bindings[2].region.allocation = f.allocations[producer_binding];
        f.backend.scripted_persistent_poll_pending_observations = 8;
        let stream = f.launch.stream;
        let c = if explicit {
            f.submit().unwrap()
        } else {
            submit(&mut f, stream, &[]).unwrap()
        };
        let pending = &f.backend.pending_compute[&c];
        assert_eq!(
            pending.explicit_success_dependencies.len(),
            usize::from(explicit)
        );
        if !cross_stream {
            assert_eq!(pending.ordered_predecessor, Some(f.producer));
        }
        let event = f.event;
        f.backend.release_event_v1(event).unwrap();
        f.backend.scripted_persistent_poll_pending_observations = 8;
        assert_eq!(f.backend.poll_v1(c).unwrap(), BackendPollV1::Pending);
        assert_producer_owns_output(&f);
        assert!(f.backend.pending_compute.contains_key(&c));
        assert!(f.backend.active_compute_lane_v1(c).is_none());
        assert!(
            f.backend
                .submissions
                .get(&c)
                .is_none_or(|record| !record.profile_dispatch_published)
        );
        f.backend.scripted_persistent_poll_pending_observations = 0;
        let producer = f.producer;
        assert_eq!(
            f.backend
                .wait_v1(producer, Instant::now() + Duration::from_secs(1))
                .unwrap(),
            BackendPollV1::Succeeded
        );
        assert_eq!(f.backend.poll_v1(c).unwrap(), BackendPollV1::Pending);
        finish_submission(&mut f, stream, c);
        assert_eq!(
            f.backend.last_launch_performance_v1().unwrap().data_path(),
            KfdRuntimeLaunchDataPathV1::PersistentDeviceReused
        );
        f.backend.release_submission_v1(c).unwrap();
        finish(f, extra, None);
    }
}

#[test]
fn native_waw_cancellation_refunds_only_the_unpublished_successor() {
    for explicit in [false, true] {
        let (mut f, extra) = fixture(explicit);
        let output = f.launch.bindings[2].region.allocation;
        let owners = f.backend.allocation_custody[&output].owners.clone();
        let retains = f.backend.compute_dependency_retain_counts.clone();
        let reservations = f.backend.compute_completion_reservations;
        let steps = f.backend.scripted_sdma.as_ref().unwrap().remaining_steps();
        let stream = f.launch.stream;
        f.backend.scripted_persistent_poll_pending_observations = 8;
        let c = if explicit {
            f.submit().unwrap()
        } else {
            submit(&mut f, stream, &[]).unwrap()
        };
        let event = f.event;
        f.backend.release_event_v1(event).unwrap();
        assert_eq!(
            f.backend.cancel_v1(c).unwrap(),
            crate::BackendCancellationV1::Cancelled
        );
        assert_producer_owns_output(&f);
        assert_eq!(f.backend.allocation_custody[&output].owners, owners);
        assert_eq!(f.backend.compute_dependency_retain_counts, retains);
        assert_eq!(f.backend.compute_completion_reservations, reservations);
        assert_eq!(
            f.backend.scripted_sdma.as_ref().unwrap().remaining_steps(),
            steps
        );
        for allocation in [f.allocations[3], extra] {
            assert!(!f.backend.allocation_custody.contains_key(&allocation));
        }
        f.backend.scripted_persistent_poll_pending_observations = 0;
        f.backend.release_submission_v1(c).unwrap();
        finish(f, extra, None);
    }
}

#[test]
fn native_waw_missing_authority_or_nonfull_output_rejects_before_admission() {
    let (mut f, extra) = fixture(true);
    let stream = f.launch.stream;
    let next = f.backend.next_handle;
    let reservations = f.backend.compute_completion_reservations;
    let output = f.launch.bindings[2].region.allocation;
    let owners = f.backend.allocation_custody[&output].owners.clone();
    assert!(matches!(
        submit(&mut f, stream, &[]),
        Err(RuntimeBackendFailureV1::Rejected(_))
    ));
    let fixture = &mut *f;
    let ordinary = BackendLaunchV1 {
        dependencies: &[fixture.event],
        ..fixture.launch.borrowed()
    };
    assert!(matches!(
        fixture.backend.submit_v1(ordinary),
        Err(RuntimeBackendFailureV1::Rejected(_))
    ));
    for case in 0..4 {
        let original = f.launch.bindings[2];
        match case {
            0 => f.launch.bindings[2].region.byte_len -= 4,
            1 => f.launch.bindings[2].region.byte_offset = 4,
            2 => f.launch.bindings[2].region.allocation = extra,
            3 => {
                f.backend.allocations.get_mut(&output).unwrap().sdma_storage =
                    KfdRuntimeSdmaStorageV1::ComputeInFlight(f.producer + 100)
            }
            _ => unreachable!(),
        }
        assert!(
            matches!(f.submit(), Err(RuntimeBackendFailureV1::Rejected(_))),
            "case {case}"
        );
        f.launch.bindings[2] = original;
        if case == 3 {
            f.backend.allocations.get_mut(&output).unwrap().sdma_storage =
                KfdRuntimeSdmaStorageV1::ComputeInFlight(f.producer);
        }
        assert_eq!(f.backend.next_handle, next);
        assert_eq!(f.backend.compute_completion_reservations, reservations);
        assert_eq!(f.backend.allocation_custody[&output].owners, owners);
        assert!(f.backend.pending_compute.is_empty());
        assert!(f.backend.compute_dependency_retain_counts.is_empty());
        assert_producer_owns_output(&f);
    }
    // ReadWrite remains outside the persistent R/R/W shape; its existing generic
    // admission path is not a new full-overwrite capability.
    f.launch.bindings[2].region.access = RuntimeAccessV1::ReadWrite;
    assert!(!three_binding_persistent_compute_shape_v1(
        f.launch.semantic_launch,
        &f.launch.bindings,
        7,
        &f.backend.allocations,
    ));
    finish(f, extra, None);
}

#[test]
fn native_waw_cancelled_explicit_intermediate_fails_without_releasing_active_output() {
    let (mut f, extra) = fixture(true);
    let b_stream = f.launch.stream;
    let b = f.submit().unwrap();
    let b_dep = dependency(&mut f, b_stream, b);
    let stream = f.backend.create_stream_v1(7).unwrap();
    let c = submit(&mut f, stream, &[b_dep]).unwrap();
    assert_eq!(
        &*f.backend.pending_compute[&c].quiescence_dependencies,
        &[f.producer]
    );
    assert_eq!(
        f.backend.cancel_v1(b).unwrap(),
        crate::BackendCancellationV1::Cancelled
    );
    f.backend.scripted_persistent_poll_pending_observations = 8;
    assert!(matches!(
        f.backend.poll_v1(c).unwrap(),
        BackendPollV1::Failed { .. }
    ));
    assert_producer_owns_output(&f);
    assert!(
        f.backend
            .submissions
            .get(&c)
            .is_none_or(|record| !record.profile_dispatch_published)
    );
    assert!(
        !f.backend
            .compute_dependency_retain_counts
            .contains_key(&f.producer)
    );
    f.backend.scripted_persistent_poll_pending_observations = 0;
    f.backend.release_event_v1(b_dep.event).unwrap();
    for id in [c, b] {
        f.backend.release_submission_v1(id).unwrap();
    }
    f.backend.destroy_stream_v1(stream).unwrap();
    finish(f, extra, None);
}

#[test]
fn native_waw_failed_fifo_intermediate_still_waits_for_output_ancestor() {
    let (mut f, extra) = fixture(true);
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
    assert_eq!(f.backend.pending_compute[&c].ordered_predecessor, Some(b));
    assert!(
        f.backend.pending_compute[&c]
            .explicit_success_dependencies
            .is_empty()
    );
    assert_eq!(
        f.backend.cancel_v1(failed).unwrap(),
        crate::BackendCancellationV1::Cancelled
    );
    f.backend.scripted_persistent_poll_pending_observations = 8;
    assert!(matches!(
        f.backend.poll_v1(b).unwrap(),
        BackendPollV1::Failed { .. }
    ));
    assert_producer_owns_output(&f);
    assert_eq!(f.backend.compute_dependency_retain_counts[&f.producer], 1);
    f.backend.flush_stream_v1(stream).unwrap();
    assert!(f.backend.pending_compute.contains_key(&c));
    assert!(f.backend.active_compute_lane_v1(c).is_none());
    assert!(
        f.backend
            .submissions
            .get(&c)
            .is_none_or(|record| !record.profile_dispatch_published)
    );
    assert_producer_owns_output(&f);
    f.backend.scripted_persistent_poll_pending_observations = 0;
    assert_eq!(f.backend.poll_v1(c).unwrap(), BackendPollV1::Pending);
    finish_submission(&mut f, stream, c);
    for event in [failed_dep.event, a_dep.event] {
        f.backend.release_event_v1(event).unwrap();
    }
    for id in [c, b, failed] {
        f.backend.release_submission_v1(id).unwrap();
    }
    f.backend.destroy_stream_v1(stream).unwrap();
    finish(f, extra, None);
}

#[test]
fn native_waw_revalidates_restored_output_before_publication() {
    let (mut f, extra) = fixture(true);
    let c = f.submit().unwrap();
    let producer = f.producer;
    assert_eq!(
        f.backend.poll_v1(producer).unwrap(),
        BackendPollV1::Succeeded
    );
    let output = f.launch.bindings[2].region.allocation;
    f.backend
        .allocations
        .get_mut(&output)
        .unwrap()
        .sdma_initialized = false;
    assert_eq!(f.backend.poll_v1(c).unwrap(), BackendPollV1::Pending);
    let stream = f.launch.stream;
    assert!(matches!(
        f.backend.flush_stream_v1(stream),
        Err(RuntimeBackendFailureV1::Quiescent(_))
    ));
    assert_eq!(
        f.backend.poll_v1(c).unwrap(),
        BackendPollV1::Failed { code: -1 }
    );
    assert!(!f.backend.submissions[&c].profile_dispatch_published);
    assert!(f.backend.active.is_none());
    f.backend
        .allocations
        .get_mut(&output)
        .unwrap()
        .sdma_initialized = true;
    f.backend.release_submission_v1(c).unwrap();
    finish(f, extra, None);
}

#[test]
fn native_waw_output_owner_drift_quarantines_both_rosters() {
    let (mut f, _extra) = fixture(true);
    let c = f.submit().unwrap();
    let output = f.launch.bindings[2].region.allocation;
    let owners = f.backend.allocation_custody[&output].owners.clone();
    f.backend.allocations.get_mut(&output).unwrap().sdma_storage =
        KfdRuntimeSdmaStorageV1::ComputeInFlight(f.producer + 100);
    assert!(matches!(
        f.backend.poll_v1(c),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    assert!(f.backend.terminal);
    assert!(matches!(
        f.backend.terminal_sdma_custody,
        Some(KfdRuntimeTerminalSdmaCustodyV1::ThreeBindingPersistentInputs(_))
    ));
    assert_eq!(f.backend.allocation_custody[&output].owners, owners);
    assert!(f.backend.pending_compute.contains_key(&c));
    assert_eq!(f.backend.compute_dependency_retain_counts[&f.producer], 1);
    assert_eq!(f.backend.compute_completion_reservations, 2);
    assert!(f.backend.active_compute_lane_v1(c).is_none());
    assert!(
        f.backend
            .submissions
            .get(&c)
            .is_none_or(|record| !record.profile_dispatch_published)
    );
    assert_eq!(
        f.backend.scripted_sdma.as_ref().unwrap().live_owner_count(),
        5
    );
    assert_eq!(
        f.backend.scripted_sdma.as_ref().unwrap().unexpected_drops(),
        0
    );
    disarm_scripted_drop_after_inspection_v1(&mut f.backend);
    drop(ManuallyDrop::into_inner(f));
}

#[cfg(feature = "hardware-qualification")]
#[test]
fn native_waw_rechecks_final_authority_after_output_restoration() {
    let (mut f, extra) = fixture(true);
    let admitted =
        crate::qualification_gfx942_r57_n3_v1::admit_gfx942_r57_n3_qualification_v1().unwrap();
    let observation = admitted.observation_v1();
    f.backend.launch_gate = KfdRuntimeLaunchGateV1::ExactGfx942R57N3(admitted);
    let c = f.submit().unwrap();
    assert_eq!(observation.authorization_calls_v1(), 0);
    assert_eq!(f.backend.poll_v1(c).unwrap(), BackendPollV1::Pending);
    assert_eq!(observation.authorization_calls_v1(), 0);
    let stream = f.launch.stream;
    assert!(matches!(
        f.backend.flush_stream_v1(stream),
        Err(RuntimeBackendFailureV1::Quiescent(_))
    ));
    assert_eq!(observation.authorization_calls_v1(), 1);
    assert_eq!(
        f.backend.poll_v1(c).unwrap(),
        BackendPollV1::Failed { code: -1 }
    );
    assert!(!f.backend.submissions[&c].profile_dispatch_published);
    assert!(f.backend.active.is_none());
    f.backend.release_submission_v1(c).unwrap();
    finish(f, extra, None);
}
