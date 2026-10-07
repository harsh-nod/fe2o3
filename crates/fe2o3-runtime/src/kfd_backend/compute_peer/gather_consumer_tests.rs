//! Real scripted gather/compute custody; no scripted compute arithmetic claim.

use super::*;

fn gathered_owner(f: &Fixture) -> (u64, &[u8]) {
    let route = f.backend.allocations[&f.allocations[1][3]];
    let owner = match &f.backend.children[1].allocations[&route.local].sdma_storage {
        KfdRuntimeSdmaStorageV1::Device(owner) => owner.as_ref(),
        KfdRuntimeSdmaStorageV1::InitializedStorage(owner) => match owner.as_ref() {
            initialized_storage::InitializedStorageOwnerV1::Scripted(owner) => owner,
            _ => panic!("scripted initialized storage expected"),
        },
        _ => panic!("gather input must be genuinely restored after compute"),
    };
    (
        owner.scripted_owner_id().unwrap(),
        owner.scripted_bytes().unwrap(),
    )
}

struct Consumer {
    gather: Gather,
    output_id: u64,
    output_bytes: Vec<u8>,
    return_id: u64,
    return_bytes: Vec<u8>,
}

impl Consumer {
    fn new(promote: bool, readback: bool) -> Self {
        let mut f = Fixture::with_layout_driver_prefixes(
            [BYTES, DESTINATION_BYTES],
            readback.then_some((0, 7, 9, 47)),
            false,
            [
                Vec::new(),
                if promote {
                    vec![ScriptedSdmaStepV1::PromoteInitializedStorage(
                        ScriptedFailureModeV1::Success,
                    )]
                } else {
                    Vec::new()
                },
            ],
        );
        let output = f.allocations[1][2];
        let returned = f.allocations[0][3];
        let (output_id, output_bytes) = initialize_bytes(&mut f, output, 0x49);
        let (return_id, return_bytes) = initialize_bytes(&mut f, returned, 0xaf);
        let mut gather = Gather::with_fixture(f);
        gather.chain(true);
        Self {
            gather,
            output_id,
            output_bytes,
            return_id,
            return_bytes,
        }
    }

    fn bindings(&self, shape: usize) -> [BackendBindingV1; 3] {
        let g = &self.gather;
        let mut bindings = std::array::from_fn(|index| BackendBindingV1 {
            region: BackendMemoryRegionV1 {
                allocation: g.f.allocations[1][[3, 1, 2][index]],
                access: if index == 2 {
                    RuntimeAccessV1::Write
                } else {
                    RuntimeAccessV1::Read
                },
                byte_offset: 0,
                byte_len: DESTINATION_BYTES as u64,
            },
            kernarg_byte_offset: index as u32 * 8,
        });
        if shape != 0 {
            bindings[0].region.byte_len = 16;
        }
        if shape == 2 {
            bindings[1].region = BackendMemoryRegionV1 {
                byte_offset: 80,
                ..bindings[0].region
            };
        }
        bindings
    }

