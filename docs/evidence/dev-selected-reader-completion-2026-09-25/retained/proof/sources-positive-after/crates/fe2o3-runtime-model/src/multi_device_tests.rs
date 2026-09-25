use alloc::{vec, vec::Vec};

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

fn correlation(seed: u8) -> ModelCorrelatedDeviceV1 {
    correlation_with_physical_id(seed, u64::from(seed) + 100)
}

fn correlation_with_physical_id(seed: u8, physical_id: u64) -> ModelCorrelatedDeviceV1 {
    let epoch = ObservationEpochV1(9);
    let pci = PciAddressV1 {
        domain: 0,
        bus: seed,
        device: 1,
        function: 0,
    };
    UntrustedDeviceInventoryV1::from_untrusted_observations(
        UntrustedKfdObservationV1 {
            domain_id: domain(),
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
            domain_id: domain(),
            epoch,
            topology_node_id: u32::from(seed),
            kfd_gpu_id: u32::from(seed) + 1,
            gpu_unique_id: physical_id,
            drm_render_minor: DRM_RENDER_MIN_MINOR_V1 + u32::from(seed),
            pci,
            vendor_id: AMD_PCI_VENDOR_ID_V1,
            device_id: MI300X_PCI_DEVICE_ID_V1,
            target: GpuTargetObservationV1::Gfx942,
            compute_partition: ComputePartitionObservationV1::Spx,
            memory_partition: MemoryPartitionObservationV1::Nps1,
        }],
        vec![UntrustedRenderObservationV1 {
            domain_id: domain(),
            epoch,
            node: DeviceNodeV1 {
                major: DRM_DEVICE_MAJOR_V1,
                minor: DRM_RENDER_MIN_MINOR_V1 + u32::from(seed),
            },
            gpu_unique_id: physical_id,
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

fn vm_observation(device: ModelDeviceAdmissionV1, id: u64) -> UntrustedVmObservationV1 {
    let correlated = device.correlation();
    UntrustedVmObservationV1 {
        domain_id: domain(),
        device: device.model_key(),
        vm_id: VmIdV1(id),
        kfd_gpu_id: correlated.kfd_gpu_id(),
        render_node: correlated.render_node(),
        pci: correlated.identity().pci,
    }
}

fn advance(
    state: MemoryLifecycleStateV1,
    transition: MemoryTransitionV1,
) -> MemoryLifecycleStateV1 {
    state.next(transition).unwrap()
}

fn sorted_admissions(
    source: ModelDeviceAdmissionV1,
    destination: ModelDeviceAdmissionV1,
) -> Vec<ModelDeviceAdmissionV1> {
    if source.model_key() < destination.model_key() {
        vec![source, destination]
    } else {
        vec![destination, source]
    }
}

fn sorted_devices(source: DeviceKeyV1, destination: DeviceKeyV1) -> Vec<DeviceKeyV1> {
    if source < destination {
        vec![source, destination]
    } else {
        vec![destination, source]
    }
}

#[allow(clippy::too_many_arguments)]
fn append_allocation(
    mut memory: MemoryLifecycleStateV1,
    vm: VmKeyV1,
    source: DeviceKeyV1,
    destination: DeviceKeyV1,
    id: u64,
    base: u64,
    kind: MemoryKindV1,
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
    memory = advance(
        memory,
        MemoryTransitionV1::ReserveVa {
            key: reservation,
            range: GpuVaRangeV1 {
                base,
                byte_len: 0x4000,
            },
            alignment: MEMORY_PAGE_BYTES_V1,
        },
    );
    memory = advance(
        memory,
        MemoryTransitionV1::Allocate {
            key: allocation,
            reservation,
            handle: UntrustedAllocationHandleObservationV1(id + 300),
            spec: MemoryAllocationSpecV1 {
                byte_len: 0x4000,
                alignment: MEMORY_PAGE_BYTES_V1,
                kind,
                coherence: match kind {
                    MemoryKindV1::DeviceLocal => MemoryCoherenceV1::ExplicitVisibility,
                    _ => MemoryCoherenceV1::HostCoherent,
                },
            },
        },
    );
    memory = advance(
        memory,
        MemoryTransitionV1::BeginMap {
            key: mapping,
            target_devices: sorted_devices(source, destination),
            access,
        },
    );
    memory = advance(
        memory,
        MemoryTransitionV1::ObserveMap {
            key: mapping,
            progress: PartialProgressObservationV1 {
                n_success: 2,
                status: PartialOperationStatusV1::Succeeded,
            },
        },
    );
    (memory, mapping)
}

fn append_alias_mapping(
    mut memory: MemoryLifecycleStateV1,
    allocation: MemoryAllocationKeyV1,
    source: DeviceKeyV1,
    destination: DeviceKeyV1,
    mapping_id: u64,
    access: MemoryAccessV1,
) -> (MemoryLifecycleStateV1, MemoryMappingKeyV1) {
    let mapping = MemoryMappingKeyV1 {
        allocation,
        id: MappingIdV1(mapping_id),
    };
    memory = advance(
        memory,
        MemoryTransitionV1::BeginMap {
            key: mapping,
            target_devices: sorted_devices(source, destination),
            access,
        },
    );
    memory = advance(
        memory,
        MemoryTransitionV1::ObserveMap {
            key: mapping,
            progress: PartialProgressObservationV1 {
                n_success: 2,
                status: PartialOperationStatusV1::Succeeded,
            },
        },
    );
    (memory, mapping)
}

struct Fixture {
    identity: DeviceIdentityStateV1,
    memory: MemoryLifecycleStateV1,
    source: ModelDeviceAdmissionV1,
    destination: ModelDeviceAdmissionV1,
    source_vm: ModelVmAdmissionV1,
    destination_vm: ModelVmAdmissionV1,
    topology: ModelPeerTopologyV1,
    source_mapping: MemoryMappingKeyV1,
    destination_mapping: MemoryMappingKeyV1,
    destination_alias: MemoryMappingKeyV1,
    destination_readonly_alias: MemoryMappingKeyV1,
    host_mapping: MemoryMappingKeyV1,
}

fn peer_observation(
    source: ModelDeviceAdmissionV1,
    destination: ModelDeviceAdmissionV1,
    source_vm: ModelVmAdmissionV1,
    destination_vm: ModelVmAdmissionV1,
) -> UntrustedPeerTopologyObservationV1 {
    UntrustedPeerTopologyObservationV1 {
        schema_version: MULTI_DEVICE_MODEL_SCHEMA_VERSION_V1,
        domain_id: domain(),
        topology_id: PeerTopologyIdV1::from_untrusted_digest(digest(20)),
        observation_epoch: ObservationEpochV1(9),
        source_device: source.model_key(),
        destination_device: destination.model_key(),
        source_profile: source.correlation().profile_id(),
        destination_profile: destination.correlation().profile_id(),
        source_vm: source_vm.model_key(),
        destination_vm: destination_vm.model_key(),
        peer_access_supported: true,
        virtual_memory_management_supported: true,
    }
}

fn fixture() -> Fixture {
    fixture_with_correlations(correlation(4), correlation(5))
}

fn fixture_with_correlations(
    source_correlation: ModelCorrelatedDeviceV1,
    destination_correlation: ModelCorrelatedDeviceV1,
) -> Fixture {
    let identity = DeviceIdentityStateV1::new(domain());
    let (identity, source) = identity
        .register_device_model_only(source_correlation, DeviceGenerationV1(1))
        .unwrap();
    let (identity, destination) = identity
        .register_device_model_only(destination_correlation, DeviceGenerationV1(1))
        .unwrap();
    let (identity, source_vm) = identity
        .register_vm_model_only(source, vm_observation(source, 10))
        .unwrap();
    let (identity, destination_vm) = identity
        .register_vm_model_only(destination, vm_observation(destination, 11))
        .unwrap();
    let admissions = sorted_admissions(source, destination);
    let memory = advance(
        MemoryLifecycleStateV1::new(domain()),
        MemoryTransitionV1::AcquireVm {
            admission: source_vm,
            mapping_devices: admissions.clone(),
            handle: UntrustedVmHandleObservationV1(100),
            aperture: GpuVaRangeV1 {
                base: 0x1_0000,
                byte_len: 0x10_0000,
            },
        },
    );
    let mut memory = advance(
        memory,
        MemoryTransitionV1::AcquireVm {
            admission: destination_vm,
            mapping_devices: admissions,
            handle: UntrustedVmHandleObservationV1(101),
            aperture: GpuVaRangeV1 {
                base: 0x1_0000,
                byte_len: 0x10_0000,
            },
        },
    );
    let (next, source_mapping) = append_allocation(
        memory,
        source_vm.model_key(),
        source.model_key(),
        destination.model_key(),
        200,
        0x2_0000,
        MemoryKindV1::DeviceLocal,
        MemoryAccessV1::ReadWrite,
    );
    memory = next;
    let (next, destination_mapping) = append_allocation(
        memory,
        destination_vm.model_key(),
        source.model_key(),
        destination.model_key(),
        201,
        0x3_0000,
        MemoryKindV1::DeviceLocal,
        MemoryAccessV1::ReadWrite,
    );
    memory = next;
    let (next, destination_alias) = append_alias_mapping(
        memory,
        destination_mapping.allocation,
        source.model_key(),
        destination.model_key(),
        500,
        MemoryAccessV1::ReadWrite,
    );
    memory = next;
    let (next, destination_readonly_alias) = append_alias_mapping(
        memory,
        destination_mapping.allocation,
        source.model_key(),
        destination.model_key(),
        501,
        MemoryAccessV1::Read,
    );
    memory = next;
    let (memory, host_mapping) = append_allocation(
        memory,
        source_vm.model_key(),
        source.model_key(),
        destination.model_key(),
        202,
        0x4_0000,
        MemoryKindV1::HostVisibleCoherent,
        MemoryAccessV1::ReadWrite,
    );
    let topology = admit_peer_topology_model_only_v1(
        &identity,
        &memory,
        peer_observation(source, destination, source_vm, destination_vm),
    )
    .unwrap();
    Fixture {
        identity,
        memory,
        source,
        destination,
        source_vm,
        destination_vm,
        topology,
        source_mapping,
        destination_mapping,
        destination_alias,
        destination_readonly_alias,
        host_mapping,
    }
}

fn mechanism() -> PeerTransferMechanismV1 {
    PeerTransferMechanismV1::DeclaredPeerCopy {
        contract_identity: digest(21),
    }
}

fn request(
    fixture: &Fixture,
    transfer_id: u64,
    source_mapping: MemoryMappingKeyV1,
    destination_mapping: MemoryMappingKeyV1,
    source_offset: u64,
    destination_offset: u64,
) -> PeerTransferRequestV1 {
    PeerTransferRequestV1::new(
        transfer_id,
        fixture.topology.topology_id(),
        mechanism(),
        PeerTransferRegionV1::new(
            source_mapping,
            fixture.source.model_key(),
            fixture.source.model_key(),
            MemoryAccessV1::Read,
            source_offset,
        ),
        PeerTransferRegionV1::new(
            destination_mapping,
            fixture.destination.model_key(),
            fixture.source.model_key(),
            MemoryAccessV1::ReadWrite,
            destination_offset,
        ),
        0x400,
        0x100,
    )
}

fn completion(seed: u8) -> PeerTransferCompletionIdV1 {
    PeerTransferCompletionIdV1::from_untrusted_digest(digest(seed))
}

fn retention(transfer: PeerTransferRequestV1, id: u64) -> PeerTransferRetentionV1 {
    PeerTransferRetentionV1::new(
        MemoryPublicationKeyV1 {
            mapping: transfer.source().mapping(),
            id: MemoryPublicationIdV1(id),
        },
        MemoryPublicationKeyV1 {
            mapping: transfer.destination().mapping(),
            id: MemoryPublicationIdV1(id + 1),
        },
    )
}

fn current_observation(fixture: &Fixture) -> UntrustedPeerTopologyObservationV1 {
    peer_observation(
        fixture.source,
        fixture.destination,
        fixture.source_vm,
        fixture.destination_vm,
    )
}

fn replacement_identity_and_observation(
    fixture: &Fixture,
) -> (DeviceIdentityStateV1, UntrustedPeerTopologyObservationV1) {
    let identity = fixture
        .identity
        .retire_vm_model_only(fixture.source_vm)
        .unwrap()
        .retire_vm_model_only(fixture.destination_vm)
        .unwrap()
        .retire_device_model_only(fixture.source)
        .unwrap()
        .retire_device_model_only(fixture.destination)
        .unwrap();
    let (identity, source) = identity
        .register_device_model_only(fixture.source.correlation(), DeviceGenerationV1(2))
        .unwrap();
    let (identity, destination) = identity
        .register_device_model_only(fixture.destination.correlation(), DeviceGenerationV1(2))
        .unwrap();
    let (identity, source_vm) = identity
        .register_vm_model_only(
            source,
            vm_observation(source, fixture.source_vm.model_key().id.0),
        )
        .unwrap();
    let (identity, destination_vm) = identity
        .register_vm_model_only(
            destination,
            vm_observation(destination, fixture.destination_vm.model_key().id.0),
        )
        .unwrap();
    let observation = peer_observation(source, destination, source_vm, destination_vm);
    (identity, observation)
}

fn registry(fixture: &Fixture, incarnation: u8) -> PeerTransferRegistryV1 {
    PeerTransferRegistryV1::new_model_only(
        fixture.identity.clone(),
        fixture.memory.clone(),
        fixture.topology,
        current_observation(fixture),
        digest(incarnation),
    )
    .unwrap()
}

#[test]
fn topology_binds_exact_devices_profiles_vms_and_generations() {
    let fixture = fixture();
    assert_eq!(
        fixture.topology.authority_domain(),
        AuthorityDomainV1::ModelOnly
    );
    assert_eq!(fixture.topology.source_device(), fixture.source.model_key());
    assert_eq!(
        fixture.topology.destination_device(),
        fixture.destination.model_key()
    );
    assert_eq!(fixture.topology.source_vm(), fixture.source_vm.model_key());
    assert_eq!(
        fixture.topology.destination_vm(),
        fixture.destination_vm.model_key()
    );

    let valid = peer_observation(
        fixture.source,
        fixture.destination,
        fixture.source_vm,
        fixture.destination_vm,
    );
    let mut hostile = valid;
    hostile.peer_access_supported = false;
    assert_eq!(
        admit_peer_topology_model_only_v1(&fixture.identity, &fixture.memory, hostile),
        Err(PeerTopologyAdmissionErrorV1::PeerAccessUnavailable)
    );
    hostile = valid;
    hostile.virtual_memory_management_supported = false;
    assert_eq!(
        admit_peer_topology_model_only_v1(&fixture.identity, &fixture.memory, hostile),
        Err(PeerTopologyAdmissionErrorV1::VirtualMemoryManagementUnavailable)
    );
    hostile = valid;
    hostile.destination_profile = DeviceAdmissionProfileIdV1::from_untrusted_digest(digest(90));
    assert_eq!(
        admit_peer_topology_model_only_v1(&fixture.identity, &fixture.memory, hostile),
        Err(PeerTopologyAdmissionErrorV1::DeviceProfileMismatch)
    );
    hostile = valid;
    hostile.observation_epoch = ObservationEpochV1(10);
    assert_eq!(
        admit_peer_topology_model_only_v1(&fixture.identity, &fixture.memory, hostile),
        Err(PeerTopologyAdmissionErrorV1::ObservationEpochMismatch)
    );
    hostile = valid;
    hostile.destination_device.generation = DeviceGenerationV1(2);
    hostile.destination_vm.device.generation = DeviceGenerationV1(2);
    assert_eq!(
        admit_peer_topology_model_only_v1(&fixture.identity, &fixture.memory, hostile),
        Err(PeerTopologyAdmissionErrorV1::DestinationDeviceNotCurrent)
    );
    hostile = valid;
    hostile.destination_device = fixture.source.model_key();
    hostile.destination_vm = fixture.source_vm.model_key();
    assert_eq!(
        admit_peer_topology_model_only_v1(&fixture.identity, &fixture.memory, hostile),
        Err(PeerTopologyAdmissionErrorV1::SameDevice)
    );

    let substituted_identity = DeviceIdentityStateV1::new(domain());
    let (substituted_identity, substituted_source) = substituted_identity
        .register_device_model_only(correlation_with_physical_id(14, 104), DeviceGenerationV1(1))
        .unwrap();
    let (substituted_identity, substituted_destination) = substituted_identity
        .register_device_model_only(correlation_with_physical_id(15, 105), DeviceGenerationV1(1))
        .unwrap();
    let (substituted_identity, _) = substituted_identity
        .register_vm_model_only(
            substituted_source,
            vm_observation(substituted_source, fixture.source_vm.model_key().id.0),
        )
        .unwrap();
    let (substituted_identity, _) = substituted_identity
        .register_vm_model_only(
            substituted_destination,
            vm_observation(
                substituted_destination,
                fixture.destination_vm.model_key().id.0,
            ),
        )
        .unwrap();
    assert_eq!(
        PeerTransferRegistryV1::new_model_only(
            substituted_identity,
            fixture.memory.clone(),
            fixture.topology,
            current_observation(&fixture),
            digest(91),
        )
        .unwrap_err()
        .error(),
        PeerTransferErrorV1::Topology(PeerTopologyAdmissionErrorV1::MemoryVmMismatch)
    );
}

#[test]
fn peer_transfer_retains_exact_regions_until_visibility_consumption() {
    let fixture = fixture();
    let mut registry = registry(&fixture, 30);
    let transfer = request(
        &fixture,
        1,
        fixture.source_mapping,
        fixture.destination_mapping,
        0x100,
        0x200,
    );
    let retained = retention(transfer, 1_000);
    let completion = completion(31);
    let reserved = registry
        .reserve_model_only(
            &fixture.identity,
            current_observation(&fixture),
            transfer,
            completion,
            retained,
        )
        .unwrap();
    let binding = reserved.binding();
    assert_eq!(registry.retained_transfer_count(), 1);
    for publication in [retained.source(), retained.destination()] {
        assert!(registry.memory_state().publications().iter().any(|record| {
            record.key == publication
                && record.owner
                    == MemoryPublicationOwnerV1::PeerTransfer(binding.publication_owner())
                && record.state == MemoryPublicationStateV1::Live
        }));
        assert_eq!(
            registry
                .memory_state()
                .next(MemoryTransitionV1::ReleasePublication { key: publication }),
            Err(MemoryTransitionErrorV1::ResourceInUse(
                MemoryRecordRefV1::Publication(publication)
            ))
        );
    }
    assert_eq!(
        registry
            .memory_state()
            .next(MemoryTransitionV1::BeginUnmap {
                key: transfer.destination().mapping(),
            }),
        Err(MemoryTransitionErrorV1::ResourceInUse(
            MemoryRecordRefV1::Mapping(transfer.destination().mapping())
        ))
    );
    let published = reserved
        .publish_model_only(
            &mut registry,
            &fixture.identity,
            current_observation(&fixture),
            40,
        )
        .unwrap();
    let failure = published
        .poll_model_only(
            &mut registry,
            &fixture.identity,
            current_observation(&fixture),
            PeerTransferCompletionObservationV1::Completed {
                completion,
                acquire_sequence: 40,
            },
        )
        .unwrap_err();
    assert_eq!(failure.error(), PeerTransferErrorV1::InvalidOrdering);
    let published = failure.into_retained();
    let visible = match published
        .poll_model_only(
            &mut registry,
            &fixture.identity,
            current_observation(&fixture),
            PeerTransferCompletionObservationV1::Completed {
                completion,
                acquire_sequence: 41,
            },
        )
        .unwrap()
    {
        PeerTransferPollV1::Completed(visible) => visible,
        other => panic!("unexpected peer transfer poll: {other:?}"),
    };
    assert_eq!(visible.request(), transfer);
    assert_eq!(visible.visible_device(), fixture.destination.model_key());
    assert_eq!(visible.acquire_sequence(), 41);
    let receipt = visible
        .release_after_visibility_consumed_model_only(
            &mut registry,
            &fixture.identity,
            current_observation(&fixture),
        )
        .unwrap();
    assert_eq!(receipt.request(), transfer);
    assert_eq!(receipt.acquire_sequence(), Some(41));
    assert_eq!(registry.retained_transfer_count(), 0);
    let (identity, memory) = registry.into_states().unwrap();
    identity.validate_global_invariants().unwrap();
    memory.validate_global_invariants().unwrap();
}

#[test]
fn peer_publication_owner_rejects_different_transfer_release() {
    let fixture = fixture();
    let first_request = request(
        &fixture,
        1,
        fixture.source_mapping,
        fixture.destination_mapping,
        0,
        0,
    );
    let second_request = request(
        &fixture,
        2,
        fixture.source_mapping,
        fixture.destination_mapping,
        0x800,
        0x800,
    );
    let first_retention = retention(first_request, 1_050);
    let mut registry = registry(&fixture, 31);
    let first = registry
        .reserve_model_only(
            &fixture.identity,
            current_observation(&fixture),
            first_request,
            completion(32),
            first_retention,
        )
        .unwrap();
    let second = registry
        .reserve_model_only(
            &fixture.identity,
            current_observation(&fixture),
            second_request,
            completion(33),
            retention(second_request, 1_060),
        )
        .unwrap();
    assert_eq!(
        registry
            .memory_state()
            .release_peer_transfer_publication(first_retention.source(), second.binding()),
        Err(MemoryTransitionErrorV1::BindingMismatch(
            MemoryRecordRefV1::Publication(first_retention.source())
        ))
    );
    first
        .cancel_before_publication_model_only(&mut registry)
        .unwrap();
    second
        .cancel_before_publication_model_only(&mut registry)
        .unwrap();
    assert_eq!(registry.retained_transfer_count(), 0);
}

#[test]
fn peer_transfer_cancellation_and_ambiguity_have_distinct_custody() {
    let fixture = fixture();
    let transfer = request(
        &fixture,
        1,
        fixture.source_mapping,
        fixture.destination_mapping,
        0,
        0,
    );
    let mut cancelled_registry = registry(&fixture, 32);
    let cancelled = cancelled_registry
        .reserve_model_only(
            &fixture.identity,
            current_observation(&fixture),
            transfer,
            completion(33),
            retention(transfer, 1_100),
        )
        .unwrap()
        .cancel_before_publication_model_only(&mut cancelled_registry)
        .unwrap();
    assert_eq!(cancelled.acquire_sequence(), None);
    assert_eq!(cancelled_registry.retained_transfer_count(), 0);

    let mut ambiguous_registry = registry(&fixture, 34);
    let retained = retention(transfer, 1_200);
    let published = ambiguous_registry
        .reserve_model_only(
            &fixture.identity,
            current_observation(&fixture),
            transfer,
            completion(35),
            retained,
        )
        .unwrap()
        .publish_model_only(
            &mut ambiguous_registry,
            &fixture.identity,
            current_observation(&fixture),
            50,
        )
        .unwrap();
    let quarantine = match published
        .poll_model_only(
            &mut ambiguous_registry,
            &fixture.identity,
            current_observation(&fixture),
            PeerTransferCompletionObservationV1::Indeterminate,
        )
        .unwrap()
    {
        PeerTransferPollV1::Indeterminate(quarantine) => quarantine,
        other => panic!("unexpected peer transfer poll: {other:?}"),
    };
    assert_eq!(quarantine.request(), transfer);
    assert_eq!(ambiguous_registry.retained_transfer_count(), 1);
    assert!(ambiguous_registry.into_states().is_err());
}

#[test]
fn publish_rejects_stale_replacement_and_lost_capability_observations() {
    let fixture = fixture();
    let transfer = request(
        &fixture,
        1,
        fixture.source_mapping,
        fixture.destination_mapping,
        0,
        0,
    );

    let stale_identity = fixture
        .identity
        .retire_vm_model_only(fixture.source_vm)
        .unwrap();
    let mut stale_registry = registry(&fixture, 70);
    let reserved = stale_registry
        .reserve_model_only(
            &fixture.identity,
            current_observation(&fixture),
            transfer,
            completion(71),
            retention(transfer, 4_000),
        )
        .unwrap();
    let failure = reserved
        .publish_model_only(
            &mut stale_registry,
            &stale_identity,
            current_observation(&fixture),
            80,
        )
        .unwrap_err();
    assert_eq!(
        failure.error(),
        PeerTransferErrorV1::Topology(PeerTopologyAdmissionErrorV1::VmNotCurrent)
    );
    failure
        .into_retained()
        .cancel_before_publication_model_only(&mut stale_registry)
        .unwrap();

    let (replacement_identity, replacement_observation) =
        replacement_identity_and_observation(&fixture);
    let mut replacement_registry = registry(&fixture, 72);
    let reserved = replacement_registry
        .reserve_model_only(
            &fixture.identity,
            current_observation(&fixture),
            transfer,
            completion(73),
            retention(transfer, 4_100),
        )
        .unwrap();
    let failure = reserved
        .publish_model_only(
            &mut replacement_registry,
            &replacement_identity,
            replacement_observation,
            81,
        )
        .unwrap_err();
    assert_eq!(
        failure.error(),
        PeerTransferErrorV1::Topology(PeerTopologyAdmissionErrorV1::MemoryVmMismatch)
    );
    failure
        .into_retained()
        .cancel_before_publication_model_only(&mut replacement_registry)
        .unwrap();

    for (offset, expected) in [
        (0_u64, PeerTopologyAdmissionErrorV1::PeerAccessUnavailable),
        (
            100_u64,
            PeerTopologyAdmissionErrorV1::VirtualMemoryManagementUnavailable,
        ),
    ] {
        let mut lost_capability = current_observation(&fixture);
        if offset == 0 {
            lost_capability.peer_access_supported = false;
        } else {
            lost_capability.virtual_memory_management_supported = false;
        }
        let mut capability_registry = registry(&fixture, 74 + offset as u8);
        let reserved = capability_registry
            .reserve_model_only(
                &fixture.identity,
                current_observation(&fixture),
                transfer,
                completion(75 + offset as u8),
                retention(transfer, 4_200 + offset),
            )
            .unwrap();
        let failure = reserved
            .publish_model_only(
                &mut capability_registry,
                &fixture.identity,
                lost_capability,
                82 + offset,
            )
            .unwrap_err();
        assert_eq!(failure.error(), PeerTransferErrorV1::Topology(expected));
        failure
            .into_retained()
            .cancel_before_publication_model_only(&mut capability_registry)
            .unwrap();
    }
}

#[test]
fn submitted_currentness_loss_requires_explicit_quarantine() {
    let fixture = fixture();
    let transfer = request(
        &fixture,
        1,
        fixture.source_mapping,
        fixture.destination_mapping,
        0,
        0,
    );
    let retained = retention(transfer, 4_500);
    let mut registry = registry(&fixture, 80);
    let published = registry
        .reserve_model_only(
            &fixture.identity,
            current_observation(&fixture),
            transfer,
            completion(81),
            retained,
        )
        .unwrap()
        .publish_model_only(
            &mut registry,
            &fixture.identity,
            current_observation(&fixture),
            90,
        )
        .unwrap();
    let mut lost_capability = current_observation(&fixture);
    lost_capability.peer_access_supported = false;
    let failure = published
        .poll_model_only(
            &mut registry,
            &fixture.identity,
            lost_capability,
            PeerTransferCompletionObservationV1::Pending,
        )
        .unwrap_err();
    assert_eq!(
        failure.error(),
        PeerTransferErrorV1::Topology(PeerTopologyAdmissionErrorV1::PeerAccessUnavailable)
    );
    let quarantine = failure
        .into_retained()
        .quarantine_currentness_loss_model_only(&mut registry)
        .unwrap();
    assert_eq!(quarantine.request(), transfer);
    assert_eq!(registry.retained_transfer_count(), 1);
    assert!(registry.into_states().is_err());
}

#[test]
fn visibility_release_revalidates_currentness_or_quarantines() {
    let fixture = fixture();
    let transfer = request(
        &fixture,
        1,
        fixture.source_mapping,
        fixture.destination_mapping,
        0,
        0,
    );
    let completion = completion(83);
    let mut registry = registry(&fixture, 82);
    let published = registry
        .reserve_model_only(
            &fixture.identity,
            current_observation(&fixture),
            transfer,
            completion,
            retention(transfer, 4_600),
        )
        .unwrap()
        .publish_model_only(
            &mut registry,
            &fixture.identity,
            current_observation(&fixture),
            100,
        )
        .unwrap();
    let visible = match published
        .poll_model_only(
            &mut registry,
            &fixture.identity,
            current_observation(&fixture),
            PeerTransferCompletionObservationV1::Completed {
                completion,
                acquire_sequence: 101,
            },
        )
        .unwrap()
    {
        PeerTransferPollV1::Completed(visible) => visible,
        other => panic!("unexpected peer transfer poll: {other:?}"),
    };
    let (replacement_identity, replacement_observation) =
        replacement_identity_and_observation(&fixture);
    let failure = visible
        .release_after_visibility_consumed_model_only(
            &mut registry,
            &replacement_identity,
            replacement_observation,
        )
        .unwrap_err();
    assert_eq!(
        failure.error(),
        PeerTransferErrorV1::Topology(PeerTopologyAdmissionErrorV1::MemoryVmMismatch)
    );
    let quarantine = failure
        .into_retained()
        .quarantine_currentness_loss_model_only(&mut registry)
        .unwrap();
    assert_eq!(quarantine.request(), transfer);
    assert_eq!(registry.retained_transfer_count(), 1);
    assert!(registry.into_states().is_err());
}

#[test]
fn hostile_ranges_access_aliases_and_generation_substitution_reject() {
    let fixture = fixture();
    let mut registry = registry(&fixture, 36);
    let valid = request(
        &fixture,
        1,
        fixture.source_mapping,
        fixture.destination_mapping,
        0x100,
        0x200,
    );
    let cases = [
        (
            PeerTransferRequestV1::new(
                2,
                valid.topology_id(),
                valid.mechanism(),
                PeerTransferRegionV1::new(
                    fixture.source_mapping,
                    fixture.source.model_key(),
                    fixture.source.model_key(),
                    MemoryAccessV1::Read,
                    u64::MAX - 7,
                ),
                valid.destination(),
                16,
                8,
            ),
            PeerTransferAdmissionErrorV1::InvalidRange,
        ),
        (
            PeerTransferRequestV1::new(
                3,
                valid.topology_id(),
                valid.mechanism(),
                PeerTransferRegionV1::new(
                    fixture.source_mapping,
                    fixture.source.model_key(),
                    fixture.source.model_key(),
                    MemoryAccessV1::Read,
                    1,
                ),
                valid.destination(),
                8,
                8,
            ),
            PeerTransferAdmissionErrorV1::InvalidAlignment,
        ),
        (
            PeerTransferRequestV1::new(
                4,
                valid.topology_id(),
                valid.mechanism(),
                valid.source(),
                PeerTransferRegionV1::new(
                    fixture.destination_readonly_alias,
                    fixture.destination.model_key(),
                    fixture.source.model_key(),
                    MemoryAccessV1::ReadWrite,
                    0,
                ),
                8,
                8,
            ),
            PeerTransferAdmissionErrorV1::InvalidAccess,
        ),
        (
            PeerTransferRequestV1::new(
                5,
                valid.topology_id(),
                valid.mechanism(),
                PeerTransferRegionV1::new(
                    fixture.host_mapping,
                    fixture.source.model_key(),
                    fixture.source.model_key(),
                    MemoryAccessV1::Read,
                    0,
                ),
                valid.destination(),
                8,
                8,
            ),
            PeerTransferAdmissionErrorV1::UnsupportedMemoryKind,
        ),
        (
            PeerTransferRequestV1::new(
                6,
                valid.topology_id(),
                valid.mechanism(),
                valid.source(),
                PeerTransferRegionV1::new(
                    fixture.source_mapping,
                    fixture.destination.model_key(),
                    fixture.source.model_key(),
                    MemoryAccessV1::ReadWrite,
                    0,
                ),
                8,
                8,
            ),
            PeerTransferAdmissionErrorV1::AliasedEndpoints,
        ),
        (
            PeerTransferRequestV1::new(
                7,
                valid.topology_id(),
                valid.mechanism(),
                valid.source(),
                PeerTransferRegionV1::new(
                    fixture.destination_mapping,
                    DeviceKeyV1 {
                        generation: DeviceGenerationV1(2),
                        ..fixture.destination.model_key()
                    },
                    fixture.source.model_key(),
                    MemoryAccessV1::ReadWrite,
                    0,
                ),
                8,
                8,
            ),
            PeerTransferAdmissionErrorV1::EndpointBindingMismatch,
        ),
        (
            PeerTransferRequestV1::new(
                8,
                valid.topology_id(),
                valid.mechanism(),
                PeerTransferRegionV1::new(
                    fixture.destination_mapping,
                    fixture.destination.model_key(),
                    fixture.source.model_key(),
                    MemoryAccessV1::Read,
                    0,
                ),
                PeerTransferRegionV1::new(
                    fixture.source_mapping,
                    fixture.source.model_key(),
                    fixture.source.model_key(),
                    MemoryAccessV1::ReadWrite,
                    0,
                ),
                8,
                8,
            ),
            PeerTransferAdmissionErrorV1::EndpointBindingMismatch,
        ),
        (
            PeerTransferRequestV1::new(
                9,
                PeerTopologyIdV1::from_untrusted_digest(digest(99)),
                valid.mechanism(),
                valid.source(),
                valid.destination(),
                8,
                8,
            ),
            PeerTransferAdmissionErrorV1::TopologyMismatch,
        ),
    ];
    for (index, (request, expected)) in cases.into_iter().enumerate() {
        assert_eq!(
            registry
                .reserve_model_only(
                    &fixture.identity,
                    current_observation(&fixture),
                    request,
                    completion(50 + index as u8),
                    retention(request, 2_000 + 2 * index as u64),
                )
                .unwrap_err(),
            PeerTransferErrorV1::Admission(expected)
        );
    }
}

#[test]
fn allocation_alias_conflicts_and_cross_registry_request_substitution_reject() {
    let fixture = fixture();
    let first_request = request(
        &fixture,
        1,
        fixture.source_mapping,
        fixture.destination_mapping,
        0,
        0x100,
    );
    let alias_request = request(
        &fixture,
        2,
        fixture.source_mapping,
        fixture.destination_alias,
        0x800,
        0x200,
    );
    let mut conflict_registry = registry(&fixture, 60);
    let first = conflict_registry
        .reserve_model_only(
            &fixture.identity,
            current_observation(&fixture),
            first_request,
            completion(61),
            retention(first_request, 3_000),
        )
        .unwrap();
    assert_eq!(
        conflict_registry
            .reserve_model_only(
                &fixture.identity,
                current_observation(&fixture),
                alias_request,
                completion(62),
                retention(alias_request, 3_100)
            )
            .unwrap_err(),
        PeerTransferErrorV1::ResourceConflict
    );
    first
        .cancel_before_publication_model_only(&mut conflict_registry)
        .unwrap();

    let second_request = request(
        &fixture,
        first_request.transfer_id(),
        fixture.source_mapping,
        fixture.destination_mapping,
        0x800,
        0x900,
    );
    let shared_completion = completion(63);
    let shared_retention = retention(first_request, 3_200);
    let mut first_registry = registry(&fixture, 64);
    let mut second_registry = registry(&fixture, 64);
    let first_token = first_registry
        .reserve_model_only(
            &fixture.identity,
            current_observation(&fixture),
            first_request,
            shared_completion,
            shared_retention,
        )
        .unwrap();
    let second_token = second_registry
        .reserve_model_only(
            &fixture.identity,
            current_observation(&fixture),
            second_request,
            shared_completion,
            shared_retention,
        )
        .unwrap();
    let failure = first_token
        .publish_model_only(
            &mut second_registry,
            &fixture.identity,
            current_observation(&fixture),
            70,
        )
        .unwrap_err();
    assert_eq!(failure.error(), PeerTransferErrorV1::TokenMismatch);
    failure
        .into_retained()
        .cancel_before_publication_model_only(&mut first_registry)
        .unwrap();
    second_token
        .cancel_before_publication_model_only(&mut second_registry)
        .unwrap();

    let substituted_fixture = fixture_with_correlations(
        correlation_with_physical_id(14, 104),
        correlation_with_physical_id(15, 105),
    );
    let substituted_request = request(
        &substituted_fixture,
        first_request.transfer_id(),
        substituted_fixture.source_mapping,
        substituted_fixture.destination_mapping,
        0,
        0x100,
    );
    assert_eq!(substituted_request, first_request);
    let mut original_registry = registry(&fixture, 65);
    let mut substituted_registry = registry(&substituted_fixture, 65);
    let original_token = original_registry
        .reserve_model_only(
            &fixture.identity,
            current_observation(&fixture),
            first_request,
            shared_completion,
            shared_retention,
        )
        .unwrap();
    let substituted_token = substituted_registry
        .reserve_model_only(
            &substituted_fixture.identity,
            current_observation(&substituted_fixture),
            substituted_request,
            shared_completion,
            shared_retention,
        )
        .unwrap();
    let failure = original_token
        .publish_model_only(
            &mut substituted_registry,
            &substituted_fixture.identity,
            current_observation(&substituted_fixture),
            71,
        )
        .unwrap_err();
    assert_eq!(failure.error(), PeerTransferErrorV1::TokenMismatch);
    failure
        .into_retained()
        .cancel_before_publication_model_only(&mut original_registry)
        .unwrap();
    substituted_token
        .cancel_before_publication_model_only(&mut substituted_registry)
        .unwrap();
}
