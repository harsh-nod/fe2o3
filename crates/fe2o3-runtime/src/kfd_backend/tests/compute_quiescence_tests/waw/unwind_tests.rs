//! Native dependency adapters over scripted owners; no GPU execution.

use super::*;
use std::os::unix::process::ExitStatusExt;

fn inspect_and_drop(mode: &str) {
    let failed_order = mode.ends_with("failed-order");
    let (mut f, _) = fixture(mode == "observe-explicit" || failed_order);
    f.backend.scripted_persistent_poll_pending_observations = 8;
    let stream = if failed_order {
        f.producer_stream
    } else {
        f.launch.stream
    };
    let child = if failed_order {
        let failed = f.submit().unwrap();
        let failed_stream = f.launch.stream;
        let dep = dependency(&mut f, failed_stream, failed);
        let child = submit(&mut f, stream, &[dep]).unwrap();
        assert_eq!(
            f.backend.cancel_v1(failed).unwrap(),
            crate::BackendCancellationV1::Cancelled
        );
        assert!(matches!(
            f.backend.submissions[&failed].status,
            BackendPollV1::Failed { .. }
        ));
        child
    } else if mode == "observe-explicit" {
        f.submit().unwrap()
    } else {
        let child = submit(&mut f, stream, &[]).unwrap();
        assert!(
            f.backend.pending_compute[&child]
                .explicit_success_dependencies
                .is_empty()
        );
        child
    };
    if mode != "observe-explicit" {
        assert_eq!(
            f.backend.pending_compute[&child].ordered_predecessor,
            Some(f.producer)
        );
    }
    let recipe = Arc::downgrade(&f.backend.pending_compute[&child].launch);
    let snapshot = |backend: &KfdRuntimeBackendV1| {
        let pending = &backend.pending_compute[&child];
        (
            (
                pending.id,
                pending.module,
                pending.launch.unaccounted_copy_for_test(),
                pending.retained_allocations.clone(),
                pending.ordered_predecessor,
                pending.explicit_success_dependencies.clone(),
                pending.explicit_dependency_cursor,
                pending.quiescence_dependencies.clone(),
                pending.quiescence_cursor,
                pending.dependency_depth,
            ),
            (
                Arc::as_ptr(&pending.launch) as usize,
                pending.retained_allocations.as_ptr() as usize,
                pending.explicit_success_dependencies.as_ptr() as usize,
                pending.quiescence_dependencies.as_ptr() as usize,
            ),
            backend.pending_compute_streams.clone(),
            backend.stream_compute_lanes.clone(),
            backend.compute_dependency_retain_counts.clone(),
            backend.compute_module_retain_counts.clone(),
            backend.compute_completion_reservations,
            backend
                .allocation_custody
                .iter()
                .map(|(&id, custody)| {
                    (
                        id,
                        (
                            custody.owners.clone(),
                            custody.sole_stream,
                            custody.owner_counts,
                        ),
                    )
                })
                .collect::<HashMap<_, _>>(),
            backend.scripted_sdma.as_ref().unwrap().remaining_steps(),
        )
    };
    let owner_ids = |backend: &KfdRuntimeBackendV1| {
        let active = backend.active.as_ref().unwrap();
        let Some(ActiveComputeExecutionV1::ScriptedThreeBindingPersistent { devices, .. }) =
            active.execution.as_ref()
        else {
            panic!("scripted producer owns the published three-binding roster");
        };
        (
            active.id,
            devices
                .each_ref()
                .map(|device| device.scripted_owner_id().unwrap()),
        )
    };
    let before = snapshot(&f.backend);
    let owners = owner_ids(&f.backend);
    assert_producer_owns_output(&f);
    f.backend.scripted_persistent_poll_pending_observations = 0;
    f.backend.scripted_persistent_transition_failure =
        Some(ScriptedPersistentTransitionFailureV1::UnwindBeforeTake);
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if mode.starts_with("observe") {
            // Exercise the observer adapter after the same ownership move used
            // by the SPI, without its earlier exclusive-blocker observation.
            let pending = f.backend.pending_compute.remove(&child).unwrap();
            f.backend.observe_pending_compute_v1(pending).map(|_| ())
        } else {
            f.backend.flush_stream_v1(stream)
        }
    }))
    .expect_err("dependency observation must propagate the injected panic");
    assert_eq!(
        panic.downcast_ref::<&str>().copied(),
        Some("scripted three-binding unwind before active take")
    );
    assert!(f.backend.terminal);
    assert_eq!(f.backend.scripted_persistent_transition_failure, None);
    assert_eq!(snapshot(&f.backend), before);
    assert_eq!(owner_ids(&f.backend), owners);
    assert!(recipe.upgrade().is_some());
    assert_producer_owns_output(&f);
    assert!(f.backend.terminal_sdma_custody.is_none());
    assert!(matches!(
        f.backend.poll_v1(child),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    assert_eq!(snapshot(&f.backend), before);
    eprintln!("pending dependency unwind custody inspected; dropping unfinished backend");
    drop(ManuallyDrop::into_inner(f));
    panic!("unfinished dependency backend Drop returned");
}

#[test]
fn pending_dependency_unwind_restores_exact_owner_before_abort() {
    const CHILD: &str = "FE2O3_TEST_PENDING_DEPENDENCY_UNWIND_DROP";
    const TEST: &str = "kfd_backend::tests::compute_quiescence_tests::waw::unwind_tests::pending_dependency_unwind_restores_exact_owner_before_abort";
    const CASES: [&str; 5] = [
        "observe-explicit",
        "observe-ordered",
        "progress-ordered",
        "observe-failed-order",
        "progress-failed-order",
    ];
    if let Some(mode) = std::env::var_os(CHILD) {
        // Piped crash handlers ignore RLIMIT_CORE; keep deliberate aborts local.
        rustix::process::set_dumpable_behavior(rustix::process::DumpableBehavior::NotDumpable)
            .unwrap();
        rustix::process::setrlimit(
            rustix::process::Resource::Core,
            rustix::process::Rlimit {
                current: Some(0),
                maximum: Some(0),
            },
        )
        .unwrap();
        let mode = mode.to_str().unwrap();
        assert!(CASES.contains(&mode));
        inspect_and_drop(mode);
        unreachable!();
    }
    for mode in CASES {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture"])
            .env(CHILD, mode)
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.signal(), Some(6), "{mode}: {stderr}");
        assert!(
            stderr.contains(
                "pending dependency unwind custody inspected; dropping unfinished backend"
            ),
            "{mode}: {stderr}"
        );
        assert!(
            !stderr.contains("unfinished dependency backend Drop returned"),
            "{mode}: {stderr}"
        );
        eprintln!("verified {mode}: pending owner restored; unfinished Drop SIGABRT");
    }
}
