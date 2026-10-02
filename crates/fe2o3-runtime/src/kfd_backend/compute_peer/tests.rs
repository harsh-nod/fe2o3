use super::*;
use crate::BackendCancellationV1;
use crate::kfd_backend::compute_xgmi::{Route, Stage};
use crate::kfd_backend::kfd_backend_sdma_seam::ScriptedExecutionOutcomeV1;
use std::mem::ManuallyDrop;
use std::panic::{AssertUnwindSafe, catch_unwind};

const BYTES: usize = 64;
const ALLOCATIONS: usize = 10;

fn region(allocation: u64, access: RuntimeAccessV1) -> BackendMemoryRegionV1 {
    BackendMemoryRegionV1 {
        allocation,
        access,
        byte_offset: 0,
        byte_len: BYTES as u64,
    }
}

fn host_steps() -> [ScriptedSdmaStepV1; 2] {
    [
        ScriptedSdmaStepV1::Allocate {
            kind: ScriptedBufferKindV1::Host,
            byte_len: BYTES,
        },
        ScriptedSdmaStepV1::Write {
            offset: 0,
            byte_len: BYTES,
        },
    ]
}

fn release_steps() -> Vec<ScriptedSdmaStepV1> {
    let mut steps: Vec<_> = host_steps().into_iter().collect();
    steps.extend([
        ScriptedSdmaStepV1::Submit {
            direction: Gfx942PersistentSdmaDirectionV1::HostToDevice,
            host_offset: 0,
            device_offset: 0,
            copy_bytes: BYTES as u32,
            outcome: ScriptedFailureModeV1::Success,
        },
        ScriptedSdmaStepV1::Wait(ScriptedExecutionOutcomeV1::Completed {
            direction: None,
            copy_bytes: None,
        }),
        ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success),
        ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
        ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
        ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
    ]);
    steps
}

fn readback_steps() -> Vec<ScriptedSdmaStepV1> {
    let mut steps: Vec<_> = host_steps().into_iter().collect();
    steps.extend([
        ScriptedSdmaStepV1::Submit {
            direction: Gfx942PersistentSdmaDirectionV1::DeviceToHost,
            host_offset: 0,
            device_offset: 0,
            copy_bytes: BYTES as u32,
            outcome: ScriptedFailureModeV1::Success,
        },
        ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Pending),
        ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Completed {
            direction: None,
            copy_bytes: None,
        }),
        ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success),
        ScriptedSdmaStepV1::Read {
            offset: 0,
            byte_len: BYTES as u64,
        },
        ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
    ]);
    steps.extend(host_steps());
    steps.extend([
        ScriptedSdmaStepV1::Write {
            offset: 0,
            byte_len: BYTES,
        },
        ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
    ]);
    steps
}

struct Fixture {
    backend: ManuallyDrop<KfdMultiDeviceRuntimeBackendV1>,
    allocations: [[u64; ALLOCATIONS]; 2],
    compute_streams: [u64; 2],
    peer_stream: u64,
    readback_stream: u64,
    kernels: [u64; 2],
    modules: [u64; 2],
    host: Option<u64>,
    setup_predecessors: Vec<u64>,
}

