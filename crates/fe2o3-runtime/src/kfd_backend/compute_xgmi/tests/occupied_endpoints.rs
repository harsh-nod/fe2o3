use super::*;

#[test]
fn native_route_occupied_children_reject_mutation_but_allow_bookkeeping() {
    let mut f = Fixture::configured(None, false, 2, 4);
    let unrelated = [f.unrelated(0), f.unrelated(1), f.unrelated(2)];
    let (completed, event, _) = f.producer(BackendPollV1::Succeeded);
    let copy = f.submit(&[]);
    f.backend.flush_stream_v1(f.stream).unwrap();
    let trace = f.root(copy).trace.clone();
    assert_eq!(
        f.backend.poll_v1(completed).unwrap(),
        BackendPollV1::Succeeded
    );
    assert_eq!(
        f.backend.wait_v1(completed, Instant::now()).unwrap(),
        BackendPollV1::Succeeded
    );
    f.backend.release_event_v1(event).unwrap();
    f.backend.release_submission_v1(completed).unwrap();
    for (child, allocation) in unrelated[..2].iter().copied().enumerate() {
        let device = 7 + child as u64;
        let stream = f.backend.create_stream_v1(device).unwrap();
        let module = f
            .backend
            .load_module_v1(device, &crate::synthetic_cov6::module())
            .unwrap();
        let kernel = f
            .backend
            .resolve_kernel_v1(module, "vecadd", [7; 32])
            .unwrap();
        busy(
            f.backend
                .allocate_v1(device, RuntimeMemoryKindV1::HostVisible, 8, 8),
        );
        busy(f.backend.release_allocation_v1(allocation));
        busy(f.backend.write_allocation_v1(allocation, 0, &[0]));
        busy(f.backend.read_allocation_v1(allocation, 0, &mut [0]));
        busy(f.backend.unload_module_v1(module));
        let geometry = crate::RuntimeLaunchGeometryV1 {
            grid: [1; 3],
            workgroup: [1; 3],
            dynamic_shared_bytes: 0,
        };
        busy(f.backend.submit_v1(BackendLaunchV1 {
            stream,
            kernel,
            explicit_kernarg: &[],
            bindings: &[],
            dependencies: &[],
            geometry,
            semantic_launch: BackendSemanticLaunchV1::Ordinary,
        }));
        busy(
            f.backend
                .submit_producer_aware_launch_v1(BackendProducerAwareLaunchV1 {
                    stream,
                    kernel,
                    explicit_kernarg: &[],
                    bindings: &[],
                    dependencies: &[],
                    geometry,
                }),
        );
        let region = BackendMemoryRegionV1 {
            allocation,
            access: RuntimeAccessV1::Read,
            byte_offset: 0,
            byte_len: 8,
        };
        busy(f.backend.copy_async_v1(
            stream,
            region,
            BackendMemoryRegionV1 {
                access: RuntimeAccessV1::Write,
                ..region
            },
            &[],
        ));
        f.backend.flush_stream_v1(stream).unwrap();
        f.backend.destroy_stream_v1(stream).unwrap();
    }
    f.backend
        .write_allocation_v1(unrelated[2], 0, &[0x45])
        .unwrap();
    let mut observed = [0];
    f.backend
        .read_allocation_v1(unrelated[2], 0, &mut observed)
        .unwrap();
    assert_eq!(observed, [0x45]);
    assert_eq!(f.root(copy).trace, trace);
    assert_eq!(
        f.backend.compute_xgmi_children,
        [Some(copy), Some(copy), None, None]
    );
    assert_eq!(
        f.backend
            .drain_v1(copy, Instant::now() + Duration::from_secs(1))
            .unwrap(),
        BackendPollV1::Succeeded
    );
    for allocation in unrelated[..2].iter().copied() {
        f.backend
            .write_allocation_v1(allocation, 0, &[0x31])
            .unwrap();
    }
    f.clean();
}

