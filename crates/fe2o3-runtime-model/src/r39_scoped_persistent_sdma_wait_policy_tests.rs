use super::r37_typed_native_sdma_wait_activation::*;
use super::r39_scoped_persistent_sdma_wait_policy::*;

fn binding(kind: R37CopyKindV1, profile: u64) -> R37WaitBindingV1 {
    let base = profile * 100;
    R37WaitBindingV1 {
        kind,
        submission: 13 + base,
        stream: 17 + base,
        source_allocation: 19 + base,
        destination_allocation: 23 + base,
        source_storage_generation: 29 + base,
        destination_storage_generation: 31 + base,
        restored_source_storage: 37 + base,
        restored_destination_storage: 41 + base,
        dependency_submission: 43 + base,
        dependency_retain_count: 2,
        source_custody_count: 3,
        destination_custody_count: 4,
        stream_owner_count: 5,
        published_index_frame: R37OrderedFrameV1 {
            predecessor: 11 + base,
            current: 13 + base,
            successor: 47 + base,
        },
        stream_frame: R37OrderedFrameV1 {
            predecessor: 7 + base,
            current: 13 + base,
            successor: 53 + base,
        },
        native_identity: R37NativeIdentityV1 {
            owner_id: 59 + base,
            request_id: 61 + base,
        },
    }
}

fn initial(kind: R37CopyKindV1, profile: u64) -> R37WaitSnapshotV1 {
    R37TypedNativeSdmaWaitModelV1::new_model_only(binding(kind, profile))
        .unwrap()
        .initial_snapshot_model_only()
}

fn observed(
    kind: R37CopyKindV1,
    profile: u64,
    observation: R37NativeWaitObservationV1,
    completion: R37CompletionDispositionV1,
) -> R37WaitSnapshotV1 {
    R37TypedNativeSdmaWaitModelV1::new_model_only(binding(kind, profile))
        .unwrap()
        .run_model_only(R37DeadlineClassV1::Positive, observation, completion)
        .unwrap()
}

fn snapshots() -> [R37WaitSnapshotV1; 13] {
    let directional = binding(R37CopyKindV1::Directional, 0);
    let same_device = binding(R37CopyKindV1::SameDevice, 1);
    [
        initial(R37CopyKindV1::Directional, 0),
        initial(R37CopyKindV1::SameDevice, 1),
        observed(
            R37CopyKindV1::Directional,
            0,
            R37NativeWaitObservationV1::Complete,
            R37CompletionDispositionV1::Settle,
        ),
        observed(
            R37CopyKindV1::Directional,
            0,
            R37NativeWaitObservationV1::Complete,
            R37CompletionDispositionV1::ContinuationReady,
        ),
        observed(
            R37CopyKindV1::Directional,
            0,
            R37NativeWaitObservationV1::ExactTypedTimeout(directional.native_identity),
            R37CompletionDispositionV1::Settle,
        ),
        observed(
            R37CopyKindV1::Directional,
            0,
            R37NativeWaitObservationV1::NonTimeoutRetryable(directional.native_identity),
            R37CompletionDispositionV1::Settle,
        ),
        observed(
            R37CopyKindV1::Directional,
            0,
            R37NativeWaitObservationV1::IdentityChange {
                stage: R37IdentityChangeStageV1::Pending,
                returned: R37NativeIdentityV1 {
                    owner_id: 71,
                    request_id: 73,
                },
            },
            R37CompletionDispositionV1::Settle,
        ),
        observed(
            R37CopyKindV1::Directional,
            0,
            R37NativeWaitObservationV1::IdentityChange {
                stage: R37IdentityChangeStageV1::Completed,
                returned: R37NativeIdentityV1 {
                    owner_id: 79,
                    request_id: 83,
                },
            },
            R37CompletionDispositionV1::Settle,
        ),
        observed(
            R37CopyKindV1::Directional,
            0,
            R37NativeWaitObservationV1::Teardown { terminal_token: 89 },
            R37CompletionDispositionV1::Settle,
        ),
        observed(
            R37CopyKindV1::SameDevice,
            1,
            R37NativeWaitObservationV1::Complete,
            R37CompletionDispositionV1::Settle,
        ),
        observed(
            R37CopyKindV1::SameDevice,
            1,
            R37NativeWaitObservationV1::Complete,
            R37CompletionDispositionV1::ContinuationReady,
        ),
        observed(
            R37CopyKindV1::SameDevice,
            1,
            R37NativeWaitObservationV1::ExactTypedTimeout(same_device.native_identity),
            R37CompletionDispositionV1::Settle,
        ),
        observed(
            R37CopyKindV1::SameDevice,
            1,
            R37NativeWaitObservationV1::Teardown { terminal_token: 97 },
            R37CompletionDispositionV1::Settle,
        ),
    ]
}

