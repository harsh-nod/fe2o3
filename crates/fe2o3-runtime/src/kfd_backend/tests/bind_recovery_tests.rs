//! Allocation-free single bind refusal recovery, with CPU scripted transport.

use super::prepared_cancellation_tests::{InputFacts, input_facts, shell_facts};
use super::*;
use std::mem::ManuallyDrop;
use std::os::unix::process::ExitStatusExt;

struct Fixture {
    backend: ManuallyDrop<KfdRuntimeBackendV1>,
    stream: u64,
    host: u64,
    device: u64,
    copy: u64,
    event: u64,
    module: u64,
    kernel: u64,
    source: PersistentFullRangeComputeSourceV1,
    completed: Option<u64>,
}

impl Fixture {
    fn new(origin: usize) -> Self {
        let mut steps = vec![
            ScriptedSdmaStepV1::Write {
                offset: 0,
                byte_len: 4096,
            },
            scripted_submit_step_v1(
                Gfx942PersistentSdmaDirectionV1::HostToDevice,
                0,
                0,
                4096,
                ScriptedFailureModeV1::Success,
            ),
            ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Completed {
                direction: None,
                copy_bytes: None,
            }),
        ];
        if origin == 1 {
            steps.push(ScriptedSdmaStepV1::PromoteInitializedStorage(
                ScriptedFailureModeV1::Success,
            ));
        }
        steps.extend(scripted_release_steps_v1());
        let (backend, stream, host, device) = scripted_direct_backend_v1(4096, steps);
        let mut backend = ManuallyDrop::new(backend);
        backend.write_allocation_v1(host, 0, &[0x6d; 4096]).unwrap();
        let (source, destination) = scripted_copy_regions_v1(host, device, 4096);
        let copy = backend
            .copy_async_v1(stream, source, destination, &[])
            .unwrap();
        assert_eq!(backend.poll_v1(copy).unwrap(), BackendPollV1::Succeeded);
        let event = backend.record_event_v1(stream, copy).unwrap();
        let module = backend
            .load_module_v1(7, &synthetic_cov6::module())
            .unwrap();
        let kernel = backend
            .resolve_kernel_v1(module, "vecadd", [7; 32])
            .unwrap();
        let completed = if origin == 2 {
            let compute =
                submit_scripted_read_v1(&mut backend, stream, kernel, device, 4096, &[event]);
            backend.flush_stream_v1(stream).unwrap();
            assert_eq!(backend.poll_v1(compute).unwrap(), BackendPollV1::Succeeded);
            Some(compute)
        } else {
            None
        };
        if origin == 1 {
            backend.normalize_h2d_ready_v1(device).unwrap();
            assert_eq!(
                backend.convert_initialized_storage_v1(device).unwrap(),
                None
            );
        }
        let source = [
            PersistentFullRangeComputeSourceV1::AuthenticatedH2d,
            PersistentFullRangeComputeSourceV1::InitializedStorage,
            PersistentFullRangeComputeSourceV1::RetainedControlReplay,
        ][origin];
        Self {
            backend,
            stream,
            host,
            device,
            copy,
            event,
            module,
            kernel,
            source,
            completed,
        }
    }

    fn admission(&self) -> PersistentFullRangeComputeAdmissionV1 {
        PersistentFullRangeComputeAdmissionV1 {
            allocation: self.device,
            access: RuntimeAccessV1::Read,
            source: self.source,
        }
    }

    fn submit(&mut self) -> u64 {
        submit_scripted_read_v1(
            &mut self.backend,
            self.stream,
            self.kernel,
            self.device,
            4096,
            &[self.event],
        )
    }

    fn stored(
        &self,
    ) -> (
        InputFacts,
        usize,
        Option<KfdRuntimeReadyPromotionPerformanceV1>,
    ) {
        let record = &self.backend.allocations[&self.device];
        let (owner, bytes, certificate, origin, boxed, promotion) = match &record.sdma_storage {
            KfdRuntimeSdmaStorageV1::H2dReady(ready) => (
                ready.owner.scripted_owner_id().unwrap(),
                ready.owner.scripted_bytes().unwrap(),
                Some(ready.owner.authenticated_sha256()),
                "ready",
                ready.as_ref() as *const _ as usize,
                ready.promotion,
            ),
            KfdRuntimeSdmaStorageV1::InitializedStorage(ready) => {
                let InitializedStorageOwnerV1::Scripted(device) = ready.as_ref() else {
                    unreachable!()
                };
                (
                    device.scripted_owner_id().unwrap(),
                    device.scripted_bytes().unwrap(),
                    None,
                    "storage",
                    ready.as_ref() as *const _ as usize,
                    None,
                )
            }
            KfdRuntimeSdmaStorageV1::Device(device) => (
                device.scripted_owner_id().unwrap(),
                device.scripted_bytes().unwrap(),
                None,
                "replay",
                device.as_ref() as *const _ as usize,
                None,
            ),
            _ => panic!("stored owner"),
        };
        (
            InputFacts {
                owner,
                bytes: bytes.as_ptr() as usize,
                digest: Sha256::digest(bytes).into(),
                certificate,
                origin,
            },
            boxed,
            promotion,
        )
    }

    fn metadata(&self) -> String {
        let record = &self.backend.allocations[&self.device];
        format!(
            "{} {:?} {:?} {:?} {} {} {} {} {:?} {:?}",
            record.bytes.as_ptr() as usize,
            record.content_sha256,
            record.last_full_host_write,
            record.native_dirty,
            record.sdma_backed,
            record.sdma_initialized,
            record.sdma_shadow_dirty,
            record.scripted_three_binding_replay,
            self.backend.retained_persistent_dispatch,
            self.backend.last_ready_promotion_performance
        )
    }

    fn finish(mut self, additional: &[u64]) {
        self.backend.release_event_v1(self.event).unwrap();
        for submission in additional
            .iter()
            .copied()
            .chain(self.completed)
            .chain([self.copy])
        {
            self.backend.release_submission_v1(submission).unwrap();
        }
        self.backend.unload_module_v1(self.module).unwrap();
        release_scripted_direct_pair_v1(&mut self.backend, self.host, self.device);
        self.backend.destroy_stream_v1(self.stream).unwrap();
        let driver = self.backend.scripted_sdma.as_ref().unwrap();
        assert!(driver.is_exhausted());
        assert_eq!(driver.live_owner_count(), 0);
        assert_eq!(driver.unexpected_drops(), 0);
        self.backend.shutdown_native_v1().unwrap();
        drop(ManuallyDrop::into_inner(self.backend));
    }
}

