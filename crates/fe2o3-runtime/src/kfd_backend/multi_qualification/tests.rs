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
        let error = KfdMultiDeviceRuntimeBackendV1::open_gfx942_r57_n3_qualification_v2(&devices)
            .unwrap_err();
        assert_eq!(error.kind(), KfdRuntimeBackendErrorKindV1::InvalidLaunch);
    }
}

#[test]
fn qualification_admission_preserves_order_and_stops_at_first_failure() {
    let mut calls = 0;
    assert!(
        super::admit_qualification_devices_v1(&[1, 1], || {
            calls += 1;
            Ok(KfdRuntimeLaunchGateV1::WorkerV3GeneratedOnly)
        })
        .is_err()
    );
    assert_eq!(calls, 0);

    let devices = [30, 10, 20];
    let admitted = super::admit_qualification_devices_v1(&devices, || {
        calls += 1;
        Ok(KfdRuntimeLaunchGateV1::WorkerV3GeneratedOnly)
    })
    .unwrap();
    assert_eq!(calls, 3);
    assert_eq!(
        admitted.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
        devices
    );

    calls = 0;
    let error = super::admit_qualification_devices_v1(&devices, || {
        calls += 1;
        if calls == 2 {
            Err(KfdRuntimeBackendErrorV1::new(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "fixture admission failed",
            ))
        } else {
            Ok(KfdRuntimeLaunchGateV1::WorkerV3GeneratedOnly)
        }
    })
    .unwrap_err();
    assert_eq!(error.kind(), KfdRuntimeBackendErrorKindV1::InvalidLaunch);
    assert_eq!(calls, 2);
}

#[test]
fn r57_device_authorities_keep_identical_local_handles_isolated() {
    use crate::qualification_gfx942_r57_n3_v1::*;

    let devices = [30, 10, 20];
    let gates = super::admit_qualification_devices_v1(&devices, super::admit_r57_n3_v2).unwrap();
    let observations: Vec<_> = gates
        .iter()
        .map(|(_, gate)| match gate {
            KfdRuntimeLaunchGateV1::ExactGfx942R57N3V2(admitted) => admitted.observation_v1(),
            _ => panic!("exact independent V2 authority required"),
        })
        .collect();
    let buffers = gfx942_r57_n3_qualification_host_buffers_v1().unwrap();
    let kernarg = gfx942_r57_n3_qualification_explicit_kernarg_v1();
    let ids = [10, 20, 30];
    let contents = [buffers.a(), buffers.b(), buffers.c_initial()];
    let allocations = core::array::from_fn::<_, 3, _>(|index| KfdRuntimeAuthorityAllocationV1 {
        allocation: ids[index],
        kind: RuntimeMemoryKindV1::DeviceLocal,
        alignment: GFX942_R57_N3_QUALIFICATION_BUFFER_ALIGNMENT_V1,
        byte_offset: 0,
        bytes: contents[index],
        content_sha256: Some(Sha256::digest(contents[index]).into()),
    });
    let bindings = core::array::from_fn::<_, 3, _>(|index| BackendBindingV1 {
        kernarg_byte_offset: GFX942_R57_N3_QUALIFICATION_ARGUMENTS_V1[index].pointer_offset,
        region: BackendMemoryRegionV1 {
            allocation: ids[index],
            access: GFX942_R57_N3_QUALIFICATION_ARGUMENTS_V1[index].access,
            byte_offset: 0,
            byte_len: GFX942_R57_N3_QUALIFICATION_BUFFER_BYTES_V1 as u64,
        },
    });
    let abi =
        GFX942_R57_N3_QUALIFICATION_ARGUMENTS_V1.map(|policy| KfdRuntimeAuthorityGlobalBufferV1 {
            explicit_argument_index: policy.explicit_argument_index,
            name: policy.name,
            kernarg_byte_offset: u64::from(policy.pointer_offset),
            pointee_alignment: policy.reconciled_pointee_alignment,
            access: if policy.access == RuntimeAccessV1::Read {
                ArgumentAccess::ReadOnly
            } else {
                ArgumentAccess::WriteOnly
            },
        });
    let request = KfdRuntimeAuthorityRequestV1 {
        module_image: gfx942_r57_n3_qualification_hsaco_v1(),
        module_sha256: GFX942_R57_N3_QUALIFICATION_HSACO_SHA256_V1,
        kernel_name: GFX942_R57_N3_QUALIFICATION_KERNEL_V1,
        signature: GFX942_R57_N3_QUALIFICATION_SIGNATURE_V2,
        explicit_kernarg: &kernarg,
        complete_kernarg_template: &kernarg,
        bindings: &bindings,
        dispatch_abi: &abi,
        allocations: &allocations,
        geometry: GFX942_R57_N3_QUALIFICATION_GEOMETRY_V1,
        semantic_launch: KfdRuntimeSemanticLaunchV1::Ordinary,
    };
    for (index, (uid, gate)) in gates.iter().enumerate() {
        assert_eq!(*uid, devices[index]);
        assert!(gate.authorize_launch_v1(request));
        assert!(!gate.authorize_launch_v1(request));
        for (child, observation) in observations.iter().enumerate() {
            assert_eq!(
                observation.authorization_calls_v1(),
                if child <= index { 2 } else { 0 }
            );
        }
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
