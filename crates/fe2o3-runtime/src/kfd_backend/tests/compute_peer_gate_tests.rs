use super::*;

const OWNER: u64 = 900;

fn submit_gated(fixture: &mut ScriptedActiveProducerFixtureV1, ordered: bool) -> u64 {
    let dependencies = [BackendLaunchProducerV1 {
        event: fixture.event,
        producer_submission: fixture.producer,
    }];
    let launch = fixture.launch.borrowed();
    let mut collected = fixture
        .backend
        .preflight_compute_v1(launch, ComputeDependencyRosterV1::Exact(&dependencies))
        .unwrap();
    let id = fixture.backend.next_handle;
    collected.peer_gate = Some(PeerComputeGateV1::waiting(OWNER, id, ordered));
    assert_eq!(
        fixture
            .backend
            .submit_collected_compute_v1(launch, collected)
            .unwrap(),
        id
    );
    id
}

fn resolve(backend: &mut KfdRuntimeBackendV1, id: u64, result: PeerComputeResultV1, ordered: bool) {
    let pending = backend.pending_compute.get_mut(&id).unwrap();
    pending.peer_gate = Some(
        pending
            .peer_gate
            .unwrap()
            .resolve(OWNER, id, result, ordered)
            .unwrap(),
    );
}

#[test]
fn child_peer_gate_closed_blocks_all_compute_progress_but_observes_its_parent() {
    for cross_stream in [false, true] {
        for ingress in 0..7 {
            let mut fixture = ScriptedActiveProducerFixtureV1::new(cross_stream);
            let id = submit_gated(&mut fixture, true);
            assert_eq!(
                fixture.backend.active.as_ref().unwrap().id,
                fixture.producer
            );
            assert!(
                !fixture
                    .backend
                    .pending_compute_can_publish_under_deadline_v1(id)
            );
            assert_eq!(fixture.backend.poll_v1(id).unwrap(), BackendPollV1::Pending);
            assert!(
                fixture
                    .backend
                    .exact_submission_quiescent_v1(fixture.producer)
            );
            assert!(fixture.backend.active.is_none());
            let module_retains = fixture.backend.compute_module_retain_counts.clone();
            let reservations = fixture.backend.compute_completion_reservations;
            let driver_steps = fixture
                .backend
                .scripted_sdma
                .as_ref()
                .unwrap()
                .remaining_steps();
            for _ in 0..2 {
                match ingress {
                    0 => assert_eq!(fixture.backend.poll_v1(id).unwrap(), BackendPollV1::Pending),
                    1 => assert_eq!(
                        fixture.backend.wait_v1(id, Instant::now()).unwrap(),
                        BackendPollV1::Pending
                    ),
                    2 => assert_eq!(
                        fixture
                            .backend
                            .wait_v1(id, Instant::now() + Duration::from_millis(1))
                            .unwrap(),
                        BackendPollV1::Pending
                    ),
                    3 => fixture
                        .backend
                        .flush_stream_v1(fixture.launch.stream)
                        .unwrap(),
                    4 => {
                        let pending = fixture.backend.pending_compute.remove(&id).unwrap();
                        assert_eq!(
                            fixture
                                .backend
                                .progress_pending_compute_v1(pending)
                                .unwrap(),
                            BackendPollV1::Pending
                        );
                    }
                    5 => {
                        let pending = fixture.backend.pending_compute.remove(&id).unwrap();
                        assert_eq!(
                            fixture.backend.observe_pending_compute_v1(pending).unwrap(),
                            BackendPollV1::Pending
                        );
                    }
                    6 => assert_eq!(
                        fixture
                            .backend
                            .drain_v1(id, Instant::now() + Duration::from_millis(1))
                            .unwrap(),
                        BackendPollV1::Pending
                    ),
                    _ => unreachable!(),
                }
                assert!(fixture.backend.active.is_none());
                assert_eq!(fixture.backend.compute_module_retain_counts, module_retains);
                assert_eq!(
                    fixture.backend.compute_completion_reservations,
                    reservations
                );
                assert_eq!(
                    fixture
                        .backend
                        .scripted_sdma
                        .as_ref()
                        .unwrap()
                        .remaining_steps(),
                    driver_steps
                );
                assert!(fixture.backend.pending_compute.contains_key(&id));
                assert!(!fixture.backend.submissions.contains_key(&id));
            }
            for allocation in &fixture.allocations[1..] {
                assert!(
                    matches!(fixture.backend.release_allocation_v1(*allocation), Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Busy)
                );
            }
            resolve(
                &mut fixture.backend,
                id,
                PeerComputeResultV1::Succeeded,
                true,
            );
            fixture
                .backend
                .flush_stream_v1(fixture.launch.stream)
                .unwrap();
            assert_eq!(
                fixture
                    .backend
                    .wait_v1(id, Instant::now() + Duration::from_secs(1))
                    .unwrap(),
                BackendPollV1::Succeeded
            );
            assert_eq!(
                fixture
                    .backend
                    .last_launch_performance_v1()
                    .unwrap()
                    .user_data_materializations(),
                0
            );
            fixture.finish(Some(id));
        }
    }
}

