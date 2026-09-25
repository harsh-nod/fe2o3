use alloc::vec;

use super::*;

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

fn pci(bus: u8) -> PciAddressV1 {
    PciAddressV1 {
        domain: 0,
        bus,
        device: 1,
        function: 0,
    }
}

fn range(base: u64, limit: u64) -> InclusiveRangeProjectionV1 {
    InclusiveRangeProjectionV1 { base, limit }
}

fn record(seed: u8) -> DeviceProjectionRecordV1 {
    let render_minor = DRM_RENDER_MIN_MINOR_V1 + u32::from(seed);
    let unique = 100 + u64::from(seed);
    let pci = pci(seed);
    DeviceProjectionRecordV1 {
        schema_version: DEVICE_PROJECTION_SCHEMA_VERSION_V1,
        domain_id: domain(1),
        profile_id: profile().identity(),
        source: DeviceProjectionSourceV1 {
            boot_id: [7; 16],
            topology_file_system_device: 10,
            topology_inode: 11,
            topology_generation: 12,
            process_id: 13,
            process_start_time_ticks: 14,
            mount_namespace_device: 15,
            mount_namespace_inode: 16,
            amdgpu_module_file_system_device: 17,
            amdgpu_module_inode: 18,
            kernel_release: KernelReleaseObservationV1::Linux6_8_0_124Generic,
            amdgpu_module: AmdgpuModuleObservationV1::Version6_16_13SourceA6f143bec60c0afc3263226,
        },
        kfd: KfdProjectionV1 {
            descriptor: CharacterDeviceProjectionV1 {
                file_system_device: 20,
                inode: 21,
                character_device: 22,
                node: DeviceNodeV1 {
                    major: 511,
                    minor: KFD_DEVICE_MINOR_V1,
                },
            },
            uapi_major: KFD_UAPI_MAJOR_V1,
            uapi_minor: KFD_UAPI_MINOR_V1,
            schema_identity: profile().kfd_schema_identity(),
            xnack: XnackObservationV1::Disabled,
        },
        topology: TopologyProjectionV1 {
            node_id: u32::from(seed),
            kfd_gpu_id: u32::from(seed) + 1,
            gpu_unique_id: unique,
            drm_render_minor: render_minor,
            pci,
            vendor_id: AMD_PCI_VENDOR_ID_V1,
            device_id: MI300X_PCI_DEVICE_ID_V1,
            target: GpuTargetObservationV1::Gfx942,
            compute_partition: ComputePartitionObservationV1::Spx,
            memory_partition: MemoryPartitionObservationV1::Nps1,
            firmware_version: MI300X_KFD_FIRMWARE_VERSION_V1,
            sdma_firmware_version: MI300X_SDMA_FIRMWARE_VERSION_V1,
            wavefront_size: MI300X_WAVEFRONT_SIZE_V1,
            simd_count: MI300X_SPX_SIMD_COUNT_V1,
            xcc_count: MI300X_SPX_XCC_COUNT_V1,
        },
        inventory: vec![InventoryDeviceProjectionV1 {
            topology_node_id: u32::from(seed),
            kfd_gpu_id: u32::from(seed) + 1,
            gpu_unique_id: unique,
            drm_render_minor: render_minor,
            pci,
            vendor_id: AMD_PCI_VENDOR_ID_V1,
            device_id: MI300X_PCI_DEVICE_ID_V1,
            pci_revision_id: MI300X_PCI_REVISION_V1,
            target: GpuTargetObservationV1::Gfx942,
        }],
        render: RenderProjectionV1 {
            descriptor: CharacterDeviceProjectionV1 {
                file_system_device: 30,
                inode: 31,
                character_device: 32,
                node: DeviceNodeV1 {
                    major: DRM_DEVICE_MAJOR_V1,
                    minor: render_minor,
                },
            },
            gpu_unique_id: unique,
            pci,
            vendor_id: AMD_PCI_VENDOR_ID_V1,
            device_id: MI300X_PCI_DEVICE_ID_V1,
            pci_revision_id: MI300X_PCI_REVISION_V1,
            schema_identity: profile().drm_schema_identity(),
            driver_name: DrmDriverNameObservationV1::Amdgpu,
            driver_major: DRM_DRIVER_MAJOR_V1,
            driver_minor: DRM_DRIVER_MINOR_V1,
            driver_patch: DRM_DRIVER_PATCH_V1,
            acceleration_working: true,
            family: DrmFamilyObservationV1::AmdgpuFamilyAi,
            family_id: AMDGPU_FAMILY_AI_V1,
            chip_revision: MI300X_CHIP_REVISION_V1,
            external_revision: MI300X_EXTERNAL_REVISION_V1,
            vram_lost_counter: 7,
        },
        apertures: vec![ProcessApertureProjectionV1 {
            kfd_gpu_id: u32::from(seed) + 1,
            lds: range(0x1000, 0x1fff),
            scratch: range(0x3000, 0x3fff),
            gpuvm: range(0x10_0000, 0x1f_ffff),
        }],
        commit_fence: DeviceProjectionCommitFenceV1 {
            process_reobserved_equal: true,
            descriptors_revalidated: true,
            topology_reobserved_equal: true,
            xnack_reobserved_disabled: true,
            apertures_reobserved_equal: true,
            reset_subscription_established: true,
            reset_event_mask_enabled: true,
            reset_event_descriptor_cloexec: true,
            reset_fence_initially_clear: true,
            drm_reobserved_after_subscription_equal: true,
            reset_fence_clear_before_commit: true,
        },
    }
}

