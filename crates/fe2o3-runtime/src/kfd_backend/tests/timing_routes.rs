use super::*;

#[test]
fn initial_persistent_timing_keeps_binding_exclusive_from_publication() {
    let mut performance = KfdRuntimeLaunchPerformanceV1 {
        native_binding: Duration::from_nanos(101),
        publication: Duration::from_nanos(103),
        ..KfdRuntimeLaunchPerformanceV1::default()
    };
    record_initial_persistent_timing_v1(
        &mut performance,
        Duration::from_nanos(7),
        Duration::from_nanos(11),
    );
    assert_eq!(performance.native_binding(), Duration::from_nanos(7));
    assert_eq!(performance.publication(), Duration::from_nanos(11));
}

#[test]
fn retained_persistent_retry_accumulates_only_publication() {
    let mut performance = KfdRuntimeLaunchPerformanceV1::default();
    record_initial_persistent_timing_v1(
        &mut performance,
        Duration::from_nanos(7),
        Duration::from_nanos(11),
    );
    performance.publication += Duration::from_nanos(13);
    assert_eq!(performance.native_binding(), Duration::from_nanos(7));
    assert_eq!(performance.publication(), Duration::from_nanos(24));
}

#[test]
fn launch_recycle_timing_is_the_inclusive_sum_of_its_components() {
    let performance = KfdRuntimeLaunchPerformanceV1 {
        completion_signal_recycle: Duration::from_nanos(61),
        completion_detach_restore: Duration::from_nanos(43),
        ..KfdRuntimeLaunchPerformanceV1::default()
    };
    assert_eq!(
        performance.completion_signal_recycle(),
        Duration::from_nanos(61)
    );
    assert_eq!(
        performance.completion_detach_restore(),
        Duration::from_nanos(43)
    );
    assert_eq!(performance.recycle(), Duration::from_nanos(104));
    assert_eq!(profile_host_timing_v1(performance).recycle_ns, 104);
}

#[test]
fn continuous_recycle_timing_assigns_handoff_to_detach_restore() {
    let completion_signal_recycle = Duration::from_nanos(61);
    let handoff = Duration::from_nanos(7);
    let detach_restore = Duration::from_nanos(43);
    let recycle_inclusive = completion_signal_recycle + handoff + detach_restore;
    let completion_detach_restore =
        completion_detach_restore_duration_v1(recycle_inclusive, completion_signal_recycle);
    let performance = KfdRuntimeLaunchPerformanceV1 {
        completion_signal_recycle,
        completion_detach_restore,
        ..KfdRuntimeLaunchPerformanceV1::default()
    };

    assert_eq!(completion_detach_restore, handoff + detach_restore);
    assert_eq!(performance.recycle(), recycle_inclusive);
}

#[test]
fn synchronous_directional_runtime_routes_only_through_fused_single_execute() {
    let synchronous = include_str!("../sdma_synchronous.rs");
    assert!(synchronous.contains(".execute_synchronous_single("));
    assert!(!synchronous.contains(".submit("));
    assert!(!synchronous.contains(".wait("));
    assert!(synchronous.contains("retire_native_directional_completed_v1(parts)"));
    assert!(synchronous.contains("fill_restore_shell_v1(shell, pair.device)"));

    let seam = include_str!("../kfd_backend_sdma_seam/directional.rs");
    let fused = seam
        .split("fn execute_synchronous_single(")
        .nth(1)
        .unwrap()
        .split("fn poll(")
        .next()
        .unwrap();
    assert!(fused.contains("queue.execute_synchronous_directional_persistent_sdma_copy_for_v1("));
    assert!(fused.contains("Self::Scripted(driver)"));
}

