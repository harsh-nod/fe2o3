//! Public runtime admission with scripted publication/retirement, not GPU evidence.

use super::super::materialized_cancellation::ScriptedMaterializedCancelFaultV1 as Fault;
use super::sdma_host_write_tests::discard_scripted_fixture;
use super::*;
use std::mem::ManuallyDrop;
use std::panic::{AssertUnwindSafe, catch_unwind};

const ORIGINS: [MaterializedPreparationOriginV1; 2] = [
    MaterializedPreparationOriginV1::NewBinding,
    MaterializedPreparationOriginV1::RecycledAttachment { generation: 7 },
];

struct Fixture {
    backend: ManuallyDrop<KfdRuntimeBackendV1>,
    stream: u64,
    module: u64,
    kernel: u64,
    host: u64,
    id: u64,
    streams: Vec<u64>,
    pairs: Vec<(u64, u64)>,
    ids: Vec<u64>,
}

impl Fixture {
    fn new(lane: usize, origin: MaterializedPreparationOriginV1, write: bool) -> Self {
        let steps: Vec<_> = (0..=lane)
            .flat_map(|_| scripted_release_steps_v1())
            .collect();
        let (backend, stream, host, device) =
            scripted_direct_backend_configured_v1(4096, steps, |backend| {
                backend
                    .enable_profiler_v1(KfdRuntimeProfilerConfigV1::new([0x94; 32], 256).unwrap())
                    .unwrap();
            });
        let mut backend = ManuallyDrop::new(backend);
        // The scripted publication fixture has no native queue constructor.
        for ordinal in 0..=lane {
            let queue = backend.profile_resource_v1(
                KfdProfileResourceKindV1::NativeQueue,
                KFD_PROFILE_NATIVE_QUEUE_ORDINAL_V1 + ordinal as u64,
            );
            backend.observe_profile_v1(
                queue.map(|queue| KfdRuntimeProfileEventKindV1::NativeQueueCreated { queue }),
            );
        }
        let module = backend
            .load_module_v1(7, &synthetic_cov6::module())
            .unwrap();
        let kernel = backend
            .resolve_kernel_v1(module, "vecadd", [7; 32])
            .unwrap();
        let mut streams = vec![stream];
        let mut pairs = vec![(host, device)];
        let mut ids = Vec::new();
        if lane != 0 {
            let blocker = submit_scripted_read_v1(&mut backend, stream, kernel, host, 4096, &[]);
            backend.flush_stream_v1(stream).unwrap();
            ids.push(blocker);
            streams.push(backend.create_stream_v1(7).unwrap());
            pairs.push(add_scripted_direct_pair_v1(&mut backend, 4096));
            for (allocation, memory_kind) in [
                (pairs[1].0, KfdProfileMemoryKindV1::HostVisible),
                (pairs[1].1, KfdProfileMemoryKindV1::DeviceLocalHostStaged),
            ] {
                let allocation =
                    backend.profile_resource_v1(KfdProfileResourceKindV1::Allocation, allocation);
                backend.observe_profile_v1(allocation.map(|allocation| {
                    KfdRuntimeProfileEventKindV1::AllocationCreated {
                        allocation,
                        memory_kind,
                        byte_len: 4096,
                        alignment: 8,
                    }
                }));
            }
        }
        let stream = streams[lane];
        let host = pairs[lane].0;
        backend.scripted_materialized_preparation = Some((origin, 2));
        let id = if write {
            let mut kernarg = [0; 16];
            kernarg[8..].copy_from_slice(&13_u64.to_le_bytes());
            backend
                .submit_v1(BackendLaunchV1 {
                    stream,
                    kernel,
                    explicit_kernarg: &kernarg,
                    bindings: &[BackendBindingV1 {
                        region: BackendMemoryRegionV1 {
                            allocation: host,
                            access: RuntimeAccessV1::Write,
                            byte_offset: 0,
                            byte_len: 4096,
                        },
                        kernarg_byte_offset: 0,
                    }],
                    dependencies: &[],
                    geometry: crate::RuntimeLaunchGeometryV1 {
                        grid: [64, 1, 1],
                        workgroup: [64, 1, 1],
                        dynamic_shared_bytes: 0,
                    },
                    semantic_launch: KfdRuntimeSemanticLaunchV1::Ordinary,
                })
                .unwrap()
        } else {
            submit_scripted_read_v1(&mut backend, stream, kernel, host, 4096, &[])
        };
        backend.flush_stream_v1(stream).unwrap();
        ids.push(id);
        assert_eq!(backend.active_compute_lane_v1(id), Some(lane));
        assert!(matches!(
            backend.active_compute_submission_v1(id).unwrap().execution,
            Some(ActiveComputeExecutionV1::MaterializedPrepared(_))
        ));
        Self {
            backend,
            stream,
            module,
            kernel,
            host,
            id,
            streams,
            pairs,
            ids,
        }
    }

