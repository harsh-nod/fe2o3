use super::*;

#[test]
fn persistent_elapsed_spin_profile_is_confined_to_three_public_wait_routes() {
    let live = crate::queue::live_production_source_for_tests_v1()
        .split("#[cfg(test)]\nmod tests")
        .next()
        .unwrap();
    let fixed = crate::queue::fixed_dispatch_production_source_for_tests_v1();
    fn method<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
        source
            .split(start)
            .nth(1)
            .unwrap()
            .split(end)
            .next()
            .unwrap()
    }
    let elapsed_profile = "SdmaWaitProfileV1::PersistentElapsedSpinFloor";

    for body in [
        method(
            live,
            "pub fn wait_directional_persistent_sdma_copy_for_v1(",
            "pub fn submit_directional_persistent_sdma_window_v1(",
        ),
        method(
            live,
            "pub fn wait_directional_persistent_sdma_window_for_v1(",
            "pub fn submit_same_device_persistent_sdma_window_v1(",
        ),
        method(
            live,
            "pub fn wait_same_device_persistent_sdma_window_for_v1(",
            "pub fn recycle_sdma_buffer(",
        ),
    ] {
        assert_eq!(body.matches(elapsed_profile).count(), 1);
    }
    assert_eq!(live.matches(elapsed_profile).count(), 3);

    let generic_persistent = method(
        live,
        "pub fn wait_persistent_sdma_copy_for_v1(",
        "pub fn promote_sdma_device_buffer_to_directional_persistent_allocation_v1(",
    );
    let ordinary = method(
        live,
        "pub fn wait_sdma_copy_for(",
        "pub fn wait_sdma_copy_batch_for(",
    );
    assert!(generic_persistent.contains("SdmaWaitProfileV1::Default"));
    assert!(ordinary.contains("SdmaWaitProfileV1::Default"));

    for body in [
        generic_persistent,
        ordinary,
        method(
            live,
            "pub fn execute_synchronous_directional_persistent_sdma_copy_for_v1(",
            "pub fn poll_directional_persistent_sdma_copy_v1(",
        ),
        method(
            fixed,
            "pub fn wait_and_recycle_directional_persistent_fixed_dispatch_until_v1(",
            "pub fn recycle_directional_persistent_fixed_dispatch_v1(",
        ),
    ] {
        assert!(!body.contains(elapsed_profile));
    }

    let lower = include_str!("../../sdma.rs")
        .split("#[cfg(test)]\nmod tests")
        .next()
        .unwrap();
    let fused_synchronous = lower
        .split("fn wait_for_in_current_scope_with_final_currentness(")
        .nth(1)
        .unwrap()
        .split("pub(crate) fn wait_many_for(")
        .next()
        .unwrap();
    let xgmi_single = lower
        .split("fn wait_xgmi_for_in_current_scope(")
        .nth(1)
        .unwrap()
        .split("fn wait_many_xgmi_for_in_current_scope(")
        .next()
        .unwrap();
    let xgmi_batch = lower
        .split("fn wait_many_xgmi_for_in_current_scope(")
        .nth(1)
        .unwrap()
        .split("fn observe_progress_in_current_scope(")
        .next()
        .unwrap();
    let ordinary_batch_or_striped = lower
        .split("pub(crate) fn wait_many_for_in_current_scope(")
        .nth(1)
        .unwrap()
        .split("pub(crate) fn destroy_queue(")
        .next()
        .unwrap();
    for body in [fused_synchronous, xgmi_single, ordinary_batch_or_striped] {
        assert!(!body.contains("SdmaWaitProfileV1"));
        assert!(body.contains("MonotonicWaitV1::until(deadline)"));
    }
    assert!(!xgmi_batch.contains("SdmaWaitProfileV1"));
    assert!(xgmi_batch.contains("XgmiWaitCadence::Ordinary1ms,"));
    assert!(xgmi_batch.contains("let mut wait = cadence.cursor(deadline);"));
    let cadence = include_str!("../../sdma/retained_pair_cadence.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();
    assert!(cadence.contains("Self::Ordinary1ms => MonotonicWaitV1::until(deadline)"));
    assert!(!cadence.contains("until_with_active_spin_floor"));

    let persistent_compute = fixed
        .split("pub fn wait_and_recycle_directional_persistent_fixed_dispatch_until_v1(")
        .nth(1)
        .unwrap()
        .split("pub fn recycle_directional_persistent_fixed_dispatch_v1(")
        .next()
        .unwrap();
    assert!(!persistent_compute.contains(elapsed_profile));
    assert!(persistent_compute.contains("MonotonicWaitV1::until(deadline)"));
}

