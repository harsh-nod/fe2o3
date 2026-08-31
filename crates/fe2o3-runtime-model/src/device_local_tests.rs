use alloc::vec;

use super::*;

const TEST_KFD_DYNAMIC_MAJOR: u32 = 511;

fn digest(seed: u8) -> IdentityDigestV1 {
    IdentityDigestV1::from_untrusted_bytes([seed; IDENTITY_DIGEST_BYTES_V1])
}

fn domain() -> DeviceObservationDomainIdV1 {
    DeviceObservationDomainIdV1::from_untrusted_digest(digest(1))
}

fn profile() -> DeviceAdmissionProfileV1 {
    DeviceAdmissionProfileV1::gfx942_xnack_minus_spx_nps1_kfd_1_18_drm_3_64_0(
        DeviceAdmissionProfileIdV1::from_untrusted_digest(digest(2)),
        digest(3),
        digest(4),
    )
}

fn correlation() -> ModelCorrelatedDeviceV1 {
    let domain_id = domain();
    let epoch = ObservationEpochV1(9);
    let pci = PciAddressV1 {
        domain: 0,
        bus: 5,
        device: 0,
        function: 0,
    };
    UntrustedDeviceInventoryV1::from_untrusted_observations(
        UntrustedKfdObservationV1 {
            domain_id,
            epoch,
            node: DeviceNodeV1 {
                major: TEST_KFD_DYNAMIC_MAJOR,
                minor: KFD_DEVICE_MINOR_V1,
            },
            uapi_major: KFD_UAPI_MAJOR_V1,
            uapi_minor: KFD_UAPI_MINOR_V1,
            schema_identity: digest(3),
            xnack: XnackObservationV1::Disabled,
        },
        vec![UntrustedTopologyObservationV1 {
            domain_id,
            epoch,
            topology_node_id: 2,
            kfd_gpu_id: 28_851,
            gpu_unique_id: 0x6ced_1647_a296_545c,
            drm_render_minor: DRM_RENDER_MIN_MINOR_V1,
            pci,
            vendor_id: AMD_PCI_VENDOR_ID_V1,
            device_id: MI300X_PCI_DEVICE_ID_V1,
            target: GpuTargetObservationV1::Gfx942,
            compute_partition: ComputePartitionObservationV1::Spx,
            memory_partition: MemoryPartitionObservationV1::Nps1,
        }],
        vec![UntrustedRenderObservationV1 {
            domain_id,
            epoch,
            node: DeviceNodeV1 {
                major: DRM_DEVICE_MAJOR_V1,
                minor: DRM_RENDER_MIN_MINOR_V1,
            },
            gpu_unique_id: 0x6ced_1647_a296_545c,
            pci,
            vendor_id: AMD_PCI_VENDOR_ID_V1,
            device_id: MI300X_PCI_DEVICE_ID_V1,
            pci_revision_id: 0,
            drm_schema_identity: digest(4),
            driver_name: DrmDriverNameObservationV1::Amdgpu,
            drm_major: DRM_DRIVER_MAJOR_V1,
            drm_minor: DRM_DRIVER_MINOR_V1,
            drm_patch: DRM_DRIVER_PATCH_V1,
            acceleration_working: true,
            family: DrmFamilyObservationV1::AmdgpuFamilyAi,
        }],
    )
    .unwrap()
    .correlate_model_only(&profile())
    .unwrap()
}

fn vm_observation(device: ModelDeviceAdmissionV1) -> UntrustedVmObservationV1 {
    let correlated = device.correlation();
    UntrustedVmObservationV1 {
        domain_id: correlated.domain_id(),
        device: device.model_key(),
        vm_id: VmIdV1(10),
        kfd_gpu_id: correlated.kfd_gpu_id(),
        render_node: correlated.render_node(),
        pci: correlated.identity().pci,
    }
}

fn memory_next(
    memory: MemoryLifecycleStateV1,
    transition: MemoryTransitionV1,
) -> MemoryLifecycleStateV1 {
    memory.next(transition).unwrap()
}

