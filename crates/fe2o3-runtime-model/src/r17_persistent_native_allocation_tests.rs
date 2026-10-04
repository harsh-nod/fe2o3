use alloc::{vec, vec::Vec};

use super::*;

const MIB: u64 = 1024 * 1024;

fn digest(seed: u8) -> IdentityDigestV1 {
    IdentityDigestV1::from_untrusted_bytes([seed; IDENTITY_DIGEST_BYTES_V1])
}

fn device(physical: u64, generation: u64) -> DeviceKeyV1 {
    DeviceKeyV1 {
        physical: PhysicalDeviceIdV1(physical),
        generation: DeviceGenerationV1(generation),
    }
}

fn devices() -> [DeviceKeyV1; 2] {
    [device(0x100, 3), device(0x200, 4)]
}

fn queue(device: DeviceKeyV1, id: u64, generation: u64) -> QueueKeyV1 {
    QueueKeyV1 {
        vm: VmKeyV1 {
            device,
            id: VmIdV1(11),
        },
        id: QueueInstanceIdV1(id),
        generation: QueueGenerationV1(generation),
    }
}

fn records(
    byte_len: u64,
    devices: [DeviceKeyV1; 2],
) -> (MemoryAllocationRecordV1, MemoryMappingRecordV1) {
    records_with_home(byte_len, devices, devices[0])
}

fn records_with_home(
    byte_len: u64,
    devices: [DeviceKeyV1; 2],
    home: DeviceKeyV1,
) -> (MemoryAllocationRecordV1, MemoryMappingRecordV1) {
    let vm = VmKeyV1 {
        device: home,
        id: VmIdV1(11),
    };
    let allocation_key = MemoryAllocationKeyV1 {
        vm,
        id: AllocationIdV1(12),
        generation: AllocationGenerationV1(13),
    };
    let allocation = MemoryAllocationRecordV1 {
        key: allocation_key,
        reservation: VaReservationKeyV1 {
            vm,
            id: VaReservationIdV1(14),
        },
        handle: UntrustedAllocationHandleObservationV1(15),
        spec: MemoryAllocationSpecV1 {
            byte_len,
            alignment: MEMORY_PAGE_BYTES_V1,
            kind: MemoryKindV1::DeviceLocal,
            coherence: MemoryCoherenceV1::ExplicitVisibility,
        },
        state: MemoryAllocationStateV1::Live,
    };
    let mapping = MemoryMappingRecordV1 {
        key: MemoryMappingKeyV1 {
            allocation: allocation_key,
            id: MappingIdV1(16),
        },
        target_devices: devices.to_vec(),
        access: MemoryAccessV1::ReadWrite,
        mapped_start: 0,
        mapped_end: 2,
        state: MemoryMappingStateV1::Mapped,
    };
    (allocation, mapping)
}

fn registry_with(
    owner: u64,
    byte_len: u64,
    devices: [DeviceKeyV1; 2],
) -> R17PersistentNativeAllocationRegistryV1 {
    let (allocation, mapping) = records(byte_len, devices);
    R17PersistentNativeAllocationRegistryV1::new_model_only(
        R17PersistentAllocationOwnerIdV1(owner),
        allocation,
        mapping,
        devices,
    )
    .unwrap()
}

fn registry_with_home(
    owner: u64,
    byte_len: u64,
    devices: [DeviceKeyV1; 2],
    home: DeviceKeyV1,
) -> R17PersistentNativeAllocationRegistryV1 {
    let (allocation, mapping) = records_with_home(byte_len, devices, home);
    R17PersistentNativeAllocationRegistryV1::new_model_only(
        R17PersistentAllocationOwnerIdV1(owner),
        allocation,
        mapping,
        devices,
    )
    .unwrap()
}

fn registry() -> R17PersistentNativeAllocationRegistryV1 {
    registry_with(1, 256 * MIB, devices())
}

fn compute(
    device: DeviceKeyV1,
    access: R17PersistentAccessModeV1,
    offset: u64,
    len: u64,
) -> R17PersistentUseDescriptorV1 {
    R17PersistentUseDescriptorV1 {
        class: R17PersistentUseClassV1::Compute {
            device,
            queue: queue(device, 21, 22),
        },
        access,
        range: R17PersistentUseRangeV1 {
            byte_offset: offset,
            byte_len: len,
        },
    }
}