#[test]
fn profiled_sdma_waits_observe_before_testing_the_deadline() {
    let source = include_str!("../../sdma.rs")
        .split("#[cfg(test)]\nmod tests")
        .next()
        .unwrap();
    let window = source
        .split("fn wait_persistent_window_for(")
        .nth(1)
        .unwrap()
        .split("#[allow(clippy::type_complexity)]")
        .next()
        .unwrap();
    assert!(window.contains(".observe_until_ready"));
    assert!(window.contains("self.observe_persistent_window_completion"));

    let single = source
        .split("pub(crate) fn wait_for(")
        .nth(1)
        .unwrap()
        .split("fn wait_for_in_current_scope_with_final_currentness(")
        .next()
        .unwrap();
    assert!(single.contains(".observe_until_ready"));
    assert!(single.contains("let observed ="));

    let wait_helper = include_str!("../../wait.rs")
        .split("pub(crate) fn observe_until_ready")
        .nth(1)
        .unwrap()
        .split("#[cfg(test)]")
        .next()
        .unwrap();
    assert!(
        wait_helper.find("if observe_ready()?").unwrap()
            < wait_helper.find("if self.expired()").unwrap()
    );

    let profile = source
        .split("impl SdmaWaitProfileV1")
        .nth(1)
        .unwrap()
        .split("/// Frozen claim boundary")
        .next()
        .unwrap();
    assert!(profile.contains("Self::Default => MonotonicWaitV1::until(deadline)"));
    assert!(profile.contains("MonotonicWaitV1::until_with_active_spin_floor"));
}

#[test]
fn persistent_profile_drives_one_zero_deadline_observation_before_timeout() {
    let profile = SdmaWaitProfileV1::PersistentElapsedSpinFloor(Duration::from_nanos(50_000));

    let mut pending = profile.cursor(Instant::now());
    let mut pending_observations = 0;
    assert_eq!(
        pending.observe_until_ready(|| {
            pending_observations += 1;
            Ok::<_, ()>(false)
        }),
        Ok(false)
    );
    assert_eq!(pending_observations, 1);

    let mut ready = profile.cursor(Instant::now());
    let mut ready_observations = 0;
    assert_eq!(
        ready.observe_until_ready(|| {
            ready_observations += 1;
            Ok::<_, ()>(true)
        }),
        Ok(true)
    );
    assert_eq!(ready_observations, 1);
}