#[derive(Clone, Copy)]
struct ScenarioV1 {
    site: R39WaitSiteV1,
    started_ns: u64,
    deadline_ns: u64,
    deadline_check_ns: u64,
    action_now_ns: u64,
    attempts: u32,
    next_sleep_ns: u64,
    observation: R39CompletionObservationV1,
    expected_decision: R39WaitDecisionV1,
    expected_attempts: u32,
    expected_next_sleep_ns: u64,
}

#[test]
fn exhaustive_156_case_policy_domain_preserves_every_r37_snapshot_coordinate() {
    let scenarios = [
        ScenarioV1 {
            site: R39WaitSiteV1::DirectionalPersistentSingle,
            started_ns: 0,
            deadline_ns: 0,
            deadline_check_ns: 0,
            action_now_ns: 0,
            attempts: 0,
            next_sleep_ns: 25_000,
            observation: R39CompletionObservationV1::Ready,
            expected_decision: R39WaitDecisionV1::Ready,
            expected_attempts: 0,
            expected_next_sleep_ns: 25_000,
        },
        ScenarioV1 {
            site: R39WaitSiteV1::DirectionalPersistentWindow,
            started_ns: 2,
            deadline_ns: 1,
            deadline_check_ns: 2,
            action_now_ns: 2,
            attempts: 0,
            next_sleep_ns: 25_000,
            observation: R39CompletionObservationV1::Pending,
            expected_decision: R39WaitDecisionV1::TimedOut,
            expected_attempts: 0,
            expected_next_sleep_ns: 25_000,
        },
        ScenarioV1 {
            site: R39WaitSiteV1::SameDevicePersistentWindow,
            started_ns: 0,
            deadline_ns: 100_000,
            deadline_check_ns: 49_999,
            action_now_ns: 49_999,
            attempts: 64,
            next_sleep_ns: 25_000,
            observation: R39CompletionObservationV1::Pending,
            expected_decision: R39WaitDecisionV1::Pause(R39WaitActionV1::Spin),
            expected_attempts: 65,
            expected_next_sleep_ns: 25_000,
        },
        ScenarioV1 {
            site: R39WaitSiteV1::DirectionalPersistentSingle,
            started_ns: 0,
            deadline_ns: 100_000,
            deadline_check_ns: 50_000,
            action_now_ns: 50_000,
            attempts: 64,
            next_sleep_ns: 25_000,
            observation: R39CompletionObservationV1::Pending,
            expected_decision: R39WaitDecisionV1::Pause(R39WaitActionV1::Yield),
            expected_attempts: 65,
            expected_next_sleep_ns: 25_000,
        },
        ScenarioV1 {
            site: R39WaitSiteV1::GenericPersistentSingle,
            started_ns: 0,
            deadline_ns: 100_000,
            deadline_check_ns: 1,
            action_now_ns: 1,
            attempts: 0,
            next_sleep_ns: 25_000,
            observation: R39CompletionObservationV1::Pending,
            expected_decision: R39WaitDecisionV1::Pause(R39WaitActionV1::Spin),
            expected_attempts: 1,
            expected_next_sleep_ns: 25_000,
        },
        ScenarioV1 {
            site: R39WaitSiteV1::OrdinarySingle,
            started_ns: 0,
            deadline_ns: 100_000,
            deadline_check_ns: 1,
            action_now_ns: 1,
            attempts: 63,
            next_sleep_ns: 25_000,
            observation: R39CompletionObservationV1::Pending,
            expected_decision: R39WaitDecisionV1::Pause(R39WaitActionV1::Spin),
            expected_attempts: 64,
            expected_next_sleep_ns: 25_000,
        },
        ScenarioV1 {
            site: R39WaitSiteV1::OrdinaryBatchStriped,
            started_ns: 0,
            deadline_ns: 100_000,
            deadline_check_ns: 1,
            action_now_ns: 1,
            attempts: 64,
            next_sleep_ns: 25_000,
            observation: R39CompletionObservationV1::Pending,
            expected_decision: R39WaitDecisionV1::Pause(R39WaitActionV1::Yield),
            expected_attempts: 65,
            expected_next_sleep_ns: 25_000,
        },
        ScenarioV1 {
            site: R39WaitSiteV1::FusedSynchronousDirectional,
            started_ns: 0,
            deadline_ns: 100_000,
            deadline_check_ns: 1,
            action_now_ns: 1,
            attempts: 79,
            next_sleep_ns: 25_000,
            observation: R39CompletionObservationV1::Pending,
            expected_decision: R39WaitDecisionV1::Pause(R39WaitActionV1::Yield),
            expected_attempts: 80,
            expected_next_sleep_ns: 25_000,
        },
        ScenarioV1 {
            site: R39WaitSiteV1::DirectionalPersistentSingle,
            started_ns: 0,
            deadline_ns: 100_000,
            deadline_check_ns: 99_999,
            action_now_ns: 100_000,
            attempts: 80,
            next_sleep_ns: 25_000,
            observation: R39CompletionObservationV1::Pending,
            expected_decision: R39WaitDecisionV1::Pause(R39WaitActionV1::Sleep { nanoseconds: 0 }),
            expected_attempts: 81,
            expected_next_sleep_ns: 50_000,
        },
        ScenarioV1 {
            site: R39WaitSiteV1::XgmiBatch,
            started_ns: 0,
            deadline_ns: 100_000,
            deadline_check_ns: 90_000,
            action_now_ns: 90_000,
            attempts: 100,
            next_sleep_ns: 1_000_000,
            observation: R39CompletionObservationV1::Pending,
            expected_decision: R39WaitDecisionV1::Pause(R39WaitActionV1::Sleep {
                nanoseconds: 10_000,
            }),
            expected_attempts: 101,
            expected_next_sleep_ns: 1_000_000,
        },
        ScenarioV1 {
            site: R39WaitSiteV1::PersistentCompute,
            started_ns: 0,
            deadline_ns: 100_000,
            deadline_check_ns: 1,
            action_now_ns: 1,
            attempts: u32::MAX,
            next_sleep_ns: 25_000,
            observation: R39CompletionObservationV1::Pending,
            expected_decision: R39WaitDecisionV1::Pause(R39WaitActionV1::Sleep {
                nanoseconds: 25_000,
            }),
            expected_attempts: u32::MAX,
            expected_next_sleep_ns: 50_000,
        },
        ScenarioV1 {
            site: R39WaitSiteV1::DirectionalPersistentWindow,
            started_ns: u64::MAX - 25_000,
            deadline_ns: u64::MAX,
            deadline_check_ns: u64::MAX - 25_000,
            action_now_ns: u64::MAX - 25_000,
            attempts: 64,
            next_sleep_ns: 25_000,
            observation: R39CompletionObservationV1::Pending,
            expected_decision: R39WaitDecisionV1::Pause(R39WaitActionV1::Spin),
            expected_attempts: 65,
            expected_next_sleep_ns: 25_000,
        },
    ];

    let mut checked = 0;
    for snapshot in snapshots() {
        for scenario in scenarios {
            let step = R39ScopedPersistentSdmaWaitPolicyModelV1::new_with_cursor_model_only(
                snapshot,
                scenario.site,
                scenario.started_ns,
                scenario.deadline_ns,
                scenario.attempts,
                scenario.next_sleep_ns,
            )
            .unwrap()
            .observe_model_only(
                scenario.deadline_check_ns,
                scenario.action_now_ns,
                scenario.observation,
            )
            .unwrap();
            checked += 1;
            assert_eq!(step.observation_count, 1);
            assert_eq!(step.decision, scenario.expected_decision);
            assert_eq!(step.attempts, scenario.expected_attempts);
            assert_eq!(step.next_sleep_ns, scenario.expected_next_sleep_ns);
            assert!(step.retains_full_r37_snapshot(&snapshot));
        }
    }
    assert_eq!(checked, 156);
}