#[allow(clippy::too_many_arguments)]
fn append_mapping(
    mut memory: MemoryLifecycleStateV1,
    device: ModelDeviceAdmissionV1,
    vm: VmKeyV1,
    id: u64,
    va: u64,
    byte_len: u64,
    alignment: u64,
    kind: MemoryKindV1,
    coherence: MemoryCoherenceV1,
    access: MemoryAccessV1,
) -> (MemoryLifecycleStateV1, MemoryMappingKeyV1) {
    let reservation = VaReservationKeyV1 {
        vm,
        id: VaReservationIdV1(id),
    };
    let allocation = MemoryAllocationKeyV1 {
        vm,
        id: AllocationIdV1(id + 100),
        generation: AllocationGenerationV1(1),
    };
    let mapping = MemoryMappingKeyV1 {
        allocation,
        id: MappingIdV1(id + 200),
    };
    memory = memory_next(
        memory,
        MemoryTransitionV1::ReserveVa {
            key: reservation,
            range: GpuVaRangeV1 { base: va, byte_len },
            alignment,
        },
    );
    memory = memory_next(
        memory,
        MemoryTransitionV1::Allocate {
            key: allocation,
            reservation,
            handle: UntrustedAllocationHandleObservationV1(id + 300),
            spec: MemoryAllocationSpecV1 {
                byte_len,
                alignment,
                kind,
                coherence,
            },
        },
    );
    memory = memory_next(
        memory,
        MemoryTransitionV1::BeginMap {
            key: mapping,
            target_devices: vec![device.model_key()],
            access,
        },
    );
    memory = memory_next(
        memory,
        MemoryTransitionV1::ObserveMap {
            key: mapping,
            progress: PartialProgressObservationV1 {
                n_success: 1,
                status: PartialOperationStatusV1::Succeeded,
            },
        },
    );
    (memory, mapping)
}

struct Fixture {
    identity: DeviceIdentityStateV1,
    queues: QueueLifecycleStateV1,
    memory: MemoryLifecycleStateV1,
    queue: QueueKeyV1,
    host_readwrite: MemoryMappingKeyV1,
    host_readonly: MemoryMappingKeyV1,
    device: MemoryMappingKeyV1,
    scratch: MemoryMappingKeyV1,
}

