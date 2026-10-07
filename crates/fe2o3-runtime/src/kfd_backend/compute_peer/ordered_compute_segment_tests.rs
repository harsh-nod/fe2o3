//! Mixed source-producer and destination-list custody using real scripted owners.
//! Scripted compute retains its input bytes; these tests do not emulate arithmetic.

use super::*;

struct Mixed {
    p: PendingList,
    first: u64,
    second_producer: Option<u64>,
    second_event: Option<u64>,
    source: u64,
    bytes: Vec<u8>,
}

impl Mixed {
    fn new(first_compute: bool, second_compute: bool, direct: bool, readback: bool) -> Self {
        let mut p = PendingList::with_additional_source(
            first_compute.then_some((true, false)),
            direct,
            readback,
            readback,
            Some((5, 0x75)),
        );
        let first = p.list;
        let source = p.f.allocations[0][5];
        let bytes = (0..BYTES)
            .map(|index| 0x75_u8.wrapping_add((index as u8).wrapping_mul(13)))
            .collect();
        let second_producer = second_compute.then(|| p.f.launch(0, 3, true, true));
        let second_event = second_producer.map(|id| p.f.event(0, id));
        if !first_compute && second_compute {
            // The setup-only FIFO predecessor must retire before a settled
            // source list can acquire this child. Observing that exact active
            // result does not publish the queued source producer behind it.
            for predecessor in p.f.setup_predecessors.clone() {
                for _ in 0..32 {
                    if p.f.backend.poll_v1(predecessor).unwrap() == BackendPollV1::Succeeded {
                        break;
                    }
                }
                assert_eq!(
                    p.f.backend.poll_v1(predecessor).unwrap(),
                    BackendPollV1::Succeeded
                );
            }
            let producer = second_producer.unwrap();
            let route = p.f.native(producer);
            assert!(
                p.f.backend.children[0]
                    .pending_compute
                    .contains_key(&route.local)
            );
        }
        Self {
            p,
            first,
            second_producer,
            second_event,
            source,
            bytes,
        }
    }

    fn append(&mut self, predecessor_event: u64) -> Result<u64, Failure> {
        let mut events: Vec<_> = self.second_event.into_iter().collect();
        events.push(predecessor_event);
        let [_, destination] = regions(&self.p.f);
        self.p.f.backend.peer_copy_segments_v1(
            self.p.f.peer_stream,
            BackendMemoryRegionV1 {
                allocation: self.source,
                byte_offset: 3,
                byte_len: 53,
                access: RuntimeAccessV1::Read,
            },
            destination,
            &descriptors(),
            &events,
        )
    }

    fn admit(&mut self) {
        let event = self.p.event;
        let tail = self.append(event).unwrap();
        let mut expected: Vec<_> = self.second_producer.into_iter().collect();
        expected.push(self.first);
        assert_eq!(self.p.f.copy(tail).dependencies, expected);
        assert_eq!(
            self.p.f.copy(tail).compute_producer.is_some(),
            self.second_producer.is_some()
        );
        assert!(self.p.f.copy(tail).staging.is_empty());
        self.p.f.backend.release_event_v1(event).unwrap();
        self.p.list = tail;
        self.p.event = self
            .p
            .f
            .backend
            .record_event_v1(self.p.f.peer_stream, tail)
            .unwrap();
    }

    fn release_events(&mut self) {
        self.p.release_events();
        if let Some(event) = self.second_event.take() {
            self.p.f.backend.release_event_v1(event).unwrap();
        }
    }

    fn expected(&self) -> Vec<u8> {
        let mut expected = self.p.expected_input(descriptors().len());
        for segment in descriptors() {
            let from = 3 + segment.source_offset as usize;
            let to = 5 + segment.destination_offset as usize;
            let len = segment.byte_len as usize;
            expected[to..to + len].copy_from_slice(&self.bytes[from..from + len]);
        }
        expected
    }

    fn drive(&mut self, target: u64) {
        for _ in 0..512 {
            self.p
                .f
                .backend
                .progress_stream_v1(self.p.f.readback_stream)
                .unwrap();
            if self.p.f.backend.poll_v1(target).unwrap() != BackendPollV1::Pending {
                return;
            }
        }
        panic!("mixed ordered lists did not settle from the final readback stream");
    }
}