#[test]
fn child_peer_gate_success_does_not_replace_external_order_or_backing_readiness() {
    for bad_backing in [false, true] {
        let mut fixture = ScriptedActiveProducerFixtureV1::new(true);
        let id = submit_gated(&mut fixture, false);
        resolve(
            &mut fixture.backend,
            id,
            PeerComputeResultV1::Succeeded,
            false,
        );
        fixture
            .backend
            .flush_stream_v1(fixture.launch.stream)
            .unwrap();
        assert!(fixture.backend.pending_compute.contains_key(&id));
        assert!(fixture.backend.active.is_none());
        assert!(
            !fixture
                .backend
                .pending_compute_can_publish_under_deadline_v1(id)
        );
        if bad_backing {
            fixture
                .backend
                .allocations
                .get_mut(&fixture.allocations[2])
                .unwrap()
                .sdma_initialized = false;
        }
        resolve(
            &mut fixture.backend,
            id,
            PeerComputeResultV1::Succeeded,
            true,
        );
        let result = fixture.backend.flush_stream_v1(fixture.launch.stream);
        if bad_backing {
            assert!(matches!(result, Err(RuntimeBackendFailureV1::Quiescent(_))));
            assert_eq!(
                fixture.backend.poll_v1(id).unwrap(),
                BackendPollV1::Failed { code: -1 }
            );
            fixture
                .backend
                .allocations
                .get_mut(&fixture.allocations[2])
                .unwrap()
                .sdma_initialized = true;
        } else {
            result.unwrap();
            assert_eq!(
                fixture
                    .backend
                    .wait_v1(id, Instant::now() + Duration::from_secs(1))
                    .unwrap(),
                BackendPollV1::Succeeded
            );
        }
        fixture.finish(Some(id));
    }
}

#[test]
fn child_peer_gate_cancellation_releases_consumer_not_live_parent() {
    let mut fixture = ScriptedActiveProducerFixtureV1::new(true);
    let id = submit_gated(&mut fixture, false);
    assert_eq!(
        fixture.backend.cancel_v1(id).unwrap(),
        crate::BackendCancellationV1::Cancelled
    );
    assert!(!fixture.backend.pending_compute.contains_key(&id));
    assert_eq!(
        fixture.backend.active.as_ref().unwrap().id,
        fixture.producer
    );
    assert!(
        !fixture
            .backend
            .allocation_custody
            .contains_key(&fixture.allocations[3])
    );
    fixture.finish(Some(id));
}