fn local_sdma(
    device: DeviceKeyV1,
    access: R17PersistentAccessModeV1,
    offset: u64,
    len: u64,
) -> R17PersistentUseDescriptorV1 {
    R17PersistentUseDescriptorV1 {
        class: R17PersistentUseClassV1::LocalSdma {
            device,
            queue: queue(device, 31, 32),
            engine_id: 1,
        },
        access,
        range: R17PersistentUseRangeV1 {
            byte_offset: offset,
            byte_len: len,
        },
    }
}

fn mapping_observation() -> UntrustedNativeMultiDeviceMappingV1 {
    UntrustedNativeMultiDeviceMappingV1 {
        schema_version: R9_NATIVE_EVIDENCE_SCHEMA_VERSION_V1,
        operation_identity: digest(1),
        allocation_identity: digest(2),
        kfd_gpu_ids: vec![41, 73],
    }
}

fn active_mapping() -> ModelNativeMultiDeviceMappingV1 {
    begin_native_multi_device_mapping_model_only_v1(mapping_observation())
        .unwrap()
        .observe_map_cumulative_prefix_model_only_v1(0, 2, NativeProgressStatusV1::Succeeded)
        .unwrap()
}

fn route(source: DeviceKeyV1, destination: DeviceKeyV1, engine_id: u32) -> ModelNativeXgmiRouteV1 {
    let observation = UntrustedNativeXgmiRouteObservationV1 {
        schema_version: R9_NATIVE_EVIDENCE_SCHEMA_VERSION_V1,
        route_identity: digest(10),
        topology_identity: digest(11),
        topology_generation: 12,
        observation_epoch: ObservationEpochV1(13),
        source_device: source,
        destination_device: destination,
        source_kfd_gpu_id: 41,
        destination_kfd_gpu_id: 73,
        source_node_id: 5,
        destination_node_id: 6,
        hive_id: 7,
        io_link_index: 0,
        link_type: KFD_XGMI_LINK_TYPE_V1,
        min_bandwidth: 32_000,
        max_bandwidth: 64_000,
        recommended_transfer_size: 4 * MIB,
        recommended_sdma_engine_id_mask: 1 << engine_id,
        selected_sdma_engine_id: engine_id,
        link_flags: KFD_XGMI_LINK_ENABLED_FLAG_V1,
        peer_access_supported: true,
        sdma_xgmi_queue_supported: true,
    };
    let currentness = UntrustedNativeXgmiCurrentnessV1 {
        route_identity: observation.route_identity,
        topology_identity: observation.topology_identity,
        topology_generation: observation.topology_generation,
        observation_epoch: observation.observation_epoch,
        source_device: observation.source_device,
        destination_device: observation.destination_device,
        source_kfd_gpu_id: observation.source_kfd_gpu_id,
        destination_kfd_gpu_id: observation.destination_kfd_gpu_id,
        source_node_id: observation.source_node_id,
        destination_node_id: observation.destination_node_id,
        hive_id: observation.hive_id,
        io_link_index: observation.io_link_index,
        link_type: observation.link_type,
        min_bandwidth: observation.min_bandwidth,
        max_bandwidth: observation.max_bandwidth,
        recommended_transfer_size: observation.recommended_transfer_size,
        recommended_sdma_engine_id_mask: observation.recommended_sdma_engine_id_mask,
        selected_sdma_engine_id: observation.selected_sdma_engine_id,
        link_flags: observation.link_flags,
        reset_fence_current: true,
    };
    admit_native_xgmi_route_model_only_v1(&active_mapping(), observation, currentness).unwrap()
}

fn xgmi_route_metadata(
    source: DeviceKeyV1,
    destination: DeviceKeyV1,
    access: R17PersistentAccessModeV1,
    offset: u64,
    len: u64,
    engine_id: u32,
) -> R17PersistentUseDescriptorV1 {
    R17PersistentUseDescriptorV1 {
        class: R17PersistentUseClassV1::XgmiRouteMetadata {
            source_device: source,
            destination_device: destination,
            engine_id,
            route: route(source, destination, engine_id),
        },
        access,
        range: R17PersistentUseRangeV1 {
            byte_offset: offset,
            byte_len: len,
        },
    }
}

fn publish(
    lease: R17ReservedPersistentUseLeaseV1,
    registry: &mut R17PersistentNativeAllocationRegistryV1,
) -> R17PublishedPersistentUseLeaseV1 {
    lease.publish_model_only(registry).unwrap()
}

