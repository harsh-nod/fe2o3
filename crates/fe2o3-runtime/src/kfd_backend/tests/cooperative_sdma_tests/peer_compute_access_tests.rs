use super::*;

struct Consumer {
    owner: u64,
    child: usize,
    id: u64,
    stream: u64,
    routed_stream: u64,
    module: u64,
    allocation: u64,
}

impl Consumer {
    fn new(f: &mut Fixture, producer: u64, reading: bool) -> Self {
        let region = if reading { f.source } else { f.destination };
        let route = f.backend.allocations[&region.allocation];
        let device = f.backend.children[route.child].description.backend_device;
        let routed_stream = f.backend.create_stream_v1(device).unwrap();
        let stream = f.backend.streams[&routed_stream].local;
        let module = f.backend.children[route.child]
            .load_module_v1(device, &synthetic_cov6::module())
            .unwrap();
        let kernel = f.backend.children[route.child]
            .resolve_kernel_v1(module, "vecadd", [7; 32])
            .unwrap();
        let bindings = [BackendBindingV1 {
            region: BackendMemoryRegionV1 {
                allocation: route.local,
                access: RuntimeAccessV1::Read,
                byte_offset: region.byte_offset,
                byte_len: region.byte_len.min(8),
            },
            kernarg_byte_offset: 0,
        }];
        let launch = BackendLaunchV1 {
            stream,
            kernel,
            explicit_kernarg: &[0; 16],
            bindings: &bindings,
            dependencies: &[],
            geometry: crate::RuntimeLaunchGeometryV1 {
                grid: [1, 1, 1],
                workgroup: [1, 1, 1],
                dynamic_shared_bytes: 0,
            },
            semantic_launch: KfdRuntimeSemanticLaunchV1::Ordinary,
        };
        let mut collected = f.backend.children[route.child]
            .preflight_compute_v1(launch, ComputeDependencyRosterV1::Exact(&[]))
            .unwrap();
        let owner = f.backend.next_id().unwrap();
        let id = f.backend.children[route.child].next_handle;
        let ancestry = f
            .backend
            .capture_peer_launch_ancestry_v1(owner, routed_stream, &[producer])
            .unwrap();
        collected.minimum_dependency_depth = ancestry.depth();
        collected.peer_access = f
            .backend
            .prepare_peer_compute_access_v1(
                RoutedHandleV1 {
                    child: route.child,
                    local: id,
                },
                &ancestry,
                &bindings,
            )
            .unwrap();
        collected.peer_gate = Some(PeerComputeGateV1::waiting(owner, id, true));
        assert_eq!(
            f.backend
                .with_peer_launch_ancestry_v1(owner, Some(ancestry), |backend| backend.children
                    [route.child]
                    .submit_collected_compute_v1(launch, collected))
                .unwrap(),
            id
        );
        Self {
            owner,
            child: route.child,
            id,
            stream,
            routed_stream,
            module,
            allocation: route.local,
        }
    }

    fn assert_retained(&self, f: &mut Fixture) {
        let child = &mut f.backend.children[self.child];
        assert_eq!(child.poll_v1(self.id).unwrap(), BackendPollV1::Pending);
        assert!(child.active_compute_lane_v1(self.id).is_none());
        assert_eq!(child.compute_completion_reservations, 1);
        assert_eq!(child.compute_module_retain_counts[&self.module], 1);
        assert!(
            child.pending_compute[&self.id]
                .peer_gate
                .unwrap()
                .owns(self.owner, self.id)
        );
        assert!(
            matches!(child.write_allocation_v1(self.allocation, 0, &[0]), Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Busy)
        );
        assert!(
            matches!(child.read_allocation_v1(self.allocation, 0, &mut [0]), Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Busy)
        );
        assert!(
            matches!(child.release_allocation_v1(self.allocation), Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Busy)
        );
        let scratch = child.allocations.ordinary_iter().find_map(|(id, record)| {
            (record.kind == RuntimeMemoryKindV1::HostVisible).then_some(*id)
        });
        if child.allocations[&self.allocation].kind == RuntimeMemoryKindV1::DeviceLocal
            && let Some(scratch) = scratch
            && let Some(stream) = child.streams.keys().copied().find(|id| *id != self.stream)
        {
            assert!(matches!(
                child.copy_async_v1(stream, BackendMemoryRegionV1 {
                    allocation: self.allocation, access: RuntimeAccessV1::Read,
                    byte_offset: 0, byte_len: 1,
                }, BackendMemoryRegionV1 {
                    allocation: scratch, access: RuntimeAccessV1::Write,
                    byte_offset: 0, byte_len: 1,
                }, &[]),
                Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Busy
            ));
        }
    }