#[test]
fn child_peer_gate_identity_rejects_before_admission_effects_and_seals_late_corruption() {
    let mut fixture = ScriptedActiveProducerFixtureV1::new(true);
    let dependencies = [BackendLaunchProducerV1 {
        event: fixture.event,
        producer_submission: fixture.producer,
    }];
    let launch = fixture.launch.borrowed();
    let mut collected = fixture
        .backend
        .preflight_compute_v1(launch, ComputeDependencyRosterV1::Exact(&dependencies))
        .unwrap();
    let next = fixture.backend.next_handle;
    let reservations = fixture.backend.compute_completion_reservations;
    collected.peer_gate = Some(PeerComputeGateV1::waiting(OWNER, next + 1, true));
    assert!(matches!(
        fixture
            .backend
            .submit_collected_compute_v1(launch, collected),
        Err(RuntimeBackendFailureV1::Rejected(_))
    ));
    assert_eq!(fixture.backend.next_handle, next);
    assert_eq!(
        fixture.backend.compute_completion_reservations,
        reservations
    );
    let id = submit_gated(&mut fixture, true);
    let original = fixture.backend.pending_compute[&id].peer_gate;
    fixture
        .backend
        .pending_compute
        .get_mut(&id)
        .unwrap()
        .peer_gate = Some(PeerComputeGateV1::waiting(OWNER, id + 1, true));
    assert!(matches!(
        fixture.backend.poll_v1(id),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    assert!(fixture.backend.terminal);
    assert!(fixture.backend.pending_compute.contains_key(&id));
    assert_eq!(
        fixture.backend.active.as_ref().unwrap().id,
        fixture.producer
    );
    assert_eq!(
        fixture.backend.compute_completion_reservations,
        reservations + 1
    );
    // Disarm only scripted teardown after asserting terminal custody.
    fixture.backend.terminal = false;
    fixture
        .backend
        .pending_compute
        .get_mut(&id)
        .unwrap()
        .peer_gate = original;
    fixture.backend.cancel_v1(id).unwrap();
    fixture.finish(Some(id));
}

#[test]
fn child_peer_gate_transition_is_exact_monotonic_and_failure_cannot_reopen() {
    let waiting = PeerComputeGateV1::waiting(OWNER, 17, false);
    for (owner, consumer) in [(0, 17), (OWNER + 1, 17), (OWNER, 0), (OWNER, 18)] {
        assert_eq!(
            waiting.resolve(owner, consumer, PeerComputeResultV1::Succeeded, true),
            Err(())
        );
    }
    for result in [PeerComputeResultV1::Succeeded, PeerComputeResultV1::Failed] {
        let settled = waiting.resolve(OWNER, 17, result, true).unwrap();
        assert_eq!(
            settled.resolve(OWNER, 17, PeerComputeResultV1::Pending, true),
            Err(())
        );
        assert_eq!(settled.resolve(OWNER, 17, result, false), Err(()));
        assert_eq!(settled.resolve(OWNER, 17, result, true), Ok(settled));
        let other = if result == PeerComputeResultV1::Succeeded {
            PeerComputeResultV1::Failed
        } else {
            PeerComputeResultV1::Succeeded
        };
        assert_eq!(settled.resolve(OWNER, 17, other, true), Err(()));
    }
}

#[test]
fn child_peer_gate_terminal_and_unwind_restore_exact_pending_custody() {
    for unwind in [false, true] {
        let mut fixture = ScriptedActiveProducerFixtureV1::new(true);
        let id = submit_gated(&mut fixture, false);
        let reservations = fixture.backend.compute_completion_reservations;
        let modules = fixture.backend.compute_module_retain_counts.clone();
        let dependencies = fixture.backend.compute_dependency_retain_counts.clone();
        let gate = fixture.backend.pending_compute[&id].peer_gate;
        let pending = fixture.backend.pending_compute.remove(&id).unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            fixture
                .backend
                .observe_peer_compute_gate_with_v1(pending, |backend, pending| {
                    assert_eq!(pending.id, id);
                    assert!(!backend.pending_compute.contains_key(&id));
                    assert_eq!(backend.active.as_ref().unwrap().id, fixture.producer);
                    if unwind {
                        std::panic::panic_any(0xabc_u32);
                    }
                    // A returned terminal error must seal even if the observer did not.
                    Err(RuntimeBackendFailureV1::Terminal(
                        KfdRuntimeBackendErrorV1 {
                            kind: KfdRuntimeBackendErrorKindV1::Native,
                            detail: "injected peer observation failure".to_owned(),
                        },
                    ))
                })
        }));
        if unwind {
            assert_eq!(*result.err().unwrap().downcast::<u32>().unwrap(), 0xabc);
        } else {
            assert!(matches!(
                result.unwrap(),
                Err(RuntimeBackendFailureV1::Terminal(_))
            ));
        }
        assert!(fixture.backend.terminal);
        assert_eq!(fixture.backend.pending_compute[&id].peer_gate, gate);
        assert_eq!(
            fixture.backend.compute_completion_reservations,
            reservations
        );
        assert_eq!(fixture.backend.compute_module_retain_counts, modules);
        assert_eq!(
            fixture.backend.compute_dependency_retain_counts,
            dependencies
        );
        assert_eq!(
            fixture.backend.active.as_ref().unwrap().id,
            fixture.producer
        );
        assert!(!fixture.backend.submissions.contains_key(&id));
        assert_eq!(
            fixture.backend.pending_compute_streams[&fixture.launch.stream].front(),
            Some(&id)
        );
        for allocation in &fixture.allocations[1..] {
            assert!(fixture.backend.allocation_custody.contains_key(allocation));
        }
        // Only the deterministic scripted fixture may resume for teardown.
        fixture.backend.terminal = false;
        fixture.backend.cancel_v1(id).unwrap();
        fixture.finish(Some(id));
    }
}

