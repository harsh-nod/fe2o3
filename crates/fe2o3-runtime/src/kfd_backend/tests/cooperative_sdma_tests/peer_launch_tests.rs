//! Public router lifecycle over scripted native owners, not hardware evidence.

use super::*;
use std::mem::ManuallyDrop;
mod inherited_peer_tests;

fn directed(f: &mut Fixture) -> u64 {
    f.backend
        .submit_directed_scalar_peer_copy_v1(BackendDirectedScalarPeerCopyV1 {
            route: directed_route(f),
            dependencies: &[],
        })
        .unwrap()
}

fn load(f: &mut Fixture) -> (u64, u64) {
    let module = f
        .backend
        .load_module_v1(8, &synthetic_cov6::module())
        .unwrap();
    let kernel = f
        .backend
        .resolve_kernel_v1(module, "vecadd", [7; 32])
        .unwrap();
    (module, kernel)
}

fn launch(
    f: &mut Fixture,
    stream: u64,
    kernel: u64,
    dependencies: &[BackendLaunchProducerV1],
) -> u64 {
    try_launch(f, stream, kernel, dependencies).unwrap()
}

fn try_launch(
    f: &mut Fixture,
    stream: u64,
    kernel: u64,
    dependencies: &[BackendLaunchProducerV1],
) -> Result<u64, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
    let mut kernarg = [0; 16];
    kernarg[8..].copy_from_slice(&1024_u64.to_le_bytes());
    f.backend
        .submit_producer_aware_launch_v1(BackendProducerAwareLaunchV1 {
            stream,
            kernel,
            explicit_kernarg: &kernarg,
            bindings: &[BackendBindingV1 {
                region: BackendMemoryRegionV1 {
                    access: RuntimeAccessV1::Read,
                    byte_len: 4096,
                    ..f.destination
                },
                kernarg_byte_offset: 0,
            }],
            dependencies,
            geometry: crate::RuntimeLaunchGeometryV1 {
                grid: [64, 1, 1],
                workgroup: [64, 1, 1],
                dynamic_shared_bytes: 0,
            },
        })
}

fn routed(f: &Fixture, id: u64) -> RoutedHandleV1 {
    let RoutedSubmissionV1::Native { route, .. } = f.backend.submissions[&id] else {
        panic!("native consumer");
    };
    route
}

fn complete(f: &mut Fixture, consumer: u64) -> BackendPollV1 {
    for _ in 0..8 {
        let status = f.backend.poll_v1(consumer).unwrap();
        if status != BackendPollV1::Pending {
            return status;
        }
    }
    panic!("scripted consumer did not settle");
}