#[test]
fn bind_recovery_restores_exact_owner_box_and_metadata_without_allocation() {
    for origin in 0..3 {
        let mut f = Fixture::new(origin);
        let submission = f.submit();
        let pending_before = super::initial_publication_tests::pending_facts(
            &f.backend.pending_compute[&submission],
        );
        for _ in 0..3 {
            let before = f.stored();
            let metadata = f.metadata();
            let admission = f.admission();
            let steps = f.backend.scripted_sdma.as_ref().unwrap().remaining_steps();
            let (taken, extraction_allocations) = counted_allocations_for_test_v1(|| {
                f.backend
                    .take_persistent_compute_input_v1(admission, submission)
            });
            let (input, restoration) = taken.unwrap();
            assert_eq!(input_facts(&input), before.0);
            assert_eq!(restoration.admission.allocation, f.device);
            assert_eq!(restoration.admission.source, f.source);
            assert_eq!(restoration.submission, submission);
            assert_eq!(restoration.promotion, before.2);
            let shell = restoration
                .restore_shell
                .as_ref()
                .or_else(|| {
                    f.backend.allocations[&f.device]
                        .persistent_storage_restore
                        .as_ref()
                })
                .unwrap();
            let shell_addresses = shell_facts(shell);
            let expected_box = shell_addresses[[0, 3, 1][origin]];
            assert_ne!(expected_box, 0);
            if origin != 1 {
                assert_eq!(expected_box, before.1);
                assert_eq!(extraction_allocations, 0);
            } else {
                assert_eq!(extraction_allocations, 3);
                assert!(restoration.restore_shell.is_none());
            }
            let (restored, allocations) = counted_allocations_for_test_v1(|| {
                f.backend
                    .restore_persistent_bind_input_v1(input, restoration)
            });
            restored.unwrap();
            assert_eq!(allocations, 0, "origin {origin}");
            let after = f.stored();
            assert_eq!(after.0, before.0);
            assert_eq!(after.1, expected_box);
            assert_eq!(after.2, before.2);
            assert_eq!(f.metadata(), metadata);
            assert_eq!(
                super::initial_publication_tests::pending_facts(
                    &f.backend.pending_compute[&submission]
                ),
                pending_before
            );
            assert!(
                f.backend.allocations[&f.device]
                    .persistent_storage_restore
                    .is_none()
            );
            let driver = f.backend.scripted_sdma.as_ref().unwrap();
            assert_eq!(driver.remaining_steps(), steps);
            assert_eq!(driver.live_owner_count(), 2);
            assert_eq!(driver.unexpected_drops(), 0);
        }
        assert_eq!(
            f.backend.cancel_v1(submission).unwrap(),
            crate::BackendCancellationV1::Cancelled
        );
        f.finish(&[submission]);
    }
}