    fn finish(mut self, expected_published: usize) {
        for id in self.ids {
            for _ in 0..8 {
                if self.backend.poll_v1(id).unwrap() != BackendPollV1::Pending {
                    break;
                }
            }
            assert_ne!(self.backend.poll_v1(id).unwrap(), BackendPollV1::Pending);
            self.backend.release_submission_v1(id).unwrap();
        }
        assert!(!self.backend.any_compute_active_v1());
        assert!(self.backend.pending_compute.is_empty());
        assert!(self.backend.allocation_custody.is_empty());
        assert!(self.backend.compute_module_retain_counts.is_empty());
        assert!(self.backend.stream_compute_lanes.is_empty());
        assert_eq!(self.backend.compute_completion_reservations, 0);
        self.backend.unload_module_v1(self.module).unwrap();
        for (host, device) in self.pairs {
            release_scripted_direct_pair_v1(&mut self.backend, host, device);
        }
        let queue_count = self.streams.len();
        for stream in self.streams {
            self.backend.destroy_stream_v1(stream).unwrap();
        }
        for ordinal in 0..queue_count {
            let queue = self.backend.profile_resource_v1(
                KfdProfileResourceKindV1::NativeQueue,
                KFD_PROFILE_NATIVE_QUEUE_ORDINAL_V1 + ordinal as u64,
            );
            self.backend.observe_profile_v1(
                queue.map(|queue| KfdRuntimeProfileEventKindV1::NativeQueueDestroyed { queue }),
            );
        }
        let driver = self.backend.scripted_sdma.as_ref().unwrap();
        assert!(driver.is_exhausted());
        assert_eq!(driver.live_owner_count(), 0);
        assert_eq!(driver.unexpected_drops(), 0);
        self.backend.shutdown_native_v1().unwrap();
        let capture = self.backend.finish_profiler_v1().unwrap();
        capture.validate().unwrap();
        for published in [true, false] {
            assert_eq!(
                capture
                    .events
                    .iter()
                    .filter(|event| {
                        if published {
                            matches!(
                                event.event,
                                KfdRuntimeProfileEventKindV1::DispatchPublished { .. }
                            )
                        } else {
                            matches!(
                                event.event,
                                KfdRuntimeProfileEventKindV1::DispatchCompleted { .. }
                            )
                        }
                    })
                    .count(),
                expected_published
            );
        }
        drop(ManuallyDrop::into_inner(self.backend));
    }
}

fn content(backend: &KfdRuntimeBackendV1, host: u64) -> String {
    let record = &backend.allocations[&host];
    format!(
        "{:?} {:?} {:?} {:?} {:?}",
        record.bytes,
        record.content_sha256,
        record.last_full_host_write,
        record.native_dirty,
        record.sdma_shadow_dirty
    )
}