fn terminal(
    lease: R17PublishedPersistentUseLeaseV1,
    registry: &mut R17PersistentNativeAllocationRegistryV1,
    status: R17PersistentTerminalStatusV1,
) -> R17TerminalPersistentUseLeaseV1 {
    match lease
        .observe_model_only(registry, R17PersistentUseObservationV1::Terminal(status))
        .unwrap()
    {
        R17PersistentUsePollV1::Terminal(lease) => lease,
        _ => panic!("terminal observation must return terminal custody"),
    }
}

#[test]
fn canonical_admission_accepts_bounded_extent_and_rejects_substitution() {
    let exact = registry();
    assert_eq!(exact.byte_len(), 256 * MIB);
    assert_eq!(exact.devices(), devices());
    assert!(exact.validate_global_invariants().is_ok());
    assert_eq!(
        registry_with(2, MEMORY_PAGE_BYTES_V1, devices()).byte_len(),
        MEMORY_PAGE_BYTES_V1
    );

    for byte_len in [0, 1, 256 * MIB + 1] {
        let (allocation, mapping) = records(byte_len, devices());
        assert_eq!(
            R17PersistentNativeAllocationRegistryV1::new_model_only(
                R17PersistentAllocationOwnerIdV1(1),
                allocation,
                mapping,
                devices(),
            )
            .err(),
            Some(R17PersistentAllocationErrorV1::InvalidAllocation)
        );
    }

    let (mut allocation, mapping) = records(8 * MIB, devices());
    allocation.key.id = AllocationIdV1(0);
    assert_eq!(
        R17PersistentNativeAllocationRegistryV1::new_model_only(
            R17PersistentAllocationOwnerIdV1(1),
            allocation,
            mapping,
            devices(),
        )
        .err(),
        Some(R17PersistentAllocationErrorV1::InvalidAllocation)
    );

    let (allocation, mut mapping) = records(8 * MIB, devices());
    mapping.key.id = MappingIdV1(0);
    assert_eq!(
        R17PersistentNativeAllocationRegistryV1::new_model_only(
            R17PersistentAllocationOwnerIdV1(1),
            allocation,
            mapping,
            devices(),
        )
        .err(),
        Some(R17PersistentAllocationErrorV1::InvalidMapping)
    );
}

#[test]
fn compute_and_local_sdma_bind_exact_device_queue_and_engine() {
    let mut registry = registry();
    let before = registry.snapshot();
    let mut bad_compute = compute(devices()[0], R17PersistentAccessModeV1::Read, 0, 64);
    let R17PersistentUseClassV1::Compute { ref mut queue, .. } = bad_compute.class else {
        unreachable!()
    };
    queue.vm.id = VmIdV1(999);
    assert_eq!(
        registry.reserve_model_only(bad_compute, vec![]).err(),
        Some(R17PersistentAllocationErrorV1::InvalidClassBinding)
    );
    assert_eq!(registry.snapshot(), before);

    let mut bad_sdma = local_sdma(devices()[0], R17PersistentAccessModeV1::Write, 64, 64);
    let R17PersistentUseClassV1::LocalSdma {
        ref mut engine_id, ..
    } = bad_sdma.class
    else {
        unreachable!()
    };
    *engine_id = R17_GFX942_LOCAL_SDMA_ENGINE_COUNT_V1;
    assert_eq!(
        registry.reserve_model_only(bad_sdma, vec![]).err(),
        Some(R17PersistentAllocationErrorV1::InvalidClassBinding)
    );
    assert_eq!(registry.snapshot(), before);
    assert!(
        registry
            .reserve_model_only(
                local_sdma(devices()[0], R17PersistentAccessModeV1::Write, 64, 64),
                vec![],
            )
            .is_ok()
    );
}