fn validate(record: DeviceProjectionRecordV1) -> ValidatedDeviceProjectionV1 {
    validate_device_projection_model_only_v1(record, &profile()).unwrap()
}

#[test]
fn canonical_projection_preserves_every_model_identity_and_schema() {
    let record = record(4);
    let projection = validate(record.clone());
    assert_eq!(projection.authority_domain(), AuthorityDomainV1::ModelOnly);
    assert_eq!(projection.record(), &record);
    let correlation = projection.correlation();
    assert_eq!(correlation.domain_id(), record.domain_id);
    assert_eq!(correlation.profile_id(), record.profile_id);
    assert_eq!(correlation.epoch().0, record.source.topology_generation);
    assert_eq!(
        correlation.identity().gpu_unique_id,
        record.topology.gpu_unique_id
    );
    assert_eq!(correlation.identity().pci, record.topology.pci);
    assert_eq!(correlation.render_node(), record.render.descriptor.node);
    assert_eq!(
        correlation.drm_schema_identity(),
        record.render.schema_identity
    );
    assert_eq!(projection.record().render.vram_lost_counter, 7);
}

#[test]
fn projection_rejects_profile_source_and_commit_mutations() {
    let mut candidate = record(4);
    candidate.kfd.schema_identity = digest(99);
    assert_eq!(
        validate_device_projection_model_only_v1(candidate, &profile()),
        Err(DeviceProjectionErrorV1::ProfileMismatch)
    );
    let mut candidate = record(4);
    candidate.source.topology_generation = 0;
    assert_eq!(
        validate_device_projection_model_only_v1(candidate, &profile()),
        Err(DeviceProjectionErrorV1::SourceIdentityInvalid)
    );
    let mut candidate = record(4);
    candidate.source.kernel_release = KernelReleaseObservationV1::Other;
    assert_eq!(
        validate_device_projection_model_only_v1(candidate, &profile()),
        Err(DeviceProjectionErrorV1::UnsupportedPlatform)
    );
    let mut candidate = record(4);
    candidate.commit_fence.topology_reobserved_equal = false;
    assert_eq!(
        validate_device_projection_model_only_v1(candidate, &profile()),
        Err(DeviceProjectionErrorV1::CommitFenceIncomplete)
    );

    let mut candidate = record(4);
    candidate.commit_fence.reset_subscription_established = false;
    assert_eq!(
        validate_device_projection_model_only_v1(candidate, &profile()),
        Err(DeviceProjectionErrorV1::CommitFenceIncomplete)
    );
    let mut candidate = record(4);
    candidate.commit_fence.reset_event_mask_enabled = false;
    assert_eq!(
        validate_device_projection_model_only_v1(candidate, &profile()),
        Err(DeviceProjectionErrorV1::CommitFenceIncomplete)
    );
    let mut candidate = record(4);
    candidate.commit_fence.reset_event_descriptor_cloexec = false;
    assert_eq!(
        validate_device_projection_model_only_v1(candidate, &profile()),
        Err(DeviceProjectionErrorV1::CommitFenceIncomplete)
    );
    let mut candidate = record(4);
    candidate.commit_fence.reset_fence_initially_clear = false;
    assert_eq!(
        validate_device_projection_model_only_v1(candidate, &profile()),
        Err(DeviceProjectionErrorV1::CommitFenceIncomplete)
    );
    let mut candidate = record(4);
    candidate
        .commit_fence
        .drm_reobserved_after_subscription_equal = false;
    assert_eq!(
        validate_device_projection_model_only_v1(candidate, &profile()),
        Err(DeviceProjectionErrorV1::CommitFenceIncomplete)
    );
    let mut candidate = record(4);
    candidate.commit_fence.reset_fence_clear_before_commit = false;
    assert_eq!(
        validate_device_projection_model_only_v1(candidate, &profile()),
        Err(DeviceProjectionErrorV1::CommitFenceIncomplete)
    );

    let first = validate(record(4));
    let mut changed_counter = record(4);
    changed_counter.render.vram_lost_counter += 1;
    let second = validate(changed_counter);
    assert_ne!(first.record(), second.record());
}

