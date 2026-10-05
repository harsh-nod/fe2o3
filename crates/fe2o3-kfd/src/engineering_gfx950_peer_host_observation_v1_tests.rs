use super::*;

fn policy(cache: bool, operational: bool, profile: bool) -> PerformanceOptions {
    PerformanceOptions {
        cache_kernel_admission: cache,
        operational_currentness: operational,
        profile,
    }
}

fn snapshot(at: Instant) -> Gfx950EngineeringPeerHostObservationV1 {
    Gfx950EngineeringPeerHostObservationV1 {
        group: 7,
        observed_at: at,
        shared_full_currentness: false,
        participants: (0..2)
            .map(|rank| Gfx950EngineeringPeerHostParticipantV1 {
                rank,
                unique_id: u64::MAX - rank as u64,
                queue_epoch: 3,
                cache_kernel_admission: true,
                raw_timestamp_queue: false,
                counters: PerformanceCountersV1::default(),
            })
            .collect(),
        shared: Gfx950EngineeringSharedHostCountersV1::default(),
    }
}

fn empty_group() -> Gfx950EngineeringPeerGroupV1 {
    Gfx950EngineeringPeerGroupV1 {
        incarnation: 7,
        contexts: Vec::new(),
        buffers: BTreeMap::new(),
        next_buffer: 1,
        poisoned: false,
        closed: false,
        shared_full_currentness: false,
        projection_mlp_scratch: None,
    }
}

#[test]
fn host_observation_timer_gate_preserves_legacy_disabled_and_profiled_behavior() {
    assert!(!timers_enabled(None, false));
    assert!(timers_enabled(None, true));
    for cache in [false, true] {
        for operational in [false, true] {
            for profile in [false, true] {
                let options = policy(cache, operational, profile);
                assert_eq!(timers_enabled(Some(options), false), profile);
                assert!(timers_enabled(Some(options), true));
                assert_eq!(options.cache_kernel_admission, cache);
                assert_eq!(options.operational_currentness, operational);
                assert_eq!(options.profile, profile);
                assert!(require_observational_policy(false, Some(options)).is_ok());
                assert_eq!(
                    require_observational_policy(true, Some(options)).is_ok(),
                    !operational && !profile
                );
            }
        }
    }
}

#[test]
fn host_observation_configuration_requires_once_fresh_full_unprofiled_policy() {
    for options in [
        None,
        Some(policy(false, false, false)),
        Some(policy(true, false, false)),
    ] {
        require_fresh_observation(false, options, 1, 1, 0).unwrap();
        for (enabled, buffer, kernel, write) in [
            (true, 1, 1, 0),
            (false, 2, 1, 0),
            (false, 1, 2, 0),
            (false, 1, 1, 1),
            (false, 0, 1, 0),
            (false, 1, 0, 0),
        ] {
            assert!(require_fresh_observation(enabled, options, buffer, kernel, write).is_err());
        }
    }
    for options in [
        policy(false, true, false),
        policy(true, false, true),
        policy(true, true, true),
    ] {
        assert!(require_fresh_observation(false, Some(options), 1, 1, 0).is_err());
    }
}

#[test]
fn host_observation_shared_scopes_count_once_and_keep_nanoseconds_separate() {
    let mut shared = Gfx950EngineeringSharedHostCountersV1::default();
    shared
        .record(SharedScope::GroupFence, Duration::from_nanos(17))
        .unwrap();
    shared
        .record(SharedScope::Publication, Duration::from_nanos(29))
        .unwrap();
    shared
        .record(SharedScope::Publication, Duration::from_nanos(0))
        .unwrap();
    assert_eq!(
        shared,
        Gfx950EngineeringSharedHostCountersV1 {
            group_full_checks: 1,
            group_full_ns: 17,
            publication_full_checks: 2,
            publication_full_ns: 29,
        }
    );
}

