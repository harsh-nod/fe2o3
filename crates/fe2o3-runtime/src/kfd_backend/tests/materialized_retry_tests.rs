//! Public poll of accepted Prepared owners with scripted submission outcomes.

use super::*;

const ORIGINS: [MaterializedPreparationOriginV1; 2] = [
    MaterializedPreparationOriginV1::NewBinding,
    MaterializedPreparationOriginV1::RecycledAttachment { generation: 7 },
];
const RETRY_FAULTS: [Fault; 8] = [
    Fault::SubmitRejected,
    Fault::SubmitTerminal,
    Fault::SubmitUnwind,
    Fault::OuterErrorAfterRetry,
    Fault::OuterUnwindAfterRetry,
    Fault::OuterErrorAfterPublish,
    Fault::OuterUnwindAfterPublish,
    Fault::ProfileUnwind,
];

impl Fixture {
    fn arm_prepared(&mut self, origin: MaterializedPreparationOriginV1, retries: usize) {
        self.backend.scripted_materialized_preparation = Some((origin, retries));
        self.backend.flush_stream_v1(self.stream).unwrap();
        self.assert_handoff();
        let Some(ActiveComputeExecutionV1::MaterializedPrepared(prepared)) = &self
            .backend
            .active_compute_submission_v1(self.first)
            .unwrap()
            .execution
        else {
            panic!("public initial publication must return an actual Prepared owner")
        };
        assert_eq!(prepared.origin, origin);
        assert_eq!(prepared.scripted.as_ref().unwrap().1, retries);
        assert_eq!(self.published_count(), 0);
    }

    fn mutate_active(&mut self, operation: impl FnOnce(&mut ActiveSubmissionV1)) {
        self.backend
            .with_compute_lane_state_v1(self.lane, |backend| {
                operation(backend.active.as_mut().unwrap())
            });
    }

    fn accepted_facts(&self) -> String {
        let b = &self.backend;
        let active = b.active_compute_submission_v1(self.first).unwrap();
        let allocation = &b.allocations[&self.host];
        format!(
            "{:?} {:p} {:p} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:p} {:?} {:?} {:?}",
            active,
            active.resident_descriptors.as_ptr(),
            active.writebacks.as_ptr(),
            active.resident_descriptors,
            active.writebacks,
            b.allocation_custody
                .get(&self.host)
                .map(|custody| &custody.owners),
            b.compute_module_retain_counts,
            b.compute_dependency_retain_counts,
            b.event_submission_retain_counts,
            allocation.bytes,
            allocation.native_dirty,
            allocation.sdma_shadow_dirty,
            Arc::as_ptr(&allocation.bytes),
            b.compute_completion_reservations,
            b.stream_compute_lanes,
            b.stream_submission_tails
        )
    }

    fn prepared_facts(&self) -> String {
        let active = self
            .backend
            .active_compute_submission_v1(self.first)
            .unwrap();
        let (origin, profile, specs) = match active.execution.as_ref().unwrap() {
            ActiveComputeExecutionV1::MaterializedPrepared(prepared) => (
                prepared.origin,
                &prepared.profile,
                &prepared.scripted.as_ref().unwrap().0,
            ),
            ActiveComputeExecutionV1::MaterializedBinding(root) => (
                root.origin,
                &root.profile,
                &root.scripted.as_ref().unwrap().0,
            ),
            _ => panic!("prepared or retained submission attempt required"),
        };
        let Some(Ok(bindings)) = &profile.bindings else {
            panic!("profile bindings retained");
        };
        format!(
            "{origin:?} {:?} {:?} {:p} {:?} {:p} {:?} {:?}",
            profile.launch,
            profile.semantic_contract,
            bindings.as_ptr(),
            bindings,
            specs.as_ptr(),
            specs,
            specs
                .iter()
                .map(|spec| Arc::as_ptr(&spec.bytes))
                .collect::<Vec<_>>()
        )
    }
}