#[test]
fn sdma_copy_manifest_digest_is_frozen() {
    assert!(
        GFX942_SDMA_COPY_MANIFEST_V1
            .contains(fe2o3_kfd_uapi::KFD_SDMA_QUEUE_SCHEMA_MANIFEST_SHA256)
    );
    assert!(
        GFX942_SDMA_COPY_MANIFEST_V1
            .contains(crate::topology::GFX942_SDMA_TOPOLOGY_CAPABILITY_MANIFEST_SHA256_V1)
    );
    for required in [
        "nonblocking-whole-submission-poll-observes-every-entry-before-pending",
        "prebinds-one-exact-tail-per-active-shard",
        "tail-ready-with-pending-prefix-fails-terminally",
        "private-lifetime-bound-all-ready-witness",
        "immediate-abort-on-unwind-ordered-custody-move-without-reobservation-or-revalidation",
        "timeout-retains-the-whole-submission-and-retry-starts-a-new-native-wait-epoch",
        "ordinary-wait-uses-a-compile-time-disabled-profile",
        "without-cpu-cost-syscalls-or-counters",
        "explicit-available-unavailable-invalid-status",
        "clear-all-three-cpu-cost-values-without-an-operational-error",
        "closed-diagnostic-spin-budget-recorded-in-every-successful-profile",
        "host-thread-cpu-and-context-switch-measurements-are-not-proof",
        "profiled-host-overhead-is-not-unprofiled-overhead",
        "ordinary-wrapper-always-selects-current",
        "nonprofiled-noncurrent-rejected",
        "closed-roster:current-or-250000ns-or-500000ns-or-1000000ns-or-1500000ns-or-3000000ns",
        "wall-minus-thread-cpu-minus-requested-sleep-is-not-avoidable-latency",
        "spin-changes-host-observation-wakeup-cpu-and-context-switch-behavior-without-device-duration-causality",
        "no-unbounded-input",
        "subsequent-sleep-requests-capped-at-25000ns",
        "actual-scheduler-wake-latency-unbounded",
        "no-completion-authority-from-pause-schedule",
        "striped-tail-fence-premise=each-copy-submission-ends-in-the-exact-mtype-3-system-1-snoop-1-fence",
        "each-bound-owner-engine-index-is-exactly-queue-ordinal-modulo-two",
        "preserves-the-exact-sealed-plan-shards-and-ordered-completion-roster-in-terminal-custody",
        "retryable-no-native-effect-availability-detached-compute-foreign-buffer-and-recoverable-preparation-failures-preserve-inputs-and-do-not-poison",
        "every-striped-submit-poll-or-wait-process-teardown-return-invokes-one-central-terminalizer",
        "striped-wait-timeout-retains-exact-pending-custody-and-does-not-invoke-the-terminalizer",
        "and-invokes-the-same-terminalizer",
        "r46-model-is-not-an-executable-rust-refinement",
        "public-constructor-requires-caller-owned-move-only-inline-root",
        "confirmed-owner-rooted-before-closing-route-and-disarm",
        "owner-free-diagnostic-errors",
        "original-creator-panic-preserved-and-secondary-payloads-forgotten",
        "occupied-root-reentry-inert-and-drop-aborts",
        "no-full-consuming-memory-primitive-refinement-or-native-panic-injection-proof",
        "no-parity-or-striped-tail-wait-speedup-measured-for-this-revision",
        "no-claim-that-spin-closes-the-observed-hip-gap",
    ] {
        assert!(GFX942_SDMA_COPY_MANIFEST_V1.contains(required));
    }
    let digest = Sha256::digest(GFX942_SDMA_COPY_MANIFEST_V1);
    let mut rendered = String::with_capacity(64);
    for byte in digest {
        use core::fmt::Write;
        write!(&mut rendered, "{byte:02x}").unwrap();
    }
    assert_eq!(rendered, GFX942_SDMA_COPY_MANIFEST_SHA256_V1);
}

#[test]
fn logical_mux_manifest_digest_and_exclusions_are_frozen() {
    for required in [
        "two-persistent-native-queues",
        "logical-lanes=closed-counts:2,4,8,14,16",
        "requests:2..126",
        "at-most-63-per-native-shard",
        "each-native-shard-is-stable-filter-of-that-order",
        "one-write-pointer-publication-and-one-doorbell-per-native-queue",
        "bind-two-exact-native-tail-fences",
        "advance-only-after-both-native-publications-closing-currentness-successful-live-model-retake-and-restored-owner-commit",
        "lower-layer-classified-no-native-effect-before-first-publication",
        "facade-caught-rust-unwind-after-entering-live-owner-memory-operation-is-conservatively-post-effect",
        "restoring-owner-helper-resumes-only-to-the-enclosing-facade-catch",
        "nested-retirement-suffix-unwind-after-submission-ownership-move-causes-lower-immediate-abort",
        "without-typed-custody-or-guaranteed-owner-restoration-or-explicit-poison",
        "no-continued-execution-after-either-unwind-class",
        "typed-panic-recovery",
        "hip-stream-independence",
        "formal-refinement",
        "performance",
    ] {
        assert!(GFX942_SDMA_LOGICAL_MUX_MANIFEST_V2.contains(required));
    }
    let digest = Sha256::digest(GFX942_SDMA_LOGICAL_MUX_MANIFEST_V2);
    let mut rendered = String::with_capacity(64);
    for byte in digest {
        use core::fmt::Write;
        write!(&mut rendered, "{byte:02x}").unwrap();
    }
    assert_eq!(rendered, GFX942_SDMA_LOGICAL_MUX_MANIFEST_SHA256_V2);
}
