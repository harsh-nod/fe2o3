//! Native copy/typed compute composition through real scripted storage owners.

use super::*;

#[derive(Clone, Copy)]
enum CopyKind {
    H2d,
    D2h,
    D2d,
}

struct Fixture {
    backend: KfdRuntimeBackendV1,
    stream: u64,
    copy_stream: u64,
    ready: [u64; 2],
    pair: (u64, u64),
    other: (u64, u64),
    module: u64,
    kernel: u64,
    copy: u64,
    event: u64,
    owner: u64,
}

impl Fixture {
    fn new(kind: CopyKind, publish_compute: bool) -> Self {
        Self::with_intermediate(kind, publish_compute, false)
    }

    fn with_intermediate(
        kind: CopyKind,
        publish_compute: bool,
        complete_intermediate: bool,
    ) -> Self {
        let mut steps = Vec::new();
        if matches!(kind, CopyKind::H2d) {
            steps.push(ScriptedSdmaStepV1::Write {
                offset: 0,
                byte_len: 4096,
            });
        }
        steps.push(match kind {
            CopyKind::H2d | CopyKind::D2h => scripted_submit_step_v1(
                if matches!(kind, CopyKind::H2d) {
                    Gfx942PersistentSdmaDirectionV1::HostToDevice
                } else {
                    Gfx942PersistentSdmaDirectionV1::DeviceToHost
                },
                0,
                0,
                2048,
                ScriptedFailureModeV1::Success,
            ),
            CopyKind::D2d => scripted_same_device_submit_window_step_v1(
                [SameDeviceSdmaCopyRequestV1 {
                    source_offset: 0,
                    destination_offset: 0,
                    copy_bytes: 2048,
                }],
                ScriptedFailureModeV1::Success,
            ),
        });
        if matches!(kind, CopyKind::D2d) {
            steps.extend([
                ScriptedSdmaStepV1::PollSameDevice(ScriptedSameDeviceExecutionOutcomeV1::Pending),
                ScriptedSdmaStepV1::PollSameDevice(
                    ScriptedSameDeviceExecutionOutcomeV1::Completed {
                        copy_bytes: None,
                        requests: None,
                        swap_allocations: false,
                    },
                ),
                ScriptedSdmaStepV1::RetireSameDevice(ScriptedFailureModeV1::Success),
            ]);
        } else {
            steps.extend([
                ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Pending),
                ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Completed {
                    direction: None,
                    copy_bytes: None,
                }),
                ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success),
            ]);
        }
        if complete_intermediate {
            steps.extend([
                scripted_submit_step_v1(
                    Gfx942PersistentSdmaDirectionV1::HostToDevice,
                    0,
                    0,
                    2048,
                    ScriptedFailureModeV1::Success,
                ),
                ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Completed {
                    direction: None,
                    copy_bytes: None,
                }),
                ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success),
            ]);
        }
        if publish_compute {
            steps.push(ScriptedSdmaStepV1::PromoteInitializedStorage(
                ScriptedFailureModeV1::Success,
            ));
        }
        steps.extend(scripted_same_device_release_steps_v1());
        steps.extend(scripted_release_steps_v1());
        steps.extend(scripted_release_steps_v1());
        let (mut backend, stream, ready) =
            scripted_persistent_backend_with_steps_v1::<2>(4096, steps);
        let copy_stream = backend.create_stream_v1(7).unwrap();
        let pair = add_scripted_direct_pair_v1(&mut backend, 4096);
        let other = add_scripted_direct_pair_v1(&mut backend, 4096);
        let owner = device_owner(&backend, pair.1).0;
        if matches!(kind, CopyKind::H2d) {
            backend
                .write_allocation_v1(pair.0, 0, &[0x5a; 4096])
                .unwrap();
        } else {
            let source = if matches!(kind, CopyKind::D2d) {
                other.1
            } else {
                pair.1
            };
            let KfdRuntimeSdmaStorageV1::Device(device) =
                &mut backend.allocations.get_mut(&source).unwrap().sdma_storage
            else {
                unreachable!()
            };
            device.scripted_bytes_mut().unwrap().fill(0x5a);
        }
        let (source, destination) = match kind {
            CopyKind::H2d => (pair.0, pair.1),
            CopyKind::D2h => (pair.1, pair.0),
            CopyKind::D2d => (other.1, pair.1),
        };
        let (source, destination) = scripted_copy_regions_v1(source, destination, 2048);
        let copy = backend
            .copy_async_v1(copy_stream, source, destination, &[])
            .unwrap();
        let event = backend.record_event_v1(copy_stream, copy).unwrap();
        let module = backend
            .load_module_v1(7, &synthetic_cov6::three_binding_module())
            .unwrap();
        let kernel = backend
            .resolve_kernel_v1(module, "vecadd", [7; 32])
            .unwrap();
        Self {
            backend,
            stream,
            copy_stream,
            ready,
            pair,
            other,
            module,
            kernel,
            copy,
            event,
            owner,
        }
    }

    fn submit(
        &mut self,
        stream: u64,
        deps: &[BackendLaunchProducerV1],
    ) -> Result<u64, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let bindings = [self.pair.1, self.ready[0], self.ready[1]].map(|allocation| {
            let index = if allocation == self.pair.1 {
                0
            } else if allocation == self.ready[0] {
                1
            } else {
                2
            };
            BackendBindingV1 {
                region: BackendMemoryRegionV1 {
                    allocation,
                    access: if index == 2 {
                        RuntimeAccessV1::Write
                    } else {
                        RuntimeAccessV1::Read
                    },
                    byte_offset: 0,
                    byte_len: 4096,
                },
                kernarg_byte_offset: index * 8,
            }
        });
        let mut explicit_kernarg = [0; 32];
        explicit_kernarg[24..].copy_from_slice(&1024_u64.to_le_bytes());
        self.backend
            .submit_producer_aware_launch_v1(BackendProducerAwareLaunchV1 {
                stream,
                kernel: self.kernel,
                explicit_kernarg: &explicit_kernarg,
                bindings: &bindings,
                dependencies: deps,
                geometry: crate::RuntimeLaunchGeometryV1 {
                    grid: [64, 1, 1],
                    workgroup: [64, 1, 1],
                    dynamic_shared_bytes: 0,
                },
            })
    }

    fn intermediate(&mut self, overlap: bool) -> u64 {
        let pair = if overlap { self.pair } else { self.other };
        let (source, destination) = scripted_copy_regions_v1(pair.0, pair.1, 2048);
        let id = self
            .backend
            .copy_async_v1(self.stream, source, destination, &[self.event])
            .unwrap();
        assert!(matches!(
            self.backend.active_sdma[&id].phase,
            ActiveSdmaPhaseV1::Ready
        ));
        id
    }

    fn finish_compute(&mut self, stream: u64, id: u64) {
        self.backend.flush_stream_v1(stream).unwrap();
        assert_eq!(
            self.backend
                .wait_v1(id, Instant::now() + Duration::from_secs(1))
                .unwrap(),
            BackendPollV1::Succeeded
        );
        assert_eq!(
            self.backend
                .last_launch_performance_v1()
                .unwrap()
                .user_data_materializations(),
            0
        );
        assert_eq!(device_owner(&self.backend, self.pair.1).0, self.owner);
        assert!(
            self.backend.allocations[&self.pair.1]
                .content_sha256
                .is_none()
        );
    }

    fn finish(mut self, submissions: &[u64]) {
        for &id in submissions {
            self.backend.release_submission_v1(id).unwrap();
        }
        self.backend.release_submission_v1(self.copy).unwrap();
        assert!(self.backend.compute_dependency_retain_counts.is_empty());
        assert!(self.backend.sdma_dependency_retain_counts.is_empty());
        assert!(self.backend.allocation_custody.is_empty());
        self.backend.unload_module_v1(self.module).unwrap();
        for id in self.ready {
            self.backend.allocations.get_mut(&id).unwrap().sdma_backed = false;
            self.backend.release_allocation_v1(id).unwrap();
        }
        release_scripted_direct_pair_v1(&mut self.backend, self.pair.0, self.pair.1);
        release_scripted_direct_pair_v1(&mut self.backend, self.other.0, self.other.1);
        self.backend.destroy_stream_v1(self.stream).unwrap();
        self.backend.destroy_stream_v1(self.copy_stream).unwrap();
        let driver = self.backend.scripted_sdma.as_ref().unwrap();
        assert!(driver.is_exhausted());
        assert_eq!(driver.live_owner_count(), 0);
        assert_eq!(driver.unexpected_drops(), 0);
        self.backend.shutdown_native_v1().unwrap();
    }
}