impl Fixture {
    fn new(readback: bool) -> Self {
        let mut local_allocations = [[0; ALLOCATIONS]; 2];
        let mut local_streams = [0; 2];
        let children = (0..2)
            .map(|index| {
                let mut steps = if readback && index == 1 {
                    readback_steps()
                } else {
                    Vec::new()
                };
                steps.extend((0..ALLOCATIONS).flat_map(|_| release_steps()));
                if readback && index == 1 {
                    steps.push(ScriptedSdmaStepV1::Recycle(
                        ScriptedRecycleOutcomeV1::Success,
                    ));
                }
                let (mut child, stream, allocations) =
                    scripted_persistent_backend_with_steps_v1::<ALLOCATIONS>(BYTES, steps);
                child.description.backend_device = 7 + index as u64;
                for record in child.allocations.values_mut() {
                    record.device = 7 + index as u64;
                }
                for device in child.streams.values_mut() {
                    *device = 7 + index as u64;
                }
                child.peer_visible_device_allocations = true;
                child.scripted_persistent_poll_pending_observations = 3;
                local_allocations[index] = allocations;
                local_streams[index] = stream;
                child
            })
            .collect();
        // Preserve uncertain scripted custody on assertion failures, just as the
        // native failure tests retain their real roots until process exit.
        let mut backend =
            ManuallyDrop::new(KfdMultiDeviceRuntimeBackendV1::from_backends(children).unwrap());
        let allocations = std::array::from_fn(|child| {
            local_allocations[child].map(|local| {
                let global = backend.next_id().unwrap();
                backend
                    .allocations
                    .insert(global, RoutedHandleV1 { child, local });
                global
            })
        });
        let compute_streams = std::array::from_fn(|child| {
            let global = backend.next_id().unwrap();
            backend.streams.insert(
                global,
                RoutedHandleV1 {
                    child,
                    local: local_streams[child],
                },
            );
            global
        });
        let modules = std::array::from_fn(|index| {
            backend
                .load_module_v1(7 + index as u64, &synthetic_cov6::three_binding_module())
                .unwrap()
        });
        let kernels = modules.map(|module| {
            backend
                .resolve_kernel_v1(module, "vecadd", [7; 32])
                .unwrap()
        });
        let peer_stream = backend.create_stream_v1(8).unwrap();
        let readback_stream = backend.create_stream_v1(8).unwrap();
        let host = readback.then(|| {
            let child = &mut backend.children[1];
            // Only host storage is added here; computed output is never installed.
            let local = child.next_id().unwrap();
            let owner = child.scripted_sdma.as_ref().unwrap().test_host_owner(BYTES);
            child.allocations.insert(
                local,
                AllocationRecordV1 {
                    device: 8,
                    kind: RuntimeMemoryKindV1::HostVisible,
                    alignment: 8,
                    bytes: vec![0; BYTES].into(),
                    content_sha256: None,
                    last_full_host_write: None,
                    native_dirty: Vec::new(),
                    sdma_storage: KfdRuntimeSdmaStorageV1::Host(owner),
                    sdma_backed: true,
                    sdma_initialized: true,
                    sdma_shadow_dirty: false,
                    persistent_storage_restore: None,
                    scripted_three_binding_replay: false,
                },
            );
            child.staged_context_bytes += BYTES as u64;
            let global = backend.next_id().unwrap();
            backend
                .allocations
                .insert(global, RoutedHandleV1 { child: 1, local });
            global
        });
        backend.compute_xgmi_routes.insert(
            (0, 1),
            Route::Scripted {
                failure: None,
                unwind: false,
                pending_samples: 2,
            },
        );
        Self {
            backend,
            allocations,
            compute_streams,
            peer_stream,
            readback_stream,
            kernels,
            modules,
            host,
            setup_predecessors: Vec::new(),
        }
    }

    fn launch(&mut self, child: usize, first: usize, queued: bool, aware: bool) -> u64 {
        let globals: [u64; 3] = std::array::from_fn(|index| self.allocations[child][first + index]);
        if queued
            && !self.backend.children[child].any_compute_active_v1()
            && self.backend.children[child].pending_compute.is_empty()
        {
            // A real, disjoint persistent FIFO predecessor keeps the target
            // queued without changing its authenticated input readiness.
            let predecessor = self.launch(child, 7, false, false);
            self.setup_predecessors.push(predecessor);
        }
        let bindings: [_; 3] = std::array::from_fn(|index| BackendBindingV1 {
            region: region(
                globals[index],
                if index == 2 {
                    RuntimeAccessV1::Write
                } else {
                    RuntimeAccessV1::Read
                },
            ),
            kernarg_byte_offset: (index * 8) as u32,
        });
        let mut kernarg = [0; 32];
        kernarg[24..].copy_from_slice(&16_u64.to_le_bytes());
        let geometry = crate::RuntimeLaunchGeometryV1 {
            grid: [64, 1, 1],
            workgroup: [64, 1, 1],
            dynamic_shared_bytes: 0,
        };
        if aware {
            self.backend
                .submit_producer_aware_launch_v1(BackendProducerAwareLaunchV1 {
                    stream: self.compute_streams[child],
                    kernel: self.kernels[child],
                    explicit_kernarg: &kernarg,
                    bindings: &bindings,
                    dependencies: &[],
                    geometry,
                })
                .unwrap()
        } else {
            self.backend
                .submit_v1(BackendLaunchV1 {
                    stream: self.compute_streams[child],
                    kernel: self.kernels[child],
                    explicit_kernarg: &kernarg,
                    bindings: &bindings,
                    dependencies: &[],
                    geometry,
                    semantic_launch: KfdRuntimeSemanticLaunchV1::Ordinary,
                })
                .unwrap()
        }
    }