#[test]
fn child_peer_gate_failed_input_waits_for_both_ordering_prefixes_not_other_inputs() {
    for native_failure in [false, true] {
        for observe_only in [false, true] {
            let mut backend = KfdRuntimeBackendV1::mock();
            let stream = backend.create_stream_v1(7).unwrap();
            let foreign = backend.create_stream_v1(7).unwrap();
            let allocation = backend
                .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
                .unwrap();
            for (id, owner, status) in [
                (40, stream, BackendPollV1::Pending),
                (
                    50,
                    foreign,
                    if native_failure {
                        BackendPollV1::Failed { code: -9 }
                    } else {
                        BackendPollV1::Pending
                    },
                ),
                (51, foreign, BackendPollV1::Pending),
            ] {
                backend.submissions.insert(
                    id,
                    SubmissionRecordV1 {
                        stream: owner,
                        status,
                        dependency_depth: 1,
                        profile_dispatch_published: false,
                    },
                );
            }
            let mut pending = pending_compute_for_test_v1(41, stream, allocation, vec![50, 51]);
            pending.ordered_predecessor = Some(40);
            let waiting = PeerComputeGateV1::waiting(OWNER, 41, false);
            pending.peer_gate = Some(if native_failure {
                waiting
            } else {
                waiting
                    .resolve(OWNER, 41, PeerComputeResultV1::Failed, false)
                    .unwrap()
            });
            backend.pending_compute.insert(41, pending);
            backend
                .pending_compute_streams
                .insert(stream, VecDeque::from([41]));
            index_pending_compute_custody_for_test_v1(&mut backend, 41);
            backend.compute_completion_reservations = 1;
            for id in [40, 50, 51] {
                backend.compute_dependency_retain_counts.insert(id, 1);
            }
            backend.stream_submission_tails.insert(stream, 41);
            let progress = |backend: &mut KfdRuntimeBackendV1| {
                let pending = backend.pending_compute.remove(&41).unwrap();
                if observe_only {
                    backend.observe_pending_compute_v1(pending)
                } else {
                    backend.progress_pending_compute_v1(pending)
                }
            };
            assert_eq!(progress(&mut backend).unwrap(), BackendPollV1::Pending);
            assert_eq!(backend.compute_completion_reservations, 1);
            assert_eq!(backend.compute_module_retain_counts[&9], 1);
            assert_eq!(backend.compute_dependency_retain_counts.len(), 3);
            let state = if native_failure {
                PeerComputeResultV1::Pending
            } else {
                PeerComputeResultV1::Failed
            };
            resolve(&mut backend, 41, state, true);
            assert_eq!(progress(&mut backend).unwrap(), BackendPollV1::Pending);
            backend.submissions.get_mut(&40).unwrap().status = BackendPollV1::Failed { code: -7 };
            assert_eq!(
                progress(&mut backend).unwrap(),
                BackendPollV1::Failed { code: -1 }
            );
            assert_eq!(backend.submissions[&51].status, BackendPollV1::Pending);
            assert!(backend.compute_dependency_retain_counts.is_empty());
            assert!(backend.compute_module_retain_counts.is_empty());
            assert!(backend.allocation_custody.is_empty());
            assert_eq!(backend.compute_completion_reservations, 0);
            for id in [40, 41, 50, 51] {
                backend.release_submission_v1(id).unwrap();
            }
            backend.release_allocation_v1(allocation).unwrap();
            backend.destroy_stream_v1(stream).unwrap();
            backend.destroy_stream_v1(foreign).unwrap();
            backend.shutdown_native_v1().unwrap();
        }
    }
}