fn device_owner(backend: &KfdRuntimeBackendV1, allocation: u64) -> (u64, &[u8]) {
    let device = match &backend.allocations[&allocation].sdma_storage {
        KfdRuntimeSdmaStorageV1::Device(device) => device.as_ref(),
        KfdRuntimeSdmaStorageV1::InitializedStorage(storage) => match storage.as_ref() {
            InitializedStorageOwnerV1::Scripted(device) => device,
            _ => unreachable!(),
        },
        _ => panic!("expected exact restored scripted storage"),
    };
    (
        device.scripted_owner_id().unwrap(),
        device.scripted_bytes().unwrap(),
    )
}

#[test]
fn native_sdma_cancelled_intermediate_preserves_explicit_failure_and_fifo_quiescence() {
    for explicit in [false, true] {
        let mut f = ManuallyDrop::new(Fixture::new(CopyKind::H2d, !explicit));
        let b = f.intermediate(false);
        let b_stream = f.stream;
        let event = f.backend.record_event_v1(b_stream, b).unwrap();
        let stream = if explicit {
            f.backend.create_stream_v1(7).unwrap()
        } else {
            b_stream
        };
        let deps = [BackendLaunchProducerV1 {
            event,
            producer_submission: b,
        }];
        let c = f
            .submit(stream, if explicit { &deps } else { &[] })
            .unwrap();
        assert_eq!(
            &*f.backend.pending_compute[&c].quiescence_dependencies,
            &[f.copy]
        );
        assert_eq!(
            f.backend.pending_compute[&c].dependency_depth,
            if explicit { 3 } else { 1 }
        );
        let a_event = f.event;
        f.backend.release_event_v1(a_event).unwrap();
        f.backend.release_event_v1(event).unwrap();
        assert_eq!(
            f.backend.cancel_v1(b).unwrap(),
            crate::BackendCancellationV1::Cancelled
        );
        assert!(f.backend.sdma_dependency_retain_counts.is_empty());
        assert_eq!(f.backend.compute_dependency_retain_counts[&f.copy], 1);
        let copy = f.copy;
        assert!(
            matches!(f.backend.release_submission_v1(copy), Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Busy)
        );
        if explicit {
            assert!(matches!(
                f.backend.poll_v1(c).unwrap(),
                BackendPollV1::Failed { .. }
            ));
            assert!(f.backend.active_sdma.contains_key(&copy));
            assert!(
                !f.backend
                    .compute_dependency_retain_counts
                    .contains_key(&copy)
            );
            assert_eq!(f.backend.poll_v1(copy).unwrap(), BackendPollV1::Pending);
            assert_eq!(f.backend.poll_v1(copy).unwrap(), BackendPollV1::Succeeded);
            f.backend.destroy_stream_v1(stream).unwrap();
        } else {
            let steps = f.backend.scripted_sdma.as_ref().unwrap().remaining_steps();
            assert_eq!(f.backend.poll_v1(c).unwrap(), BackendPollV1::Pending);
            assert_eq!(
                f.backend.scripted_sdma.as_ref().unwrap().remaining_steps(),
                steps - 1
            );
            assert_eq!(f.backend.pending_compute[&c].quiescence_cursor, 0);
            assert_eq!(f.backend.poll_v1(c).unwrap(), BackendPollV1::Pending);
            assert!(f.backend.active.is_none());
            assert_eq!(f.backend.compute_dependency_retain_counts[&copy], 1);
            let (_, bytes) = device_owner(&f.backend, f.pair.1);
            assert_eq!(&bytes[..2048], &[0x5a; 2048]);
            assert_eq!(&bytes[2048..], &[0; 2048]);
            f.finish_compute(stream, c);
            let (_, bytes) = device_owner(&f.backend, f.pair.1);
            assert_eq!(&bytes[..2048], &[0x5a; 2048]);
            assert_eq!(&bytes[2048..], &[0; 2048]);
        }
        ManuallyDrop::into_inner(f).finish(&[c, b]);
    }
}