#[test]
fn profile_allowlist_and_seven_exclusions_are_exact() {
    let scoped = [
        R39WaitSiteV1::DirectionalPersistentSingle,
        R39WaitSiteV1::DirectionalPersistentWindow,
        R39WaitSiteV1::SameDevicePersistentWindow,
    ];
    for site in scoped {
        assert_eq!(
            r39_wait_profile_model_only(site),
            R39WaitProfileV1::ScopedPersistentSdma {
                active_spin_floor_ns: 50_000,
            }
        );
    }

    let excluded = [
        R39WaitSiteV1::GenericPersistentSingle,
        R39WaitSiteV1::OrdinarySingle,
        R39WaitSiteV1::OrdinaryBatchStriped,
        R39WaitSiteV1::FusedSynchronousDirectional,
        R39WaitSiteV1::XgmiSingle,
        R39WaitSiteV1::XgmiBatch,
        R39WaitSiteV1::PersistentCompute,
    ];
    for site in excluded {
        assert_eq!(r39_wait_profile_model_only(site), R39WaitProfileV1::Default);
    }
}

#[test]
fn floor_addition_is_checked_clamped_and_strict_at_its_boundary() {
    let site = R39WaitSiteV1::DirectionalPersistentSingle;
    assert_eq!(
        r39_active_spin_until_model_only(site, 10, 100_000),
        Some(50_010)
    );
    assert_eq!(
        r39_active_spin_until_model_only(site, 10, 20_000),
        Some(20_000)
    );
    assert_eq!(
        r39_active_spin_until_model_only(site, u64::MAX - 1, u64::MAX),
        Some(u64::MAX)
    );

    let snapshot = initial(R37CopyKindV1::Directional, 0);
    let before = R39ScopedPersistentSdmaWaitPolicyModelV1::new_with_cursor_model_only(
        snapshot, site, 0, 100_000, 64, 25_000,
    )
    .unwrap()
    .observe_model_only(49_999, 49_999, R39CompletionObservationV1::Pending)
    .unwrap();
    let boundary = R39ScopedPersistentSdmaWaitPolicyModelV1::new_with_cursor_model_only(
        snapshot, site, 0, 100_000, 64, 25_000,
    )
    .unwrap()
    .observe_model_only(50_000, 50_000, R39CompletionObservationV1::Pending)
    .unwrap();
    assert_eq!(
        before.decision,
        R39WaitDecisionV1::Pause(R39WaitActionV1::Spin)
    );
    assert_eq!(
        boundary.decision,
        R39WaitDecisionV1::Pause(R39WaitActionV1::Yield)
    );
}

