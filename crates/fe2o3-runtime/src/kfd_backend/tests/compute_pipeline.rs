use super::*;

#[test]
fn runtime_compute_pipeline_quarantine_is_lane_wide_and_custody_preserving() {
    let mut pipeline = RuntimeComputePipelineV1::vacant();
    pipeline
        .insert_published(pipelined_active_for_test_v1(2))
        .unwrap();
    pipeline
        .insert_published(pipelined_active_for_test_v1(3))
        .unwrap();
    pipeline.quarantine_all();
    assert_eq!(
        pipeline.phase(2),
        Some(RuntimeComputePipelinePhaseV1::Quarantined)
    );
    assert_eq!(
        pipeline.phase(3),
        Some(RuntimeComputePipelinePhaseV1::Quarantined)
    );
    assert_eq!(pipeline.len(), 2);
    assert!(pipeline.contains(2));
    assert!(pipeline.contains(3));
}

#[test]
fn runtime_compute_pipeline_drop_aborts_for_every_live_logical_phase() {
    use std::os::unix::process::ExitStatusExt;

    const CHILD: &str = "FE2O3_TEST_RUNTIME_PIPELINE_ABORT_CHILD";
    if let Some(case) = std::env::var_os(CHILD) {
        let mut backend = KfdRuntimeBackendV1::mock();
        match case.to_str().expect("test case is ASCII") {
            "queued" => {
                backend
                    .pending_compute
                    .insert(2, pending_compute_for_test_v1(2, 7, 100, vec![]));
            }
            "published" => {
                backend
                    .compute_pipeline
                    .insert_published(pipelined_active_for_test_v1(2))
                    .unwrap();
            }
            "recycled" => {
                backend
                    .compute_pipeline
                    .insert_published(pipelined_active_for_test_v1(2))
                    .unwrap();
                let (identity, active) = backend.compute_pipeline.take_physical_owner(2).unwrap();
                backend
                    .compute_pipeline
                    .restore(
                        identity,
                        RuntimeComputePipelinePhaseV1::PhysicallyRetired,
                        active,
                    )
                    .unwrap();
            }
            "completed" => {
                backend
                    .compute_pipeline
                    .insert_published(pipelined_active_for_test_v1(2))
                    .unwrap();
                let (identity, active) = backend.compute_pipeline.take_physical_owner(2).unwrap();
                backend
                    .compute_pipeline
                    .restore(identity, RuntimeComputePipelinePhaseV1::Completed, active)
                    .unwrap();
            }
            "quarantined" => {
                backend
                    .compute_pipeline
                    .insert_published(pipelined_active_for_test_v1(2))
                    .unwrap();
                backend.compute_pipeline.quarantine_all();
            }
            _ => unreachable!("known runtime pipeline Drop case"),
        }
        drop(backend);
        std::process::exit(97);
    }
    for case in [
        "queued",
        "published",
        "completed",
        "recycled",
        "quarantined",
    ] {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "kfd_backend::tests::compute_pipeline::runtime_compute_pipeline_drop_aborts_for_every_live_logical_phase",
                    "--nocapture",
                ])
                .env(CHILD, case)
                .status()
                .unwrap();
        assert_eq!(
            status.signal(),
            Some(6),
            "runtime pipeline Drop case {case} did not terminate through SIGABRT"
        );
    }
}

#[test]
fn runtime_compute_pipeline_rejects_stale_read_after_write_authority_shapes() {
    let binding = |allocation, access| BackendBindingV1 {
        region: BackendMemoryRegionV1 {
            allocation,
            access,
            byte_offset: 0,
            byte_len: 8,
        },
        kernarg_byte_offset: 0,
    };
    assert!(early_pipeline_access_is_admitted_v1(&[
        binding(1, RuntimeAccessV1::Read),
        binding(2, RuntimeAccessV1::Write),
    ]));
    assert!(!early_pipeline_access_is_admitted_v1(&[binding(
        1,
        RuntimeAccessV1::ReadWrite,
    )]));
    assert!(!early_pipeline_access_is_admitted_v1(&[
        binding(1, RuntimeAccessV1::Read),
        binding(1, RuntimeAccessV1::Write),
    ]));
    assert!(ordered_successor_lane_matches_v1(Some(1), 1));
    assert!(!ordered_successor_lane_matches_v1(Some(0), 1));
    assert!(!ordered_successor_lane_matches_v1(None, 1));
}