#[test]
fn native_sdma_typed_inputs_wait_for_directional_and_same_device_owners() {
    for kind in [CopyKind::H2d, CopyKind::D2h, CopyKind::D2d] {
        for fifo in [false, true] {
            let mut f = ManuallyDrop::new(Fixture::new(kind, true));
            let stream = if fifo { f.copy_stream } else { f.stream };
            let deps = [BackendLaunchProducerV1 {
                event: f.event,
                producer_submission: f.copy,
            }];
            let c = f.submit(stream, if fifo { &[] } else { &deps }).unwrap();
            assert!(
                f.backend.pending_compute[&c]
                    .quiescence_dependencies
                    .is_empty()
            );
            let event = f.event;
            f.backend.release_event_v1(event).unwrap();
            assert_eq!(f.backend.poll_v1(c).unwrap(), BackendPollV1::Pending);
            assert_eq!(f.backend.poll_v1(c).unwrap(), BackendPollV1::Pending);
            assert!(f.backend.active.is_none());
            f.finish_compute(stream, c);
            let (_, bytes) = device_owner(&f.backend, f.pair.1);
            assert_eq!(&bytes[..2048], &[0x5a; 2048]);
            assert_eq!(
                &bytes[2048..],
                &[if matches!(kind, CopyKind::D2h) {
                    0x5a
                } else {
                    0
                }; 2048]
            );
            ManuallyDrop::into_inner(f).finish(&[c]);
        }
    }
}