    fn native(&self, id: u64) -> RoutedHandleV1 {
        let RoutedSubmissionV1::Native { route, .. } = self.backend.submissions[&id] else {
            panic!("native compute expected");
        };
        route
    }

    fn event(&mut self, child: usize, id: u64) -> u64 {
        self.backend
            .record_event_v1(self.compute_streams[child], id)
            .unwrap()
    }

    fn peer(&mut self, events: &[u64]) -> u64 {
        self.backend
            .peer_copy_v1(
                self.peer_stream,
                region(self.allocations[0][2], RuntimeAccessV1::Read),
                region(self.allocations[1][3], RuntimeAccessV1::Write),
                events,
            )
            .unwrap()
    }

    fn copy(&self, id: u64) -> &CooperativeCopySubmissionV1 {
        let RoutedSubmissionV1::CooperativeCopy(copy) = &self.backend.submissions[&id] else {
            panic!("cooperative root expected");
        };
        copy
    }

    fn drive(&mut self, stream: u64, id: u64) {
        for _ in 0..64 {
            self.backend.flush_stream_v1(stream).unwrap();
            if self.backend.poll_v1(id).unwrap() != BackendPollV1::Pending {
                return;
            }
        }
        panic!("bounded scripted progress did not settle {id}");
    }

    fn clean(mut self) {
        for id in self.setup_predecessors.clone() {
            let RoutedSubmissionV1::Native { stream, .. } = self.backend.submissions[&id] else {
                panic!("setup predecessor remains native");
            };
            if self.backend.poll_v1(id).unwrap() == BackendPollV1::Pending {
                self.drive(stream, id);
            }
            assert_eq!(self.backend.poll_v1(id).unwrap(), BackendPollV1::Succeeded);
        }
        let mut events: Vec<_> = self.backend.events.keys().copied().collect();
        events.sort_unstable();
        for event in events.into_iter().rev() {
            self.backend.release_event_v1(event).unwrap();
        }
        let mut submissions: Vec<_> = self.backend.submissions.keys().copied().collect();
        submissions.sort_unstable();
        for id in submissions.into_iter().rev() {
            self.backend.release_submission_v1(id).unwrap();
        }
        assert!(self.backend.producer_aware_native.is_empty());
        assert!(self.backend.cooperative_dependency_retain_counts.is_empty());
        for allocations in self.allocations {
            for allocation in allocations {
                self.backend.release_allocation_v1(allocation).unwrap();
            }
        }
        if let Some(host) = self.host {
            self.backend.release_allocation_v1(host).unwrap();
        }
        for module in self.modules {
            self.backend.unload_module_v1(module).unwrap();
        }
        let streams: Vec<_> = self.backend.streams.keys().copied().collect();
        for stream in streams {
            self.backend.destroy_stream_v1(stream).unwrap();
        }
        for child in &self.backend.children {
            let driver = child.scripted_sdma.as_ref().unwrap();
            assert!(
                driver.is_exhausted(),
                "remaining {}",
                driver.remaining_steps()
            );
            assert_eq!(driver.live_owner_count(), 0);
            assert_eq!(driver.unexpected_drops(), 0);
        }
        self.backend.shutdown_native_v1().unwrap();
        drop(ManuallyDrop::into_inner(self.backend));
    }
}