#[test]
fn projection_rejects_kfd_topology_render_and_aperture_mutations() {
    let mut candidate = record(4);
    candidate.kfd.descriptor.node.major = 0;
    assert_eq!(
        validate_device_projection_model_only_v1(candidate, &profile()),
        Err(DeviceProjectionErrorV1::KfdDescriptorInvalid)
    );
    let mut candidate = record(4);
    candidate.topology.firmware_version += 1;
    assert_eq!(
        validate_device_projection_model_only_v1(candidate, &profile()),
        Err(DeviceProjectionErrorV1::TopologyProfileMismatch)
    );
    let mut candidate = record(4);
    candidate.render.chip_revision += 1;
    assert_eq!(
        validate_device_projection_model_only_v1(candidate, &profile()),
        Err(DeviceProjectionErrorV1::RenderProfileMismatch)
    );
    let mut candidate = record(4);
    candidate.render.gpu_unique_id += 1;
    assert_eq!(
        validate_device_projection_model_only_v1(candidate, &profile()),
        Err(DeviceProjectionErrorV1::CrossSourceIdentityMismatch)
    );
    let mut candidate = record(4);
    candidate.apertures[0].lds.limit = 0x2000;
    assert_eq!(
        validate_device_projection_model_only_v1(candidate, &profile()),
        Err(DeviceProjectionErrorV1::InvalidAperture(5))
    );
}

#[test]
fn projection_requires_one_selected_match_in_the_complete_inventory() {
    let mut candidate = record(4);
    candidate.inventory[0].gpu_unique_id += 1;
    assert_eq!(
        validate_device_projection_model_only_v1(candidate, &profile()),
        Err(DeviceProjectionErrorV1::SelectedInventoryMismatch)
    );

    let mut candidate = record(4);
    candidate.inventory.push(candidate.inventory[0]);
    candidate.apertures.push(candidate.apertures[0]);
    assert_eq!(
        validate_device_projection_model_only_v1(candidate, &profile()),
        Err(DeviceProjectionErrorV1::InvalidInventory)
    );

    let mut candidate = record(4);
    let mut preceding = candidate.inventory[0];
    preceding.topology_node_id += 1;
    preceding.kfd_gpu_id += 1;
    preceding.gpu_unique_id += 1;
    preceding.drm_render_minor += 1;
    preceding.pci.bus += 1;
    candidate.inventory.insert(0, preceding);
    let mut aperture = candidate.apertures[0];
    aperture.kfd_gpu_id += 1;
    candidate.apertures.push(aperture);
    candidate.apertures.sort_by_key(|entry| entry.kfd_gpu_id);
    assert_eq!(
        validate_device_projection_model_only_v1(candidate, &profile()),
        Err(DeviceProjectionErrorV1::InvalidInventory)
    );

    let mut candidate = record(4);
    candidate.apertures[0].kfd_gpu_id += 1;
    assert_eq!(
        validate_device_projection_model_only_v1(candidate, &profile()),
        Err(DeviceProjectionErrorV1::SelectedApertureMissing)
    );
}

