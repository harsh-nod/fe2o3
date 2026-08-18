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

#[derive(Clone, Copy)]
struct ObservationFixture {
    kfd: UntrustedKfdObservationV1,
    topology: UntrustedTopologyObservationV1,
    render: UntrustedRenderObservationV1,
}

impl ObservationFixture {
    fn inventory(self) -> UntrustedDeviceInventoryV1 {
        UntrustedDeviceInventoryV1::from_untrusted_observations(
            self.kfd,
            vec![self.topology],
            vec![self.render],
        )
        .unwrap()
    }

    fn correlate(self) -> ModelCorrelatedDeviceV1 {
        self.inventory().correlate_model_only(&profile()).unwrap()
    }
}

fn observations(seed: u8) -> ObservationFixture {
    let domain_id = domain(1);
    let epoch = ObservationEpochV1(9);
    let pci = PciAddressV1 {
        domain: 0,
        bus: seed,
        device: 1,
        function: 0,
    };
    ObservationFixture {
        kfd: UntrustedKfdObservationV1 {
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
        topology: UntrustedTopologyObservationV1 {
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
        },
        render: UntrustedRenderObservationV1 {
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
            pci_revision_id: 1,
            drm_schema_identity: digest(4),
            driver_name: DrmDriverNameObservationV1::Amdgpu,
            drm_major: DRM_DRIVER_MAJOR_V1,
            drm_minor: DRM_DRIVER_MINOR_V1,
            drm_patch: DRM_DRIVER_PATCH_V1,
            acceleration_working: true,
            family: DrmFamilyObservationV1::AmdgpuFamilyAi,
        },
    }
}

fn vm_observation(token: ModelDeviceAdmissionV1, vm_id: u64) -> UntrustedVmObservationV1 {
    let correlation = token.correlation();
    UntrustedVmObservationV1 {
        domain_id: correlation.domain_id(),
        device: token.model_key(),
        vm_id: VmIdV1(vm_id),
        kfd_gpu_id: correlation.kfd_gpu_id(),
        render_node: correlation.render_node(),
        pci: correlation.identity().pci,
    }
}

#[test]
fn correlation_is_deterministic_model_only_and_binds_every_observation() {
    let fixture = observations(4);
    let first = fixture.correlate();
    let second = fixture.correlate();
    assert_eq!(first, second);
    assert_eq!(first.authority_domain(), AuthorityDomainV1::ModelOnly);
    assert_eq!(first.domain_id(), domain(1));
    assert_eq!(first.profile_id(), profile().identity());
    assert_eq!(first.identity().physical_id, PhysicalDeviceIdV1(104));
    assert_eq!(first.identity().pci, fixture.topology.pci);
    assert_eq!(first.identity().revision_id, fixture.render.pci_revision_id);
    assert_eq!(
        first.identity().family,
        DrmFamilyObservationV1::AmdgpuFamilyAi
    );
    assert_eq!(
        first.identity().partition,
        PartitionProfileV1 {
            compute: ComputePartitionObservationV1::Spx,
            memory: MemoryPartitionObservationV1::Nps1,
        }
    );
    assert_eq!(first.kfd_gpu_id(), fixture.topology.kfd_gpu_id);
    assert_eq!(first.render_node(), fixture.render.node);
    assert_eq!(first.drm_schema_identity(), profile().drm_schema_identity());
}

#[test]
fn inventory_is_bounded_and_single_gpu_profile_rejects_missing_or_ambiguous_inputs() {
    let fixture = observations(4);
    let missing_topology = UntrustedDeviceInventoryV1::from_untrusted_observations(
        fixture.kfd,
        Vec::new(),
        vec![fixture.render],
    )
    .unwrap();
    assert_eq!(
        missing_topology.correlate_model_only(&profile()),
        Err(DeviceCorrelationErrorV1::MissingTopologyDevice)
    );
    let missing_render = UntrustedDeviceInventoryV1::from_untrusted_observations(
        fixture.kfd,
        vec![fixture.topology],
        Vec::new(),
    )
    .unwrap();
    assert_eq!(
        missing_render.correlate_model_only(&profile()),
        Err(DeviceCorrelationErrorV1::MissingRenderDevice)
    );
    let ambiguous = UntrustedDeviceInventoryV1::from_untrusted_observations(
        fixture.kfd,
        vec![fixture.topology, fixture.topology],
        vec![fixture.render, fixture.render],
    )
    .unwrap();
    assert_eq!(
        ambiguous.correlate_model_only(&profile()),
        Err(DeviceCorrelationErrorV1::AmbiguousTopology { actual: 2 })
    );

    let too_many_topology = vec![fixture.topology; MAX_TOPOLOGY_OBSERVATIONS_V1 + 1];
    assert!(matches!(
        UntrustedDeviceInventoryV1::from_untrusted_observations(
            fixture.kfd,
            too_many_topology,
            vec![fixture.render]
        ),
        Err(InventoryInputErrorV1::TooManyTopologyObservations { .. })
    ));
    let too_many_renders = vec![fixture.render; MAX_RENDER_OBSERVATIONS_V1 + 1];
    assert!(matches!(
        UntrustedDeviceInventoryV1::from_untrusted_observations(
            fixture.kfd,
            vec![fixture.topology],
            too_many_renders
        ),
        Err(InventoryInputErrorV1::TooManyRenderObservations { .. })
    ));
}

#[test]
fn kfd_domain_epoch_schema_and_node_mutations_fail_closed() {
    let mut fixture = observations(4);
    fixture.kfd.epoch = ObservationEpochV1(0);
    assert_eq!(
        fixture.inventory().correlate_model_only(&profile()),
        Err(DeviceCorrelationErrorV1::ZeroObservationEpoch)
    );

    fixture = observations(4);
    fixture.topology.domain_id = domain(7);
    assert_eq!(
        fixture.inventory().correlate_model_only(&profile()),
        Err(DeviceCorrelationErrorV1::ObservationDomainMismatch)
    );
    fixture = observations(4);
    fixture.render.epoch = ObservationEpochV1(10);
    assert_eq!(
        fixture.inventory().correlate_model_only(&profile()),
        Err(DeviceCorrelationErrorV1::ObservationEpochMismatch)
    );
    fixture = observations(4);
    fixture.kfd.node.major = 0;
    assert!(matches!(
        fixture.inventory().correlate_model_only(&profile()),
        Err(DeviceCorrelationErrorV1::KfdNodeMismatch(_))
    ));
    fixture = observations(4);
    fixture.kfd.node.minor = 1;
    assert!(matches!(
        fixture.inventory().correlate_model_only(&profile()),
        Err(DeviceCorrelationErrorV1::KfdNodeMismatch(_))
    ));
    fixture = observations(4);
    fixture.kfd.uapi_minor = 19;
    assert_eq!(
        fixture.inventory().correlate_model_only(&profile()),
        Err(DeviceCorrelationErrorV1::KfdUapiMismatch {
            major: KFD_UAPI_MAJOR_V1,
            minor: 19,
        })
    );
    fixture = observations(4);
    fixture.kfd.schema_identity = digest(99);
    assert_eq!(
        fixture.inventory().correlate_model_only(&profile()),
        Err(DeviceCorrelationErrorV1::KfdSchemaMismatch)
    );

    fixture = observations(4);
    fixture.kfd.node.major = 240;
    assert!(fixture.inventory().correlate_model_only(&profile()).is_ok());
}

#[test]
fn topology_render_identity_substitution_fails_closed() {
    let mut fixture = observations(4);
    fixture.render.pci.bus += 1;
    assert_eq!(
        fixture.inventory().correlate_model_only(&profile()),
        Err(DeviceCorrelationErrorV1::PciMismatch)
    );
    fixture = observations(4);
    fixture.render.vendor_id = 0x1234;
    assert_eq!(
        fixture.inventory().correlate_model_only(&profile()),
        Err(DeviceCorrelationErrorV1::VendorMismatch)
    );
    fixture = observations(4);
    fixture.render.device_id ^= 1;
    assert_eq!(
        fixture.inventory().correlate_model_only(&profile()),
        Err(DeviceCorrelationErrorV1::DeviceIdMismatch)
    );
    fixture = observations(4);
    fixture.topology.device_id = 0x74a0;
    fixture.render.device_id = 0x74a0;
    assert_eq!(
        fixture.inventory().correlate_model_only(&profile()),
        Err(DeviceCorrelationErrorV1::UnsupportedDeviceId)
    );
    fixture = observations(4);
    fixture.topology.drm_render_minor += 1;
    assert_eq!(
        fixture.inventory().correlate_model_only(&profile()),
        Err(DeviceCorrelationErrorV1::RenderMinorMismatch)
    );
    fixture = observations(4);
    fixture.render.gpu_unique_id += 1;
    assert_eq!(
        fixture.inventory().correlate_model_only(&profile()),
        Err(DeviceCorrelationErrorV1::GpuUniqueIdMismatch)
    );
    fixture = observations(4);
    fixture.topology.target = GpuTargetObservationV1::Other(950);
    assert_eq!(
        fixture.inventory().correlate_model_only(&profile()),
        Err(DeviceCorrelationErrorV1::UnsupportedTarget)
    );
    fixture = observations(4);
    fixture.kfd.xnack = XnackObservationV1::Enabled;
    assert_eq!(
        fixture.inventory().correlate_model_only(&profile()),
        Err(DeviceCorrelationErrorV1::UnsupportedXnack)
    );
}

#[test]
fn exact_partition_and_drm_admission_profile_mutations_fail_closed() {
    let mut fixture = observations(4);
    fixture.topology.compute_partition = ComputePartitionObservationV1::Cpx;
    assert_eq!(
        fixture.inventory().correlate_model_only(&profile()),
        Err(DeviceCorrelationErrorV1::UnsupportedPartition)
    );
    fixture = observations(4);
    fixture.topology.memory_partition = MemoryPartitionObservationV1::Nps2;
    assert_eq!(
        fixture.inventory().correlate_model_only(&profile()),
        Err(DeviceCorrelationErrorV1::UnsupportedPartition)
    );
    fixture = observations(4);
    fixture.render.drm_schema_identity = digest(99);
    assert_eq!(
        fixture.inventory().correlate_model_only(&profile()),
        Err(DeviceCorrelationErrorV1::DrmSchemaMismatch)
    );
    fixture = observations(4);
    fixture.render.driver_name = DrmDriverNameObservationV1::Other;
    assert_eq!(
        fixture.inventory().correlate_model_only(&profile()),
        Err(DeviceCorrelationErrorV1::DriverNameMismatch)
    );
    fixture = observations(4);
    fixture.render.drm_patch = 1;
    assert_eq!(
        fixture.inventory().correlate_model_only(&profile()),
        Err(DeviceCorrelationErrorV1::DrmVersionMismatch {
            major: DRM_DRIVER_MAJOR_V1,
            minor: DRM_DRIVER_MINOR_V1,
            patch: 1,
        })
    );
    fixture = observations(4);
    fixture.render.acceleration_working = false;
    assert_eq!(
        fixture.inventory().correlate_model_only(&profile()),
        Err(DeviceCorrelationErrorV1::AccelerationUnavailable)
    );
    fixture = observations(4);
    fixture.render.family = DrmFamilyObservationV1::Other(9);
    assert_eq!(
        fixture.inventory().correlate_model_only(&profile()),
        Err(DeviceCorrelationErrorV1::UnsupportedFamily)
    );
}

#[test]
fn malformed_pci_ids_nodes_and_unique_ids_fail_closed() {
    let mut fixture = observations(4);
    fixture.topology.pci.device = 32;
    fixture.render.pci.device = 32;
    assert!(matches!(
        fixture.inventory().correlate_model_only(&profile()),
        Err(DeviceCorrelationErrorV1::InvalidPciAddress(_))
    ));
    fixture = observations(4);
    fixture.topology.kfd_gpu_id = 0;
    assert_eq!(
        fixture.inventory().correlate_model_only(&profile()),
        Err(DeviceCorrelationErrorV1::InvalidKfdGpuId)
    );
    fixture = observations(4);
    fixture.topology.gpu_unique_id = 0;
    assert_eq!(
        fixture.inventory().correlate_model_only(&profile()),
        Err(DeviceCorrelationErrorV1::InvalidGpuUniqueId)
    );
    fixture = observations(4);
    fixture.render.node.minor = DRM_RENDER_MIN_MINOR_V1 - 1;
    assert!(matches!(
        fixture.inventory().correlate_model_only(&profile()),
        Err(DeviceCorrelationErrorV1::InvalidRenderNode(_))
    ));
}

#[test]
fn vm_is_bound_to_exact_active_device_generation_and_substitution_is_rejected() {
    let correlation = observations(4).correlate();
    let state = DeviceIdentityStateV1::new(domain(1));
    let (state, device) = state
        .register_device_model_only(correlation, DeviceGenerationV1(1))
        .unwrap();
    let observation = vm_observation(device, 1);
    let (state, vm) = state.register_vm_model_only(device, observation).unwrap();
    assert_eq!(vm.authority_domain(), AuthorityDomainV1::ModelOnly);
    assert_eq!(vm.model_key().device, device.model_key());
    assert_eq!(state.validate_global_invariants(), Ok(()));

    let mut stale = vm_observation(device, 2);
    stale.device.generation = DeviceGenerationV1(0);
    assert_eq!(
        state.register_vm_model_only(device, stale),
        Err(DeviceAdmissionErrorV1::VmObservationMismatch)
    );
    let mut substituted = vm_observation(device, 2);
    substituted.render_node.minor += 1;
    assert_eq!(
        state.register_vm_model_only(device, substituted),
        Err(DeviceAdmissionErrorV1::VmObservationMismatch)
    );
    let mut substituted = vm_observation(device, 2);
    substituted.kfd_gpu_id += 1;
    assert_eq!(
        state.register_vm_model_only(device, substituted),
        Err(DeviceAdmissionErrorV1::VmObservationMismatch)
    );
    let mut substituted = vm_observation(device, 2);
    substituted.pci.bus += 1;
    assert_eq!(
        state.register_vm_model_only(device, substituted),
        Err(DeviceAdmissionErrorV1::VmObservationMismatch)
    );
    assert_eq!(
        state.register_vm_model_only(device, observation),
        Err(DeviceAdmissionErrorV1::DuplicateVm(vm.model_key()))
    );
}

#[test]
fn stale_device_tokens_and_generations_cannot_cross_retirement() {
    let correlation = observations(4).correlate();
    let state = DeviceIdentityStateV1::new(domain(1));
    let (state, first) = state
        .register_device_model_only(correlation, DeviceGenerationV1(1))
        .unwrap();
    let (state, vm) = state
        .register_vm_model_only(first, vm_observation(first, 1))
        .unwrap();
    assert_eq!(
        state.retire_device_model_only(first),
        Err(DeviceAdmissionErrorV1::LiveVmPreventsDeviceRetirement(
            first.model_key()
        ))
    );
    let state = state.retire_vm_model_only(vm).unwrap();
    let state = state.retire_device_model_only(first).unwrap();
    assert_eq!(
        state.register_vm_model_only(first, vm_observation(first, 2)),
        Err(DeviceAdmissionErrorV1::DeviceNotActive(first.model_key()))
    );
    assert!(matches!(
        state.register_device_model_only(correlation, DeviceGenerationV1(1)),
        Err(DeviceAdmissionErrorV1::StaleDeviceGeneration { .. })
    ));
    let (state, second) = state
        .register_device_model_only(correlation, DeviceGenerationV1(2))
        .unwrap();
    assert_ne!(first.model_key(), second.model_key());
    assert!(matches!(
        state.register_vm_model_only(second, vm_observation(first, 3)),
        Err(DeviceAdmissionErrorV1::VmObservationMismatch)
    ));
}

#[test]
fn physical_identity_and_active_correlation_substitution_fail_closed() {
    let first_correlation = observations(4).correlate();
    let state = DeviceIdentityStateV1::new(domain(1));
    let (state, first) = state
        .register_device_model_only(first_correlation, DeviceGenerationV1(1))
        .unwrap();

    let mut collision_fixture = observations(5);
    collision_fixture.topology.pci = first_correlation.identity().pci;
    collision_fixture.render.pci = first_correlation.identity().pci;
    let collision = collision_fixture.correlate();
    assert_eq!(
        state.register_device_model_only(collision, DeviceGenerationV1(1)),
        Err(DeviceAdmissionErrorV1::ActiveCorrelationSubstitution)
    );

    let state = state.retire_device_model_only(first).unwrap();
    let mut changed_fixture = observations(4);
    changed_fixture.topology.pci.bus += 1;
    changed_fixture.render.pci.bus += 1;
    let changed = changed_fixture.correlate();
    assert_eq!(
        state.register_device_model_only(changed, DeviceGenerationV1(2)),
        Err(DeviceAdmissionErrorV1::PhysicalIdentitySubstitution(
            first.model_key().physical
        ))
    );
}

#[test]
fn admission_domains_and_failure_paths_are_isolated_and_atomic() {
    let correlation = observations(4).correlate();
    let state = DeviceIdentityStateV1::new(domain(9));
    let before = state.clone();
    assert_eq!(
        state.register_device_model_only(correlation, DeviceGenerationV1(1)),
        Err(DeviceAdmissionErrorV1::ObservationDomainMismatch)
    );
    assert_eq!(state, before);
    assert_eq!(state.authority_domain(), AuthorityDomainV1::ModelOnly);
}

#[test]
fn device_and_vm_admission_sets_enforce_versioned_bounds() {
    let mut state = DeviceIdentityStateV1::new(domain(1));
    for index in 0..MAX_MODEL_DEVICE_ADMISSIONS_V1 {
        let correlation = observations(index as u8 + 1).correlate();
        (state, _) = state
            .register_device_model_only(correlation, DeviceGenerationV1(1))
            .unwrap();
    }
    let overflow = observations(MAX_MODEL_DEVICE_ADMISSIONS_V1 as u8 + 1).correlate();
    assert_eq!(
        state.register_device_model_only(overflow, DeviceGenerationV1(1)),
        Err(DeviceAdmissionErrorV1::CapacityExceeded {
            kind: AdmissionRecordKindV1::Device,
            maximum: MAX_MODEL_DEVICE_ADMISSIONS_V1,
        })
    );

    let correlation = observations(90).correlate();
    let state = DeviceIdentityStateV1::new(domain(1));
    let (mut state, device) = state
        .register_device_model_only(correlation, DeviceGenerationV1(1))
        .unwrap();
    for vm_id in 1..=MAX_MODEL_VM_ADMISSIONS_V1 as u64 {
        (state, _) = state
            .register_vm_model_only(device, vm_observation(device, vm_id))
            .unwrap();
    }
    assert_eq!(
        state.register_vm_model_only(
            device,
            vm_observation(device, MAX_MODEL_VM_ADMISSIONS_V1 as u64 + 1)
        ),
        Err(DeviceAdmissionErrorV1::CapacityExceeded {
            kind: AdmissionRecordKindV1::Vm,
            maximum: MAX_MODEL_VM_ADMISSIONS_V1,
        })
    );
}