#[test]
fn compute_peer_queued_and_eager_persistent_keep_exact_owners_and_one_payload_charge() {
    for queued in [false, true] {
        let mut f = Fixture::new(false);
        let account = fe2o3_resource_accounting::ResourceCreditAccountV1::new(
            fe2o3_resource_accounting::ResourceVectorV1::ZERO.with(
                fe2o3_resource_accounting::ResourceKindV1::ControlResidentBytes,
                4096,
            ),
            4,
        )
        .unwrap();
        f.backend.children[0].launch_payload_account = Some(account.clone());
        let producer = f.launch(0, 0, queued, true);
        let route = f.native(producer);
        assert_eq!(
            f.backend.children[0]
                .pending_compute
                .contains_key(&route.local),
            queued
        );
        let original = if queued {
            let source = f.backend.allocations[&f.allocations[0][2]];
            let KfdRuntimeSdmaStorageV1::H2dReady(ready) =
                &f.backend.children[0].allocations[&source.local].sdma_storage
            else {
                panic!("queued producer retains original authenticated storage");
            };
            ready.owner.scripted_owner_id()
        } else {
            let active = f.backend.children[0]
                .active_compute_submission_v1(route.local)
                .unwrap();
            assert!(
                active.ordinary_recipe.is_none(),
                "exercise the real persistent eager path"
            );
            let Some(ActiveComputeExecutionV1::ScriptedThreeBindingPersistent { devices, .. }) =
                active.execution.as_ref()
            else {
                panic!("persistent producer retains actual scripted custody");
            };
            devices[2].scripted_owner_id()
        };
        let charged = account.usage();
        assert_eq!(charged.retained_records, 1);
        let destination = f.backend.allocations[&f.allocations[1][3]];
        let KfdRuntimeSdmaStorageV1::H2dReady(ready) =
            &f.backend.children[1].allocations[&destination.local].sdma_storage
        else {
            panic!("initial destination owner expected");
        };
        let destination_owner = ready.owner.scripted_owner_id();
        let event = f.event(0, producer);
        let peer = f.peer(&[event]);
        assert!(f.copy(peer).compute_producer.is_some());
        assert!(f.copy(peer).compute_xgmi.is_some());
        assert!(f.copy(peer).staging.is_empty());
        assert_eq!(account.usage(), charged);
        f.backend.release_event_v1(event).unwrap();
        assert!(f.backend.release_submission_v1(producer).is_err());
        for _ in 0..3 {
            assert_eq!(f.backend.poll_v1(peer).unwrap(), BackendPollV1::Pending);
            assert_eq!(
                f.backend.wait_v1(peer, Instant::now()).unwrap(),
                BackendPollV1::Pending
            );
        }
        assert_eq!(
            f.backend.drain_v1(peer, Instant::now()).unwrap(),
            BackendPollV1::Pending
        );
        assert_eq!(f.backend.completed_compute_xgmi_copies, 0);
        assert!(f.backend.compute_xgmi_children.iter().all(Option::is_none));
        f.drive(f.peer_stream, peer);
        assert_eq!(f.backend.poll_v1(peer).unwrap(), BackendPollV1::Succeeded);
        assert!(f.backend.children[0].exact_submission_quiescent_v1(route.local));
        let performance = f.backend.children[0].last_launch_performance_v1().unwrap();
        assert_eq!(
            performance.data_path(),
            KfdRuntimeLaunchDataPathV1::PersistentDeviceReused
        );
        assert_eq!(performance.user_data_materializations(), 0);
        let source = f.backend.allocations[&f.allocations[0][2]];
        let KfdRuntimeSdmaStorageV1::Device(owner) =
            &f.backend.children[0].allocations[&source.local].sdma_storage
        else {
            panic!("original owner must be restored");
        };
        assert_eq!(owner.scripted_owner_id(), original);
        let KfdRuntimeSdmaStorageV1::Device(owner) =
            &f.backend.children[1].allocations[&destination.local].sdma_storage
        else {
            panic!("original destination owner must be restored");
        };
        assert_eq!(owner.scripted_owner_id(), destination_owner);
        assert_eq!(f.backend.completed_compute_xgmi_copies, 0);
        assert!(f.backend.compute_xgmi_children.iter().all(Option::is_none));
        assert!(f.copy(peer).compute_producer.is_none());
        assert_eq!(account.usage(), charged);
        f.backend.release_submission_v1(peer).unwrap();
        f.backend.release_submission_v1(producer).unwrap();
        assert_eq!(
            account.usage().used,
            fe2o3_resource_accounting::ResourceVectorV1::ZERO
        );
        f.clean();
    }
}