#[test]
fn xgmi_route_metadata_fixes_direction_engine_roster_and_owner_relative_access() {
    let [source, destination] = devices();
    let mut source_registry = registry();
    assert!(
        source_registry
            .reserve_model_only(
                xgmi_route_metadata(
                    source,
                    destination,
                    R17PersistentAccessModeV1::Read,
                    0,
                    64,
                    4,
                ),
                vec![],
            )
            .is_ok()
    );
    let before = source_registry.snapshot();
    assert_eq!(
        source_registry
            .reserve_model_only(
                xgmi_route_metadata(
                    source,
                    destination,
                    R17PersistentAccessModeV1::Write,
                    128,
                    64,
                    4,
                ),
                vec![],
            )
            .err(),
        Some(R17PersistentAllocationErrorV1::InvalidClassBinding)
    );
    assert_eq!(source_registry.snapshot(), before);

    let mut destination_registry = registry_with_home(2, 256 * MIB, devices(), destination);
    assert!(
        destination_registry
            .reserve_model_only(
                xgmi_route_metadata(
                    source,
                    destination,
                    R17PersistentAccessModeV1::Write,
                    0,
                    64,
                    15,
                ),
                vec![],
            )
            .is_ok()
    );
    let mut wrong_engine = xgmi_route_metadata(
        source,
        destination,
        R17PersistentAccessModeV1::Write,
        128,
        64,
        15,
    );
    let R17PersistentUseClassV1::XgmiRouteMetadata {
        ref mut engine_id, ..
    } = wrong_engine.class
    else {
        unreachable!()
    };
    *engine_id = 16;
    assert_eq!(
        destination_registry
            .reserve_model_only(wrong_engine, vec![])
            .err(),
        Some(R17PersistentAllocationErrorV1::InvalidClassBinding)
    );
}

#[test]
fn range_checks_are_nonzero_nonoverflowing_and_allocation_relative() {
    let mut registry = registry_with(1, MIB, devices());
    for descriptor in [
        compute(devices()[0], R17PersistentAccessModeV1::Read, 0, 0),
        compute(devices()[0], R17PersistentAccessModeV1::Read, u64::MAX, 2),
        compute(devices()[0], R17PersistentAccessModeV1::Read, MIB - 1, 2),
    ] {
        let before = registry.snapshot();
        assert_eq!(
            registry.reserve_model_only(descriptor, vec![]).err(),
            Some(R17PersistentAllocationErrorV1::InvalidRange)
        );
        assert_eq!(registry.snapshot(), before);
    }
}

#[test]
fn overlapping_readers_are_compatible_across_execution_classes() {
    let mut registry = registry();
    let compute = registry
        .reserve_model_only(
            compute(devices()[0], R17PersistentAccessModeV1::Read, 0, 4096),
            vec![],
        )
        .unwrap();
    let sdma = registry
        .reserve_model_only(
            local_sdma(devices()[0], R17PersistentAccessModeV1::Read, 0, 4096),
            vec![],
        )
        .unwrap();
    let _compute = publish(compute, &mut registry);
    let _sdma = publish(sdma, &mut registry);
    assert_eq!(registry.snapshot().published_count, 2);
    assert!(registry.validate_global_invariants().is_ok());
}

#[test]
fn overlapping_writer_publish_is_atomic_and_retains_move_only_token() {
    let mut registry = registry();
    let reader = registry
        .reserve_model_only(
            compute(devices()[0], R17PersistentAccessModeV1::Read, 0, 4096),
            vec![],
        )
        .unwrap();
    let _reader = publish(reader, &mut registry);
    let writer = registry
        .reserve_model_only(
            local_sdma(devices()[0], R17PersistentAccessModeV1::Write, 2048, 4096),
            vec![],
        )
        .unwrap();
    let before = registry.snapshot();
    let (error, writer) = writer
        .publish_model_only(&mut registry)
        .unwrap_err()
        .into_parts();
    assert_eq!(error, R17PersistentAllocationErrorV1::ConflictingUse);
    assert_eq!(registry.snapshot(), before);
    assert_eq!(
        writer
            .cancel_before_publication_model_only(&mut registry)
            .unwrap()
            .outcome(),
        R17PersistentReleaseOutcomeV1::CancelledBeforePublication
    );
}

#[test]
fn disjoint_writer_can_publish_while_other_use_is_active() {
    let mut registry = registry();
    let left = registry
        .reserve_model_only(
            compute(devices()[0], R17PersistentAccessModeV1::Write, 0, 4096),
            vec![],
        )
        .unwrap();
    let right = registry
        .reserve_model_only(
            local_sdma(devices()[0], R17PersistentAccessModeV1::Write, 4096, 4096),
            vec![],
        )
        .unwrap();
    let _left = publish(left, &mut registry);
    let _right = publish(right, &mut registry);
    assert_eq!(registry.snapshot().published_count, 2);
}

