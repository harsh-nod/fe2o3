//! Router scheduling/custody tests; packet IO and fences are exercised in KFD.

use super::*;

const PACKET_BYTES: usize = GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 as usize;
const TWO_PACKET_BYTES: usize = PACKET_BYTES + BYTES;

fn fixture(failure: Option<Stage>, unwind: bool, pending_samples: usize) -> Fixture {
    Fixture::with_bytes(
        failure,
        unwind,
        pending_samples,
        2,
        TWO_PACKET_BYTES,
        Vec::new(),
    )
}

fn begin(f: &mut Fixture, copy: u64) {
    for _ in 0..2 {
        assert_eq!(
            f.backend.progress_cooperative_copy(copy).unwrap(),
            BackendPollV1::Pending
        );
    }
    assert_eq!(f.root(copy).trace, [Stage::Create, Stage::Copy]);
    assert_eq!(f.root(copy).phase, Phase::Published);
}

fn assert_held(f: &Fixture, copy: u64) {
    let root = f.root(copy);
    assert!(!root.is_quiescent());
    assert!(root.scripted_owners.iter().all(Option::is_some));
    assert!(root.shells.iter().all(Option::is_some));
    assert_eq!(f.backend.compute_xgmi_children, [Some(copy), Some(copy)]);
    for source in [false, true] {
        assert!(matches!(
            f.record(source).sdma_storage,
            KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::ComputeXgmi(owner)) if owner == copy
        ));
    }
    assert_eq!(f.copy(copy).status(), BackendPollV1::Pending);
    assert_eq!(f.backend.completed_compute_xgmi_copies, 0);
    assert_eq!(f.backend.cooperative_staging_bytes, 0);
    assert!(!root.trace.contains(&Stage::Finish));
    assert!(!root.trace.contains(&Stage::Retire));
    assert!(!root.trace.contains(&Stage::Restore));
}

fn assert_observers_unchanged(f: &mut Fixture, copy: u64) {
    let trace = f.root(copy).trace.clone();
    let packet = f.root(copy).packet_index;
    let between = f.root(copy).between_packets;
    let generation = f.backend.cooperative_progress_generation;
    for _ in 0..3 {
        assert_eq!(f.backend.poll_v1(copy).unwrap(), BackendPollV1::Pending);
        assert_eq!(
            f.backend.wait_v1(copy, Instant::now()).unwrap(),
            BackendPollV1::Pending
        );
        assert_eq!(
            f.backend.drain_v1(copy, Instant::now()).unwrap(),
            BackendPollV1::Pending
        );
    }
    assert_eq!(f.root(copy).trace, trace);
    assert_eq!(f.root(copy).packet_index, packet);
    assert_eq!(f.root(copy).between_packets, between);
    assert_eq!(f.backend.cooperative_progress_generation, generation);
}

#[test]
fn packetized_native_route_admits_bounded_full_extents_without_staging() {
    let limit = 4096 * PACKET_BYTES as u64;
    for (bytes, packets) in [
        (1, 1),
        (PACKET_BYTES as u64, 1),
        (TWO_PACKET_BYTES as u64, 2),
        (limit, 4096),
    ] {
        assert_eq!(
            Gfx942ComputeXgmiPacketPlanV1::new(bytes).unwrap().count(),
            packets
        );
    }
    for bytes in [0, limit + 1, u64::MAX] {
        assert!(Gfx942ComputeXgmiPacketPlanV1::new(bytes).is_none());
    }
    let mut f = fixture(None, false, 0);
    let copy = f.submit(&[]);
    assert_eq!(f.root(copy).packet_plan.count(), 2);
    assert_eq!(f.source.byte_len, TWO_PACKET_BYTES as u64);
    assert!(f.copy(copy).staging.is_empty());
    assert_eq!(f.copy(copy).scratch_byte_len, 0);
    assert_eq!(f.backend.cooperative_staging_bytes, 0);
    assert_eq!(
        f.backend.cancel_v1(copy).unwrap(),
        crate::BackendCancellationV1::Cancelled
    );
    assert!(f.root(copy).trace.is_empty());
    assert!(f.root(copy).is_quiescent());
    f.clean();
}

#[test]
fn packetized_native_route_keeps_partial_and_private_allocations_staged() {
    for private in [false, true] {
        let mut f = fixture(None, false, 0);
        if private {
            f.backend.children[0].peer_visible_device_allocations = false;
        } else {
            f.source.byte_len -= 1;
            f.destination.byte_len -= 1;
        }
        let copy = f.submit(&[]);
        assert!(f.copy(copy).compute_xgmi.is_none());
        assert_eq!(f.copy(copy).staging.len() as u64, f.source.byte_len);
        assert_eq!(
            f.backend.cancel_v1(copy).unwrap(),
            crate::BackendCancellationV1::Cancelled
        );
        f.clean();
    }
}