#[test]
fn materialized_cancel_both_origins_on_primary_and_auxiliary_preserves_content() {
    for lane in 0..2 {
        for origin in ORIGINS {
            for write in [false, true] {
                let mut f = Fixture::new(lane, origin, write);
                let before = content(&f.backend, f.host);
                assert_eq!(f.backend.poll_v1(f.id).unwrap(), BackendPollV1::Pending);
                let active = f.backend.active_compute_submission_v1(f.id).unwrap();
                let Some(ActiveComputeExecutionV1::MaterializedPrepared(prepared)) =
                    &active.execution
                else {
                    panic!("retry preserved preparation")
                };
                assert_eq!(prepared.origin, origin);
                assert_eq!(!active.writebacks.is_empty(), write);
                assert_eq!(
                    f.backend.cancel_v1(f.id).unwrap(),
                    crate::BackendCancellationV1::Cancelled
                );
                assert_eq!(
                    f.backend.poll_v1(f.id).unwrap(),
                    BackendPollV1::Failed { code: -2 }
                );
                assert!(!f.backend.submissions[&f.id].profile_dispatch_published);
                assert_eq!(content(&f.backend, f.host), before);
                assert!(!f.backend.stream_submission_tails.contains_key(&f.stream));
                assert!(!f.backend.stream_compute_lanes.contains_key(&f.stream));
                assert_eq!(
                    f.backend.cancel_v1(f.id).unwrap(),
                    crate::BackendCancellationV1::TooLate
                );
                f.finish(lane);
            }
        }
    }
}

#[test]
fn materialized_cancel_preserves_queued_successors_and_their_retains() {
    for lane in 0..2 {
        for origin in ORIGINS {
            for explicit in [false, true] {
                let mut f = Fixture::new(lane, origin, false);
                let event = explicit.then(|| f.backend.record_event_v1(f.stream, f.id).unwrap());
                let dependencies: Vec<_> = event.into_iter().collect();
                let successor = submit_scripted_read_v1(
                    &mut f.backend,
                    f.stream,
                    f.kernel,
                    f.host,
                    4096,
                    &dependencies,
                );
                assert!(f.backend.pending_compute.contains_key(&successor));
                let retains = f.backend.compute_dependency_retain_counts.clone();
                let owners = f.backend.allocation_custody[&f.host].owners.clone();
                assert_eq!(
                    f.backend.cancel_v1(f.id).unwrap(),
                    crate::BackendCancellationV1::Cancelled
                );
                assert_eq!(f.backend.compute_dependency_retain_counts, retains);
                assert_eq!(
                    f.backend.allocation_custody[&f.host].owners,
                    owners
                        .into_iter()
                        .filter(|owner| owner.submission != f.id)
                        .collect::<std::collections::VecDeque<_>>()
                );
                assert_eq!(f.backend.stream_submission_tails[&f.stream], successor);
                let flush = f.backend.flush_stream_v1(f.stream);
                if explicit {
                    assert!(matches!(flush, Err(RuntimeBackendFailureV1::Quiescent(_))));
                    assert!(!f.backend.terminal);
                } else {
                    flush.unwrap();
                }
                let result = f.backend.poll_v1(successor).unwrap();
                if explicit {
                    assert!(matches!(result, BackendPollV1::Failed { .. }));
                } else {
                    assert_eq!(result, BackendPollV1::Succeeded);
                }
                f.ids.push(successor);
                if let Some(event) = event {
                    f.backend.release_event_v1(event).unwrap();
                }
                f.finish(lane + usize::from(!explicit));
            }
        }
    }
}

#[test]
fn materialized_prepared_retry_can_still_publish() {
    for lane in 0..2 {
        for origin in ORIGINS {
            let f = Fixture::new(lane, origin, false);
            f.finish(lane + 1);
        }
    }
}