#[test]
fn native_route_occupied_dependency_observation_defers_without_failing_copy() {
    // The native write leg allocates and retires scratch even for a HostVisible endpoint.
    let mut f = Fixture::with_destination_steps(
        None,
        false,
        2,
        2,
        vec![
            ScriptedSdmaStepV1::Allocate {
                kind: ScriptedBufferKindV1::Host,
                byte_len: 8,
            },
            ScriptedSdmaStepV1::Write {
                offset: 0,
                byte_len: 8,
            },
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
        ],
    );
    let source = f.unrelated(0);
    let destination = f.unrelated(1);
    f.backend
        .write_allocation_v1(source, 0, &[0x6d; 8])
        .unwrap();
    let other_stream = f.backend.create_stream_v1(8).unwrap();
    let (producer, event, producer_route) = f.producer(BackendPollV1::Pending);
    let copy = f.submit(&[]);
    let dependent = f
        .backend
        .peer_copy_v1(
            other_stream,
            BackendMemoryRegionV1 {
                allocation: source,
                access: RuntimeAccessV1::Read,
                byte_offset: 0,
                byte_len: 8,
            },
            BackendMemoryRegionV1 {
                allocation: destination,
                access: RuntimeAccessV1::Write,
                byte_offset: 0,
                byte_len: 8,
            },
            &[event],
        )
        .unwrap();
    f.backend.flush_stream_v1(f.stream).unwrap();
    f.backend.flush_stream_v1(other_stream).unwrap();
    assert_eq!(
        f.backend.poll_v1(dependent).unwrap(),
        BackendPollV1::Pending
    );
    assert_eq!(f.copy(dependent).dependency_cursor, 0);
    f.backend.children[producer_route.child]
        .submissions
        .get_mut(&producer_route.local)
        .unwrap()
        .status = BackendPollV1::Succeeded;
    f.backend.flush_stream_v1(other_stream).unwrap();
    assert_eq!(f.copy(dependent).phase, CooperativeCopyPhaseV1::Read);
    assert_eq!(
        f.backend.poll_v1(dependent).unwrap(),
        BackendPollV1::Pending
    );
    assert_eq!(
        f.backend.poll_v1(producer).unwrap(),
        BackendPollV1::Succeeded
    );
    assert_eq!(f.root(copy).trace.len(), 3);
    f.backend.assert_cooperative_indexes_consistent();
    assert_eq!(
        f.backend
            .drain_v1(copy, Instant::now() + Duration::from_secs(1))
            .unwrap(),
        BackendPollV1::Succeeded
    );
    f.backend.flush_stream_v1(other_stream).unwrap();
    assert_eq!(
        f.backend.poll_v1(dependent).unwrap(),
        BackendPollV1::Succeeded
    );
    let mut copied = [0; 8];
    f.backend
        .read_allocation_v1(destination, 0, &mut copied)
        .unwrap();
    assert_eq!(copied, [0x6d; 8]);
    assert_eq!(
        f.backend.children[1]
            .scripted_sdma
            .as_ref()
            .unwrap()
            .remaining_steps(),
        2
    );
    f.clean();
}

#[test]
fn native_route_disjoint_pairs_progress_independently_through_flush_and_drain() {
    let mut f = Fixture::configured(None, false, 2, 4);
    let allocation = |child| {
        f.backend
            .allocations
            .iter()
            .find_map(|(id, route)| (route.child == child).then_some(*id))
            .unwrap()
    };
    let source = allocation(2);
    let destination = allocation(3);
    let other_stream = f.backend.create_stream_v1(10).unwrap();
    let first = f.submit(&[]);
    let second = f
        .backend
        .peer_copy_v1(
            other_stream,
            BackendMemoryRegionV1 {
                allocation: source,
                access: RuntimeAccessV1::Read,
                byte_offset: 0,
                byte_len: BYTES as u64,
            },
            BackendMemoryRegionV1 {
                allocation: destination,
                access: RuntimeAccessV1::Write,
                byte_offset: 0,
                byte_len: BYTES as u64,
            },
            &[],
        )
        .unwrap();
    f.backend.flush_stream_v1(f.stream).unwrap();
    f.backend.flush_stream_v1(other_stream).unwrap();
    assert_eq!(
        f.backend.compute_xgmi_children,
        [Some(first), Some(first), Some(second), Some(second)]
    );
    let first_trace = f.root(first).trace.clone();
    assert_eq!(
        f.backend
            .drain_v1(second, Instant::now() + Duration::from_secs(1))
            .unwrap(),
        BackendPollV1::Succeeded
    );
    assert_eq!(
        f.backend.compute_xgmi_children,
        [Some(first), Some(first), None, None]
    );
    assert_eq!(f.root(first).trace, first_trace);
    assert_eq!(f.backend.poll_v1(first).unwrap(), BackendPollV1::Pending);
    f.backend.assert_cooperative_indexes_consistent();
    assert_eq!(
        f.backend
            .drain_v1(first, Instant::now() + Duration::from_secs(1))
            .unwrap(),
        BackendPollV1::Succeeded
    );
    assert!(f.backend.compute_xgmi_children.iter().all(Option::is_none));
    f.clean();
}

#[test]
fn native_route_retained_slot_or_reservation_mismatch_poison_both_without_retirement() {
    for reservation in [false, true] {
        let mut f = Fixture::configured(None, false, 1, 2);
        let copy = f.submit(&[]);
        f.backend.flush_stream_v1(f.stream).unwrap();
        if reservation {
            f.backend.compute_xgmi_children[1] = Some(copy + 1);
        } else {
            f.record_mut(false).sdma_storage =
                KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::ComputeXgmi(copy + 1));
        }
        assert!(matches!(
            f.backend.progress_cooperative_copy(copy),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(f.backend.terminal && f.backend.children.iter().all(|child| child.terminal));
        assert_eq!(
            f.root(copy).trace,
            [Stage::Create, Stage::Copy, Stage::Poll]
        );
        assert!(f.root(copy).scripted_owners.iter().all(Option::is_some));
        assert!(f.root(copy).shells.iter().all(Option::is_some));
    }
}