#[test]
fn packetized_native_route_samples_and_publications_have_exact_progress_generation() {
    let mut f = fixture(None, false, 2);
    let identities = [
        f.owner(true).scripted_owner_id(),
        f.owner(false).scripted_owner_id(),
    ];
    let copy = f.submit(&[]);
    begin(&mut f, copy);
    for packet in 0..2 {
        assert_eq!(f.root(copy).packet_index, packet);
        for _ in 0..2 {
            let generation = f.backend.cooperative_progress_generation;
            assert_eq!(
                f.backend.progress_cooperative_copy(copy).unwrap(),
                BackendPollV1::Pending
            );
            assert_eq!(f.backend.cooperative_progress_generation, generation);
            assert_held(&f, copy);
        }
        let generation = f.backend.cooperative_progress_generation;
        assert_eq!(
            f.backend.progress_cooperative_copy(copy).unwrap(),
            BackendPollV1::Pending
        );
        assert_eq!(f.backend.cooperative_progress_generation, generation + 1);
        assert_held(&f, copy);
        assert_observers_unchanged(&mut f, copy);
        assert_eq!(
            f.backend.cancel_v1(copy).unwrap(),
            crate::BackendCancellationV1::TooLate
        );
        busy(f.backend.release_submission_v1(copy));
        busy(f.backend.release_allocation_v1(f.source.allocation));
        busy(f.backend.release_allocation_v1(f.destination.allocation));
        busy(f.backend.shutdown_native_v1());
        if packet == 0 {
            assert!(f.root(copy).between_packets);
            assert_eq!(f.root(copy).phase, Phase::Published);
            let generation = f.backend.cooperative_progress_generation;
            assert_eq!(
                f.backend.progress_cooperative_copy(copy).unwrap(),
                BackendPollV1::Pending
            );
            assert_eq!(f.backend.cooperative_progress_generation, generation + 1);
            assert_eq!(f.root(copy).trace.last(), Some(&Stage::NextPacket));
            assert!(!f.root(copy).between_packets);
        } else {
            assert_eq!(f.root(copy).phase, Phase::Ready);
        }
    }
    assert_eq!(
        f.root(copy)
            .scripted_owners
            .each_ref()
            .map(|owner| owner.as_ref().unwrap().scripted_owner_id()),
        identities
    );
    assert_eq!(
        f.backend.progress_cooperative_copy(copy).unwrap(),
        BackendPollV1::Succeeded
    );
    assert_eq!(f.root(copy).phase, Phase::Retired);
    assert!(f.root(copy).is_quiescent());
    assert_eq!(f.backend.compute_xgmi_children, [None, None]);
    assert_eq!(
        [
            f.owner(true).scripted_owner_id(),
            f.owner(false).scripted_owner_id()
        ],
        identities
    );
    assert!(
        f.owner(false)
            .scripted_bytes()
            .unwrap()
            .iter()
            .all(|byte| *byte == 0x53)
    );
    for stage in [
        Stage::Create,
        Stage::Copy,
        Stage::NextPacket,
        Stage::Finish,
        Stage::Retire,
        Stage::Restore,
    ] {
        assert_eq!(
            f.root(copy)
                .trace
                .iter()
                .filter(|seen| **seen == stage)
                .count(),
            1
        );
    }
    assert_eq!(
        f.root(copy)
            .trace
            .iter()
            .filter(|seen| **seen == Stage::Poll)
            .count(),
        6
    );
    // Scripted copies never claim a native hardware completion count.
    assert_eq!(f.backend.completed_compute_xgmi_copies, 0);
    f.clean();
}

#[test]
fn packetized_native_route_resumes_expired_drain_between_packets() {
    let mut f = fixture(None, false, 0);
    let copy = f.submit(&[]);
    begin(&mut f, copy);
    assert_eq!(
        f.backend.progress_cooperative_copy(copy).unwrap(),
        BackendPollV1::Pending
    );
    assert!(f.root(copy).between_packets);
    assert_held(&f, copy);
    assert_observers_unchanged(&mut f, copy);
    assert_eq!(
        f.backend
            .drain_v1(copy, Instant::now() + Duration::from_secs(1))
            .unwrap(),
        BackendPollV1::Succeeded
    );
    assert_eq!(
        f.root(copy).trace,
        [
            Stage::Create,
            Stage::Copy,
            Stage::Poll,
            Stage::NextPacket,
            Stage::Poll,
            Stage::Finish,
            Stage::Retire,
            Stage::Restore
        ]
    );
    assert!(f.root(copy).is_quiescent());
    assert_eq!(f.backend.compute_xgmi_children, [None, None]);
    f.clean();
}