fn fixture() -> Fixture {
    let identity = DeviceIdentityStateV1::new(domain());
    let (identity, device) = identity
        .register_device_model_only(correlation(), DeviceGenerationV1(1))
        .unwrap();
    let (identity, vm) = identity
        .register_vm_model_only(device, vm_observation(device))
        .unwrap();
    let vm_key = vm.model_key();
    let mut memory = memory_next(
        MemoryLifecycleStateV1::new(domain()),
        MemoryTransitionV1::AcquireVm {
            admission: vm,
            mapping_devices: vec![device],
            handle: UntrustedVmHandleObservationV1(100),
            aperture: GpuVaRangeV1 {
                base: 0x1_0000,
                byte_len: 0x1000_0000,
            },
        },
    );

    let mut bindings = vec![];
    for index in 0_u64..COMPUTE_AQL_RESOURCE_COUNT_V1 as u64 {
        let (next, mapping) = append_mapping(
            memory,
            device,
            vm_key,
            200 + index,
            0x2_0000 + index * MEMORY_PAGE_BYTES_V1,
            MEMORY_PAGE_BYTES_V1,
            MEMORY_PAGE_BYTES_V1,
            MemoryKindV1::QueueStorage,
            MemoryCoherenceV1::HostCoherent,
            MemoryAccessV1::ReadWrite,
        );
        memory = next;
        bindings.push(ComputeAqlResourceBindingV1 {
            mapping,
            publication: MemoryPublicationKeyV1 {
                mapping,
                id: MemoryPublicationIdV1(600 + index),
            },
            expected_kind: MemoryKindV1::QueueStorage,
            expected_coherence: MemoryCoherenceV1::HostCoherent,
            expected_access: MemoryAccessV1::ReadWrite,
        });
    }
    let queue = QueueKeyV1 {
        vm: vm_key,
        id: QueueInstanceIdV1(700),
        generation: QueueGenerationV1(1),
    };
    let plan = ComputeAqlQueuePlanV1 {
        schema_version: QUEUE_LIFECYCLE_SCHEMA_VERSION_V1,
        target: ComputeAqlTargetProfileV1::Gfx942XnackMinusSpxNps1Kfd1_18,
        domain_id: domain(),
        plan_id: QueuePlanIdV1::from_untrusted_digest(digest(5)),
        current_device: device,
        queue,
        initial_configuration: QueueConfigurationIdV1::from_untrusted_digest(digest(6)),
        resources: ComputeAqlQueueResourcesV1 {
            ring: bindings[0],
            control: bindings[1],
            eop: bindings[2],
            context_save: bindings[3],
        },
    };
    let admission = QueueLifecycleStateV1::new(domain())
        .admit_compute_aql_plan(&identity, &memory, plan)
        .unwrap();
    let (queues, memory) = admission.into_states();
    let queues = queues
        .next(&identity, &memory, QueueTransitionV1::BeginCreate { queue })
        .unwrap();
    let queues = queues
        .next(
            &identity,
            &memory,
            QueueTransitionV1::ObserveCreate {
                queue,
                observation: QueueCreateObservationV1 {
                    status: QueueSyscallStatusV1::Succeeded,
                    queue_id_field: CreateQueueIdFieldObservationV1::Returned(
                        UntrustedQueueIdObservationV1(0),
                    ),
                },
            },
        )
        .unwrap();

    let (memory, host_readwrite) = append_mapping(
        memory,
        device,
        vm_key,
        1_000,
        0x10_0000,
        0x20_000,
        MEMORY_PAGE_BYTES_V1,
        MemoryKindV1::HostVisibleCoherent,
        MemoryCoherenceV1::HostCoherent,
        MemoryAccessV1::ReadWrite,
    );
    let (memory, host_readonly) = append_mapping(
        memory,
        device,
        vm_key,
        2_000,
        0x20_0000,
        0x20_000,
        MEMORY_PAGE_BYTES_V1,
        MemoryKindV1::HostVisibleCoherent,
        MemoryCoherenceV1::HostCoherent,
        MemoryAccessV1::Read,
    );
    let (memory, device_mapping) = append_mapping(
        memory,
        device,
        vm_key,
        3_000,
        0x30_0000,
        0x20_000,
        MEMORY_PAGE_BYTES_V1,
        MemoryKindV1::DeviceLocal,
        MemoryCoherenceV1::ExplicitVisibility,
        MemoryAccessV1::ReadWrite,
    );
    let (memory, scratch) = append_mapping(
        memory,
        device,
        vm_key,
        4_000,
        0x40_0000,
        0x40_0000,
        MEMORY_PAGE_BYTES_V1,
        MemoryKindV1::ScratchContextSave,
        MemoryCoherenceV1::ExplicitVisibility,
        MemoryAccessV1::ReadWrite,
    );
    Fixture {
        identity,
        queues,
        memory,
        queue,
        host_readwrite,
        host_readonly,
        device: device_mapping,
        scratch,
    }
}

fn mechanism() -> DeviceLocalTransferMechanismV1 {
    DeviceLocalTransferMechanismV1::CopyKernel {
        artifact: RuntimeArtifactIdV1::from_untrusted_digest(digest(20)),
        contract_identity: digest(21),
    }
}

fn transfer_request(
    fixture: &Fixture,
    direction: DeviceLocalTransferDirectionV1,
) -> DeviceLocalTransferRequestV1 {
    let (source, destination) = match direction {
        DeviceLocalTransferDirectionV1::Upload => (fixture.host_readonly, fixture.device),
        DeviceLocalTransferDirectionV1::Download => (fixture.device, fixture.host_readwrite),
    };
    DeviceLocalTransferRequestV1::new(
        1,
        fixture.queue,
        direction,
        mechanism(),
        DeviceLocalTransferSliceV1::new(source, 256),
        DeviceLocalTransferSliceV1::new(destination, 512),
        4096,
        256,
    )
}

