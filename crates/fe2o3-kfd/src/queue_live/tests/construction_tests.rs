use super::*;

#[test]
fn userptr_inner_creation_failure_is_always_terminal() {
    const CHILD_ENV: &str = "FE2O3_TEST_USERPTR_CREATION_GATE_POISON";
    if std::env::var_os(CHILD_ENV).is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("userptr_inner_creation_failure_is_always_terminal")
            .arg("--nocapture")
            .env(CHILD_ENV, "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "child failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }

    let teardown_arm = arm_process_global_kfd_runtime_gate_for_teardown_v1();
    let failure = barrier_probe_creation_failure(
        ComputeAqlQueueSessionErrorV1::Contract("pre-create USERPTR failure"),
        Gfx942BarrierProbeRingBackingV1::UserptrOneX,
    );
    assert!(matches!(
        failure,
        Gfx942BarrierProbeFailureV1::TerminalCreation { .. }
    ));
    assert_eq!(
        failure.backing(),
        Gfx942BarrierProbeRingBackingV1::UserptrOneX
    );
    teardown_arm.confirm_destroyed();

    use std::os::fd::AsFd;
    let file = std::fs::File::open("/dev/null").unwrap();
    assert!(matches!(
        LinuxKfdRuntimeEnabledV1::enable(file.as_fd(), std::process::id()),
        Err(LinuxDoorbellErrorV1::Runtime(
            "process-global gate poisoned"
        ))
    ));
}

#[test]
fn userptr_control_entry_failure_is_terminal_for_every_queue_backing() {
    const CHILD_ENV: &str = "FE2O3_TEST_USERPTR_CONTROL_GATE_POISON";
    if std::env::var_os(CHILD_ENV).is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("userptr_control_entry_failure_is_terminal_for_every_queue_backing")
            .arg("--nocapture")
            .env(CHILD_ENV, "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "child failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }

    let teardown_arm = arm_process_global_kfd_runtime_gate_for_teardown_v1();
    for backing in [
        Gfx942BarrierProbeRingBackingV1::Gfx942ExecutableOneX,
        Gfx942BarrierProbeRingBackingV1::ExecutableGttOneX,
        Gfx942BarrierProbeRingBackingV1::UserptrOneX,
    ] {
        let error = construction_primary::terminal_control_failure_for_test(
            ComputeAqlQueueSessionErrorV1::Contract("control fault injection"),
        );
        assert!(error.is_terminal_creation());
        let failure = barrier_probe_creation_failure(error, backing);
        assert!(matches!(
            failure,
            Gfx942BarrierProbeFailureV1::TerminalCreation { .. }
        ));
        assert_eq!(failure.backing(), backing);
    }
    teardown_arm.confirm_destroyed();

    use std::os::fd::AsFd;
    let file = std::fs::File::open("/dev/null").unwrap();
    assert!(matches!(
        LinuxKfdRuntimeEnabledV1::enable(file.as_fd(), std::process::id()),
        Err(LinuxDoorbellErrorV1::Runtime(
            "process-global gate poisoned"
        ))
    ));
}

#[test]
fn barrier_probe_poll_bound_rejects_before_device_consumption() {
    assert_eq!(
        Gfx942BarrierProbePollBoundV1::new(0),
        Err(Gfx942BarrierProbePollBoundErrorV1::Zero)
    );
    assert_eq!(Gfx942BarrierProbePollBoundV1::new(1).unwrap().get(), 1);
    assert_eq!(
        Gfx942BarrierProbePollBoundV1::new(Gfx942BarrierProbePollBoundV1::maximum())
            .unwrap()
            .get(),
        Gfx942BarrierProbePollBoundV1::maximum()
    );
    assert_eq!(
        Gfx942BarrierProbePollBoundV1::new(Gfx942BarrierProbePollBoundV1::maximum() + 1),
        Err(Gfx942BarrierProbePollBoundErrorV1::ExceedsMaximum {
            requested: Gfx942BarrierProbePollBoundV1::maximum() + 1,
            maximum: Gfx942BarrierProbePollBoundV1::maximum(),
        })
    );
}