    fn submit(
        &mut self,
        bindings: &[BackendBindingV1],
        dependencies: &[BackendLaunchProducerV1],
    ) -> Result<u64, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let f = &mut self.gather.f;
        let mut kernarg = [0; 32];
        let count = bindings
            .iter()
            .filter(|binding| binding.region.access == RuntimeAccessV1::Read)
            .map(|binding| binding.region.byte_len / 4)
            .min()
            .unwrap();
        kernarg[24..].copy_from_slice(&count.to_le_bytes());
        f.backend
            .submit_producer_aware_launch_v1(BackendProducerAwareLaunchV1 {
                stream: f.compute_streams[1],
                kernel: f.kernels[1],
                explicit_kernarg: &kernarg,
                bindings,
                dependencies,
                geometry: crate::RuntimeLaunchGeometryV1 {
                    grid: [64, 1, 1],
                    workgroup: [64, 1, 1],
                    dynamic_shared_bytes: 0,
                },
            })
    }

    fn dependency(&self) -> BackendLaunchProducerV1 {
        BackendLaunchProducerV1 {
            event: *self.gather.peer_events.last().unwrap(),
            producer_submission: *self.gather.peers.last().unwrap(),
        }
    }

    fn admit(&mut self, shape: usize) -> u64 {
        let bindings = self.bindings(shape);
        let dependency = self.dependency();
        self.submit(&bindings, &[dependency]).unwrap()
    }

    fn cancel_unissued(mut self, consumer: Option<u64>) {
        if let Some(id) = consumer {
            assert_eq!(
                self.gather.f.backend.cancel_v1(id).unwrap(),
                BackendCancellationV1::Cancelled
            );
        }
        self.gather.release_events();
        for id in self.gather.peers.iter().copied().rev() {
            assert_eq!(
                self.gather.f.backend.cancel_v1(id).unwrap(),
                BackendCancellationV1::Cancelled
            );
        }
        for id in self.gather.producers.into_iter().rev() {
            assert_eq!(
                self.gather.f.backend.cancel_v1(id).unwrap(),
                BackendCancellationV1::Cancelled
            );
        }
        assert!(self.gather.f.backend.deferred_compute_retains.is_empty());
        self.gather.f.clean();
    }

    fn publish_oldest(&mut self) {
        let g = &mut self.gather;
        let oldest = g.peers[0];
        for _ in 0..48 {
            g.f.backend
                .progress_cooperative_copy_step_v1(oldest)
                .unwrap();
            if !g
                .f
                .copy(oldest)
                .compute_xgmi
                .as_ref()
                .unwrap()
                .is_quiescent()
            {
                g.f.backend
                    .progress_cooperative_copy_step_v1(oldest)
                    .unwrap();
                assert!(
                    g.f.backend
                        .compute_xgmi_children
                        .iter()
                        .all(|owner| *owner == Some(oldest))
                );
                assert_eq!(g.f.copy(oldest).status(), BackendPollV1::Pending);
                return;
            }
        }
        panic!("oldest native root did not publish under bounded scripted progress");
    }

    fn assert_waiting(&self, id: u64) {
        let f = &self.gather.f;
        let root = f.backend.deferred_compute_v1(id).unwrap();
        assert!(root.route.is_none());
        assert_eq!(root.status, BackendPollV1::Pending);
        assert!(
            f.backend
                .peer_launch_retains
                .retains(*self.gather.peers.last().unwrap())
        );
        assert!(!f.backend.children[1].any_compute_active_v1());
        assert!(f.backend.children[1].pending_compute.is_empty());
        for allocation in [
            f.allocations[1][3],
            f.allocations[1][1],
            f.allocations[1][2],
        ] {
            let route = f.backend.allocations[&allocation];
            assert!(
                !f.backend.children[1]
                    .allocation_custody
                    .contains_key(&route.local)
            );
        }
        f.backend.assert_deferred_compute_indexes_consistent_v1();
    }
}

#[test]
fn gathered_frame_compute_latest_only_final_readback_preserves_all_native_owners() {
    for late in [false, true] {
        let mut c = Consumer::new(true, true);
        if late {
            c.publish_oldest();
        }
        let consumer = c.admit(0);
        c.assert_waiting(consumer);
        let g = &mut c.gather;
        let consumer_event = g.f.event(1, consumer);
        g.f.backend.compute_xgmi_routes.insert(
            (1, 0),
            Route::Scripted {
                failure: None,
                unwind: false,
                pending_samples: 2,
            },
        );
        let stream = g.f.backend.create_stream_v1(7).unwrap();
        let outgoing =
            g.f.backend
                .peer_copy_v1(
                    stream,
                    BackendMemoryRegionV1 {
                        allocation: g.f.allocations[1][2],
                        access: RuntimeAccessV1::Read,
                        byte_offset: 17,
                        byte_len: 47,
                    },
                    BackendMemoryRegionV1 {
                        allocation: g.f.allocations[0][3],
                        access: RuntimeAccessV1::Write,
                        byte_offset: 7,
                        byte_len: 47,
                    },
                    &[consumer_event],
                )
                .unwrap();
        let outgoing_event = g.f.backend.record_event_v1(stream, outgoing).unwrap();
        let host = g.f.host.unwrap();
        let readback =
            g.f.backend
                .copy_async_v1(
                    g.f.readback_stream,
                    BackendMemoryRegionV1 {
                        allocation: g.f.allocations[0][3],
                        access: RuntimeAccessV1::Read,
                        byte_offset: 7,
                        byte_len: 47,
                    },
                    BackendMemoryRegionV1 {
                        allocation: host,
                        access: RuntimeAccessV1::Write,
                        byte_offset: 9,
                        byte_len: 47,
                    },
                    &[outgoing_event],
                )
                .unwrap();
        g.f.backend.release_event_v1(outgoing_event).unwrap();
        g.f.backend.release_event_v1(consumer_event).unwrap();
        g.release_events();
        let generation = g.f.backend.cooperative_progress_generation;
        for id in [consumer, outgoing, readback] {
            assert_eq!(g.f.backend.poll_v1(id).unwrap(), BackendPollV1::Pending);
            assert_eq!(
                g.f.backend.wait_v1(id, Instant::now()).unwrap(),
                BackendPollV1::Pending
            );
            assert_eq!(
                g.f.backend.drain_v1(id, Instant::now()).unwrap(),
                BackendPollV1::Pending
            );
        }
        assert_eq!(g.f.backend.cooperative_progress_generation, generation);
        let readback_stream = g.f.readback_stream;
        g.drive(readback_stream, readback);
        for id in [consumer, outgoing, readback]
            .into_iter()
            .chain(g.peers.iter().copied())
        {
            assert_eq!(g.f.backend.poll_v1(id).unwrap(), BackendPollV1::Succeeded);
        }
        assert_eq!(
            gathered_owner(&g.f),
            (g.destination_id, g.expected.as_slice())
        );
        assert_eq!(
            restored(&g.f, g.f.allocations[1][2]),
            (c.output_id, c.output_bytes.as_slice())
        );
        c.return_bytes[7..54].copy_from_slice(&c.output_bytes[17..64]);
        assert_eq!(
            restored(&g.f, g.f.allocations[0][3]),
            (c.return_id, c.return_bytes.as_slice())
        );
        let host_route = g.f.backend.allocations[&host];
        let KfdRuntimeSdmaStorageV1::Host(owner) =
            &g.f.backend.children[0].allocations[&host_route.local].sdma_storage
        else {
            unreachable!()
        };
        let mut expected_host = vec![0; BYTES];
        expected_host[9..56].copy_from_slice(&c.output_bytes[17..64]);
        assert_eq!(owner.scripted_bytes().unwrap(), expected_host);
        assert!(g.f.backend.deferred_compute_retains.is_empty());
        assert!(
            !g.f.backend
                .peer_launch_retains
                .retains(*g.peers.last().unwrap())
        );
        assert_eq!(g.f.backend.completed_compute_xgmi_copies, 0);
        c.gather.f.clean();
    }
}