#[test]
fn child_peer_gate_pending_native_input_still_retires_its_active_ordered_prefix() {
    let mut fixture = ScriptedActiveProducerFixtureV1::new(false);
    let foreign = fixture.backend.create_stream_v1(7).unwrap();
    let unrelated = fixture.backend.next_id().unwrap();
    fixture.backend.submissions.insert(
        unrelated,
        SubmissionRecordV1 {
            stream: foreign,
            status: BackendPollV1::Pending,
            dependency_depth: 1,
            profile_dispatch_published: false,
        },
    );
    let event = fixture.backend.record_event_v1(foreign, unrelated).unwrap();
    let dependencies = [
        BackendLaunchProducerV1 {
            event,
            producer_submission: unrelated,
        },
        BackendLaunchProducerV1 {
            event: fixture.event,
            producer_submission: fixture.producer,
        },
    ];
    let launch = fixture.launch.borrowed();
    let mut collected = fixture
        .backend
        .preflight_compute_v1(launch, ComputeDependencyRosterV1::Exact(&dependencies))
        .unwrap();
    let id = fixture.backend.next_handle;
    collected.peer_gate = Some(PeerComputeGateV1::waiting(OWNER, id, true));
    fixture
        .backend
        .submit_collected_compute_v1(launch, collected)
        .unwrap();
    assert_eq!(fixture.backend.poll_v1(id).unwrap(), BackendPollV1::Pending);
    assert_eq!(
        fixture.backend.pending_compute[&id].explicit_dependency_cursor,
        0
    );
    assert_eq!(
        fixture.backend.submissions[&unrelated].status,
        BackendPollV1::Pending
    );
    assert!(
        fixture
            .backend
            .exact_submission_quiescent_v1(fixture.producer)
    );
    assert!(fixture.backend.active.is_none());
    fixture.backend.cancel_v1(id).unwrap();
    fixture.backend.release_event_v1(event).unwrap();
    fixture.backend.release_submission_v1(unrelated).unwrap();
    fixture.backend.destroy_stream_v1(foreign).unwrap();
    fixture.finish(Some(id));
}

#[test]
fn child_peer_gate_success_keeps_existing_native_blocker_progress() {
    let mut fixture = ScriptedActiveProducerFixtureV1::new(true);
    let module = fixture
        .backend
        .load_module_v1(7, &synthetic_cov6::module())
        .unwrap();
    let kernel = fixture
        .backend
        .resolve_kernel_v1(module, "vecadd", [7; 32])
        .unwrap();
    let mut kernarg = [0_u8; 16];
    kernarg[8..].copy_from_slice(&13_u64.to_le_bytes());
    let bindings = [BackendBindingV1 {
        region: BackendMemoryRegionV1 {
            allocation: fixture.allocations[3],
            access: RuntimeAccessV1::Read,
            byte_offset: 0,
            byte_len: 64,
        },
        kernarg_byte_offset: 0,
    }];
    let launch = BackendLaunchV1 {
        stream: fixture.launch.stream,
        kernel,
        explicit_kernarg: &kernarg,
        bindings: &bindings,
        dependencies: &[],
        geometry: fixture.launch.geometry,
        semantic_launch: KfdRuntimeSemanticLaunchV1::Ordinary,
    };
    let mut collected = fixture
        .backend
        .preflight_compute_v1(launch, ComputeDependencyRosterV1::Exact(&[]))
        .unwrap();
    let id = fixture.backend.next_handle;
    collected.peer_gate = Some(PeerComputeGateV1::waiting(OWNER, id, true));
    fixture
        .backend
        .submit_collected_compute_v1(launch, collected)
        .unwrap();
    assert_eq!(fixture.backend.poll_v1(id).unwrap(), BackendPollV1::Pending);
    assert_eq!(
        fixture.backend.active.as_ref().unwrap().id,
        fixture.producer
    );
    resolve(
        &mut fixture.backend,
        id,
        PeerComputeResultV1::Succeeded,
        true,
    );
    assert_eq!(fixture.backend.poll_v1(id).unwrap(), BackendPollV1::Pending);
    assert!(
        fixture
            .backend
            .exact_submission_quiescent_v1(fixture.producer)
    );
    fixture.backend.cancel_v1(id).unwrap();
    fixture.backend.unload_module_v1(module).unwrap();
    fixture.finish(Some(id));
}