#[test]
fn timeout_capture_always_precedes_terminal_poison() {
    #[derive(Default)]
    struct State {
        poisoned: bool,
        observed_before_poison: bool,
    }

    for observation in [Ok(7_u8), Err(11_u8)] {
        let mut state = State::default();
        let result = observe_then_poison(
            &mut state,
            |state| {
                state.observed_before_poison = !state.poisoned;
                observation
            },
            |state| state.poisoned = true,
        );
        assert_eq!(result, observation);
        assert!(state.observed_before_poison);
        assert!(state.poisoned);
    }
}

#[test]
fn timeout_memory_errors_preserve_currentness_classification() {
    for error in [
        MemorySessionError::ProcessChanged,
        MemorySessionError::ProcessVmStatePoisoned,
        MemorySessionError::SharedSessionQuarantined,
    ] {
        assert_eq!(
            map_timeout_memory_observation_error(error),
            Gfx942CompletionErrorV1::Currentness
        );
    }
    assert_eq!(
        map_timeout_memory_observation_error(MemorySessionError::InvalidAllocationAuthority),
        Gfx942CompletionErrorV1::Observation
    );
}

#[test]
fn barrier_success_snapshot_accepts_only_the_exact_redacted_contract() {
    for read in [0, 1] {
        for header in [
            fe2o3_aql::AQL_SYSTEM_SCOPED_BARRIER_AND_HEADER_V1,
            fe2o3_aql::AQL_INVALID_PACKET_HEADER_V1,
        ] {
            let observation = BarrierSnapshotInput {
                header,
                ..BarrierSnapshotInput::valid(read)
            }
            .observation();
            assert_eq!(
                validate_barrier_probe_success_snapshot(observation),
                Ok(observation)
            );
        }
    }

    let valid = BarrierSnapshotInput::valid(1);
    let hostile = [
        BarrierSnapshotInput {
            packet_count: 2,
            ..valid
        }
        .observation(),
        BarrierSnapshotInput { write: 2, ..valid }.observation(),
        BarrierSnapshotInput { read: 2, ..valid }.observation(),
        BarrierSnapshotInput {
            header: 0x1402,
            ..valid
        }
        .observation(),
        BarrierSnapshotInput { setup: 1, ..valid }.observation(),
        BarrierSnapshotInput { kind: 0, ..valid }.observation(),
        BarrierSnapshotInput {
            signal: Gfx942TimeoutSignalObservationV1::Pending,
            ..valid
        }
        .observation(),
        BarrierSnapshotInput { reason: 1, ..valid }.observation(),
    ];
    for observation in hostile {
        assert_eq!(
            validate_barrier_probe_success_snapshot(observation),
            Err(Gfx942CompletionErrorV1::Observation)
        );
    }
}

#[test]
fn detached_returning_destroy_observes_exact_private_generation() {
    let mut poisoned = false;
    let generation = admit_detached_returning_destroy(
        &mut poisoned,
        detached_preflight(false, 3, Some(17), 3, 3, None),
    )
    .unwrap();
    assert_eq!(generation, 17);
    assert!(!poisoned);
}

#[test]
fn detached_returning_destroy_preserves_never_published_generation_zero() {
    let mut poisoned = false;
    let generation = admit_detached_returning_destroy(
        &mut poisoned,
        detached_preflight(false, 0, Some(0), 0, 0, None),
    )
    .unwrap();
    assert_eq!(generation, 0);
    assert!(!poisoned);
}

#[test]
fn detached_returning_destroy_rejects_fabricated_unbound_phase() {
    for (dispatch_attached, generation) in [(true, None), (false, None)] {
        let mut poisoned = false;
        let error = admit_detached_returning_destroy(
            &mut poisoned,
            detached_preflight(dispatch_attached, 0, generation, 0, 0, None),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ComputeAqlQueueSessionErrorV1::DispatchBinding(
                Gfx942DispatchBindingErrorV1::ResourcePhase
            )
        ));
        assert!(poisoned);
    }
}

#[test]
fn detached_returning_destroy_cardinality_mismatch_is_terminal() {
    let mut poisoned = false;
    let error = admit_detached_returning_destroy(
        &mut poisoned,
        detached_preflight(false, 4, Some(29), 4, 3, None),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        ComputeAqlQueueSessionErrorV1::DispatchBinding(Gfx942DispatchBindingErrorV1::InvalidData {
            index: 3,
            detail: "detached returning-destroy cardinality",
        })
    ));
    assert!(poisoned);
    assert!(matches!(
        admit_detached_returning_destroy(
            &mut poisoned,
            detached_preflight(false, 4, Some(29), 4, 4, None),
        ),
        Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
            Gfx942DispatchBindingErrorV1::Poisoned
        ))
    ));
}

