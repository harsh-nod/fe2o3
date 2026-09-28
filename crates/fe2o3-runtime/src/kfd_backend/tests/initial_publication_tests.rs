//! First-submit ownership and pending handoff, using CPU scripted owners.

use super::super::prepared_publication::ScriptedPreparedPublicationFaultV1 as Fault;
use super::prepared_cancellation_tests::InputFacts;
use super::prepared_publication_tests::{inputs, ledger, shells};
use super::*;
use std::mem::ManuallyDrop;
use std::os::unix::process::ExitStatusExt;
use std::panic::{AssertUnwindSafe, catch_unwind};

struct Fixture<const N: usize> {
    backend: ManuallyDrop<KfdRuntimeBackendV1>,
    stream: u64,
    module: u64,
    allocations: [u64; N],
    copy_pair: (u64, u64),
    copy: u64,
    event: u64,
    first: u64,
    first_event: u64,
    trailing: u64,
}

fn pending_facts(pending: &PendingComputeSubmissionV1) -> String {
    format!(
        "{pending:?} {:p} {:p} {:p} {:p} {:p} {:p}",
        Arc::as_ptr(&pending.launch),
        pending.launch.explicit_kernarg.as_ptr(),
        pending.launch.bindings.as_ptr(),
        pending.retained_allocations.as_ptr(),
        pending.explicit_success_dependencies.as_ptr(),
        pending.quiescence_dependencies.as_ptr(),
    )
}

fn stored_input(backend: &KfdRuntimeBackendV1, allocation: u64) -> InputFacts {
    let (owner, bytes, certificate) = match &backend.allocations[&allocation].sdma_storage {
        KfdRuntimeSdmaStorageV1::H2dReady(ready) => (
            ready.owner.scripted_owner_id().unwrap(),
            ready.owner.scripted_bytes().unwrap(),
            Some(ready.owner.authenticated_sha256()),
        ),
        KfdRuntimeSdmaStorageV1::Device(device) => (
            device.scripted_owner_id().unwrap(),
            device.scripted_bytes().unwrap(),
            None,
        ),
        KfdRuntimeSdmaStorageV1::InitializedStorage(owner) => {
            let InitializedStorageOwnerV1::Scripted(device) = owner.as_ref() else {
                panic!("scripted initialized storage");
            };
            (
                device.scripted_owner_id().unwrap(),
                device.scripted_bytes().unwrap(),
                None,
            )
        }
        _ => panic!("stored scripted owner"),
    };
    InputFacts {
        owner,
        bytes: bytes.as_ptr() as usize,
        digest: Sha256::digest(bytes).into(),
        certificate,
        origin: if certificate.is_some() {
            "ready"
        } else {
            "storage"
        },
    }
}

impl<const N: usize> Fixture<N> {
    fn new(initialized: bool) -> Self {
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
        if initialized {
            steps.extend((0..N).map(|_| {
                ScriptedSdmaStepV1::PromoteInitializedStorage(ScriptedFailureModeV1::Success)
            }));
        }
        steps.extend(scripted_release_steps_v1());
        steps.extend((0..N).flat_map(|_| {
            [
                ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
                ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
            ]
        }));
        let (backend, stream, allocations) =
            scripted_persistent_backend_with_steps_v1::<N>(4096, steps);
        let mut backend = ManuallyDrop::new(backend);
        if initialized {
            for allocation in allocations {
                backend.normalize_h2d_ready_v1(allocation).unwrap();
            }
        }
        let copy_pair = add_scripted_direct_pair_v1(&mut backend, 4096);
        backend
            .write_allocation_v1(copy_pair.0, 0, &[0xc7; 4096])
            .unwrap();
        let (source, destination) = scripted_copy_regions_v1(copy_pair.0, copy_pair.1, 4096);
        let copy = backend
            .copy_async_v1(stream, source, destination, &[])
            .unwrap();
        assert_eq!(backend.poll_v1(copy).unwrap(), BackendPollV1::Succeeded);
        let event = backend.record_event_v1(stream, copy).unwrap();
        let module = backend
            .load_module_v1(
                7,
                &if N == 1 {
                    synthetic_cov6::module()
                } else {
                    synthetic_cov6::three_binding_module()
                },
            )
            .unwrap();
        let kernel = backend
            .resolve_kernel_v1(module, "vecadd", [7; 32])
            .unwrap();
        let submit = |backend: &mut KfdRuntimeBackendV1| {
            if N == 1 {
                submit_scripted_read_v1(backend, stream, kernel, allocations[0], 4096, &[event])
            } else {
                submit_scripted_three_binding_with_dependencies_v1(
                    backend,
                    stream,
                    kernel,
                    [allocations[0], allocations[1], allocations[2]],
                    4096,
                    &[event],
                )
            }
        };
        let first = submit(&mut backend);
        let first_event = backend.record_event_v1(stream, first).unwrap();
        let trailing = submit(&mut backend);
        assert!(backend.active.is_none());
        assert_eq!(backend.pending_compute_streams[&stream], [first, trailing]);
        assert_eq!(backend.compute_dependency_retain_counts[&copy], 2);
        assert_eq!(backend.compute_dependency_retain_counts[&first], 1);
        assert_eq!(
            backend.pending_compute[&first]
                .explicit_success_dependencies
                .as_ref(),
            &[copy]
        );
        assert_eq!(
            backend.pending_compute[&first].ordered_predecessor,
            Some(copy)
        );
        Self {
            backend,
            stream,
            module,
            allocations,
            copy_pair,
            copy,
            event,
            first,
            first_event,
            trailing,
        }
    }