#[test]
fn dependencies_are_unique_known_same_owner_and_gate_publication() {
    let mut registry = registry();
    let producer = registry
        .reserve_model_only(
            compute(devices()[0], R17PersistentAccessModeV1::Read, 0, 64),
            vec![],
        )
        .unwrap();
    let dependency = producer.dependency_model_only();
    let producer = publish(producer, &mut registry);

    let before = registry.snapshot();
    assert_eq!(
        registry
            .reserve_model_only(
                compute(devices()[0], R17PersistentAccessModeV1::Read, 128, 64),
                vec![dependency.clone(), dependency.clone()],
            )
            .err(),
        Some(R17PersistentAllocationErrorV1::DuplicateDependency)
    );
    assert_eq!(registry.snapshot(), before);

    let mut foreign_registry = registry_with(999, 256 * MIB, devices());
    let foreign = foreign_registry
        .reserve_model_only(
            compute(devices()[0], R17PersistentAccessModeV1::Read, 128, 64),
            vec![],
        )
        .unwrap()
        .dependency_model_only();
    assert_eq!(
        registry
            .reserve_model_only(
                compute(devices()[0], R17PersistentAccessModeV1::Read, 128, 64),
                vec![foreign],
            )
            .err(),
        Some(R17PersistentAllocationErrorV1::WrongOwner)
    );

    let consumer = registry
        .reserve_model_only(
            compute(devices()[0], R17PersistentAccessModeV1::Read, 128, 64),
            vec![dependency],
        )
        .unwrap();
    let (error, consumer) = consumer
        .publish_model_only(&mut registry)
        .unwrap_err()
        .into_parts();
    assert_eq!(error, R17PersistentAllocationErrorV1::DependencyNotReady);
    let producer = terminal(
        producer,
        &mut registry,
        R17PersistentTerminalStatusV1::Succeeded,
    );
    let _consumer = publish(consumer, &mut registry);
    producer.release_model_only(&mut registry).unwrap();
}

#[test]
fn failed_terminal_is_exact_but_never_satisfies_a_dependency() {
    let mut registry = registry();
    let producer = registry
        .reserve_model_only(
            compute(devices()[0], R17PersistentAccessModeV1::Read, 0, 64),
            vec![],
        )
        .unwrap();
    let dependency = producer.dependency_model_only();
    let producer = publish(producer, &mut registry);
    let consumer = registry
        .reserve_model_only(
            compute(devices()[0], R17PersistentAccessModeV1::Read, 128, 64),
            vec![dependency],
        )
        .unwrap();
    let producer = terminal(
        producer,
        &mut registry,
        R17PersistentTerminalStatusV1::Failed { code: -5 },
    );
    let (error, consumer) = consumer
        .publish_model_only(&mut registry)
        .unwrap_err()
        .into_parts();
    assert_eq!(error, R17PersistentAllocationErrorV1::DependencyNotReady);
    assert_eq!(
        producer
            .release_model_only(&mut registry)
            .unwrap_err()
            .error(),
        R17PersistentAllocationErrorV1::DependentRetained
    );
    consumer
        .cancel_before_publication_model_only(&mut registry)
        .unwrap();
}

#[test]
fn successful_named_predecessor_orders_writer_but_unrelated_terminal_still_conflicts() {
    let mut registry = registry();
    let producer = registry
        .reserve_model_only(
            compute(devices()[0], R17PersistentAccessModeV1::Write, 0, 4096),
            vec![],
        )
        .unwrap();
    let producer_dependency = producer.dependency_model_only();
    let producer = terminal(
        publish(producer, &mut registry),
        &mut registry,
        R17PersistentTerminalStatusV1::Succeeded,
    );

    let successor = registry
        .reserve_model_only(
            local_sdma(devices()[0], R17PersistentAccessModeV1::Write, 0, 4096),
            vec![producer_dependency.clone()],
        )
        .unwrap();
    let successor = publish(successor, &mut registry);
    assert!(registry.validate_global_invariants().is_ok());
    let successor = terminal(
        successor,
        &mut registry,
        R17PersistentTerminalStatusV1::Succeeded,
    );

    let third = registry
        .reserve_model_only(
            compute(devices()[0], R17PersistentAccessModeV1::Write, 0, 4096),
            vec![producer_dependency],
        )
        .unwrap();
    let (error, third) = third
        .publish_model_only(&mut registry)
        .unwrap_err()
        .into_parts();
    assert_eq!(error, R17PersistentAllocationErrorV1::ConflictingUse);
    third
        .cancel_before_publication_model_only(&mut registry)
        .unwrap();
    producer.release_model_only(&mut registry).unwrap();
    successor.release_model_only(&mut registry).unwrap();
    assert!(registry.validate_global_invariants().is_ok());
}