#[test]
fn auxiliary_destroy_requires_retired_dispatch_ledger_before_taking_custody() {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct Ledger {
        attached: bool,
        count: usize,
        generation: Option<u64>,
        identities: usize,
        cursor: Option<usize>,
    }

    struct TestLane<'a> {
        ledger: Ledger,
        drops: &'a core::cell::Cell<usize>,
    }

    impl Drop for TestLane<'_> {
        fn drop(&mut self) {
            self.drops.set(self.drops.get() + 1);
        }
    }

    fn preflight(lane: &TestLane<'_>) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        let ledger = lane.ledger;
        admit_auxiliary_destroy_dispatch_ledger_v1(
            ledger.attached,
            ledger.count,
            ledger.generation,
            ledger.identities,
            ledger.cursor,
        )
    }

    let attached = Ledger {
        attached: true,
        count: 0,
        generation: None,
        identities: 0,
        cursor: None,
    };
    let detached = Ledger {
        attached: false,
        generation: Some(1),
        ..attached
    };
    let retired = Ledger {
        cursor: Some(0),
        ..detached
    };
    for ledger in [attached, detached, retired] {
        let drops = core::cell::Cell::new(0);
        let mut state = Some(TestLane {
            ledger,
            drops: &drops,
        });
        let released = take_after_auxiliary_destroy_preflight_v1(&mut state, preflight)
            .expect("attached or fully retired dispatch may be destroyed");
        assert!(state.is_none());
        assert_eq!(released.ledger, ledger);
        assert_eq!(drops.get(), 0);
        drop(released);
        assert_eq!(drops.get(), 1);
    }

    let outstanding = Ledger {
        count: 1,
        identities: 1,
        ..detached
    };
    for ledger in [
        outstanding,
        Ledger {
            identities: 1,
            ..detached
        },
        Ledger {
            count: 1,
            ..detached
        },
        Ledger {
            generation: None,
            ..detached
        },
        Ledger {
            generation: Some(0),
            ..detached
        },
        Ledger {
            cursor: Some(1),
            ..detached
        },
        Ledger {
            generation: Some(1),
            ..attached
        },
        Ledger {
            cursor: Some(0),
            ..attached
        },
        Ledger {
            count: 1,
            identities: 1,
            ..attached
        },
    ] {
        let drops = core::cell::Cell::new(0);
        let mut state = Some(TestLane {
            ledger,
            drops: &drops,
        });
        assert!(take_after_auxiliary_destroy_preflight_v1(&mut state, preflight).is_err());
        assert_eq!(state.as_ref().unwrap().ledger, ledger);
        assert_eq!(drops.get(), 0);
        if ledger == outstanding {
            // Model the final detached-data release, then retry the same custody path.
            state.as_mut().unwrap().ledger = retired;
            let released = take_after_auxiliary_destroy_preflight_v1(&mut state, preflight)
                .expect("retiring the last detached allocation permits destruction");
            assert!(state.is_none());
            assert_eq!(drops.get(), 0);
            drop(released);
        } else {
            drop(state);
        }
        assert_eq!(drops.get(), 1);
    }
}

#[test]
fn detached_storage_identity_rejects_cross_queue_substitution_and_reordering() {
    let first_queue = [(1_u64, 11_u64), (1, 12), (1, 13)];
    let second_queue = [(2_u64, 11_u64), (2, 12), (2, 13)];
    assert_eq!(
        first_ordered_identity_mismatch(&first_queue, &second_queue),
        Some(0)
    );

    let reordered = [first_queue[1], first_queue[0], first_queue[2]];
    assert_eq!(
        first_ordered_identity_mismatch(&first_queue, &reordered),
        Some(0)
    );

    let substituted = [first_queue[0], second_queue[1], first_queue[2]];
    assert_eq!(
        first_ordered_identity_mismatch(&first_queue, &substituted),
        Some(1)
    );

    let mut poisoned = false;
    let error = admit_detached_returning_destroy(
        &mut poisoned,
        detached_preflight(false, 3, Some(11), 3, 3, Some(1)),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        ComputeAqlQueueSessionErrorV1::DispatchBinding(Gfx942DispatchBindingErrorV1::InvalidData {
            index: 1,
            detail: "detached returning-destroy storage identity",
        })
    ));
    assert!(poisoned);
}