    fn assert_handoff(&self) {
        let backend = &self.backend;
        let active = backend.active.as_ref().unwrap();
        assert_eq!(active.id, self.first);
        assert!(!backend.pending_compute.contains_key(&self.first));
        assert!(!backend.submissions.contains_key(&self.first));
        assert_eq!(backend.pending_compute.len(), 1);
        assert_eq!(
            backend.pending_compute_streams[&self.stream],
            [self.trailing]
        );
        assert_eq!(backend.compute_dependency_retain_counts.len(), 2);
        assert_eq!(backend.compute_dependency_retain_counts[&self.copy], 1);
        assert_eq!(backend.compute_dependency_retain_counts[&self.first], 1);
        assert_eq!(backend.compute_module_retain_counts[&self.module], 2);
        assert_eq!(backend.compute_completion_reservations, 2);
        assert_eq!(backend.stream_submission_tails[&self.stream], self.trailing);
        assert_eq!(backend.stream_compute_lanes[&self.stream], 0);
        assert_eq!(backend.events[&self.event].submission, self.copy);
        assert_eq!(backend.events[&self.first_event].submission, self.first);
        assert_eq!(backend.event_submission_retain_counts[&self.copy], 1);
        assert_eq!(backend.event_submission_retain_counts[&self.first], 1);
        assert_eq!(active.ordered_predecessor, Some(self.copy));
        assert!(!active.deferred_ordered_predecessor_retain);
        assert_eq!(active.allocations.len(), N);
        assert_eq!(active.performance.user_data_materializations(), 0);
        for allocation in self.allocations {
            assert!(active.allocations.contains(&allocation));
            assert!(matches!(backend.allocations[&allocation].sdma_storage,
                KfdRuntimeSdmaStorageV1::ComputeInFlight(id) if id == self.first));
            let custody = &backend.allocation_custody[&allocation];
            assert_eq!(custody.owners.len(), 2);
            assert_eq!(
                custody.owner_counts[RuntimeAllocationCustodyKindV1::Compute.index()],
                2
            );
            assert_eq!(
                custody
                    .owners
                    .iter()
                    .map(|owner| (owner.submission, owner.stream, owner.kind))
                    .collect::<Vec<_>>(),
                vec![
                    (
                        self.first,
                        self.stream,
                        RuntimeAllocationCustodyKindV1::Compute
                    ),
                    (
                        self.trailing,
                        self.stream,
                        RuntimeAllocationCustodyKindV1::Compute
                    )
                ]
            );
        }
    }