#[test]
fn compute_peer_final_readback_stream_alone_drives_pending_compute_and_native_restore() {
    let mut f = Fixture::new(true);
    let producer = f.launch(0, 0, true, true);
    let event = f.event(0, producer);
    let peer = f.peer(&[event]);
    let peer_event = f.backend.record_event_v1(f.peer_stream, peer).unwrap();
    let readback = f
        .backend
        .copy_async_v1(
            f.readback_stream,
            region(f.allocations[1][3], RuntimeAccessV1::Read),
            region(f.host.unwrap(), RuntimeAccessV1::Write),
            &[peer_event],
        )
        .unwrap();
    f.backend.release_event_v1(peer_event).unwrap();
    f.backend.release_event_v1(event).unwrap();
    let before = f.backend.children[1]
        .scripted_sdma
        .as_ref()
        .unwrap()
        .remaining_steps();
    assert_eq!(f.backend.poll_v1(readback).unwrap(), BackendPollV1::Pending);
    assert_eq!(
        f.backend.wait_v1(readback, Instant::now()).unwrap(),
        BackendPollV1::Pending
    );
    assert_eq!(
        f.backend.children[1]
            .scripted_sdma
            .as_ref()
            .unwrap()
            .remaining_steps(),
        before
    );
    f.drive(f.readback_stream, readback);
    for id in [producer, peer, readback] {
        assert_eq!(f.backend.poll_v1(id).unwrap(), BackendPollV1::Succeeded);
    }
    assert_eq!(f.backend.completed_compute_xgmi_copies, 0);
    assert!(f.backend.compute_xgmi_children.iter().all(Option::is_none));
    // Completion and owner restoration are real scripted transitions; this is
    // deliberately not a CPU emulation or qualification of vecadd output bytes.
    f.clean();
}

#[test]
fn compute_peer_cancel_before_native_custody_and_failed_producer_release_retains() {
    for cancel_peer in [false, true] {
        let mut f = Fixture::new(false);
        let producer = f.launch(0, 0, true, true);
        let event = f.event(0, producer);
        let peer = f.peer(&[event]);
        f.backend.release_event_v1(event).unwrap();
        if cancel_peer {
            assert_eq!(
                f.backend.cancel_v1(peer).unwrap(),
                BackendCancellationV1::Cancelled
            );
        }
        assert_eq!(
            f.backend.cancel_v1(producer).unwrap(),
            BackendCancellationV1::Cancelled
        );
        if !cancel_peer {
            assert!(matches!(
                f.backend.flush_stream_v1(f.peer_stream),
                Err(RuntimeBackendFailureV1::Quiescent(error))
                    if error.kind() == KfdRuntimeBackendErrorKindV1::Native
                        && error.detail()
                            == "multi-device cooperative flush ended in quiescent failure"
            ));
            assert!(matches!(
                f.backend.poll_v1(peer).unwrap(),
                BackendPollV1::Failed { .. }
            ));
        }
        assert!(f.copy(peer).is_quiescent());
        assert!(f.copy(peer).compute_xgmi.as_ref().unwrap().is_quiescent());
        assert!(
            !f.backend
                .cooperative_dependency_retain_counts
                .contains_key(&producer)
        );
        assert_eq!(f.backend.completed_compute_xgmi_copies, 0);
        assert!(f.backend.compute_xgmi_children.iter().all(Option::is_none));
        assert!(f.copy(peer).compute_producer.is_none());
        f.clean();
    }
}