#[test]
fn peer_launch_public_router_admits_pending_and_published_producers_without_observer_progress() {
    for stage in 0..3 {
        let mut destination_steps = phase_steps(false, 0, 2048, true);
        destination_steps.push(ScriptedSdmaStepV1::PromoteInitializedStorage(
            ScriptedFailureModeV1::Success,
        ));
        if stage == 2 {
            destination_steps.push(ScriptedSdmaStepV1::Recycle(
                ScriptedRecycleOutcomeV1::Success,
            ));
        }
        let mut f = ManuallyDrop::new(fixture(
            4096,
            phase_steps(true, 0, 2048, true),
            destination_steps,
        ));
        f.source.byte_len = 2048;
        f.destination.byte_len = 2048;
        let producer = directed(&mut f);
        if stage != 0 {
            for _ in 0..96 {
                f.backend
                    .progress_retained_directed_peer_v1(producer)
                    .unwrap();
                if !f.backend.children[stage - 1]
                    .published_sdma_submissions
                    .is_empty()
                {
                    break;
                }
            }
            assert!(
                !f.backend.children[stage - 1]
                    .published_sdma_submissions
                    .is_empty()
            );
        }
        let producer_stream = f.stream;
        let event = f
            .backend
            .record_event_v1(producer_stream, producer)
            .unwrap();
        let stream = f.backend.create_stream_v1(8).unwrap();
        let (module, kernel) = load(&mut f);
        let before = f.steps();
        let consumer = launch(
            &mut f,
            stream,
            kernel,
            &[BackendLaunchProducerV1 {
                event,
                producer_submission: producer,
            }],
        );
        let route = routed(&f, consumer);
        assert_eq!(f.steps(), before);
        assert!(
            f.backend.children[route.child]
                .pending_compute
                .contains_key(&route.local)
        );
        f.assert_observation_only(consumer);
        f.backend.release_event_v1(event).unwrap();
        assert!(matches!(
            f.backend.release_submission_v1(producer),
            Err(RuntimeBackendFailureV1::Rejected(_))
        ));
        let later = if stage == 0 {
            let extra = synthetic_fanout_destination(&mut f, 4096);
            let mut late_route = directed_route(&f);
            late_route.destination.allocation = extra.1;
            let late = f
                .backend
                .submit_directed_scalar_peer_copy_v1(BackendDirectedScalarPeerCopyV1 {
                    route: late_route,
                    dependencies: &[],
                })
                .unwrap();
            Some((late, extra))
        } else {
            None
        };
        let later_native = if stage == 2 {
            let extra = synthetic_fanout_destination(&mut f, 4096);
            let local = f.backend.allocations[&extra.1].local;
            let host = f.backend.children[1]
                .scripted_sdma
                .as_ref()
                .unwrap()
                .test_host_owner(4096);
            let record = f.backend.children[1].allocations.get_mut(&local).unwrap();
            record.sdma_storage = KfdRuntimeSdmaStorageV1::Host(host);
            record.sdma_backed = true;
            record.sdma_initialized = true;
            let original = f.destination;
            f.destination.allocation = extra.1;
            let later = launch(&mut f, stream, kernel, &[]);
            f.destination = original;
            Some((later, extra))
        } else {
            None
        };
        let before = f.steps();
        assert_eq!(
            f.backend.drain_v1(consumer, Instant::now()).unwrap(),
            BackendPollV1::Pending
        );
        assert_eq!(f.steps(), before);
        if stage == 1 {
            for _ in 0..96 {
                f.backend.flush_stream_v1(stream).unwrap();
                if f.backend.poll_v1(consumer).unwrap() == BackendPollV1::Succeeded {
                    break;
                }
            }
        } else {
            assert_eq!(
                f.backend
                    .drain_v1(consumer, Instant::now() + Duration::from_secs(1))
                    .unwrap(),
                BackendPollV1::Succeeded
            );
        }
        if let Some((late, extra)) = later {
            assert_eq!(f.backend.poll_v1(late).unwrap(), BackendPollV1::Pending);
            assert!(
                matches!(&f.backend.submissions[&late], RoutedSubmissionV1::CooperativeCopy(copy) if copy.phase == CooperativeCopyPhaseV1::Dependencies && copy.sdma_leaf.is_none())
            );
            assert_eq!(
                f.backend.cancel_v1(late).unwrap(),
                crate::BackendCancellationV1::Cancelled
            );
            f.backend.release_submission_v1(late).unwrap();
            f.backend.release_allocation_v1(extra.1).unwrap();
            f.backend.destroy_stream_v1(extra.0).unwrap();
        }
        if let Some((late, extra)) = later_native {
            let route = routed(&f, late);
            assert!(
                f.backend.children[route.child]
                    .pending_compute
                    .contains_key(&route.local),
                "draining a target cannot publish its later native successor"
            );
            assert_eq!(
                f.backend.cancel_v1(late).unwrap(),
                crate::BackendCancellationV1::Cancelled
            );
            f.backend.release_submission_v1(late).unwrap();
            let local = f.backend.allocations[&extra.1].local;
            f.backend.children[1]
                .allocations
                .get_mut(&local)
                .unwrap()
                .sdma_backed = false;
            f.backend.release_allocation_v1(extra.1).unwrap();
            f.backend.destroy_stream_v1(extra.0).unwrap();
        }
        assert_eq!(
            f.backend.poll_v1(producer).unwrap(),
            BackendPollV1::Succeeded
        );
        assert_eq!(complete(&mut f, consumer), BackendPollV1::Succeeded);
        assert!(f.backend.peer_launch_retains.is_empty());
        assert_eq!(
            f.backend.children[route.child]
                .next_dependency_depth_v1(None, &[route.local])
                .unwrap(),
            3
        );
        assert!(f.bytes(false)[..2048].iter().all(|byte| *byte == 0x53));
        assert!(f.bytes(false)[2048..].iter().all(|byte| *byte == 0x17));
        assert_eq!(
            f.backend.children[route.child]
                .last_launch_performance_v1()
                .unwrap()
                .user_data_materializations(),
            0
        );
        f.backend.release_submission_v1(consumer).unwrap();
        f.backend.unload_module_v1(module).unwrap();
        f.backend.destroy_stream_v1(stream).unwrap();
        ManuallyDrop::into_inner(f).clean(&[producer]);
    }
}