    fn finish(self, f: &mut Fixture) {
        let child = &mut f.backend.children[self.child];
        assert_eq!(
            child.cancel_v1(self.id).unwrap(),
            crate::BackendCancellationV1::Cancelled
        );
        child.release_submission_v1(self.id).unwrap();
        child.unload_module_v1(self.module).unwrap();
        assert!(f.backend.peer_launch_retains.release(self.owner));
        f.backend.destroy_stream_v1(self.routed_stream).unwrap();
    }
}

fn submit(f: &mut Fixture) -> (u64, BackendDirectedPeerRouteV1) {
    let route = directed_route(f);
    let id = f
        .backend
        .submit_directed_scalar_peer_copy_v1(BackendDirectedScalarPeerCopyV1 {
            route,
            dependencies: &[],
        })
        .unwrap();
    (id, route)
}

fn progress(f: &mut Fixture, id: u64, route: BackendDirectedPeerRouteV1) -> BackendPollV1 {
    f.backend
        .progress_directed_scalar_peer_copy_v1(BackendDirectedScalarProgressV1 {
            submission: id,
            route,
            producer_submissions: &[],
        })
        .unwrap()
}

fn finish_producer(f: &mut Fixture, id: u64, route: BackendDirectedPeerRouteV1) {
    for _ in 0..128 {
        if progress(f, id, route) == BackendPollV1::Succeeded {
            return;
        }
    }
    panic!("directed producer did not complete within its bounded script");
}

fn disarm(f: &mut Fixture) {
    for child in &mut f.backend.children {
        assert!(child.admitted_device.is_none() && child.queue.is_none());
        disarm_scripted_drop_after_inspection_v1(child);
    }
}

#[test]
fn peer_compute_access_synthetic_host_and_device_copies_preserve_exact_ranges() {
    for kind in [
        RuntimeMemoryKindV1::HostVisible,
        RuntimeMemoryKindV1::DeviceLocal,
    ] {
        let left = KfdRuntimeBackendV1::mock();
        let mut right = KfdRuntimeBackendV1::mock();
        right.description.backend_device = 8;
        let mut backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
        let stream = backend.create_stream_v1(8).unwrap();
        let source = backend.allocate_v1(7, kind, 24, 8).unwrap();
        let destination = backend.allocate_v1(8, kind, 24, 8).unwrap();
        backend.write_allocation_v1(source, 0, &[0x53; 24]).unwrap();
        backend
            .write_allocation_v1(destination, 0, &[0x17; 24])
            .unwrap();
        let mut f = Fixture {
            backend,
            stream,
            source: BackendMemoryRegionV1 {
                allocation: source,
                access: RuntimeAccessV1::Read,
                byte_offset: 4,
                byte_len: 8,
            },
            destination: BackendMemoryRegionV1 {
                allocation: destination,
                access: RuntimeAccessV1::Write,
                byte_offset: 8,
                byte_len: 8,
            },
        };
        let (producer, route) = submit(&mut f);
        for _ in 0..32 {
            if progress(&mut f, producer, route) == BackendPollV1::Succeeded {
                break;
            }
        }
        assert_eq!(
            f.backend.poll_v1(producer).unwrap(),
            BackendPollV1::Succeeded
        );
        let mut bytes = [0; 24];
        f.backend
            .read_allocation_v1(destination, 0, &mut bytes)
            .unwrap();
        let mut expected = [0x17; 24];
        expected[8..16].fill(0x53);
        assert_eq!(bytes, expected);
        f.backend.release_submission_v1(producer).unwrap();
        f.backend.release_allocation_v1(source).unwrap();
        f.backend.release_allocation_v1(destination).unwrap();
        f.backend.destroy_stream_v1(stream).unwrap();
        f.backend.shutdown_native_v1().unwrap();
    }
}