#[test]
fn detached_storage_identity_replacement_preserves_the_released_ordinal() {
    let mut identities = vec![11_u64, 12, 13];
    let removed_index = 1;
    assert_eq!(identities.remove(removed_index), 12);
    let mut next_insertion_index = Some(removed_index);

    insert_detached_identity_at(
        &mut identities,
        &mut next_insertion_index,
        22,
        removed_index,
    );

    assert_eq!(identities, [11, 22, 13]);
    assert_eq!(next_insertion_index, None);
    assert_eq!(
        first_ordered_identity_mismatch(&identities, &[11, 22, 13]),
        None
    );
}

#[test]
fn borrowed_initialization_queue_rejects_before_native_model_loan() {
    for replacement in [false, true] {
        for mode in 0..7 {
            let mut session =
                persistent_compute_cancellation_test_session(test_queue_key(771, 1), None, None);
            session.detached_dispatch_generation = Some(7);
            session.detached_next_insertion_index = Some(0);
            match mode {
                0 => session.terminal_poisoned = true,
                1 => session.detached_dispatch_generation = None,
                2 => session.detached_data_count = 1,
                3 => {
                    session.detached_data_count =
                        super::super::dispatch_binding::MAX_DISPATCH_DATA_LEASES_V1 + 1
                }
                4 => {
                    session.detached_data_count =
                        super::super::dispatch_binding::MAX_DISPATCH_DATA_LEASES_V1;
                    let identity = Gfx942FixedDispatchDataV1::host_visible_uninitialized(
                        crate::shared_memory::mapped_host_for_persistent_sdma_test(1, 4096),
                    )
                    .storage_identity();
                    session.detached_data_identities = vec![identity; session.detached_data_count];
                }
                5 => session.detached_next_insertion_index = Some(1),
                _ => session.detached_next_insertion_index = None,
            }
            let before = (
                session.detached_data_count,
                session.detached_dispatch_generation,
                session.detached_next_insertion_index,
                session.detached_data_identities.clone(),
            );
            let source = [0x5a; 9];
            let error = if replacement {
                session.initialize_host_visible_fixed_dispatch_data_from_slice_v1(&source)
            } else {
                session
                    .insert_initialized_host_visible_fixed_dispatch_data_from_slice_v1(1, &source)
            }
            .unwrap_err();
            match mode {
                0 => assert!(matches!(
                    error,
                    ComputeAqlQueueSessionErrorV1::DispatchBinding(
                        Gfx942DispatchBindingErrorV1::Poisoned
                    )
                )),
                1 => assert!(matches!(
                    error,
                    ComputeAqlQueueSessionErrorV1::DispatchBinding(
                        Gfx942DispatchBindingErrorV1::ResourcePhase
                    )
                )),
                2 | 3 | 5 => {
                    assert!(matches!(error, ComputeAqlQueueSessionErrorV1::Contract(_)))
                }
                4 => assert!(matches!(
                    error,
                    ComputeAqlQueueSessionErrorV1::DispatchBinding(
                        Gfx942DispatchBindingErrorV1::DataLeaseCount { .. }
                    )
                )),
                _ if replacement => assert!(matches!(
                    error,
                    ComputeAqlQueueSessionErrorV1::DispatchBinding(
                        Gfx942DispatchBindingErrorV1::ResourcePhase
                    )
                )),
                _ => assert!(matches!(
                    error,
                    ComputeAqlQueueSessionErrorV1::DispatchBinding(
                        Gfx942DispatchBindingErrorV1::InvalidData { .. }
                    )
                )),
            }
            assert_eq!(
                before,
                (
                    session.detached_data_count,
                    session.detached_dispatch_generation,
                    session.detached_next_insertion_index,
                    session.detached_data_identities.clone()
                )
            );
            assert!(session.engine.is_none());
            assert_eq!(source, [0x5a; 9]);
        }
    }
}