#[test]
fn peer_launch_public_cancellation_does_not_cancel_published_producer_dma() {
    let mut f = ManuallyDrop::new(fixture(
        4096,
        phase_steps(true, 0, 2048, true),
        phase_steps(false, 0, 2048, true),
    ));
    f.source.byte_len = 2048;
    f.destination.byte_len = 2048;
    let producer = directed(&mut f);
    for _ in 0..96 {
        f.backend
            .progress_retained_directed_peer_v1(producer)
            .unwrap();
        if !f.backend.children[1].published_sdma_submissions.is_empty() {
            break;
        }
    }
    assert!(!f.backend.children[1].published_sdma_submissions.is_empty());
    let stream = f.stream;
    let event = f.backend.record_event_v1(stream, producer).unwrap();
    let (module, kernel) = load(&mut f);
    let consumer = launch(
        &mut f,
        stream,
        kernel,
        &[BackendLaunchProducerV1 {
            event,
            producer_submission: producer,
        }],
    );
    let before = f.steps();
    assert_eq!(
        f.backend.cancel_v1(consumer).unwrap(),
        crate::BackendCancellationV1::Cancelled
    );
    assert_eq!(f.steps(), before);
    assert!(!f.backend.children[1].published_sdma_submissions.is_empty());
    assert!(f.backend.peer_launch_retains.is_empty());
    f.backend.release_submission_v1(consumer).unwrap();
    f.backend.release_event_v1(event).unwrap();
    f.backend.unload_module_v1(module).unwrap();
    for _ in 0..96 {
        if f.backend
            .progress_retained_directed_peer_v1(producer)
            .unwrap()
            == BackendPollV1::Succeeded
        {
            break;
        }
    }
    assert_eq!(
        f.backend.poll_v1(producer).unwrap(),
        BackendPollV1::Succeeded
    );
    ManuallyDrop::into_inner(f).clean(&[producer]);
}

#[test]
fn peer_launch_public_fifo_failure_is_not_explicit_success_failure() {
    for explicit in [false, true] {
        let steps = if explicit {
            vec![]
        } else {
            vec![ScriptedSdmaStepV1::PromoteInitializedStorage(
                ScriptedFailureModeV1::Success,
            )]
        };
        let mut f = ManuallyDrop::new(fixture(4096, vec![], steps));
        let producer = directed(&mut f);
        let stream = f.stream;
        let event = f.backend.record_event_v1(stream, producer).unwrap();
        let (module, kernel) = load(&mut f);
        let dependency = [BackendLaunchProducerV1 {
            event,
            producer_submission: producer,
        }];
        let consumer = launch(
            &mut f,
            stream,
            kernel,
            if explicit { &dependency } else { &[] },
        );
        assert_eq!(
            f.backend.cancel_v1(producer).unwrap(),
            crate::BackendCancellationV1::Cancelled
        );
        let result = f.backend.flush_stream_v1(stream);
        if explicit {
            assert!(matches!(result, Err(RuntimeBackendFailureV1::Quiescent(_))));
        } else {
            result.unwrap();
        }
        assert_eq!(
            complete(&mut f, consumer),
            if explicit {
                BackendPollV1::Failed { code: -1 }
            } else {
                BackendPollV1::Succeeded
            }
        );
        assert!(f.backend.peer_launch_retains.is_empty());
        f.backend.release_submission_v1(consumer).unwrap();
        f.backend.release_event_v1(event).unwrap();
        f.backend.unload_module_v1(module).unwrap();
        ManuallyDrop::into_inner(f).clean(&[producer]);
    }
}

