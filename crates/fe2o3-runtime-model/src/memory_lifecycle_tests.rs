use alloc::{vec, vec::Vec};

use super::*;

const TEST_KFD_DYNAMIC_MAJOR: u32 = 511;

fn digest(seed: u8) -> IdentityDigestV1 {
    IdentityDigestV1::from_untrusted_bytes([seed; IDENTITY_DIGEST_BYTES_V1])
}

fn domain(seed: u8) -> DeviceObservationDomainIdV1 {
    DeviceObservationDomainIdV1::from_untrusted_digest(digest(seed))
}

fn profile() -> DeviceAdmissionProfileV1 {
    DeviceAdmissionProfileV1::gfx942_xnack_minus_spx_nps1_kfd_1_18_drm_3_64_0(
        DeviceAdmissionProfileIdV1::from_untrusted_digest(digest(2)),
        digest(3),
        digest(4),
    )
}

fn correlation(seed: u8) -> ModelCorrelatedDeviceV1 {
    let domain_id = domain(1);
    let epoch = ObservationEpochV1(9);
    let pci = PciAddressV1 {
        domain: 0,
        bus: seed,
        device: 1,
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
            topology_node_id: u32::from(seed),
            kfd_gpu_id: u32::from(seed) + 1,
            gpu_unique_id: u64::from(seed) + 100,
            drm_render_minor: DRM_RENDER_MIN_MINOR_V1 + u32::from(seed),
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
                minor: DRM_RENDER_MIN_MINOR_V1 + u32::from(seed),
            },
            gpu_unique_id: u64::from(seed) + 100,
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

fn vm_observation(device: ModelDeviceAdmissionV1, vm_id: u64) -> UntrustedVmObservationV1 {
    let correlated = device.correlation();
    UntrustedVmObservationV1 {
        domain_id: correlated.domain_id(),
        device: device.model_key(),
        vm_id: VmIdV1(vm_id),
        kfd_gpu_id: correlated.kfd_gpu_id(),
        render_node: correlated.render_node(),
        pci: correlated.identity().pci,
    }
}

#[derive(Clone, Copy)]
struct AdmissionFixture {
    first: ModelDeviceAdmissionV1,
    second: ModelDeviceAdmissionV1,
    first_vm: ModelVmAdmissionV1,
    second_vm: ModelVmAdmissionV1,
}

fn admissions() -> AdmissionFixture {
    let identity = DeviceIdentityStateV1::new(domain(1));
    let (identity, first) = identity
        .register_device_model_only(correlation(4), DeviceGenerationV1(1))
        .unwrap();
    let (identity, second) = identity
        .register_device_model_only(correlation(5), DeviceGenerationV1(1))
        .unwrap();
    let (identity, first_vm) = identity
        .register_vm_model_only(first, vm_observation(first, 10))
        .unwrap();
    let (_, second_vm) = identity
        .register_vm_model_only(second, vm_observation(second, 11))
        .unwrap();
    AdmissionFixture {
        first,
        second,
        first_vm,
        second_vm,
    }
}

fn mixed_generations_of_one_physical_device() -> (
    ModelVmAdmissionV1,
    ModelDeviceAdmissionV1,
    ModelDeviceAdmissionV1,
) {
    let identity = DeviceIdentityStateV1::new(domain(1));
    let correlated = correlation(4);
    let (identity, old_device) = identity
        .register_device_model_only(correlated, DeviceGenerationV1(1))
        .unwrap();
    let (identity, old_vm) = identity
        .register_vm_model_only(old_device, vm_observation(old_device, 90))
        .unwrap();
    let identity = identity.retire_vm_model_only(old_vm).unwrap();
    let identity = identity.retire_device_model_only(old_device).unwrap();
    let (_, new_device) = identity
        .register_device_model_only(correlated, DeviceGenerationV1(2))
        .unwrap();
    (old_vm, old_device, new_device)
}

fn advance(
    state: MemoryLifecycleStateV1,
    transition: MemoryTransitionV1,
) -> MemoryLifecycleStateV1 {
    let next = state.next(transition).unwrap();
    next.validate_global_invariants().unwrap();
    next
}

fn acquire(
    state: MemoryLifecycleStateV1,
    vm: ModelVmAdmissionV1,
    devices: Vec<ModelDeviceAdmissionV1>,
    handle: u64,
) -> MemoryLifecycleStateV1 {
    advance(
        state,
        MemoryTransitionV1::AcquireVm {
            admission: vm,
            mapping_devices: devices,
            handle: UntrustedVmHandleObservationV1(handle),
            aperture: GpuVaRangeV1 {
                base: 0x1_0000,
                byte_len: 0x10_0000,
            },
        },
    )
}

fn reservation(vm: VmKeyV1, id: u64) -> VaReservationKeyV1 {
    VaReservationKeyV1 {
        vm,
        id: VaReservationIdV1(id),
    }
}

fn allocation(vm: VmKeyV1, id: u64, generation: u64) -> MemoryAllocationKeyV1 {
    MemoryAllocationKeyV1 {
        vm,
        id: AllocationIdV1(id),
        generation: AllocationGenerationV1(generation),
    }
}

fn mapping(allocation: MemoryAllocationKeyV1, id: u64) -> MemoryMappingKeyV1 {
    MemoryMappingKeyV1 {
        allocation,
        id: MappingIdV1(id),
    }
}

fn spec() -> MemoryAllocationSpecV1 {
    MemoryAllocationSpecV1 {
        byte_len: MEMORY_PAGE_BYTES_V1,
        alignment: MEMORY_PAGE_BYTES_V1,
        kind: MemoryKindV1::HostVisibleCoherent,
        coherence: MemoryCoherenceV1::HostCoherent,
    }
}

fn live_allocation(
    devices: AdmissionFixture,
) -> (
    MemoryLifecycleStateV1,
    VaReservationKeyV1,
    MemoryAllocationKeyV1,
) {
    let vm = devices.first_vm.model_key();
    let reservation = reservation(vm, 20);
    let allocation = allocation(vm, 30, 1);
    let state = acquire(
        MemoryLifecycleStateV1::new(domain(1)),
        devices.first_vm,
        vec![devices.first, devices.second],
        100,
    );
    let state = advance(
        state,
        MemoryTransitionV1::ReserveVa {
            key: reservation,
            range: GpuVaRangeV1 {
                base: 0x2_0000,
                byte_len: MEMORY_PAGE_BYTES_V1,
            },
            alignment: MEMORY_PAGE_BYTES_V1,
        },
    );
    let state = advance(
        state,
        MemoryTransitionV1::Allocate {
            key: allocation,
            reservation,
            handle: UntrustedAllocationHandleObservationV1(200),
            spec: spec(),
        },
    );
    (state, reservation, allocation)
}

fn live_monotonic_allocation(
    devices: AdmissionFixture,
) -> (
    MemoryLifecycleStateV1,
    VaReservationKeyV1,
    MemoryAllocationKeyV1,
) {
    let vm = devices.first_vm.model_key();
    let reservation = reservation(vm, 20);
    let allocation = allocation(vm, 30, 1);
    let state = acquire(
        MemoryLifecycleStateV1::new_monotonic_non_reusable(domain(1)),
        devices.first_vm,
        vec![devices.first, devices.second],
        100,
    );
    let state = advance(
        state,
        MemoryTransitionV1::ReserveVa {
            key: reservation,
            range: GpuVaRangeV1 {
                base: 0x2_0000,
                byte_len: MEMORY_PAGE_BYTES_V1,
            },
            alignment: MEMORY_PAGE_BYTES_V1,
        },
    );
    let state = advance(
        state,
        MemoryTransitionV1::Allocate {
            key: allocation,
            reservation,
            handle: UntrustedAllocationHandleObservationV1(200),
            spec: spec(),
        },
    );
    (state, reservation, allocation)
}

fn map_succeeded(
    state: MemoryLifecycleStateV1,
    key: MemoryMappingKeyV1,
    devices: &[DeviceKeyV1],
) -> MemoryLifecycleStateV1 {
    let state = advance(
        state,
        MemoryTransitionV1::BeginMap {
            key,
            target_devices: devices.to_vec(),
            access: MemoryAccessV1::ReadWrite,
        },
    );
    advance(
        state,
        MemoryTransitionV1::ObserveMap {
            key,
            progress: PartialProgressObservationV1 {
                n_success: devices.len(),
                status: PartialOperationStatusV1::Succeeded,
            },
        },
    )
}

fn unmap_and_release(
    state: MemoryLifecycleStateV1,
    key: MemoryMappingKeyV1,
    device_count: usize,
) -> MemoryLifecycleStateV1 {
    let state = advance(state, MemoryTransitionV1::BeginUnmap { key });
    let state = advance(
        state,
        MemoryTransitionV1::ObserveUnmap {
            key,
            progress: PartialProgressObservationV1 {
                n_success: device_count,
                status: PartialOperationStatusV1::Succeeded,
            },
        },
    );
    advance(state, MemoryTransitionV1::ReleaseMapping { key })
}

#[test]
fn partial_map_and_unmap_retain_exact_device_progress_until_bottom_up_release() {
    let devices = admissions();
    let (state, reservation, allocation) = live_allocation(devices);
    let mapping = mapping(allocation, 40);
    let targets = vec![devices.first.model_key(), devices.second.model_key()];
    let state = advance(
        state,
        MemoryTransitionV1::BeginMap {
            key: mapping,
            target_devices: targets.clone(),
            access: MemoryAccessV1::ReadWrite,
        },
    );
    let state = advance(
        state,
        MemoryTransitionV1::ObserveMap {
            key: mapping,
            progress: PartialProgressObservationV1 {
                n_success: 1,
                status: PartialOperationStatusV1::Failed,
            },
        },
    );
    assert_eq!(state.mappings()[0].state, MemoryMappingStateV1::MapFailed);
    assert_eq!(
        state.mappings()[0].retained_device_superset(),
        &targets[..1]
    );
    assert_eq!(
        state.next(MemoryTransitionV1::ReleaseAllocation { key: allocation }),
        Err(MemoryTransitionErrorV1::ResourceInUse(
            MemoryRecordRefV1::Allocation(allocation)
        ))
    );

    let state = advance(state, MemoryTransitionV1::BeginUnmap { key: mapping });
    let state = advance(
        state,
        MemoryTransitionV1::ObserveUnmap {
            key: mapping,
            progress: PartialProgressObservationV1 {
                n_success: 0,
                status: PartialOperationStatusV1::Failed,
            },
        },
    );
    assert_eq!(
        state.mappings()[0].retained_device_superset(),
        &targets[..1]
    );
    let state = advance(state, MemoryTransitionV1::BeginUnmap { key: mapping });
    let state = advance(
        state,
        MemoryTransitionV1::ObserveUnmap {
            key: mapping,
            progress: PartialProgressObservationV1 {
                n_success: 1,
                status: PartialOperationStatusV1::Succeeded,
            },
        },
    );
    let state = advance(state, MemoryTransitionV1::ReleaseMapping { key: mapping });
    let state = advance(
        state,
        MemoryTransitionV1::ReleaseAllocation { key: allocation },
    );
    let state = advance(
        state,
        MemoryTransitionV1::ReleaseVaReservation { key: reservation },
    );
    let state = advance(
        state,
        MemoryTransitionV1::RetireVm {
            key: devices.first_vm.model_key(),
        },
    );
    assert_eq!(state.vms()[0].state, MemoryVmStateV1::Retired);
    assert_eq!(state.reservations().len(), 1);
    assert_eq!(state.allocations().len(), 1);
    assert_eq!(state.mappings().len(), 1);
}

#[test]
fn publications_block_unmap_and_partial_unmap_retains_the_unreported_suffix() {
    let devices = admissions();
    let (state, _, allocation) = live_allocation(devices);
    let mapping = mapping(allocation, 40);
    let targets = vec![devices.first.model_key(), devices.second.model_key()];
    let state = advance(
        state,
        MemoryTransitionV1::BeginMap {
            key: mapping,
            target_devices: targets.clone(),
            access: MemoryAccessV1::Read,
        },
    );
    let state = advance(
        state,
        MemoryTransitionV1::ObserveMap {
            key: mapping,
            progress: PartialProgressObservationV1 {
                n_success: 2,
                status: PartialOperationStatusV1::Succeeded,
            },
        },
    );
    let publication = MemoryPublicationKeyV1 {
        mapping,
        id: MemoryPublicationIdV1(50),
    };
    let state = advance(
        state,
        MemoryTransitionV1::PublishMapping { key: publication },
    );
    assert_eq!(
        state.publications()[0].owner,
        MemoryPublicationOwnerV1::Generic
    );
    assert_eq!(
        state.next(MemoryTransitionV1::BeginUnmap { key: mapping }),
        Err(MemoryTransitionErrorV1::ResourceInUse(
            MemoryRecordRefV1::Mapping(mapping)
        ))
    );
    let state = advance(
        state,
        MemoryTransitionV1::ReleasePublication { key: publication },
    );
    let state = advance(state, MemoryTransitionV1::BeginUnmap { key: mapping });
    let state = advance(
        state,
        MemoryTransitionV1::ObserveUnmap {
            key: mapping,
            progress: PartialProgressObservationV1 {
                n_success: 1,
                status: PartialOperationStatusV1::Failed,
            },
        },
    );
    assert_eq!(
        state.mappings()[0].retained_device_superset(),
        &targets[1..]
    );
    assert!(matches!(
        state.next(MemoryTransitionV1::ReleaseAllocation { key: allocation }),
        Err(MemoryTransitionErrorV1::ResourceInUse(_))
    ));
}

#[test]
fn unchanged_cumulative_unmap_retry_does_not_advance_twice() {
    let devices = admissions();
    let (state, _, allocation) = live_allocation(devices);
    let mapping = mapping(allocation, 42);
    let targets = vec![devices.first.model_key(), devices.second.model_key()];
    let state = advance(
        state,
        MemoryTransitionV1::BeginMap {
            key: mapping,
            target_devices: targets.clone(),
            access: MemoryAccessV1::ReadWrite,
        },
    );
    let state = advance(
        state,
        MemoryTransitionV1::ObserveMap {
            key: mapping,
            progress: PartialProgressObservationV1 {
                n_success: targets.len(),
                status: PartialOperationStatusV1::Succeeded,
            },
        },
    );
    let state = advance(state, MemoryTransitionV1::BeginUnmap { key: mapping });
    let state = advance(
        state,
        MemoryTransitionV1::ObserveUnmap {
            key: mapping,
            progress: PartialProgressObservationV1 {
                n_success: 1,
                status: PartialOperationStatusV1::Failed,
            },
        },
    );
    assert_eq!(state.mappings()[0].mapped_start, 1);
    let state = advance(state, MemoryTransitionV1::BeginUnmap { key: mapping });
    let state = advance(
        state,
        MemoryTransitionV1::ObserveUnmap {
            key: mapping,
            progress: PartialProgressObservationV1 {
                n_success: 1,
                status: PartialOperationStatusV1::Failed,
            },
        },
    );
    assert_eq!(state.mappings()[0].mapped_start, 1);
    assert_eq!(
        state.mappings()[0].retained_device_superset(),
        &targets[1..]
    );
    assert!(matches!(
        state.next(MemoryTransitionV1::ReleaseMapping { key: mapping }),
        Err(MemoryTransitionErrorV1::IllegalState(_))
    ));
}

#[test]
fn failed_full_cumulative_unmap_progress_remains_ambiguous_and_unreleasable() {
    let devices = admissions();
    let (state, _, allocation) = live_allocation(devices);
    let mapping = mapping(allocation, 43);
    let targets = vec![devices.first.model_key(), devices.second.model_key()];
    let state = advance(
        state,
        MemoryTransitionV1::BeginMap {
            key: mapping,
            target_devices: targets.clone(),
            access: MemoryAccessV1::ReadWrite,
        },
    );
    let state = advance(
        state,
        MemoryTransitionV1::ObserveMap {
            key: mapping,
            progress: PartialProgressObservationV1 {
                n_success: targets.len(),
                status: PartialOperationStatusV1::Succeeded,
            },
        },
    );
    let state = advance(state, MemoryTransitionV1::BeginUnmap { key: mapping });
    let state = advance(
        state,
        MemoryTransitionV1::ObserveUnmap {
            key: mapping,
            progress: PartialProgressObservationV1 {
                n_success: targets.len(),
                status: PartialOperationStatusV1::Failed,
            },
        },
    );
    assert_eq!(state.mappings()[0].state, MemoryMappingStateV1::Ambiguous);
    assert_eq!(state.mappings()[0].mapped_start, 0);
    assert_eq!(state.mappings()[0].retained_device_superset(), targets);
    assert!(matches!(
        state.next(MemoryTransitionV1::ReleaseMapping { key: mapping }),
        Err(MemoryTransitionErrorV1::IllegalState(_))
    ));
    assert!(matches!(
        state.next(MemoryTransitionV1::ReleaseAllocation { key: allocation }),
        Err(MemoryTransitionErrorV1::ResourceInUse(_))
    ));
}

#[test]
fn malformed_or_indeterminate_progress_is_fail_closed_and_unreleasable() {
    let devices = admissions();
    let (state, _, allocation) = live_allocation(devices);
    let ambiguous_map = mapping(allocation, 40);
    let targets = vec![devices.first.model_key(), devices.second.model_key()];
    let state = advance(
        state,
        MemoryTransitionV1::BeginMap {
            key: ambiguous_map,
            target_devices: targets.clone(),
            access: MemoryAccessV1::ReadWrite,
        },
    );
    let state = advance(
        state,
        MemoryTransitionV1::ObserveMap {
            key: ambiguous_map,
            progress: PartialProgressObservationV1 {
                n_success: 1,
                status: PartialOperationStatusV1::Succeeded,
            },
        },
    );
    assert_eq!(state.mappings()[0].state, MemoryMappingStateV1::Ambiguous);
    assert_eq!(state.mappings()[0].retained_device_superset(), targets);
    assert!(matches!(
        state.next(MemoryTransitionV1::ReleaseMapping { key: ambiguous_map }),
        Err(MemoryTransitionErrorV1::IllegalState(_))
    ));
    assert!(matches!(
        state.next(MemoryTransitionV1::ReleaseAllocation { key: allocation }),
        Err(MemoryTransitionErrorV1::ResourceInUse(_))
    ));

    let (state, _, allocation) = live_allocation(devices);
    let mapping = mapping(allocation, 41);
    let state = advance(
        state,
        MemoryTransitionV1::BeginMap {
            key: mapping,
            target_devices: targets.clone(),
            access: MemoryAccessV1::ReadWrite,
        },
    );
    let state = advance(
        state,
        MemoryTransitionV1::ObserveMap {
            key: mapping,
            progress: PartialProgressObservationV1 {
                n_success: targets.len(),
                status: PartialOperationStatusV1::Succeeded,
            },
        },
    );
    let state = advance(state, MemoryTransitionV1::BeginUnmap { key: mapping });
    let state = advance(
        state,
        MemoryTransitionV1::ObserveUnmap {
            key: mapping,
            progress: PartialProgressObservationV1 {
                n_success: targets.len() + 1,
                status: PartialOperationStatusV1::Indeterminate,
            },
        },
    );
    assert_eq!(state.mappings()[0].state, MemoryMappingStateV1::Ambiguous);
    assert_eq!(state.mappings()[0].retained_device_superset(), targets);
}

#[test]
fn ranges_device_sets_and_cross_vm_substitutions_reject_failure_atomically() {
    let devices = admissions();
    let (state, _, live_allocation_key) = live_allocation(devices);
    let before = state.clone();
    let wrong_mapping = mapping(live_allocation_key, 41);
    assert_eq!(
        state.next(MemoryTransitionV1::BeginMap {
            key: wrong_mapping,
            target_devices: vec![devices.second.model_key()],
            access: MemoryAccessV1::Read,
        }),
        Err(MemoryTransitionErrorV1::DeviceSetMismatch(
            MemoryRecordRefV1::Mapping(wrong_mapping)
        ))
    );
    assert_eq!(state, before);

    let (stale_vm, old_device, new_device) = mixed_generations_of_one_physical_device();
    assert_eq!(
        MemoryLifecycleStateV1::new(domain(1)).next(MemoryTransitionV1::AcquireVm {
            admission: stale_vm,
            mapping_devices: vec![old_device, new_device],
            handle: UntrustedVmHandleObservationV1(102),
            aperture: GpuVaRangeV1 {
                base: 0x1_0000,
                byte_len: 0x10_0000,
            },
        }),
        Err(MemoryTransitionErrorV1::DeviceSetMismatch(
            MemoryRecordRefV1::Vm(stale_vm.model_key())
        ))
    );

    let state = acquire(
        state,
        devices.second_vm,
        vec![devices.first, devices.second],
        101,
    );
    let second_reservation = reservation(devices.second_vm.model_key(), 21);
    let state = advance(
        state,
        MemoryTransitionV1::ReserveVa {
            key: second_reservation,
            range: GpuVaRangeV1 {
                base: 0x2_0000,
                byte_len: MEMORY_PAGE_BYTES_V1,
            },
            alignment: MEMORY_PAGE_BYTES_V1,
        },
    );
    let cross_vm = allocation(devices.first_vm.model_key(), 31, 1);
    assert_eq!(
        state.next(MemoryTransitionV1::Allocate {
            key: cross_vm,
            reservation: second_reservation,
            handle: UntrustedAllocationHandleObservationV1(201),
            spec: spec(),
        }),
        Err(MemoryTransitionErrorV1::BindingMismatch(
            MemoryRecordRefV1::Allocation(cross_vm)
        ))
    );

    let overlap = reservation(devices.first_vm.model_key(), 22);
    assert_eq!(
        state.next(MemoryTransitionV1::ReserveVa {
            key: overlap,
            range: GpuVaRangeV1 {
                base: 0x2_0000,
                byte_len: MEMORY_PAGE_BYTES_V1,
            },
            alignment: MEMORY_PAGE_BYTES_V1,
        }),
        Err(MemoryTransitionErrorV1::AddressConflict(overlap))
    );
    let overflow = reservation(devices.first_vm.model_key(), 23);
    assert!(matches!(
        state.next(MemoryTransitionV1::ReserveVa {
            key: overflow,
            range: GpuVaRangeV1 {
                base: u64::MAX - (MEMORY_PAGE_BYTES_V1 - 1),
                byte_len: MEMORY_PAGE_BYTES_V1,
            },
            alignment: MEMORY_PAGE_BYTES_V1,
        }),
        Err(MemoryTransitionErrorV1::InvalidRange(_))
    ));
    let misaligned = reservation(devices.first_vm.model_key(), 24);
    assert!(matches!(
        state.next(MemoryTransitionV1::ReserveVa {
            key: misaligned,
            range: GpuVaRangeV1 {
                base: 0x4_0001,
                byte_len: MEMORY_PAGE_BYTES_V1,
            },
            alignment: MEMORY_PAGE_BYTES_V1,
        }),
        Err(MemoryTransitionErrorV1::InvalidAlignment(_))
    ));
}

#[test]
fn allocation_generations_and_opaque_handles_cannot_be_substituted() {
    let devices = admissions();
    let vm = devices.first_vm.model_key();
    let first_reservation = reservation(vm, 20);
    let second_reservation = reservation(vm, 21);
    let mut state = acquire(
        MemoryLifecycleStateV1::new(domain(1)),
        devices.first_vm,
        vec![devices.first],
        100,
    );
    for (key, base) in [
        (first_reservation, 0x2_0000),
        (second_reservation, 0x3_0000),
    ] {
        state = advance(
            state,
            MemoryTransitionV1::ReserveVa {
                key,
                range: GpuVaRangeV1 {
                    base,
                    byte_len: MEMORY_PAGE_BYTES_V1,
                },
                alignment: MEMORY_PAGE_BYTES_V1,
            },
        );
    }
    let generation_two = allocation(vm, 30, 2);
    state = advance(
        state,
        MemoryTransitionV1::Allocate {
            key: generation_two,
            reservation: first_reservation,
            handle: UntrustedAllocationHandleObservationV1(200),
            spec: spec(),
        },
    );
    let colliding = allocation(vm, 31, 1);
    assert_eq!(
        state.next(MemoryTransitionV1::Allocate {
            key: colliding,
            reservation: second_reservation,
            handle: UntrustedAllocationHandleObservationV1(200),
            spec: spec(),
        }),
        Err(MemoryTransitionErrorV1::HandleCollision(
            MemoryRecordRefV1::Allocation(colliding)
        ))
    );
    state = advance(
        state,
        MemoryTransitionV1::ReleaseAllocation {
            key: generation_two,
        },
    );
    let stale = allocation(vm, 30, 1);
    assert_eq!(
        state.next(MemoryTransitionV1::Allocate {
            key: stale,
            reservation: first_reservation,
            handle: UntrustedAllocationHandleObservationV1(200),
            spec: spec(),
        }),
        Err(MemoryTransitionErrorV1::StaleGeneration(stale))
    );
    let generation_three = allocation(vm, 30, 3);
    state = advance(
        state,
        MemoryTransitionV1::Allocate {
            key: generation_three,
            reservation: first_reservation,
            handle: UntrustedAllocationHandleObservationV1(200),
            spec: spec(),
        },
    );
    assert_eq!(state.allocations().len(), 2);
    assert_eq!(state.allocations()[1].key, generation_three);
    assert_eq!(
        state.checkpoint_released(),
        Err(MemoryTransitionErrorV1::CheckpointRequiresMonotonicIdentities)
    );
}

#[test]
fn monotonic_checkpoint_keeps_lower_live_identity_and_rejects_nonmonotonic_admission() {
    let devices = admissions();
    let vm = devices.first_vm.model_key();
    let lower_reservation = reservation(vm, 10);
    let higher_reservation = reservation(vm, 20);
    let lower_allocation = allocation(vm, 10, 1);
    let higher_allocation = allocation(vm, 20, 1);
    let mut state = acquire(
        MemoryLifecycleStateV1::new_monotonic_non_reusable(domain(1)),
        devices.first_vm,
        vec![devices.first],
        100,
    );
    for (key, base) in [
        (lower_reservation, 0x2_0000),
        (higher_reservation, 0x3_0000),
    ] {
        state = advance(
            state,
            MemoryTransitionV1::ReserveVa {
                key,
                range: GpuVaRangeV1 {
                    base,
                    byte_len: MEMORY_PAGE_BYTES_V1,
                },
                alignment: MEMORY_PAGE_BYTES_V1,
            },
        );
    }
    state = advance(
        state,
        MemoryTransitionV1::Allocate {
            key: lower_allocation,
            reservation: lower_reservation,
            handle: UntrustedAllocationHandleObservationV1(110),
            spec: spec(),
        },
    );
    state = advance(
        state,
        MemoryTransitionV1::Allocate {
            key: higher_allocation,
            reservation: higher_reservation,
            handle: UntrustedAllocationHandleObservationV1(120),
            spec: spec(),
        },
    );
    state = advance(
        state,
        MemoryTransitionV1::ReleaseAllocation {
            key: higher_allocation,
        },
    );
    state = advance(
        state,
        MemoryTransitionV1::ReleaseVaReservation {
            key: higher_reservation,
        },
    );

    let skipped_reservation = reservation(vm, 15);
    assert_eq!(
        state.next(MemoryTransitionV1::ReserveVa {
            key: skipped_reservation,
            range: GpuVaRangeV1 {
                base: 0x4_0000,
                byte_len: MEMORY_PAGE_BYTES_V1,
            },
            alignment: MEMORY_PAGE_BYTES_V1,
        }),
        Err(MemoryTransitionErrorV1::NonMonotonicIdentity(
            MemoryRecordRefV1::VaReservation(skipped_reservation)
        ))
    );

    state = state.checkpoint_released().unwrap();
    assert_eq!(state.reservations().len(), 1);
    assert_eq!(state.reservations()[0].key, lower_reservation);
    assert_eq!(state.allocations().len(), 1);
    assert_eq!(state.allocations()[0].key, lower_allocation);
    assert!(
        state
            .issued_id_high_watermarks()
            .iter()
            .any(
                |watermark| watermark.scope == MemoryIssuedIdScopeV1::Allocation(vm)
                    && watermark.last_id == 20
            )
    );

    let fresh_reservation = reservation(vm, 21);
    state = advance(
        state,
        MemoryTransitionV1::ReserveVa {
            key: fresh_reservation,
            range: GpuVaRangeV1 {
                base: 0x4_0000,
                byte_len: MEMORY_PAGE_BYTES_V1,
            },
            alignment: MEMORY_PAGE_BYTES_V1,
        },
    );
    let skipped_allocation = allocation(vm, 15, 99);
    assert_eq!(
        state.next(MemoryTransitionV1::Allocate {
            key: skipped_allocation,
            reservation: fresh_reservation,
            handle: UntrustedAllocationHandleObservationV1(115),
            spec: spec(),
        }),
        Err(MemoryTransitionErrorV1::NonMonotonicIdentity(
            MemoryRecordRefV1::Allocation(skipped_allocation)
        ))
    );
    assert!(state.validate_global_invariants().is_ok());
}

#[test]
fn checkpoint_preserves_live_parent_descendant_tombstones() {
    let devices = admissions();
    let (state, reservation, allocation) = live_monotonic_allocation(devices);
    let mapping = mapping(allocation, 40);
    let targets = [devices.first.model_key(), devices.second.model_key()];
    let state = map_succeeded(state, mapping, &targets);
    let publication = MemoryPublicationKeyV1 {
        mapping,
        id: MemoryPublicationIdV1(50),
    };
    let state = advance(
        state,
        MemoryTransitionV1::PublishMapping { key: publication },
    );
    let state = advance(
        state,
        MemoryTransitionV1::ReleasePublication { key: publication },
    );
    let state = state.checkpoint_released().unwrap();
    assert!(state.publications().is_empty());
    assert_eq!(state.mappings().len(), 1);
    assert_eq!(state.issued_id_high_watermarks().len(), 4);
    assert_eq!(
        state.next(MemoryTransitionV1::PublishMapping { key: publication }),
        Err(MemoryTransitionErrorV1::NonMonotonicIdentity(
            MemoryRecordRefV1::Publication(publication)
        ))
    );

    let state = unmap_and_release(state, mapping, targets.len());
    let state = state.checkpoint_released().unwrap();
    assert!(state.mappings().is_empty());
    assert_eq!(state.issued_id_high_watermarks().len(), 3);
    assert_eq!(
        state.next(MemoryTransitionV1::BeginMap {
            key: mapping,
            target_devices: targets.to_vec(),
            access: MemoryAccessV1::ReadWrite,
        }),
        Err(MemoryTransitionErrorV1::NonMonotonicIdentity(
            MemoryRecordRefV1::Mapping(mapping)
        ))
    );

    let state = advance(
        state,
        MemoryTransitionV1::ReleaseAllocation { key: allocation },
    );
    let state = advance(
        state,
        MemoryTransitionV1::ReleaseVaReservation { key: reservation },
    );
    let state = state.checkpoint_released().unwrap();
    assert_eq!(state.issued_id_high_watermarks().len(), 2);
    assert!(state.validate_global_invariants().is_ok());
}

#[test]
fn checkpoint_supports_more_than_the_old_journal_cap_without_identity_reuse() {
    const CYCLES: u64 = MAX_MEMORY_ALLOCATIONS_V1 as u64 + 128;
    let devices = admissions();
    let vm = devices.first_vm.model_key();
    let targets = [devices.first.model_key(), devices.second.model_key()];
    let mut state = acquire(
        MemoryLifecycleStateV1::new_monotonic_non_reusable(domain(1)),
        devices.first_vm,
        vec![devices.first, devices.second],
        100,
    );

    for id in 1..=CYCLES {
        let reservation = reservation(vm, id);
        let allocation = allocation(vm, id, 1);
        let mapping = mapping(allocation, id);
        state = advance(
            state,
            MemoryTransitionV1::ReserveVa {
                key: reservation,
                range: GpuVaRangeV1 {
                    base: 0x2_0000,
                    byte_len: MEMORY_PAGE_BYTES_V1,
                },
                alignment: MEMORY_PAGE_BYTES_V1,
            },
        );
        state = advance(
            state,
            MemoryTransitionV1::Allocate {
                key: allocation,
                reservation,
                handle: UntrustedAllocationHandleObservationV1(1_000 + id),
                spec: spec(),
            },
        );
        state = map_succeeded(state, mapping, &targets);
        let publication = MemoryPublicationKeyV1 {
            mapping,
            id: MemoryPublicationIdV1(id),
        };
        state = advance(
            state,
            MemoryTransitionV1::PublishMapping { key: publication },
        );
        state = advance(
            state,
            MemoryTransitionV1::ReleasePublication { key: publication },
        );
        state = unmap_and_release(state, mapping, targets.len());
        state = advance(
            state,
            MemoryTransitionV1::ReleaseAllocation { key: allocation },
        );
        state = advance(
            state,
            MemoryTransitionV1::ReleaseVaReservation { key: reservation },
        );
        state = state.checkpoint_released().unwrap();
        assert!(state.reservations().is_empty());
        assert!(state.allocations().is_empty());
        assert!(state.mappings().is_empty());
        assert!(state.publications().is_empty());
    }

    assert_eq!(state.issued_id_high_watermarks().len(), 2);
    assert!(
        state
            .issued_id_high_watermarks()
            .iter()
            .all(|watermark| watermark.last_id == CYCLES)
    );
    let stale_reservation = reservation(vm, 1);
    assert_eq!(
        state.next(MemoryTransitionV1::ReserveVa {
            key: stale_reservation,
            range: GpuVaRangeV1 {
                base: 0x2_0000,
                byte_len: MEMORY_PAGE_BYTES_V1,
            },
            alignment: MEMORY_PAGE_BYTES_V1,
        }),
        Err(MemoryTransitionErrorV1::NonMonotonicIdentity(
            MemoryRecordRefV1::VaReservation(stale_reservation)
        ))
    );

    let fresh_reservation = reservation(vm, CYCLES + 1);
    let state = advance(
        state,
        MemoryTransitionV1::ReserveVa {
            key: fresh_reservation,
            range: GpuVaRangeV1 {
                base: 0x2_0000,
                byte_len: MEMORY_PAGE_BYTES_V1,
            },
            alignment: MEMORY_PAGE_BYTES_V1,
        },
    );
    let stale_allocation = allocation(vm, 1, u64::MAX);
    assert_eq!(
        state.next(MemoryTransitionV1::Allocate {
            key: stale_allocation,
            reservation: fresh_reservation,
            handle: UntrustedAllocationHandleObservationV1(9_999),
            spec: spec(),
        }),
        Err(MemoryTransitionErrorV1::NonMonotonicIdentity(
            MemoryRecordRefV1::Allocation(stale_allocation)
        ))
    );

    let stale_mapping = mapping(allocation(vm, 1, 1), 1);
    assert_eq!(
        state.next(MemoryTransitionV1::BeginMap {
            key: stale_mapping,
            target_devices: targets.to_vec(),
            access: MemoryAccessV1::ReadWrite,
        }),
        Err(MemoryTransitionErrorV1::NotFound(
            MemoryRecordRefV1::Allocation(stale_mapping.allocation)
        ))
    );
    let stale_publication = MemoryPublicationKeyV1 {
        mapping: stale_mapping,
        id: MemoryPublicationIdV1(1),
    };
    assert_eq!(
        state.next(MemoryTransitionV1::PublishMapping {
            key: stale_publication,
        }),
        Err(MemoryTransitionErrorV1::NotFound(
            MemoryRecordRefV1::Mapping(stale_mapping)
        ))
    );
    assert!(state.validate_global_invariants().is_ok());
}

#[test]
fn vm_history_has_a_process_lifetime_bound_and_rejects_domain_substitution() {
    let domain_id = domain(1);
    let identity = DeviceIdentityStateV1::new(domain_id);
    let (mut identity, device) = identity
        .register_device_model_only(correlation(4), DeviceGenerationV1(1))
        .unwrap();
    let mut memory = MemoryLifecycleStateV1::new(domain_id);
    let mut first_vm = None;
    for id in 1..=MAX_MEMORY_VMS_V1 as u64 {
        let (next_identity, vm) = identity
            .register_vm_model_only(device, vm_observation(device, id))
            .unwrap();
        identity = next_identity;
        first_vm.get_or_insert(vm);
        memory = acquire(memory, vm, vec![device], 1_000 + id);
    }
    let overflow = first_vm.unwrap();
    assert_eq!(
        memory.next(MemoryTransitionV1::AcquireVm {
            admission: overflow,
            mapping_devices: vec![device],
            handle: UntrustedVmHandleObservationV1(9_999),
            aperture: GpuVaRangeV1 {
                base: 0x1_0000,
                byte_len: 0x10_0000,
            },
        }),
        Err(MemoryTransitionErrorV1::CapacityExceeded {
            kind: MemoryRecordKindV1::Vm,
            maximum: MAX_MEMORY_VMS_V1,
        })
    );
    assert_eq!(memory.vms().len(), MAX_MEMORY_VMS_V1);

    let foreign_memory = MemoryLifecycleStateV1::new(domain(9));
    assert_eq!(
        foreign_memory.next(MemoryTransitionV1::AcquireVm {
            admission: overflow,
            mapping_devices: vec![device],
            handle: UntrustedVmHandleObservationV1(9_999),
            aperture: GpuVaRangeV1 {
                base: 0x1_0000,
                byte_len: 0x10_0000,
            },
        }),
        Err(MemoryTransitionErrorV1::ObservationDomainMismatch)
    );
}

#[test]
fn transitions_copy_only_the_changed_memory_journal() {
    let devices = admissions();
    let (state, _, allocation) = live_allocation(devices);
    let key = mapping(allocation, 40);
    let state = advance(
        state,
        MemoryTransitionV1::BeginMap {
            key,
            target_devices: vec![devices.first.model_key(), devices.second.model_key()],
            access: MemoryAccessV1::ReadWrite,
        },
    );

    let next = state
        .next(MemoryTransitionV1::ObserveMap {
            key,
            progress: PartialProgressObservationV1 {
                n_success: 2,
                status: PartialOperationStatusV1::Succeeded,
            },
        })
        .unwrap();

    assert_eq!(
        state.shared_journals_for_test(&next),
        [true, true, true, false, true, true]
    );
    state.validate_global_invariants().unwrap();
    next.validate_global_invariants().unwrap();
}

#[test]
fn sequential_publication_growth_and_tail_updates_copy_subquadratically() {
    const RECORDS: usize = 512;
    const COPIES_PER_TREE_LEVEL_CEILING: usize = 96;

    let devices = admissions();
    let (state, _, allocation) = live_allocation(devices);
    let mapped = mapping(allocation, 40);
    let mut state = map_succeeded(
        state,
        mapped,
        &[devices.first.model_key(), devices.second.model_key()],
    );
    let keys: Vec<_> = (0..RECORDS)
        .map(|offset| MemoryPublicationKeyV1 {
            mapping: mapped,
            id: MemoryPublicationIdV1(10_000 + offset as u64),
        })
        .collect();

    reset_journal_copied_records_for_test();
    for &key in &keys {
        state = state
            .next(MemoryTransitionV1::PublishMapping { key })
            .unwrap();
    }
    let append_copies = journal_copied_records_for_test();
    assert!(append_copies <= RECORDS * COPIES_PER_TREE_LEVEL_CEILING);
    assert!(append_copies < RECORDS * RECORDS / 4);

    reset_journal_copied_records_for_test();
    for &key in keys.iter().rev() {
        state = state
            .next(MemoryTransitionV1::ReleasePublication { key })
            .unwrap();
    }
    let update_copies = journal_copied_records_for_test();
    assert!(update_copies <= RECORDS * COPIES_PER_TREE_LEVEL_CEILING);
    assert!(update_copies < RECORDS * RECORDS / 4);

    assert_eq!(state.publications().len(), RECORDS);
    assert!(state.publications().iter().zip(keys).all(
        |(record, key)| record.key == key && record.state == MemoryPublicationStateV1::Released
    ));
    state.validate_global_invariants().unwrap();
}

#[test]
#[ignore = "benchmark-style sequential scale check; run explicitly with --release --ignored"]
fn sequential_journal_growth_and_tail_update_benchmark() {
    use std::time::Instant;

    const RECORDS: usize = MAX_MEMORY_PUBLICATIONS_V1;

    let devices = admissions();
    let (state, _, allocation) = live_allocation(devices);
    let mapped = mapping(allocation, 40);
    let mut state = map_succeeded(
        state,
        mapped,
        &[devices.first.model_key(), devices.second.model_key()],
    );
    let keys: Vec<_> = (0..RECORDS)
        .map(|offset| MemoryPublicationKeyV1 {
            mapping: mapped,
            id: MemoryPublicationIdV1(20_000 + offset as u64),
        })
        .collect();

    reset_journal_copied_records_for_test();
    let append_started = Instant::now();
    for &key in &keys {
        state = state
            .next(MemoryTransitionV1::PublishMapping { key })
            .unwrap();
    }
    let append_elapsed = append_started.elapsed();
    let append_copies = journal_copied_records_for_test();

    reset_journal_copied_records_for_test();
    let update_started = Instant::now();
    for &key in keys.iter().rev() {
        state = state
            .next(MemoryTransitionV1::ReleasePublication { key })
            .unwrap();
    }
    let update_elapsed = update_started.elapsed();
    let update_copies = journal_copied_records_for_test();

    assert!(append_copies < RECORDS * RECORDS / 4);
    assert!(update_copies < RECORDS * RECORDS / 4);
    state.validate_global_invariants().unwrap();
    std::eprintln!(
        "sequential journal records={RECORDS}: append={append_elapsed:?} ({append_copies} copied), reverse-tail update={update_elapsed:?} ({update_copies} copied)"
    );
}

#[test]
#[ignore = "benchmark-style scale check; run explicitly with --release --ignored"]
fn large_unrelated_journal_transition_benchmark() {
    use std::{hint::black_box, time::Instant};

    const ITERATIONS: usize = 20_000;

    let devices = admissions();
    let (state, _, allocation) = live_allocation(devices);
    let published_mapping = mapping(allocation, 40);
    let state = map_succeeded(
        state,
        published_mapping,
        &[devices.first.model_key(), devices.second.model_key()],
    );
    let pending_mapping = mapping(allocation, 41);
    let state = advance(
        state,
        MemoryTransitionV1::BeginMap {
            key: pending_mapping,
            target_devices: vec![devices.first.model_key(), devices.second.model_key()],
            access: MemoryAccessV1::ReadWrite,
        },
    )
    .with_generic_publications_for_test(published_mapping, MAX_MEMORY_PUBLICATIONS_V1);
    let transition = MemoryTransitionV1::ObserveMap {
        key: pending_mapping,
        progress: PartialProgressObservationV1 {
            n_success: 2,
            status: PartialOperationStatusV1::Succeeded,
        },
    };

    let started = Instant::now();
    for _ in 0..ITERATIONS {
        black_box(state.next(transition.clone()).unwrap());
    }
    let elapsed = started.elapsed();

    assert_eq!(state.publications().len(), MAX_MEMORY_PUBLICATIONS_V1);
    std::eprintln!(
        "{ITERATIONS} transitions with {} unrelated publications: {elapsed:?}",
        state.publications().len()
    );
}