#[test]
fn gathered_frame_compute_full_contained_and_alias_reads_cancel_without_child_custody() {
    for shape in 0..3 {
        let mut c = Consumer::new(false, false);
        let account = fe2o3_resource_accounting::ResourceCreditAccountV1::new(
            fe2o3_resource_accounting::ResourceVectorV1::ZERO.with(
                fe2o3_resource_accounting::ResourceKindV1::ControlResidentBytes,
                4096,
            ),
            4,
        )
        .unwrap();
        c.gather.f.backend.children[1].launch_payload_account = Some(account.clone());
        let before = account.usage();
        let consumer = c.admit(shape);
        assert_eq!(
            account.usage().retained_records,
            before.retained_records + 1
        );
        c.assert_waiting(consumer);
        assert!(
            c.gather
                .f
                .backend
                .release_submission_v1(*c.gather.peers.last().unwrap())
                .is_err()
        );
        c.cancel_unissued(Some(consumer));
        assert_eq!(account.usage(), before);
    }
}

#[test]
fn gathered_frame_compute_late_ancestor_requires_both_native_reservations() {
    let mut c = Consumer::new(true, false);
    c.publish_oldest();
    let oldest = c.gather.peers[0];
    c.gather.f.backend.compute_xgmi_children[0] = None;
    let bindings = c.bindings(0);
    let dependency = c.dependency();
    let before = (
        c.gather.f.backend.next_handle,
        c.gather.f.backend.submissions.len(),
    );
    assert!(matches!(
        c.submit(&bindings, &[dependency]),
        Err(RuntimeBackendFailureV1::Rejected(_))
    ));
    assert_eq!(
        (
            c.gather.f.backend.next_handle,
            c.gather.f.backend.submissions.len()
        ),
        before
    );
    assert!(c.gather.f.backend.deferred_compute_retains.is_empty());
    assert!(!c.gather.f.backend.terminal);
    c.gather.f.backend.compute_xgmi_children[0] = Some(oldest);
    let consumer = c.admit(0);
    c.gather.release_events();
    for _ in 0..16 {
        if c.gather.f.copy(oldest).status() == BackendPollV1::Succeeded {
            break;
        }
        assert_eq!(
            c.gather
                .f
                .backend
                .progress_deferred_compute_v1(consumer)
                .unwrap(),
            BackendPollV1::Pending
        );
        c.assert_waiting(consumer);
    }
    assert_eq!(c.gather.f.copy(oldest).status(), BackendPollV1::Succeeded);
    assert!(c.gather.f.copy(oldest).is_quiescent());
    assert!(
        c.gather
            .f
            .backend
            .compute_xgmi_children
            .iter()
            .all(Option::is_none)
    );
    let stream = c.gather.f.compute_streams[1];
    c.gather.drive(stream, consumer);
    assert_eq!(
        c.gather.f.backend.poll_v1(consumer).unwrap(),
        BackendPollV1::Succeeded
    );
    assert_eq!(
        gathered_owner(&c.gather.f),
        (c.gather.destination_id, c.gather.expected.as_slice())
    );
    c.gather.f.clean();
}