#[test]
fn r26_measured_copies_route_through_fused_async_single_submit() {
    let production = include_str!("../persistent_completion.rs");
    let benchmark = include_str!("../../../examples/gfx942-runtime-r26-inplace-benchmark.rs");
    let measured_copy = benchmark
        .split("fn run_copy_v1(")
        .nth(1)
        .unwrap()
        .split("struct LaunchTimingV1")
        .next()
        .unwrap();
    assert!(measured_copy.contains(".copy_async(stream, source, destination, &[])"));
    assert!(measured_copy.contains("wait_for_copy_v1("));
    assert!(!measured_copy.contains("execute_synchronous_directional_sdma_v1"));

    let measured_compute = benchmark
        .split("fn run_compute_v1(")
        .nth(1)
        .unwrap()
        .split("fn observe_launch_timing_v1(")
        .next()
        .unwrap();
    assert_eq!(measured_compute.matches(".launch(").count(), 1);
    assert_eq!(measured_compute.matches(".wait(").count(), 1);

    let iteration = benchmark
        .split("fn iteration(")
        .nth(1)
        .unwrap()
        .split("fn shutdown(")
        .next()
        .unwrap();
    assert_eq!(iteration.matches("run_copy_v1(").count(), 2);
    assert!(iteration.contains("region(self.upload, RuntimeAccessV1::Read)"));
    assert!(iteration.contains("region(self.download, RuntimeAccessV1::Write)"));

    assert!(benchmark.contains("GFX942_INPLACE_TRANSFORM_QUALIFICATION_BUFFER_BYTES_V1"));
    let one_mib = 1024_u64 * 1024;
    assert!(one_mib <= u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1));

    let seam = include_str!("../kfd_backend_sdma_seam/directional.rs");
    let submit = seam
        .split("fn submit(")
        .nth(1)
        .unwrap()
        .split("fn execute_synchronous_single(")
        .next()
        .unwrap();
    let single = submit
        .split("DirectionalSdmaRequestPlanV1::Single(_)")
        .nth(1)
        .unwrap()
        .split("DirectionalSdmaRequestPlanV1::Window(_)")
        .next()
        .unwrap();
    assert!(single.contains("queue.submit_directional_persistent_sdma_copy_v1("));
    assert!(!single.contains("execute_synchronous_directional_persistent_sdma_copy_for_v1"));

    let live = include_str!("../../../../fe2o3-kfd/src/queue_live.rs");
    let fused_async = live
        .split("pub fn submit_directional_persistent_sdma_copy_v1(")
        .nth(1)
        .unwrap()
        .split("pub fn execute_synchronous_directional_persistent_sdma_copy_for_v1(")
        .next()
        .unwrap();
    assert_eq!(fused_async.matches("with_sdma_owner_memory").count(), 1);
    assert!(fused_async.contains("prepare_admitted_directional_persistent_sdma_request_v1"));
    assert!(fused_async.contains("handoff.publish(owner, memory)"));
    assert!(!fused_async.contains("wait_for_in_current_scope"));

    assert_eq!(
        production
            .matches(".poll_and_recycle_directional_persistent_fixed_dispatch_v1(dispatch)")
            .count(),
        1
    );
    assert_eq!(
        production
            .matches(".poll_directional_persistent_fixed_dispatch_v1(dispatch)")
            .count(),
        0
    );
    assert_eq!(
        production
            .matches(".recycle_directional_persistent_fixed_dispatch_v1(completed)")
            .count(),
        0
    );
    let persistent_completion = production
        .split("fn finish_indexed_persistent_poll_v1")
        .nth(1)
        .unwrap()
        .split("fn restore_indexed_scalar_completion_v1")
        .next()
        .unwrap();
    let midpoint = persistent_completion
        .find(".saturating_duration_since(active.published_at)")
        .unwrap();
    let recycle = persistent_completion
        .find("signal_recycle = observed_at.elapsed()")
        .unwrap();
    let detach = persistent_completion
        .find("detach_recycled_directional_persistent_fixed_dispatch_v1")
        .unwrap();
    assert!(midpoint < recycle);
    assert!(recycle < detach);
}