#[test]
fn child_peer_gate_ordered_successor_guard_has_a_downstream_attempt_control() {
    for result in [
        PeerComputeResultV1::Pending,
        PeerComputeResultV1::Failed,
        PeerComputeResultV1::Succeeded,
    ] {
        let mut backend = KfdRuntimeBackendV1::mock();
        let stream = backend.create_stream_v1(7).unwrap();
        let allocation = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 64, 8)
            .unwrap();
        let module = backend
            .load_module_v1(7, &synthetic_cov6::module())
            .unwrap();
        let kernel = backend
            .resolve_kernel_v1(module, "vecadd", [7; 32])
            .unwrap();
        let mut pending = pending_compute_for_test_v1(900, stream, allocation, vec![]);
        pending.module = module;
        let launch = Arc::make_mut(&mut pending.launch);
        launch.kernel = kernel;
        launch.explicit_kernarg = [0_u8; 16].into();
        launch.bindings[0].region.access = RuntimeAccessV1::Read;
        pending.ordered_predecessor = Some(899);
        pending.peer_gate = Some(
            PeerComputeGateV1::waiting(OWNER, 900, true)
                .resolve(OWNER, 900, result, true)
                .unwrap(),
        );
        let prepared = backend
            .prepare_launch(pending.launch.borrowed(), false, true)
            .unwrap();
        let PreparedLaunchStorageV1::Materialized(data) = prepared.storage else {
            panic!("ordinary host-visible guard fixture must be materialized");
        };
        // A physically retired recipe needs no native token. This is a
        // downstream-attempt control, not evidence of native publication.
        let mut predecessor = pipelined_active_for_test_v1(899);
        predecessor.stream = stream;
        predecessor.kernel = kernel;
        predecessor.ordinary_recipe = Some(Arc::clone(&pending.launch));
        predecessor.dispatch_shape_sha256 = prepared.dispatch_shape_sha256;
        predecessor.resident_descriptors = resident_descriptors_v1(&data).unwrap();
        backend
            .compute_pipeline
            .insert_published(predecessor)
            .unwrap();
        let (identity, predecessor) = backend.compute_pipeline.take_physical_owner(899).unwrap();
        backend
            .compute_pipeline
            .restore(
                identity,
                RuntimeComputePipelinePhaseV1::PhysicallyRetired,
                predecessor,
            )
            .unwrap();
        assert!(backend.compute_pipeline.has_successor_capacity());
        assert!(early_pipeline_launch_is_admitted_v1(
            pending.launch.semantic_launch,
            &pending.launch.bindings
        ));
        let observed = backend.try_publish_ordered_successor_v1(&pending, 899);
        if result == PeerComputeResultV1::Succeeded {
            assert!(
                matches!(observed, Err(RuntimeBackendFailureV1::Terminal(error))
                if error.detail().contains("lost its exact physical compute lane"))
            );
            assert!(backend.terminal);
        } else {
            assert!(!observed.unwrap());
            assert!(!backend.terminal);
            assert_eq!(
                backend.compute_pipeline.phase(899),
                Some(RuntimeComputePipelinePhaseV1::PhysicallyRetired)
            );
        }
        assert_eq!(backend.compute_pipeline.len(), 1);
        assert!(!backend.compute_pipeline.contains(pending.id));
        backend.compute_pipeline.take_commit_frontier().unwrap();
        backend.terminal = false;
        backend.unload_module_v1(module).unwrap();
        backend.release_allocation_v1(allocation).unwrap();
        backend.destroy_stream_v1(stream).unwrap();
        backend.shutdown_native_v1().unwrap();
    }
}