#[test]
fn materialized_cancel_failures_keep_exact_indexed_owners() {
    for lane in 0..2 {
        for origin in ORIGINS {
            for fault in [
                Fault::BeforeReturn,
                Fault::AfterReturn,
                Fault::WrongGeneration,
            ] {
                let mut f = Fixture::new(lane, origin, false);
                let before = content(&f.backend, f.host);
                let owners = f.backend.allocation_custody[&f.host].owners.clone();
                let modules = f.backend.compute_module_retain_counts.clone();
                let reservations = f.backend.compute_completion_reservations;
                let primary_before = (lane != 0).then(|| format!("{:?}", f.backend.active));
                let descriptors = f
                    .backend
                    .active_compute_submission_v1(f.id)
                    .unwrap()
                    .resident_descriptors
                    .clone();
                f.backend.scripted_materialized_cancel_fault = Some(fault);
                let result = catch_unwind(AssertUnwindSafe(|| f.backend.cancel_v1(f.id)));
                assert!(matches!(result, Err(_) | Ok(Err(_))));
                assert_eq!(content(&f.backend, f.host), before);
                assert_eq!(f.backend.allocation_custody[&f.host].owners, owners);
                assert_eq!(f.backend.compute_module_retain_counts, modules);
                assert_eq!(f.backend.compute_completion_reservations, reservations);
                assert_eq!(f.backend.active_compute_lane_v1(f.id), Some(lane));
                assert_eq!(f.backend.stream_compute_lanes[&f.stream], lane);
                assert!(!f.backend.submissions.contains_key(&f.id));
                let active = f.backend.active_compute_submission_v1(f.id).unwrap();
                assert_eq!(active.resident_descriptors, descriptors);
                let Some(ActiveComputeExecutionV1::MaterializedCancelling(root)) =
                    &active.execution
                else {
                    panic!("root retained")
                };
                assert_eq!(root.prepared.origin, origin);
                assert!(root.prepared.scripted.is_some());
                assert_eq!(root.scripted_returned, fault != Fault::BeforeReturn);
                assert!(root.returned.is_none());
                if let Some(before) = primary_before {
                    assert_eq!(format!("{:?}", f.backend.active), before);
                }
                assert!(f.backend.terminal);
                let remaining = f.backend.scripted_sdma.as_ref().unwrap().remaining_steps();
                for result in [
                    f.backend.cancel_v1(f.id).map(|_| ()),
                    f.backend.poll_v1(f.id).map(|_| ()),
                    f.backend.shutdown_native_v1(),
                ] {
                    assert!(matches!(result, Err(RuntimeBackendFailureV1::Terminal(_))));
                }
                assert_eq!(
                    f.backend.scripted_sdma.as_ref().unwrap().remaining_steps(),
                    remaining
                );
                discard_scripted_fixture(f.backend);
            }
        }
    }
}

#[test]
fn materialized_cancel_rejects_corrupt_custody_before_retirement() {
    for lane in 0..2 {
        for case in 0..12 {
            let mut f = Fixture::new(lane, ORIGINS[0], false);
            match case {
                0 => {
                    f.backend.compute_module_retain_counts.insert(f.module, 0);
                }
                1 => {
                    f.backend.allocation_custody.remove(&f.host);
                }
                2 => f.backend.compute_completion_reservations = 0,
                3 => {
                    f.backend.stream_compute_lanes.insert(f.stream, 1 - lane);
                }
                4 => {
                    f.backend.stream_submission_tails.remove(&f.stream);
                }
                5 => {
                    f.backend
                        .allocation_custody
                        .get_mut(&f.host)
                        .unwrap()
                        .owners[0]
                        .stream += 99
                }
                6 => {
                    f.backend
                        .allocation_custody
                        .get_mut(&f.host)
                        .unwrap()
                        .owner_counts[0] = 0
                }
                7 => {
                    let custody = f.backend.allocation_custody.get_mut(&f.host).unwrap();
                    custody.owners.push_back(custody.owners[0]);
                    custody.owner_counts[0] += 1;
                }
                8..=11 => f.backend.with_compute_lane_state_v1(lane, |backend| {
                    let descriptors = &mut backend.active.as_mut().unwrap().resident_descriptors;
                    match case {
                        8 => descriptors[0].alignment *= 2,
                        9 => descriptors[0].allocation_offset += 1,
                        10 => descriptors.push(descriptors[0]),
                        11 => descriptors[0].byte_len -= 1,
                        _ => unreachable!(),
                    }
                }),
                _ => unreachable!(),
            }
            f.backend.scripted_materialized_cancel_fault = Some(Fault::BeforeReturn);
            assert!(f.backend.cancel_v1(f.id).is_err());
            assert_eq!(
                f.backend.scripted_materialized_cancel_fault,
                Some(Fault::BeforeReturn)
            );
            assert!(matches!(
                f.backend
                    .active_compute_submission_v1(f.id)
                    .unwrap()
                    .execution,
                Some(ActiveComputeExecutionV1::MaterializedPrepared(_))
            ));
            discard_scripted_fixture(f.backend);
        }
    }
}