#[test]
fn compute_peer_completed_producer_stream_can_retire_without_publishing_trailing_compute() {
    let mut f = Fixture::new(false);
    let producer = f.launch(0, 0, true, true);
    let trailing = f.launch(0, 4, true, true);
    let target = f.native(producer);
    let tail = f.native(trailing);
    let event = f.event(0, producer);
    let peer = f.peer(&[event]);
    f.backend.release_event_v1(event).unwrap();
    for _ in 0..16 {
        f.backend.progress_cooperative_copy_step_v1(peer).unwrap();
        if f.backend.children[0].exact_submission_quiescent_v1(target.local) {
            break;
        }
    }
    assert!(f.backend.children[0].exact_submission_quiescent_v1(target.local));
    assert!(
        f.backend.children[0]
            .pending_compute
            .contains_key(&tail.local)
    );
    assert!(
        f.backend.children[0]
            .active_compute_submission_v1(tail.local)
            .is_none()
    );
    assert_eq!(
        f.backend.cancel_v1(trailing).unwrap(),
        BackendCancellationV1::Cancelled
    );
    f.backend.destroy_stream_v1(f.compute_streams[0]).unwrap();
    f.drive(f.peer_stream, peer);
    assert_eq!(f.backend.poll_v1(peer).unwrap(), BackendPollV1::Succeeded);
    f.clean();
}

#[test]
fn compute_peer_unsupported_profiles_reject_without_staging_or_owner_extraction() {
    for case in 0..5 {
        let mut f = Fixture::new(false);
        let producer = f.launch(0, 0, true, case != 0);
        let event = f.event(0, producer);
        let mut source = region(f.allocations[0][2], RuntimeAccessV1::Read);
        let mut destination = region(f.allocations[1][3], RuntimeAccessV1::Write);
        if case == 1 {
            source.byte_len -= 1;
            destination.byte_len -= 1;
        }
        if case == 2 {
            f.backend.children[1].peer_visible_device_allocations = false;
        }
        if case == 3 {
            source.allocation = f.allocations[0][0];
        }
        let before = (f.backend.next_handle, f.backend.submissions.len());
        let result = f.backend.peer_copy_v1(
            f.peer_stream,
            source,
            destination,
            if case == 4 {
                &[]
            } else {
                std::slice::from_ref(&event)
            },
        );
        assert!(
            matches!(result, Err(RuntimeBackendFailureV1::Rejected(_))),
            "case {case}: {result:?}"
        );
        assert_eq!((f.backend.next_handle, f.backend.submissions.len()), before);
        assert!(f.backend.compute_xgmi_children.iter().all(Option::is_none));
        assert_eq!(f.backend.completed_compute_xgmi_copies, 0);
        assert_eq!(
            f.backend.cancel_v1(producer).unwrap(),
            BackendCancellationV1::Cancelled
        );
        f.clean();
    }
}