#[test]
fn peer_compute_access_clean_host_read_rejects_inconsistent_backing_before_driver_effect() {
    let mut f = native_host_fixture(vec![], vec![]);
    let (producer, route) = submit(&mut f);
    let consumer = Consumer::new(&mut f, producer, true);
    f.backend.children[0]
        .allocations
        .get_mut(&consumer.allocation)
        .unwrap()
        .sdma_backed = false;
    let before = f.steps();
    let mut failed = false;
    for _ in 0..8 {
        match f
            .backend
            .progress_directed_scalar_peer_copy_v1(BackendDirectedScalarProgressV1 {
                submission: producer,
                route,
                producer_submissions: &[],
            }) {
            Ok(BackendPollV1::Pending) => {}
            Err(RuntimeBackendFailureV1::Terminal(_)) => {
                failed = true;
                break;
            }
            unexpected => panic!("unexpected corrupt-host progress: {unexpected:?}"),
        }
    }
    assert!(failed);
    assert_eq!(f.steps(), before);
    disarm(&mut f);
}

#[test]
fn peer_compute_access_cancel_consumer_preserves_live_producer_dma() {
    for reading in [true, false] {
        let mut f = fixture(
            16,
            phase_steps(true, 0, 16, true),
            phase_steps(false, 0, 16, true),
        );
        let (producer, route) = submit(&mut f);
        let consumer = Consumer::new(&mut f, producer, reading);
        for _ in 0..64 {
            progress(&mut f, producer, route);
            if !f.backend.children[consumer.child]
                .published_sdma_submissions
                .is_empty()
            {
                break;
            }
        }
        let child_index = consumer.child;
        let allocation = consumer.allocation;
        let child = &f.backend.children[child_index];
        assert_eq!(child.active_sdma.len(), 1);
        let dma = *child.active_sdma.keys().next().unwrap();
        assert!(matches!(
            child.active_sdma[&dma].phase,
            ActiveSdmaPhaseV1::DirectionalPublished(_)
        ));
        assert_eq!(child.allocation_custody[&allocation].owners.len(), 2);
        let before = f.steps();
        assert_eq!(
            f.backend.cancel_v1(producer).unwrap(),
            crate::BackendCancellationV1::TooLate
        );
        consumer.finish(&mut f);
        assert_eq!(f.steps(), before);
        let child = &mut f.backend.children[child_index];
        assert_eq!(child.allocation_custody[&allocation].owners.len(), 1);
        assert!(child.peer_dma_access_is_intact_v1(&child.active_sdma[&dma]));
        assert!(
            matches!(child.read_allocation_v1(allocation, 0, &mut [0]), Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Busy)
        );
        finish_producer(&mut f, producer, route);
        assert_eq!(f.bytes(false), [0x53; 16]);
        f.clean(&[producer]);
    }
}

#[test]
fn peer_compute_access_host_reconciliation_preserves_full_extent_and_copy_interval() {
    for reading in [true, false] {
        let mut steps = allocation_steps(4);
        steps.extend([
            ScriptedSdmaStepV1::Write {
                offset: 4,
                byte_len: 4,
            },
            ScriptedSdmaStepV1::Write {
                offset: 8,
                byte_len: 4,
            },
            ScriptedSdmaStepV1::Write {
                offset: 12,
                byte_len: 1,
            },
            if reading {
                ScriptedSdmaStepV1::Read {
                    offset: 7,
                    byte_len: 4,
                }
            } else {
                ScriptedSdmaStepV1::Write {
                    offset: 7,
                    byte_len: 4,
                }
            },
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
        ]);
        let mut f = native_host_fixture(steps, phase_steps(!reading, 14, 4, true));
        attach_native_dirty(&mut f, 4, (0..9).collect());
        if !reading {
            core::mem::swap(&mut f.source, &mut f.destination);
            f.source.access = RuntimeAccessV1::Read;
            f.destination.access = RuntimeAccessV1::Write;
            f.backend.destroy_stream_v1(f.stream).unwrap();
            f.stream = f.backend.create_stream_v1(7).unwrap();
        }
        let (producer, route) = submit(&mut f);
        let consumer = Consumer::new(&mut f, producer, reading);
        for _ in 0..64 {
            consumer.assert_retained(&mut f);
            if progress(&mut f, producer, route) == BackendPollV1::Succeeded {
                break;
            }
            f.assert_observation_only(producer);
        }
        assert_eq!(
            f.backend.poll_v1(producer).unwrap(),
            BackendPollV1::Succeeded
        );
        let mut host = vec![0x53; 24];
        host[4..13].copy_from_slice(&(0..9).collect::<Vec<_>>());
        if reading {
            let mut destination = vec![0x17; 24];
            destination[14..18].copy_from_slice(&host[7..11]);
            assert_eq!(f.bytes(false), destination);
            assert_eq!(f.bytes(true), host);
        } else {
            host[7..11].fill(0x17);
            assert_eq!(f.bytes(false), host);
        }
        assert_eq!(
            f.backend.children[0]
                .scripted_native_reconcile
                .as_ref()
                .unwrap()
                .reads,
            [(41, 0, 2, 4), (41, 0, 6, 4), (41, 0, 10, 1)]
        );
        forget_scripted_recycled(&mut f);
        consumer.finish(&mut f);
        f.clean(&[producer]);
    }
}