#[test]
fn native_sdma_unordered_binding_rejects_without_admission_and_cancel_refunds() {
    let mut f = ManuallyDrop::new(Fixture::new(CopyKind::H2d, false));
    let stream = f.stream;
    let next = f.backend.next_handle;
    assert!(
        matches!(f.submit(stream, &[]), Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Busy)
    );
    assert_eq!(f.backend.next_handle, next);
    assert!(f.backend.compute_dependency_retain_counts.is_empty());
    let b = f.intermediate(false);
    let c = f.submit(stream, &[]).unwrap();
    assert_eq!(
        &*f.backend.pending_compute[&c].quiescence_dependencies,
        &[f.copy]
    );
    assert_eq!(
        f.backend.cancel_v1(c).unwrap(),
        crate::BackendCancellationV1::Cancelled
    );
    assert!(f.backend.compute_dependency_retain_counts.is_empty());
    assert_eq!(
        f.backend.cancel_v1(b).unwrap(),
        crate::BackendCancellationV1::Cancelled
    );
    let event = f.event;
    f.backend.release_event_v1(event).unwrap();
    let copy = f.copy;
    assert_eq!(f.backend.poll_v1(copy).unwrap(), BackendPollV1::Pending);
    assert_eq!(f.backend.poll_v1(copy).unwrap(), BackendPollV1::Succeeded);
    ManuallyDrop::into_inner(f).finish(&[c, b]);
}

#[test]
fn native_sdma_successful_intermediate_authenticates_ready_shared_storage() {
    for overlap in [false, true] {
        let mut f = ManuallyDrop::new(Fixture::with_intermediate(CopyKind::H2d, true, true));
        let b = f.intermediate(overlap);
        let b_stream = f.stream;
        let event = f.backend.record_event_v1(b_stream, b).unwrap();
        let stream = f.backend.create_stream_v1(7).unwrap();
        let c = f
            .submit(
                stream,
                &[BackendLaunchProducerV1 {
                    event,
                    producer_submission: b,
                }],
            )
            .unwrap();
        assert_eq!(
            &*f.backend.pending_compute[&c].quiescence_dependencies,
            &[f.copy]
        );
        let a_event = f.event;
        f.backend.release_event_v1(a_event).unwrap();
        f.backend.release_event_v1(event).unwrap();
        assert_eq!(f.backend.poll_v1(c).unwrap(), BackendPollV1::Pending);
        assert_eq!(f.backend.poll_v1(c).unwrap(), BackendPollV1::Pending);
        assert!(matches!(
            f.backend.active_sdma[&b].phase,
            ActiveSdmaPhaseV1::Ready
        ));
        f.backend.flush_stream_v1(b_stream).unwrap();
        assert_eq!(f.backend.poll_v1(c).unwrap(), BackendPollV1::Pending);
        assert_eq!(f.backend.submissions[&b].status, BackendPollV1::Succeeded);
        assert!(f.backend.active.is_none());
        f.finish_compute(stream, c);
        let (_, bytes) = device_owner(&f.backend, f.pair.1);
        assert_eq!(&bytes[..2048], &[0x5a; 2048]);
        assert_eq!(&bytes[2048..], &[0; 2048]);
        f.backend.release_submission_v1(c).unwrap();
        f.backend.destroy_stream_v1(stream).unwrap();
        ManuallyDrop::into_inner(f).finish(&[b]);
    }
}