#[test]
fn bind_recovery_public_rejection_preserves_storage_and_queued_successor() {
    for origin in 0..3 {
        let mut f = Fixture::new(origin);
        let before = f.stored();
        let metadata = f.metadata();
        let first = f.submit();
        let first_event = f.backend.record_event_v1(f.stream, first).unwrap();
        let trailing = f.submit();
        let trailing_before =
            super::initial_publication_tests::pending_facts(&f.backend.pending_compute[&trailing]);
        assert!(f.backend.active.is_none());
        assert_eq!(
            f.backend.pending_compute_streams[&f.stream],
            [first, trailing]
        );
        f.backend.scripted_persistent_bind_rejections = 1;
        assert!(matches!(
            f.backend.flush_stream_v1(f.stream),
            Err(RuntimeBackendFailureV1::Quiescent(_))
        ));
        assert_eq!(f.backend.scripted_persistent_bind_rejections, 0);
        assert!(!f.backend.terminal);
        assert!(f.backend.active.is_none());
        assert_eq!(f.backend.pending_compute_streams[&f.stream], [trailing]);
        assert_eq!(
            super::initial_publication_tests::pending_facts(&f.backend.pending_compute[&trailing]),
            trailing_before
        );
        assert_eq!(
            f.backend.poll_v1(first).unwrap(),
            BackendPollV1::Failed { code: -1 }
        );
        assert!(!f.backend.submissions[&first].profile_dispatch_published);
        assert_eq!(f.backend.compute_completion_reservations, 1);
        assert_eq!(f.backend.compute_module_retain_counts[&f.module], 1);
        assert_eq!(f.backend.compute_dependency_retain_counts[&f.copy], 1);
        assert_eq!(f.backend.compute_dependency_retain_counts[&first], 1);
        assert_eq!(f.backend.stream_submission_tails[&f.stream], trailing);
        assert!(!f.backend.stream_compute_lanes.contains_key(&f.stream));
        assert_eq!(f.backend.allocation_custody[&f.device].owners.len(), 1);
        assert_eq!(
            f.backend.allocation_custody[&f.device].owners[0].submission,
            trailing
        );
        let after = f.stored();
        assert_eq!(after.0, before.0);
        assert_eq!(after.2, before.2);
        if origin != 1 {
            assert_eq!(after.1, before.1);
        }
        assert_eq!(f.metadata(), metadata);
        f.backend.flush_stream_v1(f.stream).unwrap();
        assert_eq!(
            f.backend.poll_v1(trailing).unwrap(),
            BackendPollV1::Succeeded
        );
        assert!(f.backend.compute_dependency_retain_counts.is_empty());
        assert!(f.backend.compute_module_retain_counts.is_empty());
        assert!(f.backend.allocation_custody.is_empty());
        assert_eq!(f.backend.compute_completion_reservations, 0);
        f.backend.release_event_v1(first_event).unwrap();
        f.finish(&[trailing, first]);
    }
}