    fn finish(mut self, cancel: bool) {
        assert_eq!(
            self.backend.cancel_v1(self.trailing).unwrap(),
            crate::BackendCancellationV1::Cancelled
        );
        if cancel {
            assert_eq!(
                self.backend.cancel_v1(self.first).unwrap(),
                crate::BackendCancellationV1::Cancelled
            );
        } else {
            if self.backend.persistent_prepared_is_armed_v1() {
                assert_eq!(
                    self.backend.poll_v1(self.first).unwrap(),
                    BackendPollV1::Pending
                );
            }
            assert_eq!(
                self.backend.poll_v1(self.first).unwrap(),
                BackendPollV1::Succeeded
            );
        }
        assert!(self.backend.compute_dependency_retain_counts.is_empty());
        assert!(self.backend.compute_module_retain_counts.is_empty());
        assert!(self.backend.allocation_custody.is_empty());
        assert_eq!(self.backend.compute_completion_reservations, 0);
        for event in [self.first_event, self.event] {
            self.backend.release_event_v1(event).unwrap();
        }
        for submission in [self.trailing, self.first, self.copy] {
            self.backend.release_submission_v1(submission).unwrap();
        }
        self.backend.unload_module_v1(self.module).unwrap();
        release_scripted_direct_pair_v1(&mut self.backend, self.copy_pair.0, self.copy_pair.1);
        for allocation in self.allocations {
            self.backend
                .allocations
                .get_mut(&allocation)
                .unwrap()
                .sdma_backed = false;
            self.backend.release_allocation_v1(allocation).unwrap();
        }
        self.backend.destroy_stream_v1(self.stream).unwrap();
        let driver = self.backend.scripted_sdma.as_ref().unwrap();
        assert!(driver.is_exhausted());
        assert_eq!(driver.live_owner_count(), 0);
        assert_eq!(driver.unexpected_drops(), 0);
        self.backend.shutdown_native_v1().unwrap();
        drop(ManuallyDrop::into_inner(self.backend));
    }
}

fn inspect<const N: usize>(initialized: bool, fault: Fault) {
    let mut f = Fixture::<N>::new(initialized);
    let before_inputs = f
        .allocations
        .map(|allocation| stored_input(&f.backend, allocation));
    let trailing_recipe = pending_facts(&f.backend.pending_compute[&f.trailing]);
    let live = f.backend.scripted_sdma.as_ref().unwrap().live_owner_count();
    let steps = f.backend.scripted_sdma.as_ref().unwrap().remaining_steps();
    f.backend.scripted_prepared_publication_fault = Some(fault);
    let result = catch_unwind(AssertUnwindSafe(|| f.backend.flush_stream_v1(f.stream)));
    let panic_message = match fault {
        Fault::Unwind => Some("scripted consuming publication unwind"),
        Fault::ProfileUnwind => Some("scripted persistent publication profile unwind"),
        Fault::InitialObserverUnwind => Some("scripted initial queue observer unwind"),
        _ => None,
    };
    if let Some(message) = panic_message {
        assert_eq!(result.unwrap_err().downcast_ref::<&str>(), Some(&message));
    } else {
        assert!(
            matches!(result, Ok(Err(RuntimeBackendFailureV1::Terminal(_)))),
            "{result:?}"
        );
    }
    f.assert_handoff();
    assert!(f.backend.terminal);
    assert_eq!(f.backend.scripted_prepared_publication_fault, None);
    assert_eq!(
        pending_facts(&f.backend.pending_compute[&f.trailing]),
        trailing_recipe
    );
    if fault == Fault::ProfileUnwind {
        let devices: Vec<_> = match f
            .backend
            .active
            .as_ref()
            .unwrap()
            .execution
            .as_ref()
            .unwrap()
        {
            ActiveComputeExecutionV1::ScriptedPersistent { device, .. } => vec![device.as_ref()],
            ActiveComputeExecutionV1::ScriptedThreeBindingPersistent { devices, .. } => {
                devices.iter().collect()
            }
            _ => panic!("published before profile observer"),
        };
        for (device, before) in devices.into_iter().zip(&before_inputs) {
            assert_eq!(device.scripted_owner_id(), Some(before.owner));
            assert_eq!(
                device.scripted_bytes().unwrap().as_ptr() as usize,
                before.bytes
            );
            assert_eq!(
                <[u8; 32]>::from(Sha256::digest(device.scripted_bytes().unwrap())),
                before.digest
            );
        }
        assert!(f.backend.terminal_sdma_custody.is_none());
    } else {
        assert_eq!(inputs(&f.backend), before_inputs);
        match f
            .backend
            .active
            .as_ref()
            .unwrap()
            .execution
            .as_ref()
            .unwrap()
        {
            ActiveComputeExecutionV1::ScriptedPersistentPrepared { input, .. } if N == 1 => {
                assert_eq!(
                    matches!(input, PreparedReceiptV1::NativeOwned),
                    matches!(fault, Fault::Terminal | Fault::Unwind)
                );
            }
            ActiveComputeExecutionV1::ScriptedThreeBindingPersistentPrepared { inputs, .. }
                if N == 3 =>
            {
                assert_eq!(
                    matches!(inputs, PreparedReceiptV1::NativeOwned),
                    matches!(fault, Fault::Terminal | Fault::Unwind)
                );
            }
            _ => panic!("exact prepared custody required"),
        }
    }
    let before = ledger(&f.backend);
    let before_shells = shells(&f.backend);
    for result in [
        f.backend.poll_v1(f.first).map(|_| ()),
        f.backend.cancel_v1(f.first).map(|_| ()),
        f.backend.shutdown_native_v1(),
    ] {
        assert!(matches!(result, Err(RuntimeBackendFailureV1::Terminal(_))));
    }
    f.assert_handoff();
    assert_eq!(ledger(&f.backend), before);
    assert_eq!(shells(&f.backend), before_shells);
    let driver = f.backend.scripted_sdma.as_ref().unwrap();
    assert_eq!(driver.live_owner_count(), live);
    assert_eq!(
        driver.remaining_steps(),
        steps - usize::from(initialized) * N
    );
    assert_eq!(driver.unexpected_drops(), 0);
    eprintln!("initial publication handoff inspected; dropping unrepaired backend");
    drop(ManuallyDrop::into_inner(f.backend));
    panic!("initial publication Drop returned");
}