#[test]
fn projection_history_links_exact_predecessors_and_rejects_reuse() {
    let projection = validate(record(4));
    let history = DeviceProjectionHistoryV1::new(domain(1));
    let (history, first) = history
        .append_model_only(projection.clone(), DeviceGenerationV1(1))
        .unwrap();
    assert_eq!(first.predecessor(), None);
    let (history, second) = history
        .append_model_only(projection.clone(), DeviceGenerationV1(2))
        .unwrap();
    assert_eq!(second.predecessor(), Some(first.current()));
    assert_eq!(history.validate_global_invariants(), Ok(()));
    assert_eq!(
        history.append_model_only(projection, DeviceGenerationV1(2)),
        Err(DeviceProjectionHistoryErrorV1::StaleGeneration)
    );
}

#[test]
fn projection_history_rejects_domain_change_without_discarding_retired_evidence() {
    let projection = validate(record(4));
    let history = DeviceProjectionHistoryV1::new(domain(1));
    let (history, first) = history
        .append_model_only(projection.clone(), DeviceGenerationV1(1))
        .unwrap();

    let mut changed_domain = record(4);
    changed_domain.domain_id = domain(9);
    let changed_domain = validate(changed_domain);
    assert_eq!(
        history.append_model_only(changed_domain, DeviceGenerationV1(2)),
        Err(DeviceProjectionHistoryErrorV1::DomainMismatch)
    );
    assert_eq!(history.entries().len(), 1);
    assert_eq!(history.entries()[0].key, first.current());

    let (history, second) = history
        .append_model_only(projection, DeviceGenerationV1(2))
        .unwrap();
    assert_eq!(history.entries().len(), 2);
    assert_eq!(second.predecessor(), Some(first.current()));
}

#[test]
fn projection_history_rejects_physical_and_correlation_substitution() {
    let first = validate(record(4));
    let history = DeviceProjectionHistoryV1::new(domain(1));
    let (history, _) = history
        .append_model_only(first, DeviceGenerationV1(1))
        .unwrap();

    let mut changed_physical = record(4);
    changed_physical.topology.pci.bus += 1;
    changed_physical.render.pci.bus += 1;
    changed_physical.inventory[0].pci.bus += 1;
    assert_eq!(
        history.append_model_only(validate(changed_physical), DeviceGenerationV1(2)),
        Err(DeviceProjectionHistoryErrorV1::PhysicalIdentitySubstitution)
    );

    let mut alias = record(5);
    alias.topology.pci = pci(4);
    alias.render.pci = pci(4);
    alias.inventory[0].pci = pci(4);
    assert_eq!(
        history.append_model_only(validate(alias), DeviceGenerationV1(2)),
        Err(DeviceProjectionHistoryErrorV1::CorrelationSubstitution)
    );
}

#[test]
fn projection_history_has_an_explicit_process_lifetime_bound() {
    let projection = validate(record(4));
    let mut history = DeviceProjectionHistoryV1::new(domain(1));
    for generation in 1..=MAX_MODEL_DEVICE_ADMISSIONS_V1 as u64 {
        (history, _) = history
            .append_model_only(projection.clone(), DeviceGenerationV1(generation))
            .unwrap();
    }
    assert_eq!(
        history.append_model_only(
            projection.clone(),
            DeviceGenerationV1(MAX_MODEL_DEVICE_ADMISSIONS_V1 as u64 + 1),
        ),
        Err(DeviceProjectionHistoryErrorV1::CapacityExceeded)
    );

    let mut changed_domain = record(4);
    changed_domain.domain_id = domain(9);
    assert_eq!(
        history.append_model_only(
            validate(changed_domain),
            DeviceGenerationV1(MAX_MODEL_DEVICE_ADMISSIONS_V1 as u64 + 2),
        ),
        Err(DeviceProjectionHistoryErrorV1::CapacityExceeded)
    );
    assert_eq!(history.entries().len(), MAX_MODEL_DEVICE_ADMISSIONS_V1);
}
