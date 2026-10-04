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
    fixture_with_scratch(true)
}

fn fixture_with_scratch(include_scratch: bool) -> Fixture {
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
    let (next, scratch) = append_mapping(
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
    memory = next;
    let scratch_binding = ComputeAqlResourceBindingV1 {
        mapping: scratch,
        publication: MemoryPublicationKeyV1 {
            mapping: scratch,
            id: MemoryPublicationIdV1(699),
        },
        expected_kind: MemoryKindV1::ScratchContextSave,
        expected_coherence: MemoryCoherenceV1::ExplicitVisibility,
        expected_access: MemoryAccessV1::ReadWrite,
    };
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
            private_scratch: include_scratch.then_some(scratch_binding),
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

fn transfer_identity(
    fixture: &Fixture,
    id: u64,
) -> (
    DispatchKeyV1,
    CompletionKeyV1,
    DeviceLocalTransferRetentionV1,
) {
    let dispatch = DispatchKeyV1 {
        queue: fixture.queue,
        id: DispatchIdV1(id),
    };
    let completion = CompletionKeyV1 {
        dispatch,
        id: CompletionIdV1(id + 1_000),
    };
    let request = transfer_request(fixture, DeviceLocalTransferDirectionV1::Upload);
    let retention = DeviceLocalTransferRetentionV1::new(
        MemoryPublicationKeyV1 {
            mapping: request.source().mapping(),
            id: MemoryPublicationIdV1(id + 2_000),
        },
        MemoryPublicationKeyV1 {
            mapping: request.destination().mapping(),
            id: MemoryPublicationIdV1(id + 3_000),
        },
    );
    (dispatch, completion, retention)
}

fn transfer_retention(
    request: DeviceLocalTransferRequestV1,
    id: u64,
) -> DeviceLocalTransferRetentionV1 {
    DeviceLocalTransferRetentionV1::new(
        MemoryPublicationKeyV1 {
            mapping: request.source().mapping(),
            id: MemoryPublicationIdV1(id),
        },
        MemoryPublicationKeyV1 {
            mapping: request.destination().mapping(),
            id: MemoryPublicationIdV1(id + 1),
        },
    )
}

#[test]
fn transfer_visibility_requires_exact_ordered_completion() {
    let fixture = fixture();
    let mut registry = DeviceLocalTransferRegistryV1::new_model_only(
        &fixture.identity,
        fixture.memory.clone(),
        fixture.queues.clone(),
        digest(70),
    )
    .unwrap();
    let request = transfer_request(&fixture, DeviceLocalTransferDirectionV1::Upload);
    let (dispatch, completion, retention) = transfer_identity(&fixture, 8);
    let reserved = registry
        .reserve_model_only(&fixture.identity, request, dispatch, completion, retention)
        .unwrap();
    assert_eq!(registry.retained_transfer_count(), 1);
    assert_eq!(
        registry
            .memory_state()
            .next(MemoryTransitionV1::BeginUnmap {
                key: request.source().mapping(),
            }),
        Err(MemoryTransitionErrorV1::ResourceInUse(
            MemoryRecordRefV1::Mapping(request.source().mapping())
        ))
    );
    let submitted = reserved
        .publish_model_only(&mut registry, &fixture.identity, 40)
        .unwrap();
    let failure = submitted
        .poll_model_only(
            &mut registry,
            &fixture.identity,
            DeviceLocalTransferCompletionObservationV1::Completed {
                completion,
                acquire_sequence: 40,
            },
        )
        .unwrap_err();
    assert_eq!(failure.error(), DeviceLocalTransferErrorV1::InvalidOrdering);
    let submitted = failure.into_retained();
    let completed = match submitted
        .poll_model_only(
            &mut registry,
            &fixture.identity,
            DeviceLocalTransferCompletionObservationV1::Completed {
                completion,
                acquire_sequence: 41,
            },
        )
        .unwrap()
    {
        DeviceLocalTransferPollV1::Completed(completed) => completed,
        other => panic!("unexpected transfer poll result: {other:?}"),
    };
    assert_eq!(
        completed.visibility(),
        DeviceLocalTransferVisibilityV1::Device
    );
    assert_eq!(completed.request(), request);
    assert_eq!(completed.acquire_sequence(), 41);
    let released = completed
        .release_after_visibility_consumed_model_only(&mut registry, &fixture.identity)
        .unwrap();
    assert_eq!(released.request(), request);
    assert_eq!(released.acquire_sequence(), Some(41));
    assert_eq!(registry.retained_transfer_count(), 0);

    let request = transfer_request(&fixture, DeviceLocalTransferDirectionV1::Download);
    let dispatch = DispatchKeyV1 {
        queue: fixture.queue,
        id: DispatchIdV1(10),
    };
    let completion = CompletionKeyV1 {
        dispatch,
        id: CompletionIdV1(1_010),
    };
    let download_request = DeviceLocalTransferRequestV1::new(
        2,
        request.queue(),
        request.direction(),
        request.mechanism(),
        request.source(),
        request.destination(),
        request.byte_len(),
        request.required_alignment(),
    );
    let reserved = registry
        .reserve_model_only(
            &fixture.identity,
            download_request,
            dispatch,
            completion,
            transfer_retention(request, 4_010),
        )
        .unwrap()
        .publish_model_only(&mut registry, &fixture.identity, 50)
        .unwrap();
    let completed = match reserved
        .poll_model_only(
            &mut registry,
            &fixture.identity,
            DeviceLocalTransferCompletionObservationV1::Completed {
                completion,
                acquire_sequence: 51,
            },
        )
        .unwrap()
    {
        DeviceLocalTransferPollV1::Completed(completed) => completed,
        other => panic!("unexpected transfer poll result: {other:?}"),
    };
    assert_eq!(
        completed.visibility(),
        DeviceLocalTransferVisibilityV1::Host
    );
    assert_eq!(completed.request(), download_request);
    completed
        .release_after_visibility_consumed_model_only(&mut registry, &fixture.identity)
        .unwrap();
    let (memory, queues) = registry.into_states().unwrap();
    memory.validate_global_invariants().unwrap();
    assert_eq!(queues, fixture.queues);
}

#[test]
fn indeterminate_transfer_quarantines_both_endpoint_publications() {
    let fixture = fixture();
    let mut registry = DeviceLocalTransferRegistryV1::new_model_only(
        &fixture.identity,
        fixture.memory.clone(),
        fixture.queues.clone(),
        digest(72),
    )
    .unwrap();
    let request = transfer_request(&fixture, DeviceLocalTransferDirectionV1::Upload);
    let (dispatch, completion, retention) = transfer_identity(&fixture, 12);
    let submitted = registry
        .reserve_model_only(&fixture.identity, request, dispatch, completion, retention)
        .unwrap()
        .publish_model_only(&mut registry, &fixture.identity, 60)
        .unwrap();
    let quarantine = match submitted
        .poll_model_only(
            &mut registry,
            &fixture.identity,
            DeviceLocalTransferCompletionObservationV1::Indeterminate,
        )
        .unwrap()
    {
        DeviceLocalTransferPollV1::Indeterminate(quarantine) => quarantine,
        other => panic!("unexpected transfer poll result: {other:?}"),
    };
    assert_eq!(quarantine.binding().transfer_id(), request.transfer_id());
    assert_eq!(quarantine.request(), request);
    assert_eq!(registry.retained_transfer_count(), 1);
    for publication in [retention.source(), retention.destination()] {
        assert!(registry.memory_state().publications().iter().any(|record| {
            record.key == publication && record.state == MemoryPublicationStateV1::Live
        }));
    }
    assert!(registry.into_states().is_err());
}

#[test]
fn transfer_tokens_cannot_substitute_a_different_request_in_the_same_incarnation() {
    let fixture = fixture();
    let mut first = DeviceLocalTransferRegistryV1::new_model_only(
        &fixture.identity,
        fixture.memory.clone(),
        fixture.queues.clone(),
        digest(73),
    )
    .unwrap();
    let mut second = DeviceLocalTransferRegistryV1::new_model_only(
        &fixture.identity,
        fixture.memory.clone(),
        fixture.queues.clone(),
        digest(73),
    )
    .unwrap();
    let first_request = transfer_request(&fixture, DeviceLocalTransferDirectionV1::Upload);
    let second_request = DeviceLocalTransferRequestV1::new(
        first_request.transfer_id(),
        first_request.queue(),
        first_request.direction(),
        first_request.mechanism(),
        DeviceLocalTransferSliceV1::new(first_request.source().mapping(), 8),
        DeviceLocalTransferSliceV1::new(first_request.destination().mapping(), 8),
        8,
        8,
    );
    let (dispatch, completion, retention) = transfer_identity(&fixture, 16);
    let first_token = first
        .reserve_model_only(
            &fixture.identity,
            first_request,
            dispatch,
            completion,
            retention,
        )
        .unwrap();
    let second_token = second
        .reserve_model_only(
            &fixture.identity,
            second_request,
            dispatch,
            completion,
            retention,
        )
        .unwrap();

    let failure = first_token
        .publish_model_only(&mut second, &fixture.identity, 70)
        .unwrap_err();
    assert_eq!(failure.error(), DeviceLocalTransferErrorV1::TokenMismatch);
    failure
        .into_retained()
        .cancel_before_publication_model_only(&mut first)
        .unwrap();
    second_token
        .cancel_before_publication_model_only(&mut second)
        .unwrap();
}

#[test]
fn transfer_admission_rejects_hostile_range_kind_access_and_generation_substitution() {
    let fixture = fixture();
    let valid = transfer_request(&fixture, DeviceLocalTransferDirectionV1::Upload);
    let mut registry = DeviceLocalTransferRegistryV1::new_model_only(
        &fixture.identity,
        fixture.memory.clone(),
        fixture.queues.clone(),
        digest(71),
    )
    .unwrap();
    let (dispatch, completion, retention) = transfer_identity(&fixture, 20);
    let valid_token = registry
        .reserve_model_only(&fixture.identity, valid, dispatch, completion, retention)
        .unwrap();
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
    for (index, (request, expected)) in cases.into_iter().enumerate() {
        let dispatch = DispatchKeyV1 {
            queue: request.queue(),
            id: DispatchIdV1(30 + index as u64),
        };
        let completion = CompletionKeyV1 {
            dispatch,
            id: CompletionIdV1(1_030 + index as u64),
        };
        assert_eq!(
            registry
                .reserve_model_only(
                    &fixture.identity,
                    request,
                    dispatch,
                    completion,
                    transfer_retention(request, 5_000 + 2 * index as u64),
                )
                .unwrap_err(),
            DeviceLocalTransferErrorV1::Admission(expected)
        );
    }
    let duplicate_dispatch = DispatchKeyV1 {
        queue: fixture.queue,
        id: DispatchIdV1(99),
    };
    let duplicate_completion = CompletionKeyV1 {
        dispatch: duplicate_dispatch,
        id: CompletionIdV1(1_099),
    };
    assert_eq!(
        registry
            .reserve_model_only(
                &fixture.identity,
                valid,
                duplicate_dispatch,
                duplicate_completion,
                transfer_retention(valid, 6_000),
            )
            .unwrap_err(),
        DeviceLocalTransferErrorV1::DuplicateIdentity
    );
    let conflicting = DeviceLocalTransferRequestV1::new(
        2,
        valid.queue(),
        valid.direction(),
        valid.mechanism(),
        valid.source(),
        valid.destination(),
        valid.byte_len(),
        valid.required_alignment(),
    );
    let conflict_dispatch = DispatchKeyV1 {
        queue: fixture.queue,
        id: DispatchIdV1(100),
    };
    let conflict_completion = CompletionKeyV1 {
        dispatch: conflict_dispatch,
        id: CompletionIdV1(1_100),
    };
    assert_eq!(
        registry
            .reserve_model_only(
                &fixture.identity,
                conflicting,
                conflict_dispatch,
                conflict_completion,
                transfer_retention(conflicting, 6_100),
            )
            .unwrap_err(),
        DeviceLocalTransferErrorV1::ResourceConflict
    );
    valid_token
        .cancel_before_publication_model_only(&mut registry)
        .unwrap();
}

fn target_with_bounds(
    fixture: &Fixture,
    max_wave: u64,
    max_workgroup: u64,
    max_queue: u64,
    maximum_resident_workgroups: u32,
) -> PrivateSegmentTargetContractV1 {
    let queue = fixture
        .queues
        .queues()
        .iter()
        .find(|record| record.plan.queue == fixture.queue)
        .unwrap();
    PrivateSegmentTargetContractV1::gfx942_model_only(
        digest(30),
        queue.plan.current_device.correlation().profile_id(),
        queue.plan.plan_id,
        maximum_resident_workgroups,
        256,
        4096,
        max_wave,
        max_workgroup,
        max_queue,
    )
    .unwrap()
}

fn target(fixture: &Fixture, max_queue: u64) -> PrivateSegmentTargetContractV1 {
    target_with_bounds(fixture, 1 << 20, 1 << 22, max_queue, 4)
}

fn metadata(private_bytes: u64, seed: u8) -> PostLinkPrivateSegmentMetadataV1 {
    PostLinkPrivateSegmentMetadataV1::new(
        RuntimeArtifactIdV1::from_untrusted_digest(digest(40)),
        digest(41),
        digest(seed),
        private_bytes,
        GFX942_WAVEFRONT_SIZE_V1,
        None,
        GFX942_MAX_FLAT_WORKGROUP_SIZE_V1,
        [u32::MAX; 3],
        false,
    )
    .unwrap()
}

fn shape(workitems: u32) -> PrivateSegmentDispatchShapeV1 {
    PrivateSegmentDispatchShapeV1::new([workitems, 1, 1], [workitems, 1, 1]).unwrap()
}

#[test]
fn private_segment_plan_uses_queue_owned_scratch_and_exact_sizing() {
    let fixture = fixture();
    let exact_metadata = metadata(12, 42);
    let exact_shape = shape(96);
    let admission = admit_private_segment_scratch_v1(
        &fixture.identity,
        &fixture.queues,
        &fixture.memory,
        PrivateSegmentAdmissionRequestV1::new(
            target(&fixture, 1 << 24),
            exact_metadata,
            fixture.queue,
            exact_shape,
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
    assert_eq!(plan.require_current_shape(exact_shape), Ok(plan));
    assert_eq!(
        plan.require_current_shape(shape(97)),
        Err(PrivateSegmentAdmissionErrorV1::DispatchShapeMismatch)
    );
    let publication = fixture
        .memory
        .publications()
        .iter()
        .find(|publication| publication.key.mapping == fixture.scratch)
        .unwrap();
    assert_eq!(
        publication.owner,
        MemoryPublicationOwnerV1::ComputeAqlQueue(fixture.queue)
    );
    assert_eq!(publication.state, MemoryPublicationStateV1::Live);
}

#[test]
fn private_segment_admission_rejects_missing_capacity_and_bad_launch_contracts() {
    let without_scratch = fixture_with_scratch(false);
    assert_eq!(
        admit_private_segment_scratch_v1(
            &without_scratch.identity,
            &without_scratch.queues,
            &without_scratch.memory,
            PrivateSegmentAdmissionRequestV1::new(
                target(&without_scratch, 1 << 24),
                metadata(12, 42),
                without_scratch.queue,
                shape(96),
            ),
        ),
        Err(PrivateSegmentAdmissionErrorV1::MissingScratchMapping)
    );

    let fixture = fixture();
    assert_eq!(
        PrivateSegmentDispatchShapeV1::new([32, 1, 1], [64, 1, 1]),
        Err(PrivateSegmentAdmissionErrorV1::InvalidDispatchShape)
    );
    let no_scratch = admit_private_segment_scratch_v1(
        &fixture.identity,
        &fixture.queues,
        &fixture.memory,
        PrivateSegmentAdmissionRequestV1::new(
            target(&fixture, 1 << 24),
            metadata(0, 42),
            fixture.queue,
            shape(96),
        ),
    )
    .unwrap();
    assert_eq!(no_scratch.packet_private_segment_bytes(), 0);

    assert_eq!(
        admit_private_segment_scratch_v1(
            &fixture.identity,
            &fixture.queues,
            &fixture.memory,
            PrivateSegmentAdmissionRequestV1::new(
                target_with_bounds(&fixture, 1024, 4096, 4096, 4),
                metadata(12, 42),
                fixture.queue,
                shape(96),
            ),
        ),
        Err(PrivateSegmentAdmissionErrorV1::ScratchPerQueueExceeded)
    );

    let oversized_shape = PrivateSegmentDispatchShapeV1::new([1025, 1, 1], [1025, 1, 1]).unwrap();
    assert_eq!(
        admit_private_segment_scratch_v1(
            &fixture.identity,
            &fixture.queues,
            &fixture.memory,
            PrivateSegmentAdmissionRequestV1::new(
                target(&fixture, 1 << 24),
                metadata(0, 42),
                fixture.queue,
                oversized_shape,
            ),
        ),
        Err(PrivateSegmentAdmissionErrorV1::InvalidDispatchShape)
    );

    let required = PostLinkPrivateSegmentMetadataV1::new(
        RuntimeArtifactIdV1::from_untrusted_digest(digest(40)),
        digest(41),
        digest(42),
        0,
        GFX942_WAVEFRONT_SIZE_V1,
        Some([8, 8, 1]),
        GFX942_MAX_FLAT_WORKGROUP_SIZE_V1,
        [u32::MAX; 3],
        false,
    )
    .unwrap();
    assert_eq!(
        admit_private_segment_scratch_v1(
            &fixture.identity,
            &fixture.queues,
            &fixture.memory,
            PrivateSegmentAdmissionRequestV1::new(
                target(&fixture, 1 << 24),
                required,
                fixture.queue,
                PrivateSegmentDispatchShapeV1::new([64, 1, 1], [64, 1, 1]).unwrap(),
            ),
        ),
        Err(PrivateSegmentAdmissionErrorV1::InvalidDispatchShape)
    );

    let bounded_workgroups = PostLinkPrivateSegmentMetadataV1::new(
        RuntimeArtifactIdV1::from_untrusted_digest(digest(40)),
        digest(41),
        digest(43),
        0,
        GFX942_WAVEFRONT_SIZE_V1,
        None,
        GFX942_MAX_FLAT_WORKGROUP_SIZE_V1,
        [1, u32::MAX, u32::MAX],
        false,
    )
    .unwrap();
    assert_eq!(
        admit_private_segment_scratch_v1(
            &fixture.identity,
            &fixture.queues,
            &fixture.memory,
            PrivateSegmentAdmissionRequestV1::new(
                target(&fixture, 1 << 24),
                bounded_workgroups,
                fixture.queue,
                PrivateSegmentDispatchShapeV1::new([129, 1, 1], [64, 1, 1]).unwrap(),
            ),
        ),
        Err(PrivateSegmentAdmissionErrorV1::InvalidDispatchShape)
    );

    let uniform_workgroups = PostLinkPrivateSegmentMetadataV1::new(
        RuntimeArtifactIdV1::from_untrusted_digest(digest(40)),
        digest(41),
        digest(44),
        0,
        GFX942_WAVEFRONT_SIZE_V1,
        None,
        GFX942_MAX_FLAT_WORKGROUP_SIZE_V1,
        [u32::MAX; 3],
        true,
    )
    .unwrap();
    assert_eq!(
        admit_private_segment_scratch_v1(
            &fixture.identity,
            &fixture.queues,
            &fixture.memory,
            PrivateSegmentAdmissionRequestV1::new(
                target(&fixture, 1 << 24),
                uniform_workgroups,
                fixture.queue,
                PrivateSegmentDispatchShapeV1::new([65, 1, 1], [64, 1, 1]).unwrap(),
            ),
        ),
        Err(PrivateSegmentAdmissionErrorV1::InvalidDispatchShape)
    );

    assert_eq!(
        admit_private_segment_scratch_v1(
            &fixture.identity,
            &fixture.queues,
            &fixture.memory,
            PrivateSegmentAdmissionRequestV1::new(
                target_with_bounds(&fixture, u64::MAX, u64::MAX, u64::MAX, 2),
                metadata(4096, 42),
                fixture.queue,
                shape(1024),
            ),
        ),
        Err(PrivateSegmentAdmissionErrorV1::ScratchCapacityInsufficient)
    );
}

#[test]
fn scratch_sizing_matches_integer_reference_across_boundary_grid() {
    let fixture = fixture();
    for private_bytes in [1_u64, 4, 255, 256, 257, 1024] {
        for workitems in [1_u32, 63, 64, 65, 96, 255, 256] {
            for resident in [1_u32, 2, 8] {
                let target = target_with_bounds(&fixture, 1 << 20, 1 << 22, 1 << 24, resident);
                let admission = admit_private_segment_scratch_v1(
                    &fixture.identity,
                    &fixture.queues,
                    &fixture.memory,
                    PrivateSegmentAdmissionRequestV1::new(
                        target,
                        metadata(private_bytes, 42),
                        fixture.queue,
                        shape(workitems),
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