#[test]
fn materialized_cancel_native_layout_uses_host_page_alignment() {
    use super::super::materialized_cancellation::materialized_return_layout_matches_v1 as matches;
    use fe2o3_kfd::Gfx942FixedDispatchDataKindV1 as Kind;
    for alignment in [1, 8, 64, 4096] {
        let mut descriptor = ResidentDataDescriptorV1 {
            allocation: 1,
            kind: RuntimeMemoryKindV1::HostVisible,
            alignment,
            allocation_offset: 0,
            byte_len: 123,
            host_content_sha256: None,
            device_may_have_modified: false,
        };
        assert!(matches(&descriptor, Kind::HostVisibleCoherent, 123, 4096));
        assert!(!matches(&descriptor, Kind::HostVisibleCoherent, 124, 4096));
        assert!(!matches(&descriptor, Kind::DeviceLocal, 123, 4096));
        if alignment != 4096 {
            assert!(!matches(
                &descriptor,
                Kind::HostVisibleCoherent,
                123,
                alignment
            ));
        }
        descriptor.kind = RuntimeMemoryKindV1::DeviceLocal;
        assert!(matches(&descriptor, Kind::DeviceLocal, 123, alignment));
    }
}

#[test]
fn materialized_cancel_with_nonempty_pipeline_is_too_late_without_mutation() {
    let mut f = Fixture::new(1, ORIGINS[0], false);
    // Construct only the scheduler boundary; no native publication is claimed.
    let successor = f.backend.active.take().unwrap();
    f.backend.auxiliary_compute_lanes[0]
        .pipeline
        .insert_published(successor)
        .unwrap();
    let owners = f.backend.allocation_custody[&f.host].owners.clone();
    let reservations = f.backend.compute_completion_reservations;
    assert_eq!(
        f.backend.cancel_v1(f.id).unwrap(),
        crate::BackendCancellationV1::TooLate
    );
    assert_eq!(f.backend.allocation_custody[&f.host].owners, owners);
    assert_eq!(f.backend.compute_completion_reservations, reservations);
    assert!(matches!(
        f.backend
            .active_compute_submission_v1(f.id)
            .unwrap()
            .execution,
        Some(ActiveComputeExecutionV1::MaterializedPrepared(_))
    ));
    discard_scripted_fixture(f.backend);
}

#[test]
fn materialized_cancel_terminal_drop_aborts_with_indexed_custody() {
    use std::os::unix::process::ExitStatusExt;
    const CHILD: &str = "FE2O3_TEST_MATERIALIZED_CANCEL_DROP";
    const TEST: &str = "kfd_backend::tests::materialized_cancellation_tests::materialized_cancel_terminal_drop_aborts_with_indexed_custody";
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
        let mut f = Fixture::new(case / 6, ORIGINS[(case / 3) % 2], false);
        f.backend.scripted_materialized_cancel_fault = Some(
            [
                Fault::BeforeReturn,
                Fault::AfterReturn,
                Fault::WrongGeneration,
            ][case % 3],
        );
        let result = catch_unwind(AssertUnwindSafe(|| f.backend.cancel_v1(f.id)));
        assert!(matches!(result, Err(_) | Ok(Err(_))));
        assert!(f.backend.terminal);
        assert_eq!(f.backend.active_compute_lane_v1(f.id), Some(case / 6));
        assert!(matches!(
            f.backend
                .active_compute_submission_v1(f.id)
                .unwrap()
                .execution,
            Some(ActiveComputeExecutionV1::MaterializedCancelling(_))
        ));
        assert!(!f.backend.submissions.contains_key(&f.id));
        eprintln!("materialized cancellation custody inspected; dropping unrepaired backend");
        drop(ManuallyDrop::into_inner(f.backend));
        panic!("terminal Drop returned");
    }
    for case in 0..12 {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture"])
            .env(CHILD, case.to_string())
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.signal(), Some(6), "case {case}: {stderr}");
        assert!(
            stderr.contains(
                "materialized cancellation custody inspected; dropping unrepaired backend"
            ),
            "case {case}: {stderr}"
        );
    }
}