#[test]
fn borrowed_initialization_queue_wiring_keeps_guards_retake_and_identity_order() {
    let source = crate::queue::fixed_dispatch_production_source_for_tests_v1();
    for (name, next, selection) in [
        (
            "insert_initialized_host_visible_fixed_dispatch_data_from_slice_v1",
            "initialize_host_visible_fixed_dispatch_data",
            "Some(data_index), bytes",
        ),
        (
            "initialize_host_visible_fixed_dispatch_data_from_slice_v1",
            "insert_host_visible_fixed_dispatch_data",
            "None, bytes",
        ),
    ] {
        let start = format!("pub fn {name}(");
        let end = format!("pub fn {next}(");
        let body = source
            .split(&start)
            .nth(1)
            .unwrap()
            .split(&end)
            .next()
            .unwrap();
        assert!(body.contains(&format!("initialize_coherent_data_settled_v1({selection})")));
        assert!(
            body.find("retain_terminal_rebind_parent_v1").unwrap()
                < body.find("settled.into_result()").unwrap()
        );
        for forbidden in ["submit_", "to_vec(", "to_owned(", "Box::from("] {
            assert!(!body.contains(forbidden));
        }
    }
    let insertion = include_str!("../../queue_live/data_insertion.rs");
    let helper = insertion
        .split("pub(super) fn initialize_coherent_data_settled_v1(")
        .nth(1)
        .unwrap();
    assert!(helper.contains("CoherentInitializationCustodyV1::new()"));
    assert!(helper.contains("DataInsertionIndexV1::RequiredHole"));
    assert!(helper.contains("DataInsertionIndexV1::Explicit"));
    let sequence = insertion
        .split("pub(in crate::queue) fn settle_data_insertion_v1")
        .nth(1)
        .unwrap()
        .split("impl<R:")
        .next()
        .unwrap();
    let mut previous = 0;
    for marker in [
        "context.require_unbound()?",
        "MAX_DISPATCH_DATA_LEASES_V1",
        "DataInsertionIndexV1::RequiredHole",
        "validate_new_detached_data_index",
        "context.reserve()?",
        "context.prepare(&mut root, parameters)?",
        "root.completed_identity()?",
        "insert_detached_identity_at",
        "*ledger.count = next_count",
        ".take_data()",
    ] {
        let at = sequence.find(marker).unwrap();
        assert!(
            at >= previous,
            "{marker} preserves borrowed initialization order"
        );
        previous = at;
    }
    assert!(insertion.contains("root.prepare(memory, parameters)"));
    assert!(insertion.contains("memory.prepare_coherent_initialization_in_place(self, source)"));
    let adapter = insertion
        .split("impl<R: DataInsertionRootV1<P>, P>")
        .nth(1)
        .unwrap()
        .split("impl ComputeAqlQueueSessionV1 {")
        .next()
        .unwrap();
    assert!(adapter.contains("self.poison_terminal()"));
    let shared = include_str!("../../shared_memory.rs");
    let forwarder = shared
        .split("pub(crate) fn prepare_coherent_initialization_in_place(")
        .nth(1)
        .unwrap()
        .split("\n    }")
        .next()
        .unwrap();
    assert!(forwarder.contains("root.prepare_with_memory(self, source)"));
    let root = shared
        .split("impl CoherentInitializationCustodyV1 {")
        .nth(1)
        .unwrap()
        .split("pub(crate) struct DeviceInitializationCustodyV1")
        .next()
        .unwrap();
    assert!(root.contains(
        "self.completed = Some(coherent_initialization::initialize_v1(memory, source)?)"
    ));
    for forbidden in ["submit_", "to_vec(", "to_owned(", "Box::from("] {
        assert!(!insertion.contains(forbidden));
        assert!(!forwarder.contains(forbidden));
        assert!(!root.contains(forbidden));
    }
}

#[test]
fn exact_detached_insertion_clears_legacy_replacement_state_at_any_valid_ordinal() {
    let mut same_ordinal = vec![11_u64, 13];
    let mut pending = Some(1);
    validate_new_detached_data_index(same_ordinal.len(), 1).unwrap();
    insert_detached_identity_at(&mut same_ordinal, &mut pending, 12, 1);
    assert_eq!(same_ordinal, [11, 12, 13]);
    assert_eq!(pending, None);

    let mut different_ordinal = vec![21_u64, 23];
    let mut pending = Some(1);
    validate_new_detached_data_index(different_ordinal.len(), 0).unwrap();
    insert_detached_identity_at(&mut different_ordinal, &mut pending, 20, 0);
    assert_eq!(different_ordinal, [20, 21, 23]);
    assert_eq!(pending, None);
}