#[test]
fn transfer_visibility_requires_exact_ordered_completion() {
    let fixture = fixture();
    let plan = admit_device_local_transfer_v1(
        &fixture.identity,
        &fixture.queues,
        &fixture.memory,
        transfer_request(&fixture, DeviceLocalTransferDirectionV1::Upload),
    )
    .unwrap();
    let publication = DeviceLocalTransferPublicationV1::new(1, fixture.queue, DispatchIdV1(8), 40);
    let planned = plan.begin();
    assert!(planned.device_visibility().is_none());
    let published = planned
        .next(DeviceLocalTransferTransitionV1::Publish(publication))
        .unwrap();
    assert!(published.device_visibility().is_none());
    assert_eq!(
        published.next(DeviceLocalTransferTransitionV1::ObserveCompletion(
            DeviceLocalTransferCompletionV1::new(publication, CompletionIdV1(9), 40),
        )),
        Err(DeviceLocalTransferTransitionErrorV1::InvalidOrdering)
    );
    let completed = published
        .next(DeviceLocalTransferTransitionV1::ObserveCompletion(
            DeviceLocalTransferCompletionV1::new(publication, CompletionIdV1(9), 41),
        ))
        .unwrap();
    assert!(completed.device_visibility().is_some());
    assert!(completed.host_visibility().is_none());

    let wrong_publication =
        DeviceLocalTransferPublicationV1::new(2, fixture.queue, DispatchIdV1(8), 40);
    assert_eq!(
        plan.begin()
            .next(DeviceLocalTransferTransitionV1::Publish(wrong_publication)),
        Err(DeviceLocalTransferTransitionErrorV1::PublicationMismatch)
    );

    let ambiguous = plan
        .begin()
        .next(DeviceLocalTransferTransitionV1::Publish(publication))
        .unwrap()
        .next(DeviceLocalTransferTransitionV1::MarkAmbiguous)
        .unwrap();
    assert!(ambiguous.device_visibility().is_none());
    assert_eq!(
        ambiguous.next(DeviceLocalTransferTransitionV1::ObserveCompletion(
            DeviceLocalTransferCompletionV1::new(publication, CompletionIdV1(9), 41),
        )),
        Err(DeviceLocalTransferTransitionErrorV1::IllegalTransition)
    );
    let cancelled = plan
        .begin()
        .next(DeviceLocalTransferTransitionV1::CancelBeforePublication)
        .unwrap();
    assert_eq!(
        cancelled.next(DeviceLocalTransferTransitionV1::Publish(publication)),
        Err(DeviceLocalTransferTransitionErrorV1::IllegalTransition)
    );

    let download = admit_device_local_transfer_v1(
        &fixture.identity,
        &fixture.queues,
        &fixture.memory,
        transfer_request(&fixture, DeviceLocalTransferDirectionV1::Download),
    )
    .unwrap();
    let download_publication =
        DeviceLocalTransferPublicationV1::new(1, fixture.queue, DispatchIdV1(10), 50);
    let download = download
        .begin()
        .next(DeviceLocalTransferTransitionV1::Publish(
            download_publication,
        ))
        .unwrap()
        .next(DeviceLocalTransferTransitionV1::ObserveCompletion(
            DeviceLocalTransferCompletionV1::new(download_publication, CompletionIdV1(11), 51),
        ))
        .unwrap();
    assert!(download.host_visibility().is_some());
    assert!(download.device_visibility().is_none());
}