#[test]
fn dependency_input_bound_is_checked_before_identity_validation() {
    let mut registry = registry();
    let dependency = registry
        .reserve_model_only(
            compute(devices()[0], R17PersistentAccessModeV1::Read, 128, 64),
            vec![],
        )
        .unwrap()
        .dependency_model_only();
    let before = registry.snapshot();
    assert_eq!(
        registry
            .reserve_model_only(
                compute(devices()[0], R17PersistentAccessModeV1::Read, 0, 64),
                vec![dependency.clone(); MAX_R17_PERSISTENT_DEPENDENCIES_V1 + 1],
            )
            .err(),
        Some(R17PersistentAllocationErrorV1::CapacityExceeded)
    );
    assert_eq!(registry.snapshot(), before);

    let mut reconstructed = registry_with(1, 256 * MIB, devices());
    let reconstructed_before = reconstructed.snapshot();
    assert_eq!(
        reconstructed
            .reserve_model_only(
                compute(devices()[0], R17PersistentAccessModeV1::Read, 0, 64),
                vec![dependency],
            )
            .err(),
        Some(R17PersistentAllocationErrorV1::WrongOwner)
    );
    assert_eq!(reconstructed.snapshot(), reconstructed_before);
}

#[test]
fn exact_64_slot_capacity_reuses_slot_with_fresh_generation() {
    let mut registry = registry();
    let mut leases = Vec::new();
    for index in 0..MAX_R17_PERSISTENT_USE_LEASES_V1 {
        leases.push(
            registry
                .reserve_model_only(
                    compute(
                        devices()[0],
                        R17PersistentAccessModeV1::Read,
                        index as u64 * 64,
                        64,
                    ),
                    vec![],
                )
                .unwrap(),
        );
    }
    let before = registry.snapshot();
    assert_eq!(before.lease_count, 64);
    assert_eq!(
        registry
            .reserve_model_only(
                compute(devices()[0], R17PersistentAccessModeV1::Read, 8192, 64),
                vec![],
            )
            .err(),
        Some(R17PersistentAllocationErrorV1::CapacityExceeded)
    );
    assert_eq!(registry.snapshot(), before);

    let old = leases.remove(0);
    let old_key = old.binding().lease;
    old.cancel_before_publication_model_only(&mut registry)
        .unwrap();
    let replacement = registry
        .reserve_model_only(
            compute(devices()[0], R17PersistentAccessModeV1::Read, 8192, 64),
            vec![],
        )
        .unwrap();
    assert_eq!(replacement.binding().lease.slot, old_key.slot);
    assert_ne!(replacement.binding().lease.generation, old_key.generation);
    assert!(registry.record(old_key).is_none());
    assert!(registry.validate_global_invariants().is_ok());
}

#[test]
fn timeout_retains_published_custody_until_exact_terminal_observation() {
    let mut registry = registry();
    let lease = registry
        .reserve_model_only(
            compute(devices()[0], R17PersistentAccessModeV1::Write, 0, 64),
            vec![],
        )
        .unwrap();
    let lease = publish(lease, &mut registry);
    let timed_out = match lease
        .observe_model_only(&mut registry, R17PersistentUseObservationV1::TimedOut)
        .unwrap()
    {
        R17PersistentUsePollV1::TimedOut(lease) => lease,
        _ => panic!("timeout must retain timed-out custody"),
    };
    assert_eq!(registry.snapshot().timed_out_count, 1);
    let timed_out = match timed_out
        .observe_model_only(&mut registry, R17PersistentUseObservationV1::Pending)
        .unwrap()
    {
        R17TimedOutUsePollV1::TimedOut(lease) => lease,
        _ => panic!("pending observation must retain timeout custody"),
    };
    let terminal = match timed_out
        .observe_model_only(
            &mut registry,
            R17PersistentUseObservationV1::Terminal(R17PersistentTerminalStatusV1::Succeeded),
        )
        .unwrap()
    {
        R17TimedOutUsePollV1::Terminal(lease) => lease,
        _ => panic!("terminal observation must return terminal custody"),
    };
    terminal.release_model_only(&mut registry).unwrap();
    assert_eq!(registry.snapshot().lease_count, 0);
}