fn retry_fault(
    lane: usize,
    origin: MaterializedPreparationOriginV1,
    fault: Fault,
    drop_unrepaired: bool,
) {
    let mut f = Fixture::new(lane, true);
    f.arm_prepared(origin, 0);
    let accepted = f.accepted_facts();
    let prepared = f.prepared_facts();
    let before = f
        .backend
        .active_compute_submission_v1(f.first)
        .unwrap()
        .performance;
    let driver_steps = f.backend.scripted_sdma.as_ref().unwrap().remaining_steps();
    f.backend.scripted_materialized_publication_fault = Some(fault);
    let result = catch_unwind(AssertUnwindSafe(|| f.backend.poll_v1(f.first)));
    let message = match fault {
        Fault::SubmitUnwind => Some("scripted ordinary submit unwind"),
        Fault::OuterUnwindAfterRetry | Fault::OuterUnwindAfterPublish => {
            Some("scripted ordinary outer lane unwind")
        }
        Fault::ProfileUnwind => Some("scripted ordinary publication profile unwind"),
        _ => None,
    };
    if let Some(message) = message {
        assert_eq!(result.unwrap_err().downcast_ref::<&str>(), Some(&message));
    } else {
        assert!(
            matches!(result, Ok(Err(RuntimeBackendFailureV1::Terminal(_)))),
            "{result:?}"
        );
    }
    f.assert_handoff();
    assert!(f.backend.terminal);
    assert_eq!(f.accepted_facts(), accepted);
    assert_eq!(f.published_count(), 0);
    assert!(f.backend.scripted_materialized_publication_fault.is_none());
    let active = f.backend.active_compute_submission_v1(f.first).unwrap();
    assert_eq!(active.performance.native_binding, before.native_binding);
    assert_eq!(
        active.performance.user_data_materializations,
        before.user_data_materializations
    );
    match active.execution.as_ref().unwrap() {
        ActiveComputeExecutionV1::MaterializedBinding(root) => {
            assert_eq!(f.prepared_facts(), prepared);
            assert!(match fault {
                Fault::SubmitRejected | Fault::SubmitTerminal | Fault::SubmitUnwind => matches!(
                    root.submission,
                    MaterializedSubmissionAttemptV1::NativeOwned
                ),
                Fault::OuterErrorAfterRetry | Fault::OuterUnwindAfterRetry =>
                    matches!(root.submission, MaterializedSubmissionAttemptV1::Retryable),
                Fault::OuterErrorAfterPublish | Fault::OuterUnwindAfterPublish => matches!(
                    root.submission,
                    MaterializedSubmissionAttemptV1::ScriptedPublished
                ),
                _ => false,
            });
        }
        ActiveComputeExecutionV1::ScriptedMaterialized => assert_eq!(fault, Fault::ProfileUnwind),
        _ => panic!("indeterminate retry must not re-arm Prepared"),
    }
    for result in [
        f.backend.poll_v1(f.first).map(|_| ()),
        f.backend.cancel_v1(f.first).map(|_| ()),
        f.backend.shutdown_native_v1(),
    ] {
        assert!(matches!(result, Err(RuntimeBackendFailureV1::Terminal(_))));
    }
    f.assert_handoff();
    assert_eq!(f.accepted_facts(), accepted);
    let driver = f.backend.scripted_sdma.as_ref().unwrap();
    assert_eq!(driver.remaining_steps(), driver_steps);
    assert_eq!(driver.unexpected_drops(), 0);
    if drop_unrepaired {
        eprintln!("ordinary retry custody inspected; dropping unrepaired backend");
        drop(ManuallyDrop::into_inner(f.backend));
        panic!("unrepaired ordinary retry Drop returned");
    }
    discard_scripted_fixture(f.backend);
}

#[test]
fn ordinary_retry_faults_preserve_indexed_attempts_and_prior_handoff() {
    for lane in 0..2 {
        for origin in ORIGINS {
            for fault in RETRY_FAULTS {
                retry_fault(lane, origin, fault, false);
            }
        }
    }
}