#[test]
fn peer_launch_public_three_binding_waits_for_each_published_peer_slot() {
    for peer_slot in 0..3 {
        let mut steps = phase_steps(false, 0, 2048, true);
        steps.extend((0..3).map(|_| {
            ScriptedSdmaStepV1::PromoteInitializedStorage(ScriptedFailureModeV1::Success)
        }));
        for _ in 0..2 {
            steps.extend([
                ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
                ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
            ]);
        }
        let mut f = ManuallyDrop::new(fixture(4096, phase_steps(true, 0, 2048, true), steps));
        f.source.byte_len = 2048;
        f.destination.byte_len = 2048;
        let mut allocations = [f.destination.allocation; 3];
        let mut extras = Vec::new();
        for (slot, allocation) in allocations.iter_mut().enumerate() {
            if slot == peer_slot {
                continue;
            }
            f.backend.children[1].native_available = false;
            *allocation = f
                .backend
                .allocate_v1(8, RuntimeMemoryKindV1::DeviceLocal, 4096, 8)
                .unwrap();
            f.backend.children[1].native_available = true;
            extras.push(*allocation);
            let local = f.backend.allocations[allocation].local;
            let device = f.backend.children[1]
                .scripted_sdma
                .as_ref()
                .unwrap()
                .test_device_owner(4096);
            let record = f.backend.children[1].allocations.get_mut(&local).unwrap();
            record.sdma_storage = KfdRuntimeSdmaStorageV1::Device(Box::new(device));
            record.sdma_backed = true;
            record.sdma_initialized = true;
        }
        let producer = directed(&mut f);
        for _ in 0..96 {
            f.backend
                .progress_retained_directed_peer_v1(producer)
                .unwrap();
            if !f.backend.children[1].published_sdma_submissions.is_empty() {
                break;
            }
        }
        assert!(!f.backend.children[1].published_sdma_submissions.is_empty());
        let producer_stream = f.stream;
        let event = f
            .backend
            .record_event_v1(producer_stream, producer)
            .unwrap();
        let stream = f.backend.create_stream_v1(8).unwrap();
        let module = f
            .backend
            .load_module_v1(8, &synthetic_cov6::three_binding_module())
            .unwrap();
        let kernel = f
            .backend
            .resolve_kernel_v1(module, "vecadd", [7; 32])
            .unwrap();
        let bindings: Vec<_> = allocations
            .iter()
            .enumerate()
            .map(|(index, allocation)| BackendBindingV1 {
                region: BackendMemoryRegionV1 {
                    allocation: *allocation,
                    access: if index == 2 {
                        RuntimeAccessV1::Write
                    } else {
                        RuntimeAccessV1::Read
                    },
                    byte_offset: 0,
                    byte_len: 4096,
                },
                kernarg_byte_offset: (index * 8) as u32,
            })
            .collect();
        let mut kernarg = [0; 32];
        kernarg[24..].copy_from_slice(&1024_u64.to_le_bytes());
        let before = f.steps();
        let consumer = f
            .backend
            .submit_producer_aware_launch_v1(BackendProducerAwareLaunchV1 {
                stream,
                kernel,
                explicit_kernarg: &kernarg,
                bindings: &bindings,
                dependencies: &[BackendLaunchProducerV1 {
                    event,
                    producer_submission: producer,
                }],
                geometry: crate::RuntimeLaunchGeometryV1 {
                    grid: [64, 1, 1],
                    workgroup: [64, 1, 1],
                    dynamic_shared_bytes: 0,
                },
            })
            .unwrap();
        assert_eq!(f.steps(), before);
        f.assert_observation_only(consumer);
        assert_eq!(
            f.backend
                .drain_v1(consumer, Instant::now() + Duration::from_secs(1))
                .unwrap(),
            BackendPollV1::Succeeded
        );
        assert_eq!(
            f.backend.children[1]
                .last_launch_performance_v1()
                .unwrap()
                .user_data_materializations(),
            0
        );
        f.backend.release_submission_v1(consumer).unwrap();
        f.backend.release_event_v1(event).unwrap();
        f.backend.unload_module_v1(module).unwrap();
        f.backend.destroy_stream_v1(stream).unwrap();
        for allocation in extras {
            let local = f.backend.allocations[&allocation].local;
            f.backend.children[1]
                .allocations
                .get_mut(&local)
                .unwrap()
                .sdma_backed = false;
            f.backend.release_allocation_v1(allocation).unwrap();
        }
        ManuallyDrop::into_inner(f).clean(&[producer]);
    }
}

