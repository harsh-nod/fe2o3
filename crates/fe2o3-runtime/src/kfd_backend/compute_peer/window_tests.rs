//! Real scripted custody and byte routing, not compute arithmetic or GPU execution.

use super::*;
use crate::kfd_backend::compute_peer::exact_full_writer;
use fe2o3_kfd::Gfx942ComputeXgmiCopyWindowV1;

const DESTINATION_BYTES: usize = 96;
const SOURCE_OFFSET: u64 = 17;
const DESTINATION_OFFSET: u64 = 11;
const COPY_BYTES: u64 = 47;

fn regions(
    f: &Fixture,
    source_offset: u64,
    destination_offset: u64,
    bytes: u64,
) -> [BackendMemoryRegionV1; 2] {
    [
        BackendMemoryRegionV1 {
            byte_offset: source_offset,
            byte_len: bytes,
            ..region(f.allocations[0][2], RuntimeAccessV1::Read)
        },
        BackendMemoryRegionV1 {
            byte_offset: destination_offset,
            byte_len: bytes,
            ..region(f.allocations[1][3], RuntimeAccessV1::Write)
        },
    ]
}

pub(super) fn initialize_bytes(f: &mut Fixture, allocation: u64, seed: u8) -> (u64, Vec<u8>) {
    let route = f.backend.allocations[&allocation];
    let child = &mut f.backend.children[route.child];
    assert!(!child.any_compute_active_v1());
    assert!(child.pending_compute.is_empty());
    let record = child.allocations.get_mut(&route.local).unwrap();
    let KfdRuntimeSdmaStorageV1::H2dReady(mut ready) =
        std::mem::replace(&mut record.sdma_storage, KfdRuntimeSdmaStorageV1::Synthetic)
    else {
        panic!("unissued fixture owner expected");
    };
    let mut owner = ready.owner.normalize();
    let bytes: Vec<_> = (0..record.bytes.len())
        .map(|index| seed.wrapping_add((index as u8).wrapping_mul(13)))
        .collect();
    owner.scripted_bytes_mut().unwrap().copy_from_slice(&bytes);
    let id = owner.scripted_owner_id().unwrap();
    let digest = Sha256::digest(&bytes).into();
    let DirectionalSdmaDeviceOwnerV1::Scripted(device) = owner else {
        unreachable!()
    };
    ready.owner = PersistentComputeReadyOwnerV1::Scripted {
        device,
        authenticated_sha256: digest,
    };
    record.bytes = bytes.clone().into();
    record.content_sha256 = Some(digest);
    record.sdma_storage = KfdRuntimeSdmaStorageV1::H2dReady(ready);
    (id, bytes)
}

pub(super) fn restored(f: &Fixture, allocation: u64) -> (u64, &[u8]) {
    let route = f.backend.allocations[&allocation];
    let KfdRuntimeSdmaStorageV1::Device(owner) =
        &f.backend.children[route.child].allocations[&route.local].sdma_storage
    else {
        panic!("whole original owner must be restored");
    };
    (
        owner.scripted_owner_id().unwrap(),
        owner.scripted_bytes().unwrap(),
    )
}

fn window(
    source_offset: u64,
    destination_offset: u64,
    bytes: u64,
) -> Gfx942ComputeXgmiCopyWindowV1 {
    Gfx942ComputeXgmiCopyWindowV1::new(
        BYTES as u64,
        DESTINATION_BYTES as u64,
        source_offset,
        destination_offset,
        bytes,
    )
    .unwrap()
}

fn admit(f: &mut Fixture, event: u64, regions: [BackendMemoryRegionV1; 2]) -> u64 {
    f.backend
        .peer_copy_v1(f.peer_stream, regions[0], regions[1], &[event])
        .unwrap()
}