#[test]
fn gathered_frame_compute_rejects_stale_event_writable_alias_and_bad_extent_before_retains() {
    for case in 0..5 {
        let mut c = Consumer::new(false, false);
        let mut bindings = c.bindings(0);
        let mut dependency = c.dependency();
        match case {
            0 => {
                dependency = BackendLaunchProducerV1 {
                    event: c.gather.peer_events[0],
                    producer_submission: c.gather.peers[0],
                }
            }
            1 => bindings[0].region.access = RuntimeAccessV1::ReadWrite,
            2 => bindings[0].region.byte_len += 1,
            3 => {
                bindings[1].region = BackendMemoryRegionV1 {
                    access: RuntimeAccessV1::Write,
                    ..bindings[0].region
                }
            }
            4 => {
                let route = c.gather.f.backend.allocations[&c.gather.f.allocations[1][3]];
                c.gather.f.backend.children[1]
                    .allocations
                    .get_mut(&route.local)
                    .unwrap()
                    .sdma_initialized = false;
            }
            _ => unreachable!(),
        }
        let before = (
            c.gather.f.backend.next_handle,
            c.gather.f.backend.submissions.len(),
            c.gather
                .f
                .backend
                .cooperative_dependency_retain_counts
                .clone(),
        );
        assert!(
            matches!(
                c.submit(&bindings, &[dependency]),
                Err(RuntimeBackendFailureV1::Rejected(_))
            ),
            "case {case}"
        );
        assert_eq!(
            (
                c.gather.f.backend.next_handle,
                c.gather.f.backend.submissions.len(),
                c.gather
                    .f
                    .backend
                    .cooperative_dependency_retain_counts
                    .clone()
            ),
            before
        );
        assert!(c.gather.f.backend.deferred_compute_retains.is_empty());
        assert!(
            !c.gather
                .f
                .backend
                .peer_launch_retains
                .retains(dependency.producer_submission)
        );
        assert!(!c.gather.f.backend.terminal);
        let route = c.gather.f.backend.allocations[&c.gather.f.allocations[1][3]];
        c.gather.f.backend.children[1]
            .allocations
            .get_mut(&route.local)
            .unwrap()
            .sdma_initialized = true;
        c.cancel_unissued(None);
    }
}

#[test]
fn gathered_frame_compute_settled_old_source_can_be_released_before_consumer_handoff() {
    let mut c = Consumer::new(true, false);
    let consumer = c.admit(0);
    c.gather.release_events();
    let oldest = c.gather.peers[0];
    assert_eq!(
        c.gather
            .f
            .backend
            .drain_v1(oldest, Instant::now() + std::time::Duration::from_secs(1))
            .unwrap(),
        BackendPollV1::Succeeded
    );
    let source = c.gather.f.allocations[0][2];
    c.gather.f.backend.release_allocation_v1(source).unwrap();
    c.assert_waiting(consumer);
    let stream = c.gather.f.compute_streams[1];
    c.gather.drive(stream, consumer);
    assert_eq!(
        c.gather.f.backend.poll_v1(consumer).unwrap(),
        BackendPollV1::Succeeded
    );
    assert_eq!(
        gathered_owner(&c.gather.f),
        (c.gather.destination_id, c.gather.expected.as_slice())
    );
    c.gather.f.clean_except(&[source]);
}

#[test]
fn gathered_frame_compute_cancelled_tail_fails_without_child_handoff_and_refunds() {
    let mut c = Consumer::new(false, false);
    let consumer = c.admit(0);
    c.gather.release_events();
    let tail = *c.gather.peers.last().unwrap();
    assert_eq!(
        c.gather.f.backend.cancel_v1(tail).unwrap(),
        BackendCancellationV1::Cancelled
    );
    assert!(matches!(
        c.gather
            .f
            .backend
            .progress_deferred_compute_v1(consumer)
            .unwrap(),
        BackendPollV1::Failed { .. }
    ));
    assert!(
        c.gather
            .f
            .backend
            .deferred_compute_v1(consumer)
            .unwrap()
            .route
            .is_none()
    );
    assert!(c.gather.f.backend.deferred_compute_retains.is_empty());
    assert!(!c.gather.f.backend.peer_launch_retains.retains(tail));
    assert!(!c.gather.f.backend.children[1].any_compute_active_v1());
    for peer in c.gather.peers[..2].iter().copied().rev() {
        assert_eq!(
            c.gather.f.backend.cancel_v1(peer).unwrap(),
            BackendCancellationV1::Cancelled
        );
    }
    for producer in c.gather.producers.into_iter().rev() {
        assert_eq!(
            c.gather.f.backend.cancel_v1(producer).unwrap(),
            BackendCancellationV1::Cancelled
        );
    }
    c.gather.f.clean();
}