#[test]
fn peer_launch_private_dma_cannot_be_bypassed_by_native_event_or_fifo() {
    let mut f = ManuallyDrop::new(fixture(
        4096,
        phase_steps(true, 0, 2048, true),
        phase_steps(false, 0, 2048, true),
    ));
    f.source.byte_len = 2048;
    f.destination.byte_len = 2048;
    let producer = directed(&mut f);
    for _ in 0..96 {
        f.backend
            .progress_retained_directed_peer_v1(producer)
            .unwrap();
        if !f.backend.children[1].published_sdma_submissions.is_empty() {
            break;
        }
    }
    let before = f.steps();
    let child = &mut f.backend.children[1];
    let dma = child.published_sdma_submissions[0];
    let active = &child.active_sdma[&dma];
    let endpoint = active.destination;
    let private_stream = active.stream;
    let device = child.description.backend_device;
    let stream = child.create_stream_v1(device).unwrap();
    let event = child.record_event_v1(private_stream, dma).unwrap();
    let module = child
        .load_module_v1(device, &synthetic_cov6::module())
        .unwrap();
    let kernel = child.resolve_kernel_v1(module, "vecadd", [7; 32]).unwrap();
    let next = child.next_handle;
    let custody = child.allocation_custody[&endpoint].owners.clone();
    let retains = child.compute_dependency_retain_counts.clone();
    let tails = child.stream_submission_tails.clone();
    let reservations = child.compute_completion_reservations;
    for fifo in [false, true] {
        let mut kernarg = [0; 16];
        kernarg[8..].copy_from_slice(&1024_u64.to_le_bytes());
        let dependencies = [BackendLaunchProducerV1 {
            event,
            producer_submission: dma,
        }];
        let result = child.submit_producer_aware_launch_v1(BackendProducerAwareLaunchV1 {
            stream: if fifo { private_stream } else { stream },
            kernel,
            explicit_kernarg: &kernarg,
            bindings: &[BackendBindingV1 {
                region: BackendMemoryRegionV1 {
                    allocation: endpoint,
                    access: RuntimeAccessV1::Read,
                    byte_offset: 0,
                    byte_len: 4096,
                },
                kernarg_byte_offset: 0,
            }],
            dependencies: if fifo { &[] } else { &dependencies },
            geometry: crate::RuntimeLaunchGeometryV1 {
                grid: [64, 1, 1],
                workgroup: [64, 1, 1],
                dynamic_shared_bytes: 0,
            },
        });
        assert!(
            matches!(result, Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Busy)
        );
        assert!(!child.terminal);
        assert_eq!(child.next_handle, next);
        assert!(child.pending_compute.is_empty());
        assert_eq!(child.compute_completion_reservations, reservations);
        assert_eq!(child.compute_dependency_retain_counts, retains);
        assert_eq!(child.allocation_custody[&endpoint].owners, custody);
        assert_eq!(child.stream_submission_tails, tails);
        assert!(child.active_sdma.contains_key(&dma));
    }
    child.release_event_v1(event).unwrap();
    child.unload_module_v1(module).unwrap();
    child.destroy_stream_v1(stream).unwrap();
    assert_eq!(f.steps(), before);
    assert_eq!(
        f.backend
            .drain_v1(producer, Instant::now() + Duration::from_secs(1))
            .unwrap(),
        BackendPollV1::Succeeded
    );
    ManuallyDrop::into_inner(f).clean(&[producer]);
}