fn publish(f: &mut Fixture, peer: u64) {
    for _ in 0..32 {
        f.backend.progress_cooperative_copy_step_v1(peer).unwrap();
        if !f.copy(peer).compute_xgmi.as_ref().unwrap().is_quiescent() {
            // The scripted creation step roots both owners; the next real
            // transfer step publishes the first packet without sampling it.
            f.backend.progress_cooperative_copy_step_v1(peer).unwrap();
            assert_eq!(f.copy(peer).status(), BackendPollV1::Pending);
            return;
        }
    }
    panic!("bounded scripted publication did not acquire owners");
}

#[test]
fn compute_peer_window_queued_and_eager_preserve_unequal_owners_and_all_guard_bytes() {
    for queued in [false, true] {
        for (source_offset, destination_offset, bytes) in [
            (63, 95, 1),
            (SOURCE_OFFSET, DESTINATION_OFFSET, COPY_BYTES),
            (0, 32, BYTES as u64),
        ] {
            let mut f = Fixture::with_layout([BYTES, DESTINATION_BYTES], None, false);
            let source = f.allocations[0][2];
            let destination = f.allocations[1][3];
            // Initial bytes are installed before launch. The scripted compute
            // retains them; it does not emulate or manufacture a computed result.
            let (source_id, original_source) = initialize_bytes(&mut f, source, 0x31);
            let (destination_id, mut expected) = initialize_bytes(&mut f, destination, 0xb7);
            let producer = f.launch(0, 0, queued, true);
            let event = f.event(0, producer);
            let request = regions(&f, source_offset, destination_offset, bytes);
            let peer = admit(&mut f, event, request);
            let exact = window(source_offset, destination_offset, bytes);
            let retained = f.copy(peer).compute_producer.as_ref().unwrap();
            assert_eq!(retained.window(), exact);
            assert_eq!(retained.id, producer);
            assert!(f.backend.producer_aware_native.contains_key(&producer));
            assert!(
                f.copy(peer)
                    .compute_xgmi
                    .as_ref()
                    .unwrap()
                    .matches_window(exact)
            );
            assert!(f.copy(peer).staging.is_empty());
            assert_eq!(f.backend.cooperative_staging_bytes, 0);
            f.backend.release_event_v1(event).unwrap();
            assert!(f.backend.release_submission_v1(producer).is_err());
            for _ in 0..2 {
                assert_eq!(f.backend.poll_v1(peer).unwrap(), BackendPollV1::Pending);
                assert_eq!(
                    f.backend.wait_v1(peer, Instant::now()).unwrap(),
                    BackendPollV1::Pending
                );
                assert_eq!(
                    f.backend.drain_v1(peer, Instant::now()).unwrap(),
                    BackendPollV1::Pending
                );
            }
            assert!(f.backend.compute_xgmi_children.iter().all(Option::is_none));
            assert!(f.copy(peer).compute_xgmi.as_ref().unwrap().is_quiescent());
            publish(&mut f, peer);
            assert_eq!(
                f.backend.cancel_v1(peer).unwrap(),
                BackendCancellationV1::TooLate
            );
            assert!(
                f.backend
                    .compute_xgmi_children
                    .iter()
                    .all(|owner| *owner == Some(peer))
            );
            f.backend.validate_compute_peer_v1(peer).unwrap();
            f.drive(f.peer_stream, peer);
            assert_eq!(f.backend.poll_v1(peer).unwrap(), BackendPollV1::Succeeded);
            let source_bytes =
                &original_source[source_offset as usize..(source_offset + bytes) as usize];
            expected[destination_offset as usize..(destination_offset + bytes) as usize]
                .copy_from_slice(source_bytes);
            assert_eq!(
                restored(&f, source),
                (source_id, original_source.as_slice())
            );
            assert_eq!(
                restored(&f, destination),
                (destination_id, expected.as_slice())
            );
            assert!(f.copy(peer).compute_producer.is_none());
            assert!(f.backend.cooperative_dependency_retain_counts.is_empty());
            assert!(f.backend.compute_xgmi_children.iter().all(Option::is_none));
            assert_eq!(f.backend.completed_compute_xgmi_copies, 0);
            f.clean();
        }
    }
}