#[test]
fn ordered_compute_segments_mixed_origins_restore_exact_frame_from_final_readback() {
    for (first_compute, second_compute) in [(false, true), (true, false), (true, true)] {
        for late in [false, true] {
            let mut m = Mixed::new(first_compute, second_compute, true, true);
            if late {
                m.p.publish();
            }
            m.admit();
            let tail = m.p.list;
            let readback = m.p.direct_readback().unwrap();
            m.release_events();
            let generation = m.p.f.backend.cooperative_progress_generation;
            for id in [m.first, tail, readback] {
                assert_eq!(m.p.f.backend.poll_v1(id).unwrap(), BackendPollV1::Pending);
                assert_eq!(
                    m.p.f.backend.wait_v1(id, Instant::now()).unwrap(),
                    BackendPollV1::Pending
                );
                assert!(m.p.f.backend.release_submission_v1(id).is_err());
            }
            assert_eq!(m.p.f.backend.cooperative_progress_generation, generation);
            assert!(m.p.f.backend.submission_retained_as_dependency(m.first));
            if let Some(producer) = m.second_producer {
                assert!(m.p.f.backend.submission_retained_as_dependency(producer));
            }
            m.drive(readback);
            for id in [m.first, tail, readback] {
                assert_eq!(m.p.f.backend.poll_v1(id).unwrap(), BackendPollV1::Succeeded);
            }
            assert_eq!(
                owner(&m.p.f, m.p.f.allocations[0][2]),
                (m.p.identities[0], m.p.source.as_slice())
            );
            assert_eq!(owner(&m.p.f, m.source).1, m.bytes);
            assert_eq!(
                owner(&m.p.f, m.p.f.allocations[1][3]),
                (m.p.identities[1], m.expected().as_slice())
            );
            let host = m.p.f.backend.allocations[&m.p.f.host.unwrap()];
            let KfdRuntimeSdmaStorageV1::Host(host) =
                &m.p.f.backend.children[1].allocations[&host.local].sdma_storage
            else {
                unreachable!()
            };
            assert_eq!(host.scripted_bytes().unwrap(), m.expected());
            assert!(
                m.p.f
                    .backend
                    .compute_xgmi_children
                    .iter()
                    .all(Option::is_none)
            );
            m.p.f.clean();
        }
    }
}

#[test]
fn ordered_compute_segments_late_frame_compute_return_readback_preserves_two_producers() {
    for (first_compute, second_compute) in [(false, true), (true, false), (true, true)] {
        let mut m = Mixed::new(first_compute, second_compute, false, true);
        m.admit();
        for _ in 0..64 {
            m.p.f.backend.progress_cooperative_copy(m.first).unwrap();
            if !m
                .p
                .f
                .copy(m.first)
                .compute_xgmi
                .as_ref()
                .unwrap()
                .is_quiescent()
            {
                break;
            }
        }
        assert_eq!(
            m.p.f.backend.compute_xgmi_children,
            [Some(m.first), Some(m.first)]
        );
        let consumer = m.p.consumer();
        m.p.assert_deferred(consumer);
        // Allocation 3 remains a Read input of the second source compute.
        // Return into a distinct initialized owner while that producer is pending.
        let return_destination = m.p.f.allocations[0][6];
        let (returned, readback) =
            super::settled::return_and_readback_to(&mut m.p, consumer, return_destination);
        m.release_events();
        m.drive(readback);
        for id in [m.first, m.p.list, consumer, returned, readback] {
            assert_eq!(m.p.f.backend.poll_v1(id).unwrap(), BackendPollV1::Succeeded);
        }
        assert_eq!(
            owner(&m.p.f, m.p.f.allocations[1][3]),
            (m.p.identities[1], m.expected().as_slice())
        );
        assert_eq!(
            owner(&m.p.f, m.p.f.allocations[1][2]),
            (m.p.identities[2], m.p.output.as_slice())
        );
        assert!(m.p.f.backend.deferred_compute_retains.is_empty());
        m.p.f.clean();
    }
}