#[test]
fn ready_wins_and_pending_times_out_after_one_zero_deadline_observation() {
    let snapshot = initial(R37CopyKindV1::Directional, 0);
    for (observation, decision) in [
        (R39CompletionObservationV1::Ready, R39WaitDecisionV1::Ready),
        (
            R39CompletionObservationV1::Pending,
            R39WaitDecisionV1::TimedOut,
        ),
    ] {
        let step = R39ScopedPersistentSdmaWaitPolicyModelV1::new_model_only(
            snapshot,
            R39WaitSiteV1::DirectionalPersistentSingle,
            7,
            7,
        )
        .unwrap()
        .observe_model_only(7, 7, observation)
        .unwrap();
        assert_eq!(step.observation_count, 1);
        assert_eq!(step.decision, decision);
        assert_eq!(step.attempts, 0);
        assert_eq!(step.next_sleep_ns, R39_DEFAULT_INITIAL_SLEEP_NS_V1);
    }
}

#[test]
fn start_after_deadline_is_admitted_but_invalid_sleep_and_observation_times_are_rejected() {
    let snapshot = initial(R37CopyKindV1::Directional, 0);
    let timed_out = R39ScopedPersistentSdmaWaitPolicyModelV1::new_model_only(
        snapshot,
        R39WaitSiteV1::DirectionalPersistentSingle,
        2,
        1,
    )
    .unwrap()
    .observe_model_only(2, 2, R39CompletionObservationV1::Pending)
    .unwrap();
    assert_eq!(timed_out.decision, R39WaitDecisionV1::TimedOut);
    for next_sleep_ns in [0, R39_DEFAULT_MAX_SLEEP_NS_V1 + 1] {
        assert!(matches!(
            R39ScopedPersistentSdmaWaitPolicyModelV1::new_with_cursor_model_only(
                snapshot,
                R39WaitSiteV1::DirectionalPersistentSingle,
                1,
                2,
                0,
                next_sleep_ns,
            ),
            Err(R39ModelErrorV1::InvalidSleep)
        ));
    }
    assert!(matches!(
        R39ScopedPersistentSdmaWaitPolicyModelV1::new_model_only(
            snapshot,
            R39WaitSiteV1::DirectionalPersistentSingle,
            2,
            3,
        )
        .unwrap()
        .observe_model_only(1, 1, R39CompletionObservationV1::Ready),
        Err(R39ModelErrorV1::InvalidObservationTime)
    ));
    assert!(matches!(
        R39ScopedPersistentSdmaWaitPolicyModelV1::new_model_only(
            snapshot,
            R39WaitSiteV1::DirectionalPersistentSingle,
            2,
            3,
        )
        .unwrap()
        .observe_model_only(2, 1, R39CompletionObservationV1::Ready),
        Err(R39ModelErrorV1::InvalidObservationTime)
    ));
}

#[test]
fn executable_model_owner_is_private_and_not_cloneable() {
    let source = include_str!("r39_scoped_persistent_sdma_wait_policy.rs");
    let private = source
        .split("struct R39WaitAuthorityV1")
        .nth(1)
        .unwrap()
        .split("pub struct R39ScopedPersistentSdmaWaitPolicyModelV1")
        .next()
        .unwrap();
    assert!(private.contains("snapshot: R37WaitSnapshotV1"));

    let owner = source
        .split("pub struct R39ScopedPersistentSdmaWaitPolicyModelV1")
        .nth(1)
        .unwrap()
        .split("impl R39ScopedPersistentSdmaWaitPolicyModelV1")
        .next()
        .unwrap();
    assert!(owner.contains("authority: R39WaitAuthorityV1"));
    assert!(!owner.contains("pub authority"));
    assert!(!owner.contains("derive(Clone"));
}