#[test]
fn exact_detached_insertion_rejects_out_of_range_without_mutation() {
    let identities = vec![31_u64, 32];
    let pending = Some(0);
    assert!(matches!(
        validate_new_detached_data_index(identities.len(), 3),
        Err(Gfx942DispatchBindingErrorV1::InvalidData {
            index: 2,
            detail: "detached insertion ordinal",
        })
    ));
    assert_eq!(identities, [31, 32]);
    assert_eq!(pending, Some(0));
}

#[test]
fn fused_async_single_prepublication_failure_script_is_fail_closed() {
    for loan_succeeded in [false, true] {
        for owner_healthy in [false, true] {
            for closing_currentness_succeeded in [false, true] {
                assert_eq!(
                    fused_async_single_prepublication_is_retryable_v1(
                        loan_succeeded,
                        owner_healthy,
                        closing_currentness_succeeded,
                    ),
                    loan_succeeded && owner_healthy && closing_currentness_succeeded,
                    "loan={loan_succeeded} owner={owner_healthy} close={closing_currentness_succeeded}",
                );
            }
        }
    }
}

#[test]
fn persistent_sdma_active_spin_floor_is_bound_to_the_frozen_manifest() {
    assert_eq!(
        PERSISTENT_SDMA_ACTIVE_SPIN_FLOOR_V1,
        Duration::from_nanos(50_000)
    );
    assert!(crate::sdma::GFX942_SDMA_COPY_MANIFEST_V1.lines().any(|line| {
        line == "persistent-sdma-wait-policy=elapsed-active-spin-floor:50000ns,checked-add-and-clamp-to-deadline,attempts-counted-during-floor,exact-floor-boundary-resumes-default-adaptive-stage,first-observation-unconditional;scope=directional-persistent-single,directional-persistent-window,same-device-persistent-window;excluded=ordinary-directional,generic-striped,fused-synchronous,xgmi,persistent-compute"
    }));
}

#[test]
fn session_manifest_digest_is_frozen() {
    assert_eq!(
        GFX942_COMPUTE_AQL_SHARED_ALLOCATION_RECORDS_V1,
        1 + 1 + 1 + 1 + 1
    );
    assert_eq!(
        fe2o3_aql::AQL_DISPATCH_ABI_SCHEMA_MANIFEST_SHA256_V1,
        "82fbd7cf0b6c8647dce3f9b11e4f13a2dadfe3423509f769a4bc6cc87bb7acd0"
    );
    assert_eq!(
        fe2o3_aql::AQL_BARRIER_AND_ABI_SCHEMA_MANIFEST_SHA256_V1,
        "bdca900cd5c6eaccbddfc5a854e956382a08ce87bec4ccd5284baacf932cdfb5"
    );
    assert_eq!(
        fe2o3_aql::AQL_FIXED_BATCH_MODEL_MANIFEST_SHA256_V2,
        "a3c74fe4aa26a62772253de267812f2fb1626247685d8c4e8ed8bbb2a5a9e34a"
    );
    assert_eq!(
        super::super::completion::GFX942_AQL_COMPLETION_MANIFEST_SHA256_V1,
        "485b21257623afce41573b27922350f125bcd7f5d339f8a89cf9a2c7c6ca77f1"
    );
    assert!(GFX942_COMPUTE_AQL_SESSION_MANIFEST_V1.contains(&format!(
        "compute_dependency_publisher_schema_sha256={}\n",
        super::super::dependency::GFX942_COMPUTE_DEPENDENCY_PUBLISHER_FOUNDATION_MANIFEST_SHA256_V1
    )));
    assert_eq!(
        super::super::dispatch_binding::GFX942_AQL_DISPATCH_BINDING_MANIFEST_SHA256_V1,
        "d4265552e99fcfefcfdcb094b0927647edd0f50a948a8a970d91e3636ae7b694"
    );
    assert!(GFX942_COMPUTE_AQL_SESSION_MANIFEST_V1.contains(&format!(
        "dispatch_binding_schema_sha256={}\n",
        super::super::dispatch_binding::GFX942_AQL_DISPATCH_BINDING_MANIFEST_SHA256_V1
    )));
    assert_eq!(
        SHARED_GTT_MEMORY_PROFILE_SHA256_V1,
        "026c8c05b6388149765ccb84a95739de6a6ddbbe89577217284b18bdcfdcbdd3"
    );
    assert_eq!(
        GFX942_QUEUE_RESOURCE_PROFILE_SHA256_V1,
        "37d45132916d2ecefdec8f53ecab817cbdbaa9b9863440353163bd460626ab02"
    );
    assert_eq!(
        fe2o3_kfd_uapi::KFD_USERPTR_MEMORY_SCHEMA_MANIFEST_SHA256,
        "c1cee09bdf884d2c14a5dbb89c1f6f7885962c75b1457caf412821490919ee9e"
    );
    assert_eq!(
        fe2o3_kfd_uapi::KFD_USERPTR_QUEUE_CONTROL_SCHEMA_MANIFEST_SHA256,
        "f1d75410d6bfacff2ea15ecfff226eb8aed7912ee324a36b8ed8550fa52bce02"
    );
    assert_eq!(
        fe2o3_kfd_uapi::KFD_RUNTIME_ENABLE_SCHEMA_SHA256,
        "fa47481b10ea4bd89438d10b82bd8197088906e55f5f0c827dc7aa5aba906288"
    );
    let digest = Sha256::digest(GFX942_COMPUTE_AQL_SESSION_MANIFEST_V1);
    let rendered: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    assert_eq!(rendered, GFX942_COMPUTE_AQL_SESSION_MANIFEST_SHA256_V1);
}