#[test]
fn peer_launch_dma_admission_is_endpoint_scoped_and_rejects_corrupt_custody() {
    for change in 0..6 {
        let mut f = ManuallyDrop::new(fixture(
            4096,
            phase_steps(true, 0, 2048, true),
            phase_steps(false, 0, 2048, true),
        ));
        f.source.byte_len = 2048;
        f.destination.byte_len = 2048;
        let producer = directed(&mut f);
        for _ in 0..96 {
            f.backend
                .progress_retained_directed_peer_v1(producer)
                .unwrap();
            if !f.backend.children[1].published_sdma_submissions.is_empty() {
                break;
            }
        }
        let dma = f.backend.children[1].published_sdma_submissions[0];
        let active = &f.backend.children[1].active_sdma[&dma];
        let endpoint = active.destination;
        let scratch = active.source;
        let private_stream = active.stream;
        let bindings = [endpoint, scratch].map(|allocation| BackendBindingV1 {
            region: BackendMemoryRegionV1 {
                allocation,
                access: RuntimeAccessV1::Read,
                byte_offset: 0,
                byte_len: 2048,
            },
            kernarg_byte_offset: 0,
        });
        let owner = f.backend.next_id().unwrap();
        let stream = f.stream;
        let ancestry = f
            .backend
            .capture_peer_launch_ancestry_v1(owner, stream, &[producer])
            .unwrap();
        let consumer = f.backend.children[1].next_handle;
        let permits = f
            .backend
            .prepare_peer_compute_access_v1(
                RoutedHandleV1 {
                    child: 1,
                    local: consumer,
                },
                &ancestry,
                &bindings,
            )
            .unwrap();
        let before = f.steps();
        let child = &mut f.backend.children[1];
        match change {
            0 => {}
            1 => child.active_sdma.get_mut(&dma).unwrap().id += 1,
            2 => child.active_sdma.get_mut(&dma).unwrap().stream += 1,
            3 => child.active_sdma.get_mut(&dma).unwrap().destination_offset += 1,
            4 => child.published_sdma_submissions.push(dma),
            5 => child
                .allocation_custody
                .get_mut(&scratch)
                .unwrap()
                .owners
                .push_back(RuntimeAllocationCustodyOwnerV1 {
                    submission: dma + 1,
                    stream: private_stream,
                    kind: RuntimeAllocationCustodyKindV1::Sdma,
                }),
            _ => unreachable!(),
        }
        let result = child.admit_peer_dma_owners_v1(
            &bindings,
            Some(PeerComputeGateV1::waiting(owner, consumer, false)),
            &permits,
        );
        if change == 0 {
            let admitted = result.unwrap();
            assert!(admitted.authorizes(endpoint, dma));
            assert!(!admitted.authorizes(scratch, dma));
        } else {
            assert!(matches!(result, Err(RuntimeBackendFailureV1::Terminal(_))));
            assert!(child.terminal);
        }
        assert!(child.active_sdma.contains_key(&dma));
        assert!(child.pending_compute.is_empty());
        assert_eq!(f.steps(), before);
        // CPU-owner inspection ends here; this is not terminal native cleanup.
        for child in &mut f.backend.children {
            disarm_scripted_drop_after_inspection_v1(child);
        }
        drop(ManuallyDrop::into_inner(f));
    }
}