#[test]
fn currentness_loss_cancels_unpublished_and_quarantines_all_published_states() {
    let mut registry = registry();
    let reserved = registry
        .reserve_model_only(
            compute(devices()[0], R17PersistentAccessModeV1::Read, 0, 64),
            vec![],
        )
        .unwrap();
    let published = registry
        .reserve_model_only(
            compute(devices()[0], R17PersistentAccessModeV1::Read, 128, 64),
            vec![],
        )
        .unwrap();
    let published = publish(published, &mut registry);
    let terminal_reserved = registry
        .reserve_model_only(
            compute(devices()[0], R17PersistentAccessModeV1::Read, 256, 64),
            vec![],
        )
        .unwrap();
    let terminal = terminal(
        publish(terminal_reserved, &mut registry),
        &mut registry,
        R17PersistentTerminalStatusV1::Succeeded,
    );
    let loss = registry
        .lose_currentness_model_only(R17PersistentQuarantineReasonV1::DeviceCurrentnessLost)
        .unwrap();
    assert_eq!(loss.cancelled_reservations, 1);
    assert_eq!(loss.quarantined_uses, 2);
    assert!(!registry.is_current());
    assert_eq!(registry.snapshot().quarantined_count, 2);
    reserved
        .reconcile_after_currentness_loss_model_only(&registry)
        .unwrap();
    assert_eq!(
        published
            .reconcile_after_currentness_loss_model_only(&registry)
            .unwrap()
            .reason(),
        R17PersistentQuarantineReasonV1::DeviceCurrentnessLost
    );
    assert_eq!(
        terminal
            .reconcile_after_currentness_loss_model_only(&registry)
            .unwrap()
            .reason(),
        R17PersistentQuarantineReasonV1::DeviceCurrentnessLost
    );
    assert!(registry.validate_global_invariants().is_ok());
}

#[test]
fn indeterminate_observation_globally_seals_before_any_later_transition() {
    let mut registry = registry();
    let first = registry
        .reserve_model_only(
            compute(devices()[0], R17PersistentAccessModeV1::Read, 0, 64),
            vec![],
        )
        .unwrap();
    let second = registry
        .reserve_model_only(
            compute(devices()[0], R17PersistentAccessModeV1::Read, 128, 64),
            vec![],
        )
        .unwrap();
    let first = publish(first, &mut registry);
    let (first, second) = (
        match first
            .observe_model_only(
                &mut registry,
                R17PersistentUseObservationV1::Indeterminate(
                    R17PersistentQuarantineReasonV1::NativeResultIndeterminate,
                ),
            )
            .unwrap()
        {
            R17PersistentUsePollV1::Quarantined(lease) => lease,
            _ => panic!("indeterminate result must quarantine"),
        },
        second,
    );
    assert_eq!(
        first.reason(),
        R17PersistentQuarantineReasonV1::NativeResultIndeterminate
    );
    let (error, _second) = second
        .publish_model_only(&mut registry)
        .unwrap_err()
        .into_parts();
    assert_eq!(error, R17PersistentAllocationErrorV1::NotCurrent);
    assert_eq!(registry.snapshot().reserved_count, 0);
}

#[test]
fn terminal_release_waits_for_reserved_dependents_then_frees_slot() {
    let mut registry = registry();
    let producer = registry
        .reserve_model_only(
            compute(devices()[0], R17PersistentAccessModeV1::Read, 0, 64),
            vec![],
        )
        .unwrap();
    let key = producer.binding().lease;
    let dependency = producer.dependency_model_only();
    let producer = terminal(
        publish(producer, &mut registry),
        &mut registry,
        R17PersistentTerminalStatusV1::Succeeded,
    );
    let consumer = registry
        .reserve_model_only(
            compute(devices()[0], R17PersistentAccessModeV1::Read, 128, 64),
            vec![dependency.clone()],
        )
        .unwrap();
    let (error, producer) = producer
        .release_model_only(&mut registry)
        .unwrap_err()
        .into_parts();
    assert_eq!(error, R17PersistentAllocationErrorV1::DependentRetained);
    consumer
        .cancel_before_publication_model_only(&mut registry)
        .unwrap();
    producer.release_model_only(&mut registry).unwrap();
    let replacement = registry
        .reserve_model_only(
            compute(devices()[0], R17PersistentAccessModeV1::Read, 256, 64),
            vec![],
        )
        .unwrap();
    assert_eq!(replacement.binding().lease.slot, key.slot);
    assert_ne!(replacement.binding().lease.generation, key.generation);
    let before = registry.snapshot();
    assert_eq!(
        registry
            .reserve_model_only(
                compute(devices()[0], R17PersistentAccessModeV1::Read, 384, 64),
                vec![dependency],
            )
            .err(),
        Some(R17PersistentAllocationErrorV1::UnknownDependency)
    );
    assert_eq!(registry.snapshot(), before);
}