#[test]
fn transfer_admission_rejects_hostile_range_kind_access_and_generation_substitution() {
    let fixture = fixture();
    let valid = transfer_request(&fixture, DeviceLocalTransferDirectionV1::Upload);
    assert!(
        admit_device_local_transfer_v1(&fixture.identity, &fixture.queues, &fixture.memory, valid)
            .is_ok()
    );
    let cases = [
        (
            DeviceLocalTransferRequestV1::new(
                1,
                fixture.queue,
                DeviceLocalTransferDirectionV1::Upload,
                mechanism(),
                DeviceLocalTransferSliceV1::new(fixture.host_readonly, u64::MAX - 3),
                DeviceLocalTransferSliceV1::new(fixture.device, 0),
                8,
                1,
            ),
            DeviceLocalTransferAdmissionErrorV1::InvalidRange,
        ),
        (
            DeviceLocalTransferRequestV1::new(
                1,
                fixture.queue,
                DeviceLocalTransferDirectionV1::Upload,
                mechanism(),
                DeviceLocalTransferSliceV1::new(fixture.host_readonly, 1),
                DeviceLocalTransferSliceV1::new(fixture.device, 0),
                8,
                8,
            ),
            DeviceLocalTransferAdmissionErrorV1::InvalidAlignment,
        ),
        (
            DeviceLocalTransferRequestV1::new(
                1,
                fixture.queue,
                DeviceLocalTransferDirectionV1::Upload,
                mechanism(),
                DeviceLocalTransferSliceV1::new(fixture.host_readonly, 0),
                DeviceLocalTransferSliceV1::new(fixture.device, 0),
                8,
                8192,
            ),
            DeviceLocalTransferAdmissionErrorV1::InvalidAlignment,
        ),
        (
            DeviceLocalTransferRequestV1::new(
                1,
                fixture.queue,
                DeviceLocalTransferDirectionV1::Upload,
                mechanism(),
                DeviceLocalTransferSliceV1::new(fixture.device, 0),
                DeviceLocalTransferSliceV1::new(fixture.host_readwrite, 0),
                8,
                1,
            ),
            DeviceLocalTransferAdmissionErrorV1::UnsupportedMemoryKinds,
        ),
        (
            DeviceLocalTransferRequestV1::new(
                1,
                fixture.queue,
                DeviceLocalTransferDirectionV1::Download,
                mechanism(),
                DeviceLocalTransferSliceV1::new(fixture.device, 0),
                DeviceLocalTransferSliceV1::new(fixture.host_readonly, 0),
                8,
                1,
            ),
            DeviceLocalTransferAdmissionErrorV1::InvalidAccess,
        ),
        (
            DeviceLocalTransferRequestV1::new(
                1,
                QueueKeyV1 {
                    generation: QueueGenerationV1(2),
                    ..fixture.queue
                },
                DeviceLocalTransferDirectionV1::Upload,
                mechanism(),
                DeviceLocalTransferSliceV1::new(fixture.host_readonly, 0),
                DeviceLocalTransferSliceV1::new(fixture.device, 0),
                8,
                1,
            ),
            DeviceLocalTransferAdmissionErrorV1::QueueNotActive,
        ),
    ];
    for (request, expected) in cases {
        assert_eq!(
            admit_device_local_transfer_v1(
                &fixture.identity,
                &fixture.queues,
                &fixture.memory,
                request,
            ),
            Err(expected)
        );
    }
}

fn target(max_queue: u64) -> PrivateSegmentTargetContractV1 {
    PrivateSegmentTargetContractV1::new(
        digest(30),
        digest(31),
        ComputeAqlTargetProfileV1::Gfx942XnackMinusSpxNps1Kfd1_18,
        64,
        4,
        256,
        4096,
        1 << 20,
        1 << 22,
        max_queue,
    )
    .unwrap()
}

fn target_with_bounds(
    max_wave: u64,
    max_workgroup: u64,
    max_queue: u64,
) -> PrivateSegmentTargetContractV1 {
    target_with_residency(max_wave, max_workgroup, max_queue, 4)
}

fn target_with_residency(
    max_wave: u64,
    max_workgroup: u64,
    max_queue: u64,
    maximum_resident_workgroups: u32,
) -> PrivateSegmentTargetContractV1 {
    PrivateSegmentTargetContractV1::new(
        digest(30),
        digest(31),
        ComputeAqlTargetProfileV1::Gfx942XnackMinusSpxNps1Kfd1_18,
        64,
        maximum_resident_workgroups,
        256,
        4096,
        max_wave,
        max_workgroup,
        max_queue,
    )
    .unwrap()
}