#[test]
fn runtime_compute_pipeline_coalesces_repeated_exact_dirty_extents() {
    let extent = NativeDirtyExtentV1 {
        compute_lane: 1,
        data_index: 2,
        allocation_offset: 8,
        data_offset: 16,
        byte_len: 32,
    };
    let mut dirty = Vec::new();
    assert!(retain_unique_native_dirty_extent_v1(&mut dirty, extent));
    assert!(!retain_unique_native_dirty_extent_v1(&mut dirty, extent));
    assert_eq!(dirty, [extent]);
    assert!(retain_unique_native_dirty_extent_v1(
        &mut dirty,
        NativeDirtyExtentV1 {
            allocation_offset: 9,
            ..extent
        }
    ));
    assert_eq!(dirty.len(), 2);
}

#[test]
fn runtime_compute_early_publication_is_ordinary_only_and_zero_materialization() {
    let bindings = [BackendBindingV1 {
        region: BackendMemoryRegionV1 {
            allocation: 1,
            access: RuntimeAccessV1::Read,
            byte_offset: 0,
            byte_len: 8,
        },
        kernarg_byte_offset: 0,
    }];
    assert!(early_pipeline_launch_is_admitted_v1(
        KfdRuntimeSemanticLaunchV1::Ordinary,
        &bindings,
    ));
    assert!(!early_pipeline_launch_is_admitted_v1(
        KfdRuntimeSemanticLaunchV1::Atomic(atomic_contract_v1()),
        &bindings,
    ));
    assert!(!early_pipeline_launch_is_admitted_v1(
        KfdRuntimeSemanticLaunchV1::Collective(collective_contract_v1()),
        &bindings,
    ));

    let source = include_str!("../compute_dispatch/pending.rs");
    let body = source
        .split("fn try_publish_ordered_successor_v1(")
        .nth(1)
        .unwrap()
        .split("fn pending_compute_can_publish_under_deadline_v1(")
        .next()
        .unwrap();
    assert!(body.contains("persistent_full_range_admission_for_launch_v1"));
    assert!(body.contains("three_binding_persistent_admission_for_launch_v1"));
    assert!(body.contains("early_pipeline_launch_is_admitted_v1"));
    assert!(body.contains("publish_indexed_ordered_successor_v1"));
    assert!(body.contains("performance.user_data_materializations = 0"));
    assert!(!body.contains("materialize_initial_data_v1"));
    assert!(!body.contains("publish_persistent_full_range_v1"));
    assert!(!body.contains("publish_three_binding_persistent_v1"));
    let adapter = include_str!("../ordered_publication.rs");
    assert!(adapter.contains("OrdinaryQueueIoV1::new"));
    let io = include_str!("../ordinary_queue_io.rs");
    assert!(io.contains("lane.submit_fixed_dispatch_classified_v1::<1>()"));
    assert!(!io.contains("materialize_initial_data_v1"));
    assert!(!io.contains("bind_fixed_dispatch"));
    assert!(!adapter.contains("materialize_initial_data_v1"));
    assert!(!adapter.contains("bind_fixed_dispatch"));
    assert!(!adapter.contains("publish_persistent_full_range_v1"));
    assert!(!adapter.contains("publish_three_binding_persistent_v1"));
}