#[test]
fn ordinary_retry_unrepaired_drop_aborts() {
    const CHILD: &str = "FE2O3_TEST_ORDINARY_RETRY_DROP";
    const TEST: &str = "kfd_backend::tests::materialized_publication_tests::retry::ordinary_retry_unrepaired_drop_aborts";
    if let Ok(case) = std::env::var(CHILD) {
        rustix::process::setrlimit(
            rustix::process::Resource::Core,
            rustix::process::Rlimit {
                current: Some(0),
                maximum: Some(0),
            },
        )
        .unwrap();
        let case: usize = case.parse().unwrap();
        retry_fault(
            case / 16,
            ORIGINS[(case / 8) % 2],
            RETRY_FAULTS[case % 8],
            true,
        );
        unreachable!();
    }
    for case in 0..32 {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture"])
            .env(CHILD, case.to_string())
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.signal(), Some(6), "case {case}: {stderr}");
        assert!(
            stderr.contains("ordinary retry custody inspected; dropping unrepaired backend"),
            "case {case}: {stderr}"
        );
    }
}

#[test]
fn ordinary_retry_preserves_preparation_and_refreshes_only_publication_timing() {
    for lane in 0..2 {
        for origin in ORIGINS {
            for cancel in [false, true] {
                let mut f = Fixture::new(lane, false);
                f.arm_prepared(origin, 3);
                let accepted = f.accepted_facts();
                let prepared = f.prepared_facts();
                let old = Instant::now()
                    .checked_sub(Duration::from_secs(3600))
                    .unwrap();
                f.mutate_active(|active| {
                    active.published_at = old;
                    active.performance.native_binding = Duration::from_millis(17);
                    active.performance.publication = Duration::from_millis(23);
                });
                let baseline = f
                    .backend
                    .active_compute_submission_v1(f.first)
                    .unwrap()
                    .performance;
                let mut previous = baseline.publication;
                // Retry must never consume a queued initial-preparation configuration.
                let sentinel = Some((MaterializedPreparationOriginV1::NewBinding, 99));
                f.backend.scripted_materialized_preparation = sentinel;
                for remaining in (0..3).rev() {
                    let (result, allocations) =
                        super::super::super::drain_capture::tests::counted(|| {
                            f.backend.poll_v1(f.first)
                        });
                    assert_eq!(result.unwrap(), BackendPollV1::Pending);
                    assert_eq!(allocations, 0);
                    f.assert_handoff();
                    assert_eq!(f.accepted_facts(), accepted);
                    assert_eq!(f.prepared_facts(), prepared);
                    assert_eq!(f.backend.scripted_materialized_preparation, sentinel);
                    assert_eq!(f.published_count(), 0);
                    let active = f.backend.active_compute_submission_v1(f.first).unwrap();
                    let Some(ActiveComputeExecutionV1::MaterializedPrepared(prepared)) =
                        &active.execution
                    else {
                        panic!("confirmed retry must re-arm Prepared");
                    };
                    assert_eq!(prepared.scripted.as_ref().unwrap().1, remaining);
                    assert_eq!(active.published_at, old);
                    assert_eq!(active.performance.native_binding, baseline.native_binding);
                    assert_eq!(
                        active.performance.user_data_materializations,
                        baseline.user_data_materializations
                    );
                    assert_eq!(active.performance.data_path, baseline.data_path);
                    assert!(active.performance.publication >= previous);
                    previous = active.performance.publication;
                }
                if cancel {
                    assert_eq!(
                        f.backend.cancel_v1(f.first).unwrap(),
                        crate::BackendCancellationV1::Cancelled
                    );
                    assert_eq!(f.published_count(), 0);
                    assert!(!f.backend.submissions[&f.first].profile_dispatch_published);
                    assert_eq!(f.backend.allocation_custody[&f.host].owners.len(), 1);
                    assert_eq!(f.backend.compute_module_retain_counts[&f.module], 1 + lane);
                    assert_eq!(
                        f.backend.compute_dependency_retain_counts[&f.predecessor],
                        1
                    );
                    assert_eq!(f.backend.compute_dependency_retain_counts[&f.first], 1);
                    assert_eq!(f.backend.event_submission_retain_counts[&f.first], 1);
                    assert_eq!(
                        pending_facts(&f.backend.pending_compute[&f.trailing]),
                        f.trailing_recipe
                    );
                } else {
                    let start = Instant::now();
                    assert_eq!(f.backend.poll_v1(f.first).unwrap(), BackendPollV1::Pending);
                    let end = Instant::now();
                    f.assert_handoff();
                    assert_eq!(f.accepted_facts(), accepted);
                    assert_eq!(f.published_count(), 1);
                    let active = f.backend.active_compute_submission_v1(f.first).unwrap();
                    assert!(matches!(
                        active.execution,
                        Some(ActiveComputeExecutionV1::ScriptedMaterialized)
                    ));
                    assert!(start <= active.published_at && active.published_at <= end);
                    assert_eq!(active.performance.native_binding, baseline.native_binding);
                    assert!(active.performance.publication >= previous);
                }
                assert_eq!(f.backend.scripted_materialized_preparation, sentinel);
                assert!(!f.backend.terminal);
                discard_scripted_fixture(f.backend);
            }
        }
    }
}