#[test]
fn peer_compute_access_publication_rejects_changed_arguments_gate_or_dirty_authority() {
    for change in 0..8 {
        let mut f = fixture(
            16,
            phase_steps(true, 0, 16, true),
            phase_steps(false, 0, 16, true),
        );
        let (producer, route) = submit(&mut f);
        let consumer = Consumer::new(&mut f, producer, true);
        for _ in 0..16 {
            progress(&mut f, producer, route);
            if !f.backend.children[0].active_sdma.is_empty() {
                break;
            }
        }
        let before = f.steps();
        let child = &mut f.backend.children[0];
        let dma = *child.active_sdma.keys().next().unwrap();
        let mut active = child.active_sdma.remove(&dma).unwrap();
        assert!(child.peer_dma_access_is_intact_v1(&active));
        match change {
            0 => active.source_offset += 1,
            1 => active.destination_offset += 1,
            2 => active.byte_len -= 1,
            3 => active.stream = consumer.stream,
            4 => {
                let pending = child.pending_compute.get_mut(&consumer.id).unwrap();
                pending.peer_gate = Some(
                    pending
                        .peer_gate
                        .unwrap()
                        .resolve(
                            consumer.owner,
                            consumer.id,
                            PeerComputeResultV1::Succeeded,
                            true,
                        )
                        .unwrap(),
                );
            }
            5 => {
                child
                    .pending_compute
                    .get_mut(&consumer.id)
                    .unwrap()
                    .peer_access = PeerComputePermitsV1::default()
            }
            6 | 7 => {
                let allocation = if change == 6 {
                    active.source
                } else {
                    active.destination
                };
                child
                    .allocations
                    .get_mut(&allocation)
                    .unwrap()
                    .native_dirty
                    .push(NativeDirtyExtentV1 {
                        compute_lane: 0,
                        data_index: 0,
                        allocation_offset: 0,
                        data_offset: 0,
                        byte_len: 1,
                    });
            }
            _ => unreachable!(),
        }
        assert!(!child.peer_dma_access_is_intact_v1(&active));
        // Exercise the publication boundary directly, retaining the existing native phase.
        assert!(matches!(
            child.publish_sdma_copy_v1(active),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(child.active_sdma.contains_key(&dma));
        assert_eq!(f.steps(), before);
        disarm(&mut f);
    }
}

#[test]
fn peer_compute_access_late_reconciliation_scratch_or_backing_change_seals_before_read() {
    for change in 0..6 {
        let mut steps = allocation_steps(4);
        steps.push(ScriptedSdmaStepV1::Write {
            offset: 4,
            byte_len: 4,
        });
        let mut f = native_host_fixture(steps, vec![]);
        attach_native_dirty(&mut f, 4, vec![0x42; 9]);
        let (producer, route) = submit(&mut f);
        let consumer = Consumer::new(&mut f, producer, true);
        for _ in 0..16 {
            progress(&mut f, producer, route);
            if !f.backend.children[0]
                .scripted_native_reconcile
                .as_ref()
                .unwrap()
                .reads
                .is_empty()
            {
                break;
            }
        }
        let child = &mut f.backend.children[0];
        let scratch = *child
            .allocations
            .ordinary_iter()
            .find(|(id, _)| **id != consumer.allocation)
            .unwrap()
            .0;
        let record = child.allocations.get_mut(&scratch).unwrap();
        match change {
            0 => record.kind = RuntimeMemoryKindV1::DeviceLocal,
            1 => record.bytes = Arc::from([0_u8; 3]),
            2 => record.sdma_shadow_dirty = true,
            3 => record.sdma_backed = false,
            4 => {
                child
                    .allocations
                    .get_mut(&consumer.allocation)
                    .unwrap()
                    .sdma_backed = false
            }
            5 => {
                let reservation = child.reserve_allocation_custody_v1(&[scratch]).unwrap();
                child.retain_allocation_custody_v1(
                    &[scratch],
                    RuntimeAllocationCustodyOwnerV1 {
                        submission: consumer.id,
                        stream: consumer.stream,
                        kind: RuntimeAllocationCustodyKindV1::Compute,
                    },
                    reservation,
                );
            }
            _ => unreachable!(),
        }
        let before = f.steps();
        assert!(matches!(
            f.backend
                .progress_directed_scalar_peer_copy_v1(BackendDirectedScalarProgressV1 {
                    submission: producer,
                    route,
                    producer_submissions: &[],
                }),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert_eq!(f.steps(), before);
        assert_eq!(
            f.backend.children[0]
                .scripted_native_reconcile
                .as_ref()
                .unwrap()
                .reads
                .len(),
            1
        );
        assert!(f.backend.children[0].native_reconciliations[0].is_some());
        disarm(&mut f);
    }
}

#[test]
fn peer_compute_access_multipart_dma_preserves_consumer_custody_and_public_exclusion() {
    let bytes = COOPERATIVE_COPY_CHUNK_BYTES_V1 + 3;
    for reading in [true, false] {
        let mut f = fixture(
            bytes + 32,
            phase_steps(true, 8, bytes, true),
            phase_steps(false, 16, bytes, true),
        );
        f.source.byte_offset = 8;
        f.destination.byte_offset = 16;
        f.source.byte_len = bytes as u64;
        f.destination.byte_len = bytes as u64;
        let route = directed_route(&f);
        let producer = f
            .backend
            .submit_directed_scalar_peer_copy_v1(BackendDirectedScalarPeerCopyV1 {
                route,
                dependencies: &[],
            })
            .unwrap();
        let consumer = Consumer::new(&mut f, producer, reading);
        for _ in 0..128 {
            consumer.assert_retained(&mut f);
            let status = f
                .backend
                .progress_directed_scalar_peer_copy_v1(BackendDirectedScalarProgressV1 {
                    submission: producer,
                    route,
                    producer_submissions: &[],
                })
                .unwrap();
            if status == BackendPollV1::Succeeded {
                break;
            }
            f.assert_observation_only(producer);
        }
        assert_eq!(
            f.backend.poll_v1(producer).unwrap(),
            BackendPollV1::Succeeded
        );
        let mut expected = vec![0x17; bytes + 32];
        expected[16..16 + bytes].fill(0x53);
        assert_eq!(f.bytes(false), expected);
        consumer.assert_retained(&mut f);
        consumer.finish(&mut f);
        f.clean(&[producer]);
    }
}

#[test]
fn peer_compute_access_transitive_dma_runs_under_genuine_gated_child_custody() {
    for reading in [true, false] {
        let mut source_steps = phase_steps(true, 0, 8, true);
        source_steps.extend(phase_steps(true, 0, 8, true));
        let mut destination_steps = phase_steps(false, 0, 8, true);
        destination_steps.extend(phase_steps(false, 0, 8, true));
        let mut f = fixture(8, source_steps, destination_steps);
        let (first, route) = submit(&mut f);
        let event = f.backend.record_event_v1(route.stream, first).unwrap();
        let second = f
            .backend
            .submit_directed_scalar_peer_copy_v1(BackendDirectedScalarPeerCopyV1 {
                route,
                dependencies: &[BackendDirectedPeerDependencyV1 {
                    event,
                    producer_submission: first,
                }],
            })
            .unwrap();
        f.backend.release_event_v1(event).unwrap();
        let consumer = Consumer::new(&mut f, second, reading);
        assert_eq!(
            f.backend.children[consumer.child].pending_compute[&consumer.id].dependency_depth,
            3
        );
        let mut observed_first_terminal = false;
        for _ in 0..128 {
            consumer.assert_retained(&mut f);
            let status = f
                .backend
                .progress_directed_scalar_peer_copy_v1(BackendDirectedScalarProgressV1 {
                    submission: second,
                    route,
                    producer_submissions: &[first],
                })
                .unwrap();
            if f.backend.poll_v1(first).unwrap() == BackendPollV1::Succeeded {
                observed_first_terminal = true;
                assert!(f.backend.peer_launch_retains.retains(first));
                assert!(matches!(f.backend.release_submission_v1(first),
                    Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Busy));
            }
            if status == BackendPollV1::Succeeded {
                break;
            }
            f.assert_observation_only(second);
        }
        assert!(observed_first_terminal);
        assert_eq!(f.backend.poll_v1(second).unwrap(), BackendPollV1::Succeeded);
        assert_eq!(f.bytes(false), [0x53; 8]);
        consumer.assert_retained(&mut f);
        consumer.finish(&mut f);
        assert!(f.backend.peer_launch_retains.is_empty());
        f.clean(&[first, second]);
    }
}