fn metadata(private_bytes: u64, seed: u8) -> PostLinkPrivateSegmentMetadataV1 {
    PostLinkPrivateSegmentMetadataV1::new(
        RuntimeArtifactIdV1::from_untrusted_digest(digest(40)),
        digest(41),
        digest(seed),
        private_bytes,
    )
    .unwrap()
}

#[test]
fn private_segment_plan_binds_post_link_metadata_queue_and_exact_sizing() {
    let fixture = fixture();
    let exact_metadata = metadata(12, 42);
    let admission = admit_private_segment_scratch_v1(
        &fixture.identity,
        &fixture.queues,
        &fixture.memory,
        PrivateSegmentAdmissionRequestV1::new(
            target(1 << 24),
            exact_metadata,
            fixture.queue,
            PrivateSegmentDispatchShapeV1::new(96),
            Some(fixture.scratch),
        ),
    )
    .unwrap();
    let PrivateSegmentAdmissionV1::Required(plan) = admission else {
        panic!("expected required scratch")
    };
    assert_eq!(plan.packet_private_segment_bytes(), 12);
    assert_eq!(plan.wave_count_per_workgroup(), 2);
    assert_eq!(plan.scratch_bytes_per_wave(), 768);
    assert_eq!(plan.scratch_bytes_per_workgroup(), 1536);
    assert_eq!(plan.scratch_bytes_per_queue(), 6144);
    assert_eq!(plan.queue(), fixture.queue);
    assert_eq!(plan.scratch_mapping(), fixture.scratch);
    assert_eq!(plan.require_current_metadata(exact_metadata), Ok(plan));
    assert_eq!(
        plan.require_current_shape(PrivateSegmentDispatchShapeV1::new(96)),
        Ok(plan)
    );
    assert_eq!(
        plan.require_current_shape(PrivateSegmentDispatchShapeV1::new(97)),
        Err(PrivateSegmentAdmissionErrorV1::DispatchShapeMismatch)
    );
    assert_eq!(
        plan.require_current_metadata(metadata(13, 43)),
        Err(PrivateSegmentAdmissionErrorV1::PostLinkMetadataMismatch)
    );
    let changed_target = PrivateSegmentTargetContractV1::new(
        digest(30),
        digest(32),
        ComputeAqlTargetProfileV1::Gfx942XnackMinusSpxNps1Kfd1_18,
        64,
        4,
        256,
        4096,
        1 << 20,
        1 << 22,
        1 << 24,
    )
    .unwrap();
    assert_eq!(
        plan.require_current_target(changed_target),
        Err(PrivateSegmentAdmissionErrorV1::TargetContractMismatch)
    );
}