#[test]
fn compute_peer_window_final_readback_drives_queued_compute_after_event_release() {
    let mut f = Fixture::with_layout(
        [BYTES, DESTINATION_BYTES],
        Some((DESTINATION_OFFSET, 7, COPY_BYTES as usize)),
        false,
    );
    let source = f.allocations[0][2];
    let destination = f.allocations[1][3];
    let (_, original) = initialize_bytes(&mut f, source, 0x29);
    let (_, mut expected) = initialize_bytes(&mut f, destination, 0xbd);
    let producer = f.launch(0, 0, true, true);
    let event = f.event(0, producer);
    let request = regions(&f, SOURCE_OFFSET, DESTINATION_OFFSET, COPY_BYTES);
    let peer = admit(&mut f, event, request);
    let peer_event = f.backend.record_event_v1(f.peer_stream, peer).unwrap();
    let readback = f
        .backend
        .copy_async_v1(
            f.readback_stream,
            BackendMemoryRegionV1 {
                access: RuntimeAccessV1::Read,
                ..request[1]
            },
            BackendMemoryRegionV1 {
                allocation: f.host.unwrap(),
                access: RuntimeAccessV1::Write,
                byte_offset: 7,
                byte_len: COPY_BYTES,
            },
            &[peer_event],
        )
        .unwrap();
    f.backend.release_event_v1(event).unwrap();
    f.backend.release_event_v1(peer_event).unwrap();
    assert_eq!(
        f.backend.drain_v1(readback, Instant::now()).unwrap(),
        BackendPollV1::Pending
    );
    assert!(f.backend.compute_xgmi_children.iter().all(Option::is_none));
    f.drive(f.readback_stream, readback);
    for id in [producer, peer, readback] {
        assert_eq!(f.backend.poll_v1(id).unwrap(), BackendPollV1::Succeeded);
    }
    let copied = &original[SOURCE_OFFSET as usize..(SOURCE_OFFSET + COPY_BYTES) as usize];
    expected[DESTINATION_OFFSET as usize..(DESTINATION_OFFSET + COPY_BYTES) as usize]
        .copy_from_slice(copied);
    assert_eq!(restored(&f, destination).1, expected);
    let host = f.backend.allocations[&f.host.unwrap()];
    let KfdRuntimeSdmaStorageV1::Host(owner) =
        &f.backend.children[host.child].allocations[&host.local].sdma_storage
    else {
        panic!("host owner expected")
    };
    let mut expected_host = vec![0; DESTINATION_BYTES];
    expected_host[7..7 + COPY_BYTES as usize].copy_from_slice(copied);
    assert_eq!(owner.scripted_bytes().unwrap(), expected_host);
    assert!(f.backend.cooperative_dependency_retain_counts.is_empty());
    f.clean();
}

