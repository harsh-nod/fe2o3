use super::super::*;
use super::validate_qualification_devices_v1;

#[test]
fn qualification_device_preflight_accepts_complete_distinct_rosters() {
    for count in 2..=8 {
        let devices: Vec<_> = (0..count).map(|index| 0x100 + index).collect();
        validate_qualification_devices_v1(&devices).unwrap();
        let mut reversed = devices;
        reversed.reverse();
        validate_qualification_devices_v1(&reversed).unwrap();
    }
    validate_qualification_devices_v1(&[1, u64::MAX]).unwrap();
}

#[test]
fn qualification_constructor_rejects_every_invalid_roster_before_native_open() {
    let invalid = [
        Vec::new(),
        vec![1],
        (1..=9).collect(),
        vec![0, 2],
        vec![1, 0],
        vec![1, 1],
        vec![1, 2, 3, 4, 5, 6, 7, 0],
        vec![1, 2, 3, 4, 5, 6, 7, 1],
    ];
    for devices in invalid {
        let error = validate_qualification_devices_v1(&devices).unwrap_err();
        assert_eq!(error.kind(), KfdRuntimeBackendErrorKindV1::InvalidLaunch);
        // Calls the real public constructor. A native open would instead fail
        // with Native on CPU-only hosts; no fake device or successful owner is used.
        let error = KfdMultiDeviceRuntimeBackendV1::open_gfx942_vecadd_qualification_v1(&devices)
            .unwrap_err();
        assert_eq!(error.kind(), KfdRuntimeBackendErrorKindV1::InvalidLaunch);
    }
}

fn routed_fixture(count: usize) -> KfdMultiDeviceRuntimeBackendV1 {
    let children = (0..count)
        .map(|index| {
            let mut child = KfdRuntimeBackendV1::mock();
            child.description.backend_device = 0x100 + index as u64;
            child
        })
        .collect();
    KfdMultiDeviceRuntimeBackendV1::from_backends(children).unwrap()
}

fn region(allocation: u64, access: RuntimeAccessV1) -> BackendMemoryRegionV1 {
    BackendMemoryRegionV1 {
        allocation,
        access,
        byte_offset: 0,
        byte_len: 16,
    }
}

#[test]
fn selected_roster_routes_copy_to_exact_child_and_preserves_idle_siblings() {
    for count in [2, 3, 8] {
        let mut backend = routed_fixture(count);
        let devices: Vec<_> = backend
            .enumerate_devices_v1()
            .unwrap()
            .into_iter()
            .map(|description| description.backend_device)
            .collect();
        assert_eq!(
            devices,
            (0..count)
                .map(|index| 0x100 + index as u64)
                .collect::<Vec<_>>()
        );
        let streams: Vec<_> = devices
            .iter()
            .map(|device| backend.create_stream_v1(*device).unwrap())
            .collect();
        let allocations: Vec<_> = devices
            .iter()
            .map(|device| {
                backend
                    .allocate_v1(*device, RuntimeMemoryKindV1::HostVisible, 16, 8)
                    .unwrap()
            })
            .collect();
        for (index, allocation) in allocations.iter().enumerate() {
            backend
                .write_allocation_v1(*allocation, 0, &[index as u8 + 1; 16])
                .unwrap();
            assert_eq!(backend.allocations[allocation].child, index);
            assert_eq!(backend.streams[&streams[index]].child, index);
        }

        let destination = count - 1;
        let before = backend.next_handle;
        assert!(matches!(
            backend.peer_copy_v1(
                streams[0],
                region(allocations[0], RuntimeAccessV1::Read),
                region(allocations[destination], RuntimeAccessV1::Write),
                &[],
            ),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch
        ));
        assert_eq!(backend.next_handle, before);
        assert!(backend.submissions.is_empty());
        assert!(backend.cooperative_allocation_owners.is_empty());
        assert_eq!(backend.cooperative_staging_bytes, 0);

        let submission = backend
            .peer_copy_v1(
                streams[destination],
                region(allocations[0], RuntimeAccessV1::Read),
                region(allocations[destination], RuntimeAccessV1::Write),
                &[],
            )
            .unwrap();
        assert_eq!(backend.poll_v1(submission).unwrap(), BackendPollV1::Pending);
        assert_eq!(
            backend
                .drain_v1(submission, Instant::now() + Duration::from_secs(1))
                .unwrap(),
            BackendPollV1::Succeeded
        );
        for (index, allocation) in allocations.iter().enumerate() {
            let mut observed = [0; 16];
            backend
                .read_allocation_v1(*allocation, 0, &mut observed)
                .unwrap();
            let expected = if index == destination {
                1
            } else {
                index as u8 + 1
            };
            assert_eq!(observed, [expected; 16]);
        }
        backend.release_submission_v1(submission).unwrap();
        for allocation in allocations {
            backend.release_allocation_v1(allocation).unwrap();
        }
        for stream in streams {
            backend.destroy_stream_v1(stream).unwrap();
        }
        backend.shutdown_native_v1().unwrap();
        assert!(
            backend
                .children
                .iter()
                .all(|child| child.queue_retired && !child.terminal)
        );
        assert!(backend.cooperative_allocation_owners.is_empty());
        assert!(backend.cooperative_dependency_retain_counts.is_empty());
        assert_eq!(backend.cooperative_staging_bytes, 0);
    }
}