#[test]
fn compute_peer_native_error_and_unwind_preserve_producer_and_both_children() {
    for unwind in [false, true] {
        for stage in [
            Stage::Create,
            Stage::Copy,
            Stage::Poll,
            Stage::Finish,
            Stage::Retire,
            Stage::Restore,
        ] {
            let mut f = Fixture::new(false);
            f.backend.compute_xgmi_routes.insert(
                (0, 1),
                Route::Scripted {
                    failure: Some(stage),
                    unwind,
                    pending_samples: 0,
                },
            );
            let producer = f.launch(0, 0, false, true);
            let event = f.event(0, producer);
            let peer = f.peer(&[event]);
            f.backend.release_event_v1(event).unwrap();
            let result = catch_unwind(AssertUnwindSafe(|| {
                for _ in 0..32 {
                    f.backend.progress_cooperative_copy_step_v1(peer)?;
                }
                Ok::<_, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>(())
            }));
            if unwind {
                assert!(result.is_err(), "{stage:?}");
            } else {
                assert!(
                    matches!(result.unwrap(), Err(RuntimeBackendFailureV1::Terminal(_))),
                    "{stage:?}"
                );
            }
            assert!(f.backend.terminal);
            assert!(f.backend.children.iter().all(|child| child.terminal));
            assert!(f.copy(peer).compute_producer.is_some());
            assert!(f.copy(peer).compute_xgmi.is_some());
            assert!(f.backend.producer_aware_native.contains_key(&producer));
            assert!(
                f.backend
                    .cooperative_dependency_retain_counts
                    .contains_key(&producer)
            );
            assert_eq!(f.backend.completed_compute_xgmi_copies, 0);
            // Terminal roots intentionally remain inside ManuallyDrop.
        }
    }
}

#[test]
fn compute_peer_deeper_successful_control_and_retain_overflow_are_checked_before_admission() {
    for control_depth in [4, MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1, usize::MAX] {
        let mut f = Fixture::new(false);
        let control = f.launch(1, 0, false, true);
        f.drive(f.compute_streams[1], control);
        assert_eq!(
            f.backend.poll_v1(control).unwrap(),
            BackendPollV1::Succeeded
        );
        let route = f.native(control);
        let old_depth = f.backend.children[1].submissions[&route.local].dependency_depth;
        // Inject only retained depth metadata on a genuinely completed control;
        // no completion, owner restoration, or publication is synthesized.
        f.backend.children[1]
            .submissions
            .get_mut(&route.local)
            .unwrap()
            .dependency_depth = control_depth;
        let control_event = f.event(1, control);
        let producer = f.launch(0, 0, true, true);
        let event = f.event(0, producer);
        let before = (f.backend.next_handle, f.backend.submissions.len());
        let result = f.backend.peer_copy_v1(
            f.peer_stream,
            region(f.allocations[0][2], RuntimeAccessV1::Read),
            region(f.allocations[1][3], RuntimeAccessV1::Write),
            &[event, control_event],
        );
        if control_depth == 4 {
            let peer = result.unwrap();
            assert_eq!(f.copy(peer).dependency_depth, 5);
            assert_eq!(
                f.backend.cancel_v1(peer).unwrap(),
                BackendCancellationV1::Cancelled
            );
        } else {
            assert!(
                matches!(result, Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity)
            );
            assert_eq!((f.backend.next_handle, f.backend.submissions.len()), before);
            assert!(f.backend.cooperative_allocation_owners.is_empty());
            assert!(f.backend.cooperative_dependency_retain_counts.is_empty());
        }
        f.backend.children[1]
            .submissions
            .get_mut(&route.local)
            .unwrap()
            .dependency_depth = old_depth;
        assert_eq!(
            f.backend.cancel_v1(producer).unwrap(),
            BackendCancellationV1::Cancelled
        );
        f.clean();
    }

    let mut f = Fixture::new(false);
    let producer = f.launch(0, 0, true, true);
    let event = f.event(0, producer);
    f.backend
        .cooperative_dependency_retain_counts
        .insert(producer, usize::MAX);
    let before = (f.backend.next_handle, f.backend.submissions.len());
    let result = f.backend.peer_copy_v1(
        f.peer_stream,
        region(f.allocations[0][2], RuntimeAccessV1::Read),
        region(f.allocations[1][3], RuntimeAccessV1::Write),
        &[event],
    );
    assert!(
        matches!(result, Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity)
    );
    assert_eq!((f.backend.next_handle, f.backend.submissions.len()), before);
    assert!(f.backend.cooperative_allocation_owners.is_empty());
    assert_eq!(
        f.backend
            .cooperative_dependency_retain_counts
            .remove(&producer),
        Some(usize::MAX)
    );
    assert_eq!(
        f.backend.cancel_v1(producer).unwrap(),
        BackendCancellationV1::Cancelled
    );
    f.clean();
}