#[test]
fn every_post_handoff_failure_keeps_original_debug_token_empty() {
    for injected in [
        InjectedPostHandoffFailureV1::EventCreation,
        InjectedPostHandoffFailureV1::ShadowInstallation,
        InjectedPostHandoffFailureV1::ShadowInitialization,
        InjectedPostHandoffFailureV1::ResourceSealing,
        InjectedPostHandoffFailureV1::ResourceMapping,
        InjectedPostHandoffFailureV1::ModelTransfer,
        InjectedPostHandoffFailureV1::EngineAdmission,
        InjectedPostHandoffFailureV1::SubmissionModel,
        InjectedPostHandoffFailureV1::QueueCreateNoEffect,
        InjectedPostHandoffFailureV1::QueueCreateIndeterminate,
        InjectedPostHandoffFailureV1::RuntimeQueueTransition,
        InjectedPostHandoffFailureV1::CreateOutputs,
        InjectedPostHandoffFailureV1::NativeQueueId,
        InjectedPostHandoffFailureV1::SessionComposition,
        InjectedPostHandoffFailureV1::PreDoorbellCurrentness,
        InjectedPostHandoffFailureV1::DoorbellMapping,
        InjectedPostHandoffFailureV1::PostDoorbellCurrentness,
    ] {
        let mut token_runtime = Some("runtime");
        let mut token_control = Some("control");
        let terminal_runtime = token_runtime.take();
        let terminal_control = token_control.take();

        assert_eq!(terminal_runtime, Some("runtime"), "{injected:?}");
        assert_eq!(terminal_control, Some("control"), "{injected:?}");
        assert!(token_runtime.is_none(), "{injected:?}");
        assert!(token_control.is_none(), "{injected:?}");
    }
}