fn inspect(origin: usize, fault: usize) {
    let mut f = Fixture::new(origin);
    let submission = f.submit();
    let pending_before =
        super::initial_publication_tests::pending_facts(&f.backend.pending_compute[&submission]);
    let admission = f.admission();
    let (mut input, mut restoration) = f
        .backend
        .take_persistent_compute_input_v1(admission, submission)
        .unwrap();
    match fault {
        0 => {
            f.backend
                .allocations
                .get_mut(&f.device)
                .unwrap()
                .sdma_storage = KfdRuntimeSdmaStorageV1::ComputeInFlight(submission + 1)
        }
        1 => {
            f.backend.allocations.remove(&f.device).unwrap();
        }
        2 => {
            if origin == 1 {
                f.backend
                    .allocations
                    .get_mut(&f.device)
                    .unwrap()
                    .persistent_storage_restore = None;
            } else {
                restoration.restore_shell = None;
            }
        }
        3 => {
            if origin == 1 {
                f.backend
                    .allocations
                    .get_mut(&f.device)
                    .unwrap()
                    .persistent_storage_restore
                    .as_mut()
                    .unwrap()
                    .initialized = None;
            } else {
                restoration.restore_shell = Some(ThreeBindingPersistentRestoreShellV1 {
                    ready: None,
                    device: None,
                    replay: None,
                    initialized: None,
                });
            }
        }
        4 => {
            restoration.admission.source = if origin == 0 {
                PersistentFullRangeComputeSourceV1::RetainedControlReplay
            } else {
                PersistentFullRangeComputeSourceV1::AuthenticatedH2d
            }
        }
        5 => {
            input = match input {
                KfdRuntimePersistentComputeInputV1::ScriptedReady(ready) => {
                    KfdRuntimePersistentComputeInputV1::ScriptedReplay(ready.owner.normalize())
                }
                KfdRuntimePersistentComputeInputV1::ScriptedReplay(device) => {
                    KfdRuntimePersistentComputeInputV1::ScriptedStorage(device)
                }
                KfdRuntimePersistentComputeInputV1::ScriptedStorage(device) => {
                    KfdRuntimePersistentComputeInputV1::ScriptedReplay(device)
                }
                _ => unreachable!(),
            };
        }
        _ => unreachable!(),
    }
    let before = input_facts(&input);
    let before_slot = f
        .backend
        .allocations
        .get(&f.device)
        .map(|record| format!("{:?}", record.sdma_storage));
    let before_shells = f
        .backend
        .allocations
        .get(&f.device)
        .and_then(|record| record.persistent_storage_restore.as_ref())
        .map(shell_facts);
    let steps = f.backend.scripted_sdma.as_ref().unwrap().remaining_steps();
    assert!(matches!(
        f.backend
            .restore_persistent_bind_input_v1(input, restoration),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    assert!(f.backend.terminal);
    let Some(KfdRuntimeTerminalSdmaCustodyV1::PersistentRuntimeInput(input)) =
        f.backend.terminal_sdma_custody.as_ref()
    else {
        panic!("exact returned input retained");
    };
    assert_eq!(input_facts(input), before);
    assert_eq!(
        f.backend
            .allocations
            .get(&f.device)
            .map(|record| format!("{:?}", record.sdma_storage)),
        before_slot
    );
    assert_eq!(
        f.backend
            .allocations
            .get(&f.device)
            .and_then(|record| record.persistent_storage_restore.as_ref())
            .map(shell_facts),
        before_shells
    );
    assert_eq!(
        super::initial_publication_tests::pending_facts(&f.backend.pending_compute[&submission]),
        pending_before
    );
    assert!(matches!(
        f.backend.shutdown_native_v1(),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    let driver = f.backend.scripted_sdma.as_ref().unwrap();
    assert_eq!(driver.remaining_steps(), steps);
    assert_eq!(driver.live_owner_count(), 2);
    assert_eq!(driver.unexpected_drops(), 0);
    eprintln!("bind recovery exact terminal custody inspected; dropping unrepaired backend");
    drop(ManuallyDrop::into_inner(f.backend));
    panic!("bind recovery Drop returned");
}

#[test]
fn bind_recovery_mismatch_retains_exact_input_until_process_teardown() {
    const CHILD: &str = "FE2O3_TEST_BIND_RECOVERY_CUSTODY";
    const TEST: &str = "kfd_backend::tests::bind_recovery_tests::bind_recovery_mismatch_retains_exact_input_until_process_teardown";
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
        inspect(case / 6, case % 6);
        unreachable!();
    }
    for case in 0..18 {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture"])
            .env(CHILD, case.to_string())
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.signal(), Some(6), "case {case}: {stderr}");
        assert!(
            stderr.contains(
                "bind recovery exact terminal custody inspected; dropping unrepaired backend"
            ),
            "case {case}: {stderr}"
        );
        eprintln!("verified bind recovery case {case}: exact input retained; Drop SIGABRT");
    }
}