#[test]
fn private_segment_admission_fails_closed_on_absent_extra_or_insufficient_scratch() {
    let fixture = fixture();
    let shape = PrivateSegmentDispatchShapeV1::new(96);
    assert_eq!(
        admit_private_segment_scratch_v1(
            &fixture.identity,
            &fixture.queues,
            &fixture.memory,
            PrivateSegmentAdmissionRequestV1::new(
                target(1 << 24),
                metadata(12, 42),
                fixture.queue,
                shape,
                None,
            ),
        ),
        Err(PrivateSegmentAdmissionErrorV1::MissingScratchMapping)
    );
    assert_eq!(
        admit_private_segment_scratch_v1(
            &fixture.identity,
            &fixture.queues,
            &fixture.memory,
            PrivateSegmentAdmissionRequestV1::new(
                target(1 << 24),
                metadata(0, 42),
                fixture.queue,
                shape,
                Some(fixture.scratch),
            ),
        ),
        Err(PrivateSegmentAdmissionErrorV1::UnexpectedScratchMapping)
    );
    assert_eq!(
        admit_private_segment_scratch_v1(
            &fixture.identity,
            &fixture.queues,
            &fixture.memory,
            PrivateSegmentAdmissionRequestV1::new(
                target_with_bounds(1024, 4096, 4096),
                metadata(12, 42),
                fixture.queue,
                shape,
                Some(fixture.scratch),
            ),
        ),
        Err(PrivateSegmentAdmissionErrorV1::ScratchPerQueueExceeded)
    );
    assert_eq!(
        admit_private_segment_scratch_v1(
            &fixture.identity,
            &fixture.queues,
            &fixture.memory,
            PrivateSegmentAdmissionRequestV1::new(
                target(1 << 24),
                metadata(12, 42),
                fixture.queue,
                shape,
                Some(fixture.device),
            ),
        ),
        Err(PrivateSegmentAdmissionErrorV1::ScratchBindingMismatch)
    );

    let no_scratch = admit_private_segment_scratch_v1(
        &fixture.identity,
        &fixture.queues,
        &fixture.memory,
        PrivateSegmentAdmissionRequestV1::new(
            target(1 << 24),
            metadata(0, 42),
            fixture.queue,
            shape,
            None,
        ),
    )
    .unwrap();
    assert_eq!(no_scratch.packet_private_segment_bytes(), 0);

    let packet_overflow_target = PrivateSegmentTargetContractV1::new(
        digest(30),
        digest(31),
        ComputeAqlTargetProfileV1::Gfx942XnackMinusSpxNps1Kfd1_18,
        64,
        4,
        256,
        u64::MAX,
        u64::MAX,
        u64::MAX,
        u64::MAX,
    )
    .unwrap();
    assert_eq!(
        admit_private_segment_scratch_v1(
            &fixture.identity,
            &fixture.queues,
            &fixture.memory,
            PrivateSegmentAdmissionRequestV1::new(
                packet_overflow_target,
                metadata(u64::from(u32::MAX) + 1, 42),
                fixture.queue,
                shape,
                Some(fixture.scratch),
            ),
        ),
        Err(PrivateSegmentAdmissionErrorV1::PrivateSegmentPacketOverflow)
    );

    let arithmetic_overflow_target = PrivateSegmentTargetContractV1::new(
        digest(30),
        digest(31),
        ComputeAqlTargetProfileV1::Gfx942XnackMinusSpxNps1Kfd1_18,
        64,
        1,
        1_u64 << 63,
        u64::from(u32::MAX),
        u64::MAX,
        u64::MAX,
        u64::MAX,
    )
    .unwrap();
    assert_eq!(
        admit_private_segment_scratch_v1(
            &fixture.identity,
            &fixture.queues,
            &fixture.memory,
            PrivateSegmentAdmissionRequestV1::new(
                arithmetic_overflow_target,
                metadata(1, 42),
                fixture.queue,
                PrivateSegmentDispatchShapeV1::new(u32::MAX),
                Some(fixture.scratch),
            ),
        ),
        Err(PrivateSegmentAdmissionErrorV1::ArithmeticOverflow)
    );
}

#[test]
fn scratch_sizing_matches_integer_reference_across_boundary_grid() {
    let fixture = fixture();
    for private_bytes in [1_u64, 4, 255, 256, 257, 1024] {
        for workitems in [1_u32, 63, 64, 65, 96, 255, 256] {
            for resident in [1_u32, 2, 8] {
                let target = target_with_residency(1 << 20, 1 << 22, 1 << 24, resident);
                let admission = admit_private_segment_scratch_v1(
                    &fixture.identity,
                    &fixture.queues,
                    &fixture.memory,
                    PrivateSegmentAdmissionRequestV1::new(
                        target,
                        metadata(private_bytes, 42),
                        fixture.queue,
                        PrivateSegmentDispatchShapeV1::new(workitems),
                        Some(fixture.scratch),
                    ),
                )
                .unwrap();
                let PrivateSegmentAdmissionV1::Required(plan) = admission else {
                    panic!("nonzero private segment must require scratch")
                };
                let wave_bytes = (private_bytes * 64).div_ceil(256) * 256;
                let waves = u64::from(workitems).div_ceil(64);
                assert_eq!(plan.scratch_bytes_per_wave(), wave_bytes);
                assert_eq!(plan.wave_count_per_workgroup(), waves);
                assert_eq!(plan.scratch_bytes_per_workgroup(), wave_bytes * waves);
                assert_eq!(
                    plan.scratch_bytes_per_queue(),
                    wave_bytes * waves * u64::from(resident)
                );
            }
        }
    }
}