#[test]
fn host_observation_shared_counter_exhaustion_never_partially_updates() {
    for scope in [SharedScope::GroupFence, SharedScope::Publication] {
        for (count, total, elapsed) in [
            (u64::MAX, 0, Duration::ZERO),
            (0, u64::MAX, Duration::from_nanos(1)),
            (0, 0, Duration::from_secs(u64::MAX)),
        ] {
            let mut shared = Gfx950EngineeringSharedHostCountersV1::default();
            match scope {
                SharedScope::GroupFence => {
                    shared.group_full_checks = count;
                    shared.group_full_ns = total;
                }
                SharedScope::Publication => {
                    shared.publication_full_checks = count;
                    shared.publication_full_ns = total;
                }
            }
            let before = shared.clone();
            assert!(shared.record(scope, elapsed).is_err());
            assert_eq!(shared, before);
        }
    }
}

#[test]
fn host_observation_delta_preserves_nested_scopes_without_additive_assumption() {
    let before = snapshot(Instant::now());
    let mut after = before.clone();
    after.observed_at += Duration::from_nanos(10);
    after.participants[0].counters.dispatch_prepare_ns = 17;
    after.participants[0].counters.full_currentness_ns = 13;
    after.participants[1].counters.dispatch_wait_ns = 19;
    after
        .shared
        .record(SharedScope::Publication, Duration::from_nanos(23))
        .unwrap();
    let delta = after.checked_delta(&before).unwrap();
    assert_eq!(delta.group_incarnation(), 7);
    assert_eq!(delta.host_elapsed_ns(), 10);
    assert_eq!(delta.participants()[0].counters().dispatch_prepare_ns, 17);
    assert_eq!(delta.participants()[0].counters().full_currentness_ns, 13);
    assert_eq!(delta.participants()[1].counters().dispatch_wait_ns, 19);
    assert_eq!(delta.shared_counters().publication_full_ns, 23);
    assert_eq!(after.group_incarnation(), 7);
    assert_eq!(after.participants()[1].rank(), 1);
    assert_eq!(after.participants()[1].unique_id(), u64::MAX - 1);
    assert_eq!(after.participants()[1].queue_epoch(), 3);
    assert!(after.participants()[1].cache_kernel_admission());
    assert!(!after.participants()[1].raw_timestamp_queue());
    assert!(!after.shared_full_currentness());
    assert_eq!(after.shared_counters().publication_full_checks, 1);
    assert_eq!(before.checked_delta(&before).unwrap().host_elapsed_ns(), 0);
}

#[test]
fn host_observation_delta_rejects_identity_roster_epoch_and_policy_changes() {
    let before = snapshot(Instant::now());
    let mut changes = Vec::new();
    let mut changed = before.clone();
    changed.group += 1;
    changes.push(changed);
    let mut changed = before.clone();
    changed.shared_full_currentness = true;
    changes.push(changed);
    let mut changed = before.clone();
    changed.participants.pop();
    changes.push(changed);
    let mut changed = before.clone();
    changed.participants.swap(0, 1);
    changes.push(changed);
    for rank in 0..2 {
        let mut changed = before.clone();
        changed.participants[rank].rank += 1;
        changes.push(changed);
        let mut changed = before.clone();
        changed.participants[rank].unique_id -= 1;
        changes.push(changed);
        let mut changed = before.clone();
        changed.participants[rank].queue_epoch += 1;
        changes.push(changed);
        let mut changed = before.clone();
        changed.participants[rank].cache_kernel_admission = false;
        changes.push(changed);
        let mut changed = before.clone();
        changed.participants[rank].raw_timestamp_queue = true;
        changes.push(changed);
    }
    for changed in changes {
        assert!(changed.checked_delta(&before).is_err());
    }
}

#[test]
fn host_observation_delta_rejects_reversed_host_time() {
    let before = snapshot(Instant::now());
    let mut after = before.clone();
    after.observed_at += Duration::from_nanos(1);
    assert!(before.checked_delta(&after).is_err());
    assert_eq!(after.checked_delta(&before).unwrap().host_elapsed_ns(), 1);
}