#[test]
fn compute_peer_window_rejects_invalid_bounds_and_destination_before_any_effect() {
    for case in 0..9 {
        let mut f = Fixture::with_layout([BYTES, DESTINATION_BYTES], None, false);
        let producer = f.launch(0, 0, true, true);
        let event = f.event(0, producer);
        let mut request = regions(&f, SOURCE_OFFSET, DESTINATION_OFFSET, COPY_BYTES);
        let destination = f.backend.allocations[&f.allocations[1][3]];
        match case {
            0 => {
                request[0].byte_len = 0;
                request[1].byte_len = 0;
            }
            1 => request[0].byte_offset += 1,
            2 => request[1].byte_offset = DESTINATION_BYTES as u64,
            3 => request[0].byte_offset = u64::MAX,
            4 => request[1].byte_len -= 1,
            5 => {
                f.backend.children[1]
                    .allocations
                    .get_mut(&destination.local)
                    .unwrap()
                    .sdma_initialized = false
            }
            6 => f.backend.children[1].peer_visible_device_allocations = false,
            7 => {
                f.backend.compute_xgmi_routes.clear();
            }
            8 => request[0].allocation = f.allocations[0][0],
            _ => unreachable!(),
        }
        let before = (
            f.backend.next_handle,
            f.backend.submissions.len(),
            f.backend.cooperative_staging_bytes,
        );
        assert!(
            matches!(
                f.backend
                    .peer_copy_v1(f.peer_stream, request[0], request[1], &[event]),
                Err(RuntimeBackendFailureV1::Rejected(_))
            ),
            "case {case}"
        );
        assert_eq!(
            (
                f.backend.next_handle,
                f.backend.submissions.len(),
                f.backend.cooperative_staging_bytes
            ),
            before
        );
        assert!(!f.backend.terminal);
        assert!(f.backend.cooperative_allocation_owners.is_empty());
        assert!(f.backend.compute_xgmi_children.iter().all(Option::is_none));
        assert!(
            f.backend.children.iter().all(|child| child
                .scripted_sdma
                .as_ref()
                .unwrap()
                .live_owner_count()
                == ALLOCATIONS)
        );
        f.backend.children[1]
            .allocations
            .get_mut(&destination.local)
            .unwrap()
            .sdma_initialized = true;
        f.backend.children[1].peer_visible_device_allocations = true;
        assert_eq!(
            f.backend.cancel_v1(producer).unwrap(),
            BackendCancellationV1::Cancelled
        );
        f.clean();
    }
}

#[test]
fn compute_peer_window_full_writer_guard_rejects_partial_and_aliased_bindings() {
    let mut f = Fixture::new(false);
    let producer = f.launch(0, 0, true, true);
    let source = f.backend.allocations[&f.allocations[0][2]];
    let record = &f.backend.children[0].allocations[&source.local];
    let original = &f.backend.producer_aware_native[&producer].bindings;
    assert!(exact_full_writer(original, source.local, record));
    // Guard-level negatives derived from an actually admitted recipe; no
    // malformed recipe is installed or described as accepted by the backend.
    for case in 0..4 {
        let mut bindings = original.to_vec();
        match case {
            0 => {
                bindings[2].region.byte_offset = 1;
                bindings[2].region.byte_len -= 1;
            }
            1 => bindings[2].region.access = RuntimeAccessV1::ReadWrite,
            2 => bindings.push(bindings[2]),
            3 => bindings[0].region.allocation = source.local,
            _ => unreachable!(),
        }
        assert!(
            !exact_full_writer(&bindings, source.local, record),
            "case {case}"
        );
    }
    assert_eq!(
        f.backend.cancel_v1(producer).unwrap(),
        BackendCancellationV1::Cancelled
    );
    f.clean();
}

#[test]
fn compute_peer_window_identity_drift_poison_pair_before_and_after_publication() {
    for published in [false, true] {
        for case in 0..3 {
            let mut f = Fixture::with_layout([BYTES, DESTINATION_BYTES], None, false);
            let producer = f.launch(0, 0, true, true);
            let event = f.event(0, producer);
            // Leave slack on each side so every injected alternate is valid in
            // bounds, but differs from the exact originally admitted operation.
            let request = regions(&f, 5, 11, 31);
            let peer = admit(&mut f, event, request);
            f.backend.release_event_v1(event).unwrap();
            let original = window(5, 11, 31);
            if published {
                publish(&mut f, peer);
            }
            if case == 2 {
                let route = f.backend.allocations[&f.allocations[1][3]];
                f.backend.children[1]
                    .allocations
                    .get_mut(&route.local)
                    .unwrap()
                    .bytes = vec![0; DESTINATION_BYTES + 1].into();
            } else {
                let RoutedSubmissionV1::CooperativeCopy(copy) =
                    f.backend.submissions.get_mut(&peer).unwrap()
                else {
                    unreachable!()
                };
                match case {
                    0 => copy.source_region.byte_offset += 1,
                    1 => copy.destination_region.byte_offset += 1,
                    _ => unreachable!(),
                }
            }
            assert!(matches!(
                f.backend.progress_cooperative_copy_step_v1(peer),
                Err(RuntimeBackendFailureV1::Terminal(_))
            ));
            assert!(f.backend.terminal);
            assert!(f.backend.children.iter().all(|child| child.terminal));
            assert!(f.copy(peer).compute_producer.is_some());
            assert!(
                f.copy(peer)
                    .compute_xgmi
                    .as_ref()
                    .unwrap()
                    .matches_window(original)
            );
            assert_eq!(
                f.copy(peer).compute_xgmi.as_ref().unwrap().is_quiescent(),
                !published
            );
            assert!(f.backend.submission_retained_as_dependency(producer));
            assert!(f.backend.producer_aware_native.contains_key(&producer));
            assert!(f.backend.children.iter().all(|child| {
                child.scripted_sdma.as_ref().unwrap().live_owner_count() == ALLOCATIONS
            }));
            assert_eq!(f.backend.completed_compute_xgmi_copies, 0);
            // Failed roots stay in ManuallyDrop, including all original owners.
        }
    }
}