#[test]
fn compute_peer_producer_completion_error_and_unwind_keep_native_plan_unpublished() {
    for fault in [
        ScriptedThreeCompletionFaultV1::Poll,
        ScriptedThreeCompletionFaultV1::AfterRetirement,
    ] {
        let mut f = Fixture::new(false);
        let producer = f.launch(0, 0, false, true);
        let event = f.event(0, producer);
        let peer = f.peer(&[event]);
        f.backend.release_event_v1(event).unwrap();
        f.backend.children[0].scripted_three_completion_fault = Some(fault);
        let result = catch_unwind(AssertUnwindSafe(|| {
            for _ in 0..16 {
                f.backend.progress_cooperative_copy_step_v1(peer)?;
            }
            Ok::<_, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>(())
        }));
        if fault == ScriptedThreeCompletionFaultV1::AfterRetirement {
            assert!(result.is_err());
        } else {
            assert!(matches!(
                result.unwrap(),
                Err(RuntimeBackendFailureV1::Terminal(_))
            ));
        }
        assert!(f.backend.terminal);
        assert!(f.backend.children.iter().all(|child| child.terminal));
        assert!(f.backend.compute_xgmi_children.iter().all(Option::is_none));
        assert_eq!(f.backend.completed_compute_xgmi_copies, 0);
        assert!(f.copy(peer).compute_xgmi.as_ref().unwrap().is_quiescent());
        assert!(f.copy(peer).compute_producer.is_some());
        assert!(f.backend.producer_aware_native.contains_key(&producer));
        assert!(
            f.backend
                .cooperative_dependency_retain_counts
                .contains_key(&producer)
        );
    }
}

#[test]
fn compute_peer_helper_attributes_quiescence_only_after_exact_producer_cancellation() {
    let mut f = Fixture::new(false);
    let producer = f.launch(0, 0, true, true);
    let route = f.native(producer);
    let event = f.event(0, producer);
    let peer = f.peer(&[event]);
    f.backend.release_event_v1(event).unwrap();
    let error = || {
        KfdRuntimeBackendV1::quiescent_error(
            KfdRuntimeBackendErrorKindV1::Native,
            "helper-level predecessor quiescence diagnostic",
        )
    };
    // This exercises the shared attribution boundary, not a fabricated native
    // prefix execution. The target is a genuinely queued, retained compute.
    assert!(!f.backend.children[0].exact_submission_quiescent_v1(route.local));
    assert_eq!(
        f.backend
            .attribute_compute_peer_progress_error_v1(peer, route, error())
            .unwrap(),
        BackendPollV1::Pending
    );
    assert_eq!(f.copy(peer).status(), BackendPollV1::Pending);
    assert!(f.copy(peer).compute_producer.is_some());
    assert!(f.backend.producer_aware_native.contains_key(&producer));
    assert!(
        f.backend
            .cooperative_dependency_retain_counts
            .contains_key(&producer)
    );
    assert!(f.backend.compute_xgmi_children.iter().all(Option::is_none));
    assert_eq!(
        f.backend.cancel_v1(producer).unwrap(),
        BackendCancellationV1::Cancelled
    );
    assert!(f.backend.children[0].exact_submission_quiescent_v1(route.local));
    assert!(matches!(
        f.backend.attribute_compute_peer_progress_error_v1(peer, route, error()),
        Err(RuntimeBackendFailureV1::Quiescent(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Native
                && error.detail() == "helper-level predecessor quiescence diagnostic"
    ));
    assert!(matches!(
        f.copy(peer).status(),
        BackendPollV1::Failed { .. }
    ));
    assert!(f.copy(peer).compute_producer.is_none());
    assert!(
        !f.backend
            .cooperative_dependency_retain_counts
            .contains_key(&producer)
    );
    assert_eq!(f.backend.completed_compute_xgmi_copies, 0);
    assert!(!f.backend.terminal);
    f.clean();
}