#[test]
fn gathered_frame_compute_retained_rank_window_and_frame_identity_drift_terminalize() {
    for case in 0..4 {
        let mut c = Consumer::new(false, false);
        let consumer = c.admit(0);
        c.gather.release_events();
        let first = c.gather.peers[0];
        if case == 3 {
            let route = c.gather.f.backend.allocations[&c.gather.f.allocations[1][3]];
            c.gather.f.backend.children[1]
                .allocations
                .get_mut(&route.local)
                .unwrap()
                .bytes = vec![0; DESTINATION_BYTES + 1].into();
        } else {
            let target = if case == 2 {
                *c.gather.peers.last().unwrap()
            } else {
                first
            };
            let RoutedSubmissionV1::CooperativeCopy(copy) =
                c.gather.f.backend.submissions.get_mut(&target).unwrap()
            else {
                unreachable!()
            };
            match case {
                0 => copy.dependency_depth = MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1,
                1 => copy.destination_region.byte_offset += 1,
                2 => copy.source_region.byte_offset += 1,
                _ => unreachable!(),
            }
        }
        assert!(matches!(
            c.gather.f.backend.progress_deferred_compute_v1(consumer),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(c.gather.f.backend.terminal);
        assert!(c.gather.f.backend.children[1].terminal);
        assert!(
            c.gather
                .f
                .backend
                .deferred_compute_v1(consumer)
                .unwrap()
                .route
                .is_none()
        );
        assert!(
            c.gather
                .f
                .backend
                .peer_launch_retains
                .retains(*c.gather.peers.last().unwrap())
        );
        assert!(!c.gather.f.backend.deferred_compute_retains.is_empty());
        assert!(
            c.gather.f.backend.children.iter().all(|child| child
                .scripted_sdma
                .as_ref()
                .unwrap()
                .live_owner_count()
                == ALLOCATIONS)
        );
    }
}

#[test]
fn gathered_frame_compute_native_ancestor_error_or_unwind_retains_consumer_and_chain() {
    for unwind in [false, true] {
        let mut f = Fixture::with_layout([BYTES, DESTINATION_BYTES], None, false);
        f.backend.compute_xgmi_routes.insert(
            (0, 1),
            Route::Scripted {
                failure: Some(Stage::Poll),
                unwind,
                pending_samples: 0,
            },
        );
        let output = f.allocations[1][2];
        let returned = f.allocations[0][3];
        let (output_id, output_bytes) = initialize_bytes(&mut f, output, 0x49);
        let (return_id, return_bytes) = initialize_bytes(&mut f, returned, 0xaf);
        let mut gather = Gather::with_fixture(f);
        gather.chain(true);
        let mut c = Consumer {
            gather,
            output_id,
            output_bytes,
            return_id,
            return_bytes,
        };
        let consumer = c.admit(0);
        c.gather.release_events();
        let result = catch_unwind(AssertUnwindSafe(|| {
            for _ in 0..96 {
                c.gather.f.backend.progress_deferred_compute_v1(consumer)?;
            }
            Ok::<_, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>(())
        }));
        if unwind {
            assert!(result.is_err());
        } else {
            assert!(matches!(
                result.unwrap(),
                Err(RuntimeBackendFailureV1::Terminal(_))
            ));
        }
        assert!(c.gather.f.backend.terminal);
        assert!(
            c.gather
                .f
                .backend
                .children
                .iter()
                .all(|child| child.terminal)
        );
        assert!(
            c.gather
                .f
                .backend
                .deferred_compute_v1(consumer)
                .unwrap()
                .route
                .is_none()
        );
        assert!(
            c.gather
                .f
                .backend
                .peer_launch_retains
                .retains(*c.gather.peers.last().unwrap())
        );
        assert!(!c.gather.f.backend.deferred_compute_retains.is_empty());
        assert!(
            c.gather
                .f
                .backend
                .compute_xgmi_children
                .iter()
                .all(|owner| *owner == Some(c.gather.peers[0]))
        );
        assert!(
            c.gather.f.backend.children.iter().all(|child| child
                .scripted_sdma
                .as_ref()
                .unwrap()
                .live_owner_count()
                == ALLOCATIONS)
        );
    }
}