#[test]
fn native_sdma_inconsistent_endpoint_or_publication_custody_is_terminal_before_admission() {
    for fault in 0..13 {
        let mut f = ManuallyDrop::new(Fixture::new(CopyKind::H2d, false));
        let copy = f.copy;
        let stream = f.stream;
        let copy_stream = f.copy_stream;
        let host = f.pair.0;
        let device = f.pair.1;
        let original_owners = f.backend.allocation_custody[&host].owners.clone();
        let original_counts = f.backend.allocation_custody[&host].owner_counts;
        let original_stream = f.backend.allocation_custody[&host].sole_stream;
        let original_queue = f.backend.active_sdma_streams[&copy_stream].clone();
        let published = f.backend.published_sdma_submissions.clone();
        let window = f.backend.active_sdma[&copy].window_bytes;
        let mut requests = None;
        match fault {
            0 => {
                f.backend
                    .allocation_custody
                    .get_mut(&host)
                    .unwrap()
                    .owners
                    .clear();
            }
            1 => {
                let owner = original_owners[0];
                f.backend
                    .allocation_custody
                    .get_mut(&host)
                    .unwrap()
                    .owners
                    .push_back(owner);
            }
            2 => {
                f.backend
                    .allocation_custody
                    .get_mut(&host)
                    .unwrap()
                    .owner_counts[1] += 1;
            }
            3 => {
                f.backend
                    .allocation_custody
                    .get_mut(&host)
                    .unwrap()
                    .sole_stream = Some(stream);
            }
            4 => {
                f.backend.allocations.get_mut(&host).unwrap().sdma_storage =
                    KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Async(copy + 1));
            }
            5 => {
                f.backend.published_sdma_submissions.clear();
            }
            6 => {
                f.backend.published_sdma_submissions.push(copy);
            }
            7 => {
                f.backend
                    .active_sdma_streams
                    .get_mut(&copy_stream)
                    .unwrap()
                    .clear();
            }
            8 => {
                f.backend
                    .active_sdma
                    .get_mut(&copy)
                    .unwrap()
                    .dependency_cursor = 1;
            }
            9 => {
                f.backend.active_sdma.get_mut(&copy).unwrap().window_bytes = 0;
            }
            10 => {
                requests = f
                    .backend
                    .active_sdma
                    .get_mut(&copy)
                    .unwrap()
                    .window_requests
                    .take();
            }
            11 => {
                f.backend.allocations.get_mut(&device).unwrap().sdma_storage =
                    KfdRuntimeSdmaStorageV1::ComputeInFlight(copy);
            }
            _ => {
                f.backend.stream_submission_tails.insert(stream, copy);
            }
        }
        let next = f.backend.next_handle;
        let steps = f.backend.scripted_sdma.as_ref().unwrap().remaining_steps();
        let deps = [BackendLaunchProducerV1 {
            event: f.event,
            producer_submission: copy,
        }];
        assert!(
            matches!(
                f.submit(stream, &deps),
                Err(RuntimeBackendFailureV1::Terminal(_))
            ),
            "fault {fault}"
        );
        assert!(f.backend.terminal);
        assert_eq!(f.backend.next_handle, next);
        assert!(f.backend.pending_compute.is_empty());
        assert!(f.backend.compute_dependency_retain_counts.is_empty());
        assert_eq!(
            f.backend.scripted_sdma.as_ref().unwrap().remaining_steps(),
            steps
        );
        // Only test metadata was corrupted; no native transition occurred. Restore it for cleanup.
        let custody = f.backend.allocation_custody.get_mut(&host).unwrap();
        custody.owners = original_owners;
        custody.owner_counts = original_counts;
        custody.sole_stream = original_stream;
        f.backend
            .active_sdma_streams
            .insert(copy_stream, original_queue);
        f.backend.published_sdma_submissions = published;
        let active = f.backend.active_sdma.get_mut(&copy).unwrap();
        active.dependency_cursor = 0;
        active.window_bytes = window;
        if requests.is_some() {
            active.window_requests = requests;
        }
        for id in [host, device] {
            f.backend.allocations.get_mut(&id).unwrap().sdma_storage =
                KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Async(copy));
        }
        f.backend.terminal = false;
        f.backend.stream_submission_tails.remove(&stream);
        let event = f.event;
        f.backend.release_event_v1(event).unwrap();
        assert_eq!(f.backend.poll_v1(copy).unwrap(), BackendPollV1::Pending);
        assert_eq!(f.backend.poll_v1(copy).unwrap(), BackendPollV1::Succeeded);
        ManuallyDrop::into_inner(f).finish(&[]);
    }
}