#[test]
fn packetized_native_route_next_publication_error_and_unwind_retain_original_owners() {
    for unwind in [false, true] {
        let mut f = fixture(Some(Stage::NextPacket), unwind, 0);
        let identities = [
            f.owner(true).scripted_owner_id(),
            f.owner(false).scripted_owner_id(),
        ];
        let copy = f.submit(&[]);
        begin(&mut f, copy);
        assert_eq!(
            f.backend.progress_cooperative_copy(copy).unwrap(),
            BackendPollV1::Pending
        );
        assert!(f.root(copy).between_packets);
        let result = catch_unwind(AssertUnwindSafe(|| {
            f.backend.progress_cooperative_copy(copy)
        }));
        if unwind {
            assert!(result.is_err());
        } else {
            assert!(matches!(
                result.unwrap(),
                Err(RuntimeBackendFailureV1::Terminal(_))
            ));
        }
        assert!(f.backend.terminal);
        assert!(f.backend.children.iter().all(|child| child.terminal));
        assert_held(&f, copy);
        assert_eq!(f.root(copy).packet_index, 0);
        assert_eq!(
            f.root(copy).trace,
            [Stage::Create, Stage::Copy, Stage::Poll, Stage::NextPacket]
        );
        assert_eq!(
            f.root(copy)
                .scripted_owners
                .each_ref()
                .map(|owner| owner.as_ref().unwrap().scripted_owner_id()),
            identities
        );
        assert!(matches!(
            f.backend.shutdown_native_v1(),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        // Terminal roots intentionally remain retained, including both native slots.
    }
}

#[test]
fn packetized_native_route_restores_all_packets_before_deferred_consumer_handoff() {
    let mut f = fixture(None, false, 1);
    let module = f
        .backend
        .load_module_v1(8, &crate::synthetic_cov6::module())
        .unwrap();
    let kernel = f
        .backend
        .resolve_kernel_v1(module, "vecadd", [7; 32])
        .unwrap();
    // A retained pending child record prevents GPU compute publication after
    // handoff. This test proves router custody, not a native compute completion.
    let (gate_submission, gate_event, gate_route) = f.producer_on(1, BackendPollV1::Pending);
    let copy = f.submit(&[]);
    let copy_event = f.backend.record_event_v1(f.stream, copy).unwrap();
    let bindings = [BackendBindingV1 {
        region: BackendMemoryRegionV1 {
            access: RuntimeAccessV1::Read,
            byte_len: BYTES as u64,
            ..f.destination
        },
        kernarg_byte_offset: 0,
    }];
    let dependencies = [
        BackendLaunchProducerV1 {
            event: copy_event,
            producer_submission: copy,
        },
        BackendLaunchProducerV1 {
            event: gate_event,
            producer_submission: gate_submission,
        },
    ];
    let consumer = f
        .backend
        .submit_producer_aware_launch_v1(BackendProducerAwareLaunchV1 {
            stream: f.stream,
            kernel,
            explicit_kernarg: &[19; 16],
            bindings: &bindings,
            dependencies: &dependencies,
            geometry: crate::RuntimeLaunchGeometryV1 {
                grid: [64, 1, 1],
                workgroup: [64, 1, 1],
                dynamic_shared_bytes: 0,
            },
        })
        .unwrap();
    f.backend.release_event_v1(copy_event).unwrap();
    f.backend.release_event_v1(gate_event).unwrap();
    let mut saw_between_packets = false;
    for _ in 0..16 {
        assert_eq!(
            f.backend.progress_deferred_compute_v1(consumer).unwrap(),
            BackendPollV1::Pending
        );
        let root = f.backend.deferred_compute_v1(consumer).unwrap();
        if let Some(route) = root.route {
            assert_eq!(f.copy(copy).status(), BackendPollV1::Succeeded);
            assert_eq!(f.root(copy).phase, Phase::Retired);
            assert!(f.root(copy).is_quiescent());
            assert_eq!(f.backend.compute_xgmi_children, [None, None]);
            let pending = &f.backend.children[route.child].pending_compute[&route.local];
            assert_eq!(&*pending.launch.explicit_kernarg, &[19; 16]);
            assert_eq!(&*pending.explicit_success_dependencies, &[gate_route.local]);
            assert!(!f.backend.children[route.child].any_compute_active_v1());
            break;
        }
        if f.copy(copy).status() == BackendPollV1::Pending && f.root(copy).phase != Phase::Prepared
        {
            assert_held(&f, copy);
            saw_between_packets |= f.root(copy).between_packets;
            let destination = f.backend.allocations[&f.destination.allocation];
            assert!(
                !f.backend.children[destination.child]
                    .allocation_custody
                    .contains_key(&destination.local)
            );
            assert!(
                f.backend.children[destination.child]
                    .pending_compute
                    .is_empty()
            );
            assert!(!f.backend.children[destination.child].any_compute_active_v1());
        }
    }
    assert!(saw_between_packets);
    assert!(
        f.backend
            .deferred_compute_v1(consumer)
            .unwrap()
            .route
            .is_some()
    );
    assert_eq!(f.backend.completed_compute_xgmi_copies, 0);
    assert_eq!(
        f.backend.cancel_v1(consumer).unwrap(),
        crate::BackendCancellationV1::Cancelled
    );
    f.backend.children[gate_route.child]
        .submissions
        .get_mut(&gate_route.local)
        .unwrap()
        .status = BackendPollV1::Failed { code: -2 };
    f.clean();
}