#[test]
fn compute_peer_window_cancel_and_producer_failure_refund_without_copying() {
    for cancel_peer in [false, true] {
        let mut f = Fixture::with_layout([BYTES, DESTINATION_BYTES], None, false);
        let destination = f.allocations[1][3];
        let (owner_id, original) = initialize_bytes(&mut f, destination, 0xb3);
        let producer = f.launch(0, 0, true, true);
        let event = f.event(0, producer);
        let request = regions(&f, SOURCE_OFFSET, DESTINATION_OFFSET, COPY_BYTES);
        let peer = admit(&mut f, event, request);
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
                Err(RuntimeBackendFailureV1::Quiescent(_))
            ));
        }
        assert!(matches!(
            f.copy(peer).status(),
            BackendPollV1::Failed { .. }
        ));
        assert!(f.copy(peer).is_quiescent());
        assert!(f.copy(peer).compute_xgmi.as_ref().unwrap().is_quiescent());
        assert!(f.copy(peer).compute_producer.is_none());
        assert!(!f.backend.submission_retained_as_dependency(producer));
        assert!(f.backend.compute_xgmi_children.iter().all(Option::is_none));
        let route = f.backend.allocations[&destination];
        let KfdRuntimeSdmaStorageV1::H2dReady(ready) =
            &f.backend.children[1].allocations[&route.local].sdma_storage
        else {
            panic!("untouched destination expected")
        };
        assert_eq!(ready.owner.scripted_owner_id(), Some(owner_id));
        assert_eq!(ready.owner.scripted_bytes().unwrap(), original);
        f.clean();
    }
}

#[test]
fn compute_peer_window_native_errors_and_unwinds_keep_window_and_original_owners() {
    for unwind in [false, true] {
        for stage in [
            Stage::Create,
            Stage::Copy,
            Stage::Poll,
            Stage::Finish,
            Stage::Retire,
            Stage::Restore,
        ] {
            let mut f = Fixture::with_layout([BYTES, DESTINATION_BYTES], None, false);
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
            let request = regions(&f, SOURCE_OFFSET, DESTINATION_OFFSET, COPY_BYTES);
            let peer = admit(&mut f, event, request);
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
            let exact = window(SOURCE_OFFSET, DESTINATION_OFFSET, COPY_BYTES);
            assert_eq!(
                f.copy(peer).compute_producer.as_ref().unwrap().window(),
                exact
            );
            assert!(
                f.copy(peer)
                    .compute_xgmi
                    .as_ref()
                    .unwrap()
                    .matches_window(exact)
            );
            assert!(f.backend.submission_retained_as_dependency(producer));
            assert!(f.backend.producer_aware_native.contains_key(&producer));
            assert!(f.backend.children.iter().all(|child| {
                child.scripted_sdma.as_ref().unwrap().live_owner_count() == ALLOCATIONS
            }));
            assert_eq!(f.backend.completed_compute_xgmi_copies, 0);
        }
    }
}