#[test]
fn production_handoff_precedes_event_create_and_all_fallible_queue_steps() {
    let source = include_str!("../../queue_live/construction_primary.rs");
    let runtime = source
        .split("fn prepare_runtime_and_shadows(")
        .nth(1)
        .unwrap()
        .split("fn map_and_retain_resources(")
        .next()
        .unwrap();
    let enable = runtime.find("E::enable_runtime").unwrap();
    let handoff = runtime
        .find("runtime.take().expect(\"validated debug runtime authority\")")
        .unwrap();
    let arm = runtime.find("E::arm_creation(memory)").unwrap();
    let event = runtime.find("E::create_event(memory)").unwrap();
    let shadows = runtime.find("E::install_shadows").unwrap();
    assert!(enable < handoff && handoff < arm && arm < event && event < shadows);
    assert!(!runtime.contains("publish_for_native_queue_creation"));

    let construct = source
        .split("pub(super) fn construct(")
        .nth(1)
        .unwrap()
        .split("fn prepare_runtime_and_shadows(")
        .next()
        .unwrap();
    let control_entry = construct
        .find("entry.enter(\"USERPTR queue-control creation\")")
        .unwrap();
    let control = construct
        .find("memory.allocate_userptr_aql_control()")
        .unwrap();
    let runtime = construct.find("self.prepare_runtime_and_shadows(").unwrap();
    let mapping = construct.find("self.map_and_retain_resources(").unwrap();
    let create = construct.find("self.create_and_assemble(").unwrap();
    let doorbell = construct.find("self.finish_doorbell()?").unwrap();
    let finish = construct.find("E::finish_creation(").unwrap();
    assert!(
        control_entry < control
            && control < runtime
            && runtime < mapping
            && mapping < create
            && create < doorbell
            && doorbell < finish
    );

    let create = source
        .split("fn create_and_assemble(")
        .nth(1)
        .unwrap()
        .split("fn finish_doorbell(")
        .next()
        .unwrap();
    let native = create.find(".create_at_native_boundary(key").unwrap();
    let publish = create.find("E::publish_shadows(").unwrap();
    let dependency = create.find("E::create_dependency_owner(key)").unwrap();
    let assembled = create
        .find("self.completed = Some(CompletedPrimaryV1")
        .unwrap();
    assert!(native < publish && publish < dependency && dependency < assembled);
    assert_eq!(create.matches("E::publish_shadows").count(), 1);
    let commit = &create[assembled..];
    assert!(!commit.contains('?'));

    let doorbell = source
        .split("fn finish_doorbell(")
        .nth(1)
        .unwrap()
        .split("fn run_rooted_construction_v1")
        .next()
        .unwrap();
    assert!(doorbell.contains("self.completed.as_mut()"));
    assert_eq!(doorbell.matches(".prepare_operation()").count(), 2);
    let map = doorbell.find("E::map_doorbell").unwrap();
    let observation = doorbell.find("E::doorbell_observation").unwrap();
    assert!(doorbell.find("session.doorbell = Some(").unwrap() < map && map < observation);
    let platform = include_str!("../../queue_live/construction_primary/environment.rs");
    for primitive in [
        "LinuxKfdRuntimeEnabledV1::enable",
        "LinuxQueueExceptionEventV1::create",
        "LinuxCwsrShadowPagesV1::install",
        "LinuxDoorbellSliceV1::map",
        "arm.finish_checked(pid)",
        "ComputeDependencySessionOwnerV1::new(key.id.0)",
    ] {
        assert!(platform.contains(primitive));
    }
    let conversion = platform.split("fn into_session(self)").nth(1).unwrap();
    for forbidden in [
        "?",
        "check_currentness",
        "prepare_operation",
        "::map(",
        "::allocate",
        "::enable(",
    ] {
        assert!(
            !conversion.contains(forbidden),
            "fallible post-gate conversion: {forbidden}"
        );
    }
}

#[test]
fn auxiliary_queue_publishes_cwsr_payload_only_at_native_create_boundary() {
    let source = include_str!("../../queue_live/construction_auxiliary.rs");
    let prepared_state = source
        .split("struct AuxiliaryConstructionV1")
        .nth(1)
        .unwrap()
        .split("struct AuxiliaryConstructionScopeV1")
        .next()
        .unwrap();
    assert!(prepared_state.contains("unpublished: Option<E::Unpublished>"));
    assert!(!prepared_state.contains("exception: QueueExceptionStateV1"));

    let body = source
        .split("fn prepare(")
        .nth(1)
        .unwrap()
        .split("pub(super) fn construct_auxiliary_compute_lane_v1")
        .next()
        .unwrap();
    let install = body
        .find("self.unpublished = Some(E::install_shadows")
        .unwrap();
    let restore = body.find("E::restore_shadow_write").unwrap();
    let native_boundary = body.find("create_at_native_boundary(key").unwrap();
    let publish = body
        .find("self.published = Some(E::publish_shadows(")
        .unwrap();
    let published_exception = body.find("exception: Some(QueueExceptionStateV1").unwrap();
    assert!(install < restore);
    assert!(restore < native_boundary);
    assert!(native_boundary < publish);
    assert!(publish < published_exception);
    assert_eq!(body.matches("E::publish_shadows(").count(), 1);
    assert!(!body.contains("engine.create(key)"));
}