#[test]
fn ordinary_retry_rejects_corrupt_custody_before_the_attempt() {
    for lane in 0..2 {
        for case in 0..15 {
            let mut f = Fixture::new(lane, true);
            f.arm_prepared(ORIGINS[case % 2], 0);
            f.backend.with_compute_lane_state_v1(lane, |b| {
                let active = b.active.as_mut().unwrap();
                match case {
                    0 => active.writebacks.clear(),
                    1 => active.writebacks[0].allocation += 1,
                    2 => active.writebacks[0].allocation_offset += 1,
                    3 => active.writebacks[0].data_index += 1,
                    4 => active.writebacks[0].data_offset += 1,
                    5 => active.writebacks[0].byte_len -= 1,
                    6 => active.writebacks.push(active.writebacks[0]),
                    7 => active.resident_descriptors[0].allocation += 1,
                    8 => active.deferred_ordered_predecessor_retain = true,
                    9 => b.compute_completion_reservations = 0,
                    10 => {
                        b.compute_module_retain_counts.remove(&f.module);
                    }
                    11 => {
                        b.allocation_custody.remove(&f.host);
                    }
                    12 => {
                        b.recycled_dispatch = Some(RecycledDispatchV1 {
                            kernel: active.kernel,
                            dispatch_shape_sha256: active.dispatch_shape_sha256,
                            descriptors: Vec::new(),
                        })
                    }
                    13 => {
                        let Some(ActiveComputeExecutionV1::MaterializedPrepared(prepared)) =
                            &mut active.execution
                        else {
                            unreachable!();
                        };
                        prepared.origin =
                            MaterializedPreparationOriginV1::RecycledAttachment { generation: 0 };
                    }
                    14 => {
                        let Some(ActiveComputeExecutionV1::MaterializedPrepared(prepared)) =
                            &mut active.execution
                        else {
                            unreachable!();
                        };
                        prepared.scripted = None;
                    }
                    _ => unreachable!(),
                }
            });
            let before = f.accepted_facts();
            f.backend.scripted_materialized_publication_fault = Some(Fault::SubmitUnwind);
            assert!(
                matches!(
                    f.backend.poll_v1(f.first),
                    Err(RuntimeBackendFailureV1::Terminal(_))
                ),
                "case {case}"
            );
            assert!(f.backend.terminal);
            assert_eq!(
                f.backend.scripted_materialized_publication_fault,
                Some(Fault::SubmitUnwind)
            );
            assert!(matches!(
                f.backend
                    .active_compute_submission_v1(f.first)
                    .unwrap()
                    .execution,
                Some(ActiveComputeExecutionV1::MaterializedPrepared(_))
            ));
            assert_eq!(f.accepted_facts(), before);
            assert_eq!(f.published_count(), 0);
            assert_eq!(f.backend.active_compute_lane_v1(f.first), Some(lane));
            assert_eq!(f.backend.selected_compute_lane, 0);
            discard_scripted_fixture(f.backend);
        }
    }
}