#[test]
fn ordered_compute_segments_restored_ancestor_releases_original_source_before_tail() {
    let mut m = Mixed::new(true, true, true, true);
    m.admit();
    let readback = m.p.direct_readback().unwrap();
    m.release_events();
    for _ in 0..128 {
        m.p.f.backend.progress_cooperative_copy(m.first).unwrap();
        if m.p.f.copy(m.first).is_quiescent() {
            break;
        }
    }
    assert_eq!(m.p.f.copy(m.first).status(), BackendPollV1::Succeeded);
    assert_eq!(m.p.f.copy(m.p.list).status(), BackendPollV1::Pending);
    assert!(
        m.p.f
            .backend
            .compute_xgmi_children
            .iter()
            .all(Option::is_none)
    );
    let source = m.p.f.allocations[0][2];
    assert_eq!(
        owner(&m.p.f, source),
        (m.p.identities[0], m.p.source.as_slice())
    );
    let producer = m.p.producer.unwrap();
    let producer_route = m.p.f.native(producer);
    assert!(
        m.p.f.backend.children[producer_route.child]
            .compute_dependency_retain_counts
            .contains_key(&producer_route.local)
    );
    assert!(matches!(m.p.f.backend.release_submission_v1(producer),
        Err(Failure::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Busy));
    assert!(m.p.f.backend.submissions.contains_key(&producer));
    m.p.f.backend.release_allocation_v1(source).unwrap();
    assert!(m.p.f.backend.release_submission_v1(m.first).is_err());
    m.drive(readback);
    assert_eq!(
        m.p.f.backend.poll_v1(readback).unwrap(),
        BackendPollV1::Succeeded
    );
    assert_eq!(owner(&m.p.f, m.p.f.allocations[1][3]).1, m.expected());
    assert!(
        !m.p.f.backend.children[producer_route.child]
            .compute_dependency_retain_counts
            .contains_key(&producer_route.local)
    );
    m.p.f.clean_except(&[source]);
}

#[test]
fn ordered_compute_segments_stale_predecessor_and_wrong_stream_reject_before_effect() {
    for wrong_stream in [false, true] {
        let mut m = Mixed::new(false, true, true, false);
        let stale = m.p.event;
        // Keep an extra public event naming the first root across normal admission.
        let stale_copy =
            m.p.f
                .backend
                .record_event_v1(m.p.f.peer_stream, m.first)
                .unwrap();
        m.admit();
        assert!(!m.p.f.backend.events.contains_key(&stale));
        let original_stream = m.p.f.peer_stream;
        if wrong_stream {
            m.p.f.peer_stream = m.p.f.backend.create_stream_v1(8).unwrap();
        }
        let before = (
            m.p.f.backend.next_handle,
            m.p.f.backend.submissions.len(),
            m.p.f.backend.cooperative_dependency_retain_counts.clone(),
        );
        let event = if wrong_stream { m.p.event } else { stale_copy };
        assert!(matches!(m.append(event), Err(Failure::Rejected(_))));
        assert_eq!(
            (
                m.p.f.backend.next_handle,
                m.p.f.backend.submissions.len(),
                m.p.f.backend.cooperative_dependency_retain_counts.clone()
            ),
            before
        );
        m.p.f.peer_stream = original_stream;
        m.p.f.backend.release_event_v1(stale_copy).unwrap();
        m.release_events();
        for id in [m.p.list, m.first, m.second_producer.unwrap()] {
            assert_eq!(
                m.p.f.backend.cancel_v1(id).unwrap(),
                BackendCancellationV1::Cancelled
            );
        }
        m.p.f.clean();
    }
}

#[test]
fn ordered_compute_segments_corrupt_independent_source_or_destination_identity_is_terminal() {
    for case in 0..5 {
        let mut m = Mixed::new(true, true, true, false);
        m.admit();
        m.release_events();
        let target = if case == 4 { m.first } else { m.p.list };
        let RoutedSubmissionV1::CooperativeCopy(copy) =
            m.p.f.backend.submissions.get_mut(&target).unwrap()
        else {
            unreachable!()
        };
        match case {
            0 => copy.dependency_depth += 1,
            1 => copy.source_region.byte_offset += 1,
            2 => {
                copy.dependencies.retain(|id| *id != m.first);
            }
            3 => {
                let root = copy.compute_xgmi.as_mut().unwrap();
                let old = root.segments_for_test_v1().unwrap();
                let changed = Arc::new(
                    Gfx942ComputeXgmiSegmentsPlanV1::new(
                        old.source_logical_bytes(),
                        old.destination_logical_bytes(),
                        old.source_offset(),
                        old.source_len(),
                        old.destination_offset(),
                        old.destination_len(),
                        &descriptors(),
                    )
                    .unwrap(),
                );
                root.replace_segments_for_test_v1(changed);
            }
            4 => copy.destination_region.byte_offset += 1,
            _ => unreachable!(),
        }
        assert!(matches!(
            m.p.f.backend.progress_stream_v1(m.p.f.peer_stream),
            Err(Failure::Terminal(_))
        ));
        assert!(m.p.f.backend.terminal);
        for id in [m.first, m.second_producer.unwrap()] {
            assert!(m.p.f.backend.submission_retained_as_dependency(id));
        }
        assert!(
            m.p.f
                .copy(m.p.list)
                .compute_xgmi
                .as_ref()
                .unwrap()
                .trace_for_test_v1()
                .is_empty()
        );
        // Corrupted fixtures intentionally retain their uncertain native owners.
    }
}

#[test]
fn ordered_compute_segments_cancelled_source_or_destination_never_extracts_tail() {
    for cancel_source in [false, true] {
        let mut m = Mixed::new(false, true, true, false);
        m.admit();
        m.release_events();
        let target = if cancel_source {
            m.second_producer.unwrap()
        } else {
            m.first
        };
        assert_eq!(
            m.p.f.backend.cancel_v1(target).unwrap(),
            BackendCancellationV1::Cancelled
        );
        for _ in 0..256 {
            match m.p.f.backend.progress_stream_v1(m.p.f.peer_stream) {
                Ok(()) => {}
                Err(Failure::Quiescent(_)) => assert!(m.p.f.copy(m.p.list).is_quiescent()),
                other => panic!("unexpected failed dependency progress: {other:?}"),
            }
            if m.p.f.copy(m.p.list).is_quiescent() {
                break;
            }
        }
        assert!(matches!(
            m.p.f.copy(m.p.list).status(),
            BackendPollV1::Failed { .. }
        ));
        assert!(m.p.f.copy(m.p.list).is_quiescent());
        assert!(
            m.p.f
                .copy(m.p.list)
                .compute_xgmi
                .as_ref()
                .unwrap()
                .trace_for_test_v1()
                .is_empty()
        );
        assert!(
            m.p.f
                .backend
                .compute_xgmi_children
                .iter()
                .all(Option::is_none)
        );
        if cancel_source {
            assert_eq!(
                m.p.f.backend.cancel_v1(m.first).unwrap(),
                BackendCancellationV1::Cancelled
            );
        }
        m.p.f.clean();
    }
}

#[test]
fn ordered_compute_segments_mixed_depth_limit_preserves_all_pre_effect_accounts() {
    let mut m = Mixed::new(false, true, true, false);
    m.admit();
    let mut ids = vec![m.first, m.p.list];
    while m.p.f.copy(m.p.list).dependency_depth < MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1 {
        let previous = m.p.list;
        let event = m.p.event;
        let tail = m.append(event).unwrap();
        assert_eq!(
            m.p.f.copy(tail).dependencies,
            [m.second_producer.unwrap(), previous]
        );
        assert_eq!(
            m.p.f.copy(tail).dependency_depth,
            m.p.f.copy(previous).dependency_depth + 1
        );
        assert_eq!(
            m.p.f
                .copy(tail)
                .compute_xgmi
                .as_ref()
                .unwrap()
                .segment_frame_v1()
                .unwrap()
                .depth(),
            m.p.f.copy(tail).dependency_depth
        );
        m.p.f.backend.release_event_v1(event).unwrap();
        m.p.list = tail;
        m.p.event =
            m.p.f
                .backend
                .record_event_v1(m.p.f.peer_stream, tail)
                .unwrap();
        ids.push(tail);
    }
    let before = (
        m.p.f.backend.next_handle,
        m.p.f.backend.submissions.len(),
        m.p.f.backend.cooperative_dependency_retain_counts.clone(),
        m.p.f.backend.cooperative_staging_bytes,
    );
    let event = m.p.event;
    assert!(
        matches!(m.append(event), Err(Failure::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity)
    );
    assert_eq!(
        (
            m.p.f.backend.next_handle,
            m.p.f.backend.submissions.len(),
            m.p.f.backend.cooperative_dependency_retain_counts.clone(),
            m.p.f.backend.cooperative_staging_bytes
        ),
        before
    );
    m.release_events();
    for id in ids.into_iter().rev() {
        assert_eq!(
            m.p.f.backend.cancel_v1(id).unwrap(),
            BackendCancellationV1::Cancelled
        );
        m.p.f.backend.release_submission_v1(id).unwrap();
    }
    assert_eq!(
        m.p.f.backend.cancel_v1(m.second_producer.unwrap()).unwrap(),
        BackendCancellationV1::Cancelled
    );
    assert!(
        m.p.f
            .backend
            .cooperative_dependency_retain_counts
            .is_empty()
    );
    m.p.f.clean();
}

#[test]
fn ordered_compute_segments_ancestor_native_fault_keeps_both_independent_roots() {
    for unwind in [false, true] {
        let mut m = Mixed::new(false, true, true, false);
        m.p.publish();
        m.admit();
        m.release_events();
        let RoutedSubmissionV1::CooperativeCopy(copy) =
            m.p.f.backend.submissions.get_mut(&m.first).unwrap()
        else {
            unreachable!()
        };
        copy.compute_xgmi
            .as_mut()
            .unwrap()
            .inject_failure_for_test_v1(Stage::NextSegment, unwind);
        let result = catch_unwind(AssertUnwindSafe(|| -> Result<(), Failure> {
            for _ in 0..128 {
                m.p.f.backend.progress_stream_v1(m.p.f.peer_stream)?;
            }
            Ok(())
        }));
        if unwind {
            assert!(result.is_err());
        } else {
            assert!(matches!(result.unwrap(), Err(Failure::Terminal(_))));
        }
        assert!(m.p.f.backend.terminal);
        assert_eq!(
            m.p.f.backend.compute_xgmi_children,
            [Some(m.first), Some(m.first)]
        );
        for id in [m.first, m.second_producer.unwrap()] {
            assert!(m.p.f.backend.submission_retained_as_dependency(id));
        }
        assert_eq!(m.p.f.copy(m.p.list).status(), BackendPollV1::Pending);
        assert!(
            m.p.f
                .copy(m.p.list)
                .compute_xgmi
                .as_ref()
                .unwrap()
                .trace_for_test_v1()
                .is_empty()
        );
        assert!(
            m.p.f
                .copy(m.first)
                .compute_xgmi
                .as_ref()
                .unwrap()
                .scripted_owners_for_test_v1()
                .iter()
                .all(Option::is_some)
        );
        assert_eq!(m.p.f.backend.cooperative_progress_quantum, None);
    }
}

#[test]
fn ordered_compute_segments_independent_third_child_progresses_without_retiring_ancestor() {
    let mut p = PendingList::with_source_profile(None, true, true, true);
    // Add a real scripted third child, with its own native compute owners and
    // teardown script. No result or completion state is manufactured here.
    let (mut child, local_stream, locals) = scripted_persistent_backend_with_steps_v1::<3>(
        BYTES,
        (0..3).flat_map(|_| release_steps_for(BYTES)),
    );
    child.description.backend_device = 9;
    child.peer_visible_device_allocations = true;
    child.scripted_persistent_poll_pending_observations = 3;
    for record in child.allocations.values_mut() {
        record.device = 9;
    }
    for device in child.streams.values_mut() {
        *device = 9;
    }
    p.f.backend.children.push(child);
    p.f.backend.device_children.insert(9, 2);
    p.f.backend.compute_xgmi_children.push(None);
    let allocations = locals.map(|local| {
        let global = p.f.backend.next_id().unwrap();
        p.f.backend
            .allocations
            .insert(global, RoutedHandleV1 { child: 2, local });
        global
    });
    let stream = p.f.backend.next_id().unwrap();
    p.f.backend.streams.insert(
        stream,
        RoutedHandleV1 {
            child: 2,
            local: local_stream,
        },
    );
    let module =
        p.f.backend
            .load_module_v1(9, &synthetic_cov6::three_binding_module())
            .unwrap();
    let kernel =
        p.f.backend
            .resolve_kernel_v1(module, "vecadd", [7; 32])
            .unwrap();
    let (source_id, bytes) = initialize_bytes(&mut p.f, allocations[2], 0x76);
    p.f.backend.compute_xgmi_routes.insert(
        (2, 1),
        Route::Scripted {
            failure: None,
            unwind: false,
            pending_samples: 2,
        },
    );
    let first = p.list;
    for _ in 0..64 {
        p.f.backend.progress_cooperative_copy(first).unwrap();
        if p.f
            .copy(first)
            .compute_xgmi
            .as_ref()
            .unwrap()
            .trace_for_test_v1()
            .contains(&Stage::Copy)
        {
            break;
        }
    }
    assert_eq!(
        p.f.backend.compute_xgmi_children,
        [Some(first), Some(first), None]
    );
    let before =
        p.f.copy(first)
            .compute_xgmi
            .as_ref()
            .unwrap()
            .trace_for_test_v1()
            .to_vec();
    let bindings = std::array::from_fn::<_, 3, _>(|index| BackendBindingV1 {
        region: region(
            allocations[index],
            if index == 2 {
                RuntimeAccessV1::Write
            } else {
                RuntimeAccessV1::Read
            },
        ),
        kernarg_byte_offset: index as u32 * 8,
    });
    let mut kernarg = [0; 32];
    kernarg[24..].copy_from_slice(&16_u64.to_le_bytes());
    let producer =
        p.f.backend
            .submit_producer_aware_launch_v1(BackendProducerAwareLaunchV1 {
                stream,
                kernel,
                bindings: &bindings,
                explicit_kernarg: &kernarg,
                dependencies: &[],
                geometry: crate::RuntimeLaunchGeometryV1 {
                    grid: [64, 1, 1],
                    workgroup: [64, 1, 1],
                    dynamic_shared_bytes: 0,
                },
            })
            .unwrap();
    let event = p.f.backend.record_event_v1(stream, producer).unwrap();
    let [_, destination] = regions(&p.f);
    let tail =
        p.f.backend
            .peer_copy_segments_v1(
                p.f.peer_stream,
                BackendMemoryRegionV1 {
                    allocation: allocations[2],
                    byte_offset: 3,
                    byte_len: 53,
                    access: RuntimeAccessV1::Read,
                },
                destination,
                &descriptors(),
                &[event, p.event],
            )
            .unwrap();
    p.f.backend.release_event_v1(event).unwrap();
    p.f.backend.release_event_v1(p.event).unwrap();
    p.list = tail;
    p.event = p.f.backend.record_event_v1(p.f.peer_stream, tail).unwrap();
    let readback = p.direct_readback().unwrap();
    p.release_events();
    let mut status = BackendPollV1::Pending;
    for _ in 0..32 {
        status =
            p.f.backend
                .progress_compute_peer_dependency_v1(tail, producer)
                .unwrap();
        if status != BackendPollV1::Pending {
            break;
        }
    }
    assert_eq!(status, BackendPollV1::Succeeded);
    assert_eq!(
        p.f.backend.compute_xgmi_children,
        [Some(first), Some(first), None]
    );
    assert_eq!(
        p.f.copy(first)
            .compute_xgmi
            .as_ref()
            .unwrap()
            .trace_for_test_v1(),
        before
    );
    for _ in 0..256 {
        p.f.backend.progress_stream_v1(p.f.readback_stream).unwrap();
        if p.f.backend.poll_v1(readback).unwrap() != BackendPollV1::Pending {
            break;
        }
    }
    assert_eq!(
        p.f.backend.poll_v1(readback).unwrap(),
        BackendPollV1::Succeeded
    );
    assert_eq!(owner(&p.f, allocations[2]), (source_id, bytes.as_slice()));
    p.f.backend.release_submission_v1(producer).unwrap();
    for allocation in allocations {
        p.f.backend.release_allocation_v1(allocation).unwrap();
    }
    p.f.backend.unload_module_v1(module).unwrap();
    p.f.clean();
}