#[test]
fn host_observation_delta_checks_every_legacy_counter_for_underflow() {
    macro_rules! each {
        ($($field:ident),+ $(,)?) => { $(
            let mut before = snapshot(Instant::now());
            before.participants[1].counters.$field = 1;
            let mut after = before.clone();
            after.participants[1].counters.$field = 0;
            assert!(after.checked_delta(&before).is_err(), stringify!($field));
            after.participants[1].counters.$field = u64::MAX;
            assert_eq!(after.checked_delta(&before).unwrap().participants[1].counters.$field,
                u64::MAX - 1);
        )+ };
    }
    each!(
        commands,
        command_ns,
        full_currentness_checks,
        full_currentness_ns,
        operational_currentness_checks,
        operational_currentness_ns,
        kernel_admissions,
        kernel_admission_ns,
        dispatches,
        dispatch_prepare_ns,
        dispatch_publish_ns,
        dispatch_wait_ns,
        completion_polls,
        reads,
        read_bytes,
        read_ns,
        writes,
        write_bytes,
        write_ns
    );
}

#[test]
fn host_observation_delta_checks_all_shared_counters_for_underflow() {
    macro_rules! each {
        ($($field:ident),+) => { $(
            let mut before = snapshot(Instant::now()); before.shared.$field = 1;
            let mut after = before.clone(); after.shared.$field = 0;
            assert!(after.checked_delta(&before).is_err(), stringify!($field));
        )+ };
    }
    each!(
        group_full_checks,
        group_full_ns,
        publication_full_checks,
        publication_full_ns
    );
}

#[test]
fn host_observation_state_never_relabels_prior_counters_after_queue_rollover() {
    let state = HostObservationState {
        queue_epoch: 9,
        shared: Default::default(),
    };
    state.require_epoch(9).unwrap();
    for epoch in [0, 8, 10, u64::MAX] {
        assert!(state.require_epoch(epoch).is_err());
    }
}

#[test]
fn host_observation_getter_refuses_invalid_closed_and_poisoned_groups_without_backend() {
    for (closed, poisoned) in [(false, false), (true, false), (false, true)] {
        let mut group = empty_group();
        group.closed = closed;
        group.poisoned = poisoned;
        assert!(group.host_observation_v1().is_err());
        assert!(group.contexts.is_empty());
        assert!(group.poisoned || group.closed);
    }
}

#[test]
fn host_observation_configuration_failure_uses_existing_terminal_poison() {
    let mut group = empty_group();
    assert!(group.enable_host_observation_v1().is_err());
    assert!(group.poisoned);
    assert!(!group.shared_full_currentness);
    assert!(group.enable_host_observation_v1().is_err());
}

#[test]
fn host_observation_does_not_extend_the_legacy_counter_wire() {
    let value = serde_json::to_value(PerformanceCountersV1::default()).unwrap();
    let fields = value.as_object().unwrap();
    let names = [
        "commands",
        "command_ns",
        "full_currentness_checks",
        "full_currentness_ns",
        "operational_currentness_checks",
        "operational_currentness_ns",
        "kernel_admissions",
        "kernel_admission_ns",
        "dispatches",
        "dispatch_prepare_ns",
        "dispatch_publish_ns",
        "dispatch_wait_ns",
        "completion_polls",
        "reads",
        "read_bytes",
        "read_ns",
        "writes",
        "write_bytes",
        "write_ns",
    ];
    assert_eq!(fields.len(), names.len());
    for name in names {
        assert_eq!(fields[name], 0);
    }
    let mut changed = value;
    changed
        .as_object_mut()
        .unwrap()
        .insert("host_observation".into(), true.into());
    assert!(serde_json::from_value::<PerformanceCountersV1>(changed).is_err());
}