#[test]
fn initial_publication_faults_retain_exclusive_active_custody() {
    const CHILD: &str = "FE2O3_TEST_INITIAL_PUBLICATION_CUSTODY";
    const TEST: &str = "kfd_backend::tests::initial_publication_tests::initial_publication_faults_retain_exclusive_active_custody";
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
        let fault = [
            Fault::Terminal,
            Fault::Unwind,
            Fault::ProfileUnwind,
            Fault::InitialObserverUnwind,
            Fault::InitialRejected,
            Fault::InitialQuiescent,
        ][case % 6];
        if case < 12 {
            inspect::<1>(case >= 6, fault);
        } else {
            inspect::<3>(case >= 18, fault);
        }
        unreachable!();
    }
    for case in 0..24 {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture"])
            .env(CHILD, case.to_string())
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.signal(), Some(6), "case {case}: {stderr}");
        assert!(
            stderr.contains("initial publication handoff inspected; dropping unrepaired backend"),
            "case {case}: {stderr}"
        );
        eprintln!("verified initial publication case {case}: exclusive custody; Drop SIGABRT");
    }
}

fn healthy<const N: usize>(initialized: bool) {
    for outcome in 0..3 {
        let mut f = Fixture::<N>::new(initialized);
        let before = f
            .allocations
            .map(|allocation| stored_input(&f.backend, allocation));
        if outcome != 0 {
            f.backend.scripted_prepared_publication_fault = Some(Fault::Retryable);
        }
        f.backend.flush_stream_v1(f.stream).unwrap();
        f.assert_handoff();
        assert!(!f.backend.terminal);
        assert_eq!(f.backend.scripted_prepared_publication_fault, None);
        if outcome != 0 {
            assert!(f.backend.persistent_prepared_is_armed_v1());
            assert_eq!(inputs(&f.backend), before);
        } else {
            match f
                .backend
                .active
                .as_ref()
                .unwrap()
                .execution
                .as_ref()
                .unwrap()
            {
                ActiveComputeExecutionV1::ScriptedPersistent { .. } if N == 1 => {}
                ActiveComputeExecutionV1::ScriptedThreeBindingPersistent { .. } if N == 3 => {}
                _ => panic!("initial success must publish without an extra poll"),
            }
        }
        f.finish(outcome == 2);
    }
}

#[test]
fn initial_publication_success_retry_and_cancel_retire_only_their_pending_handoff() {
    for initialized in [false, true] {
        healthy::<1>(initialized);
        healthy::<3>(initialized);
    }
}