#[test]
fn runtime_compute_ordering_does_not_turn_wait_for_prior_into_success_gating() {
    assert!(ordered_predecessor_completed_v1(BackendPollV1::Succeeded));
    assert!(ordered_predecessor_completed_v1(BackendPollV1::Failed {
        code: -7
    }));
    assert!(!ordered_predecessor_completed_v1(BackendPollV1::Pending));
    assert!(explicit_dependency_succeeded_v1(BackendPollV1::Succeeded));
    assert!(!explicit_dependency_succeeded_v1(BackendPollV1::Failed {
        code: -7
    }));

    let mut backend = KfdRuntimeBackendV1::mock();
    let mut predecessor = pending_compute_for_test_v1(40, 1, 100, vec![]);
    predecessor.dependency_depth = MAX_DIRECT_SDMA_COPY_DEPENDENCY_DEPTH_V1;
    backend.pending_compute.insert(40, predecessor);
    assert_eq!(backend.next_dependency_depth_v1(Some(40), &[]), Ok(1));
    assert_eq!(
        backend.next_dependency_depth_v1(Some(40), &[40]),
        Err(DirectSdmaDependencyDepthErrorV1::LimitExceeded)
    );
    backend.pending_compute.clear();
    for id in 1..=512 {
        let mut pending = pending_compute_for_test_v1(id, 1, 100, vec![]);
        pending.ordered_predecessor = id.checked_sub(1);
        assert_eq!(
            backend.next_dependency_depth_v1(pending.ordered_predecessor, &[]),
            Ok(1)
        );
        backend.pending_compute.insert(id, pending);
    }
    backend.pending_compute.clear();
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn runtime_compute_failed_explicit_dependency_preserves_stream_prefix_ordering() {
    for observe_only in [false, true] {
        for quiescent_dependency in [false, true] {
            for predecessor_status in [BackendPollV1::Succeeded, BackendPollV1::Failed { code: -9 }]
            {
                let mut backend = KfdRuntimeBackendV1::mock();
                let stream = backend.create_stream_v1(7).unwrap();
                let foreign_stream = backend.create_stream_v1(7).unwrap();
                let allocation = backend
                    .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
                    .unwrap();
                let successor_allocation = backend
                    .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
                    .unwrap();
                // The completion receipt for A stays pending while the
                // foreign dependency of B fails; C must remain behind B.
                for (id, owner_stream) in [(40, stream), (30, foreign_stream)] {
                    backend.submissions.insert(
                        id,
                        SubmissionRecordV1 {
                            stream: owner_stream,
                            status: BackendPollV1::Pending,
                            dependency_depth: 1,
                            profile_dispatch_published: false,
                        },
                    );
                }
                let mut failed = pending_compute_for_test_v1(41, stream, allocation, vec![30]);
                failed.ordered_predecessor = Some(40);
                let mut successor =
                    pending_compute_for_test_v1(42, stream, successor_allocation, vec![]);
                successor.ordered_predecessor = Some(41);
                backend.pending_compute.insert(41, failed);
                backend.pending_compute.insert(42, successor);
                backend
                    .pending_compute_streams
                    .insert(stream, VecDeque::from([41, 42]));
                for id in [41, 42] {
                    index_pending_compute_custody_for_test_v1(&mut backend, id);
                }
                backend.compute_completion_reservations = 2;
                for dependency in [40, 41, 30] {
                    backend
                        .compute_dependency_retain_counts
                        .insert(dependency, 1);
                }
                backend.stream_submission_tails.insert(stream, 42);
                backend.submissions.get_mut(&30).unwrap().status =
                    BackendPollV1::Failed { code: -2 };
                if quiescent_dependency {
                    backend.quiescent_sdma_submissions.insert(30);
                }
                let progress = |backend: &mut KfdRuntimeBackendV1, id| {
                    let pending = backend.pending_compute.remove(&id).unwrap();
                    if observe_only {
                        backend.observe_pending_compute_v1(pending)
                    } else {
                        backend.progress_pending_compute_v1(pending)
                    }
                };

                for _ in 0..2 {
                    assert_eq!(progress(&mut backend, 41).unwrap(), BackendPollV1::Pending);
                    assert_eq!(progress(&mut backend, 42).unwrap(), BackendPollV1::Pending);
                    assert_eq!(backend.pending_compute_streams[&stream], [41, 42]);
                    assert!(!backend.submissions.contains_key(&41));
                    assert!(!backend.submissions.contains_key(&42));
                    assert_eq!(backend.compute_completion_reservations, 2);
                    assert_eq!(backend.compute_module_retain_counts[&9], 2);
                    for dependency in [40, 41, 30] {
                        assert_eq!(backend.compute_dependency_retain_counts[&dependency], 1);
                    }
                    assert!(matches!(
                        backend.release_allocation_v1(allocation),
                        Err(RuntimeBackendFailureV1::Rejected(error))
                            if error.kind() == KfdRuntimeBackendErrorKindV1::Busy
                    ));
                    assert!(!backend.any_compute_active_v1());
                }

                backend.submissions.get_mut(&40).unwrap().status = predecessor_status;
                assert_eq!(
                    progress(&mut backend, 41).unwrap(),
                    BackendPollV1::Failed { code: -1 }
                );
                assert_eq!(backend.pending_compute_streams[&stream], [42]);
                assert_eq!(backend.compute_completion_reservations, 1);
                assert_eq!(backend.compute_module_retain_counts[&9], 1);
                assert!(!backend.compute_dependency_retain_counts.contains_key(&40));
                assert!(!backend.compute_dependency_retain_counts.contains_key(&30));
                assert_eq!(backend.compute_dependency_retain_counts[&41], 1);

                // The failed ordered receipt does not fail C: observation
                // leaves this now-eligible launch ready for publication.
                let successor = backend.pending_compute.remove(&42).unwrap();
                assert_eq!(
                    backend.observe_pending_compute_v1(successor).unwrap(),
                    BackendPollV1::Pending
                );
                assert_eq!(
                    backend.cancel_v1(42).unwrap(),
                    crate::BackendCancellationV1::Cancelled
                );
                for id in [40, 41, 42, 30] {
                    backend.release_submission_v1(id).unwrap();
                }
                backend.release_allocation_v1(allocation).unwrap();
                backend.release_allocation_v1(successor_allocation).unwrap();
                backend.destroy_stream_v1(stream).unwrap();
                backend.destroy_stream_v1(foreign_stream).unwrap();
                release_pending_compute_test_resources_v1(&mut backend);
                backend.shutdown_native_v1().unwrap();
            }
        }
    }
}

#[test]
fn runtime_compute_failed_ordered_predecessor_is_not_an_explicit_failure() {
    let make_backend = || {
        let mut backend = KfdRuntimeBackendV1::mock();
        let stream = backend.create_stream_v1(7).unwrap();
        let allocation = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        backend.submissions.insert(
            40,
            SubmissionRecordV1 {
                stream,
                status: BackendPollV1::Failed { code: -9 },
                dependency_depth: 1,
                profile_dispatch_published: false,
            },
        );
        (backend, stream, allocation)
    };

    let (mut ordered, stream, allocation) = make_backend();
    let mut pending = pending_compute_for_test_v1(41, stream, allocation, vec![]);
    pending.ordered_predecessor = Some(40);
    ordered
        .pending_compute_streams
        .insert(stream, VecDeque::from([41]));
    ordered.pending_compute.insert(41, pending);
    index_pending_compute_custody_for_test_v1(&mut ordered, 41);
    ordered.compute_completion_reservations = 1;
    ordered.compute_dependency_retain_counts.insert(40, 1);
    ordered.stream_submission_tails.insert(stream, 41);
    let pending = ordered.pending_compute.remove(&41).unwrap();
    assert_eq!(
        ordered.observe_pending_compute_v1(pending).unwrap(),
        BackendPollV1::Pending
    );
    assert!(ordered.pending_compute.contains_key(&41));
    assert_eq!(
        ordered.cancel_v1(41).unwrap(),
        crate::BackendCancellationV1::Cancelled
    );
    ordered.release_submission_v1(40).unwrap();
    ordered.release_submission_v1(41).unwrap();
    ordered.release_allocation_v1(allocation).unwrap();
    ordered.destroy_stream_v1(stream).unwrap();
    release_pending_compute_test_resources_v1(&mut ordered);
    ordered.shutdown_native_v1().unwrap();

    let (mut explicit, stream, allocation) = make_backend();
    let mut pending = pending_compute_for_test_v1(41, stream, allocation, vec![40]);
    pending.ordered_predecessor = Some(40);
    explicit
        .pending_compute_streams
        .insert(stream, VecDeque::from([41]));
    explicit.pending_compute.insert(41, pending);
    index_pending_compute_custody_for_test_v1(&mut explicit, 41);
    explicit.compute_completion_reservations = 1;
    explicit.compute_dependency_retain_counts.insert(40, 1);
    explicit.stream_submission_tails.insert(stream, 41);
    let pending = explicit.pending_compute.remove(&41).unwrap();
    assert_eq!(
        explicit.observe_pending_compute_v1(pending).unwrap(),
        BackendPollV1::Failed { code: -1 }
    );
    assert_eq!(
        explicit.submissions[&41].status,
        BackendPollV1::Failed { code: -1 }
    );
    explicit.release_submission_v1(40).unwrap();
    explicit.release_submission_v1(41).unwrap();
    explicit.release_allocation_v1(allocation).unwrap();
    explicit.destroy_stream_v1(stream).unwrap();
    release_pending_compute_test_resources_v1(&mut explicit);
    explicit.shutdown_native_v1().unwrap();
}

#[test]
fn runtime_compute_pipeline_publication_is_too_late_to_cancel() {
    let mut backend = KfdRuntimeBackendV1::mock();
    backend
        .compute_pipeline
        .insert_published(pipelined_active_for_test_v1(2))
        .unwrap();
    assert_eq!(
        backend.cancel_v1(2).unwrap(),
        crate::BackendCancellationV1::TooLate
    );
    let (_, active) = backend.compute_pipeline.take_commit_frontier().unwrap();
    assert_eq!(active.id, 2);
    backend.shutdown_native_v1().unwrap();
}