#[test]
fn owner_release_failure_returns_sole_registry_unchanged() {
    let mut registry = registry();
    let lease = registry
        .reserve_model_only(
            compute(devices()[0], R17PersistentAccessModeV1::Read, 0, 64),
            vec![],
        )
        .unwrap();
    let before = registry.snapshot();
    let failure = registry.release_allocation_model_only().err().unwrap();
    assert_eq!(
        failure.error(),
        R17PersistentAllocationErrorV1::IllegalState
    );
    let (_, mut registry) = failure.into_parts();
    assert_eq!(registry.snapshot(), before);
    lease
        .cancel_before_publication_model_only(&mut registry)
        .unwrap();
    let receipt = registry.release_allocation_model_only().ok().unwrap();
    assert_eq!(receipt.completed_lease_count, 1);
}

#[test]
fn identical_reconstructed_registry_rejects_foreign_incarnation_token() {
    let mut first = registry_with(1, 256 * MIB, devices());
    let mut second = registry_with(1, 256 * MIB, devices());
    let lease = first
        .reserve_model_only(
            compute(devices()[0], R17PersistentAccessModeV1::Read, 0, 64),
            vec![],
        )
        .unwrap();
    let before = second.snapshot();
    let (error, lease) = lease
        .publish_model_only(&mut second)
        .unwrap_err()
        .into_parts();
    assert_eq!(error, R17PersistentAllocationErrorV1::WrongOwner);
    assert_eq!(second.snapshot(), before);
    let _lease = publish(lease, &mut first);
}

#[test]
fn inhabited_mixed_compute_local_sdma_and_xgmi_metadata_trace_releases_owner() {
    let [source, destination] = devices();
    let mut registry = registry();
    let compute = registry
        .reserve_model_only(
            compute(source, R17PersistentAccessModeV1::Read, 0, 4096),
            vec![],
        )
        .unwrap();
    let sdma = registry
        .reserve_model_only(
            local_sdma(source, R17PersistentAccessModeV1::Read, 0, 4096),
            vec![],
        )
        .unwrap();
    let compute = terminal(
        publish(compute, &mut registry),
        &mut registry,
        R17PersistentTerminalStatusV1::Succeeded,
    );
    let sdma = terminal(
        publish(sdma, &mut registry),
        &mut registry,
        R17PersistentTerminalStatusV1::Succeeded,
    );
    compute.release_model_only(&mut registry).unwrap();
    sdma.release_model_only(&mut registry).unwrap();

    let peer = registry
        .reserve_model_only(
            xgmi_route_metadata(
                source,
                destination,
                R17PersistentAccessModeV1::Read,
                0,
                4096,
                15,
            ),
            vec![],
        )
        .unwrap();
    let peer = publish(peer, &mut registry);
    let peer = match peer
        .observe_model_only(&mut registry, R17PersistentUseObservationV1::TimedOut)
        .unwrap()
    {
        R17PersistentUsePollV1::TimedOut(peer) => peer,
        _ => panic!("expected timeout"),
    };
    let peer = match peer
        .observe_model_only(
            &mut registry,
            R17PersistentUseObservationV1::Terminal(R17PersistentTerminalStatusV1::Succeeded),
        )
        .unwrap()
    {
        R17TimedOutUsePollV1::Terminal(peer) => peer,
        _ => panic!("expected terminal"),
    };
    peer.release_model_only(&mut registry).unwrap();
    assert!(registry.validate_global_invariants().is_ok());
    let receipt = registry.release_allocation_model_only().ok().unwrap();
    assert_eq!(receipt.completed_lease_count, 3);
}
