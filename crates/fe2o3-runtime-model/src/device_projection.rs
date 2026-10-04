//! Canonical, solver-neutral projection of checked device-admission facts.

use alloc::vec::Vec;

use crate::{
    AMD_PCI_VENDOR_ID_V1, AuthorityDomainV1, ComputePartitionObservationV1, DRM_DEVICE_MAJOR_V1,
    DRM_DRIVER_MAJOR_V1, DRM_DRIVER_MINOR_V1, DRM_DRIVER_PATCH_V1, DeviceAdmissionProfileIdV1,
    DeviceAdmissionProfileV1, DeviceGenerationV1, DeviceKeyV1, DeviceNodeV1,
    DeviceObservationDomainIdV1, DrmDriverNameObservationV1, DrmFamilyObservationV1,
    GpuTargetObservationV1, IdentityDigestV1, KFD_DEVICE_MINOR_V1, KFD_UAPI_MAJOR_V1,
    KFD_UAPI_MINOR_V1, MAX_MODEL_DEVICE_ADMISSIONS_V1, MI300X_PCI_DEVICE_ID_V1,
    MemoryPartitionObservationV1, ModelCorrelatedDeviceV1, ObservationEpochV1, PciAddressV1,
    UntrustedDeviceInventoryV1, UntrustedKfdObservationV1, UntrustedRenderObservationV1,
    UntrustedTopologyObservationV1, XnackObservationV1, correlate_model_only_v1,
};

pub const DEVICE_PROJECTION_SCHEMA_VERSION_V1: u16 = 1;
pub const MAX_PROJECTED_APERTURES_V1: usize = 16;
pub const MI300X_PCI_REVISION_V1: u8 = 0;
pub const MI300X_WAVEFRONT_SIZE_V1: u32 = 64;
pub const MI300X_SPX_SIMD_COUNT_V1: u32 = 1216;
pub const MI300X_SPX_XCC_COUNT_V1: u32 = 8;
pub const MI300X_KFD_FIRMWARE_VERSION_V1: u32 = 192;
pub const MI300X_SDMA_FIRMWARE_VERSION_V1: u32 = 25;
pub const AMDGPU_FAMILY_AI_V1: u32 = 141;
pub const MI300X_CHIP_REVISION_V1: u32 = 1;
pub const MI300X_EXTERNAL_REVISION_V1: u32 = 71;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KernelReleaseObservationV1 {
    Linux6_8_0_124Generic,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AmdgpuModuleObservationV1 {
    Version6_16_13SourceA6f143bec60c0afc3263226,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeviceProjectionSourceV1 {
    pub boot_id: [u8; 16],
    pub topology_file_system_device: u64,
    pub topology_inode: u64,
    pub topology_generation: u64,
    pub process_id: u32,
    pub process_start_time_ticks: u64,
    pub mount_namespace_device: u64,
    pub mount_namespace_inode: u64,
    pub amdgpu_module_file_system_device: u64,
    pub amdgpu_module_inode: u64,
    pub kernel_release: KernelReleaseObservationV1,
    pub amdgpu_module: AmdgpuModuleObservationV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CharacterDeviceProjectionV1 {
    pub file_system_device: u64,
    pub inode: u64,
    pub character_device: u64,
    pub node: DeviceNodeV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KfdProjectionV1 {
    pub descriptor: CharacterDeviceProjectionV1,
    pub uapi_major: u32,
    pub uapi_minor: u32,
    pub schema_identity: IdentityDigestV1,
    pub xnack: XnackObservationV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TopologyProjectionV1 {
    pub node_id: u32,
    pub kfd_gpu_id: u32,
    pub gpu_unique_id: u64,
    pub drm_render_minor: u32,
    pub pci: PciAddressV1,
    pub vendor_id: u16,
    pub device_id: u16,
    pub target: GpuTargetObservationV1,
    pub compute_partition: ComputePartitionObservationV1,
    pub memory_partition: MemoryPartitionObservationV1,
    pub firmware_version: u32,
    pub sdma_firmware_version: u32,
    pub wavefront_size: u32,
    pub simd_count: u32,
    pub xcc_count: u32,
}

/// Canonical identity of every GPU in the bounded topology transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InventoryDeviceProjectionV1 {
    pub topology_node_id: u32,
    pub kfd_gpu_id: u32,
    pub gpu_unique_id: u64,
    pub drm_render_minor: u32,
    pub pci: PciAddressV1,
    pub vendor_id: u16,
    pub device_id: u16,
    pub pci_revision_id: u8,
    pub target: GpuTargetObservationV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RenderProjectionV1 {
    pub descriptor: CharacterDeviceProjectionV1,
    pub gpu_unique_id: u64,
    pub pci: PciAddressV1,
    pub vendor_id: u16,
    pub device_id: u16,
    pub pci_revision_id: u8,
    pub schema_identity: IdentityDigestV1,
    pub driver_name: DrmDriverNameObservationV1,
    pub driver_major: u32,
    pub driver_minor: u32,
    pub driver_patch: u32,
    pub acceleration_working: bool,
    pub family: DrmFamilyObservationV1,
    pub family_id: u32,
    pub chip_revision: u32,
    pub external_revision: u32,
    /// Initial wrapping DRM VRAM-loss observation. This is not an all-reset generation.
    pub vram_lost_counter: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InclusiveRangeProjectionV1 {
    pub base: u64,
    pub limit: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProcessApertureProjectionV1 {
    pub kfd_gpu_id: u32,
    pub lds: InclusiveRangeProjectionV1,
    pub scratch: InclusiveRangeProjectionV1,
    pub gpuvm: InclusiveRangeProjectionV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeviceProjectionCommitFenceV1 {
    pub process_reobserved_equal: bool,
    pub descriptors_revalidated: bool,
    pub topology_reobserved_equal: bool,
    pub xnack_reobserved_disabled: bool,
    pub apertures_reobserved_equal: bool,
    pub reset_subscription_established: bool,
    pub reset_event_mask_enabled: bool,
    pub reset_event_descriptor_cloexec: bool,
    pub reset_fence_initially_clear: bool,
    pub drm_reobserved_after_subscription_equal: bool,
    pub reset_fence_clear_before_commit: bool,
}

/// Canonical value assembled by an adapter after its concrete checks.
///
/// This record is freely constructible and grants no authority. Its single
/// `topology_generation` is a transaction label, not an independently observed
/// KFD or DRM generation and not evidence of a GPU reset.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceProjectionRecordV1 {
    pub schema_version: u16,
    pub domain_id: DeviceObservationDomainIdV1,
    pub profile_id: DeviceAdmissionProfileIdV1,
    pub source: DeviceProjectionSourceV1,
    pub kfd: KfdProjectionV1,
    pub topology: TopologyProjectionV1,
    pub inventory: Vec<InventoryDeviceProjectionV1>,
    pub render: RenderProjectionV1,
    pub apertures: Vec<ProcessApertureProjectionV1>,
    pub commit_fence: DeviceProjectionCommitFenceV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceProjectionErrorV1 {
    SchemaVersionMismatch,
    ProfileMismatch,
    SourceIdentityInvalid,
    UnsupportedPlatform,
    KfdDescriptorInvalid,
    KfdProfileMismatch,
    TopologyProfileMismatch,
    InvalidInventory,
    SelectedInventoryMismatch,
    RenderDescriptorInvalid,
    RenderProfileMismatch,
    CrossSourceIdentityMismatch,
    CommitFenceIncomplete,
    InvalidApertureCount,
    InvalidAperture(u32),
    ApertureOrderMismatch,
    SelectedApertureMissing,
    Inventory(crate::InventoryInputErrorV1),
    Correlation(crate::DeviceCorrelationErrorV1),
}

/// Validated pure projection. It remains model-only and is not syscall evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedDeviceProjectionV1 {
    record: DeviceProjectionRecordV1,
    correlation: ModelCorrelatedDeviceV1,
}

impl ValidatedDeviceProjectionV1 {
    pub const fn authority_domain(&self) -> AuthorityDomainV1 {
        AuthorityDomainV1::ModelOnly
    }

    pub const fn record(&self) -> &DeviceProjectionRecordV1 {
        &self.record
    }

    pub const fn correlation(&self) -> ModelCorrelatedDeviceV1 {
        self.correlation
    }
}

fn range_valid(range: InclusiveRangeProjectionV1) -> bool {
    range.base <= range.limit
        && range.base.is_multiple_of(4096)
        && range
            .limit
            .checked_add(1)
            .is_some_and(|end| end.is_multiple_of(4096))
}

fn ranges_overlap(left: InclusiveRangeProjectionV1, right: InclusiveRangeProjectionV1) -> bool {
    left.base <= right.limit && right.base <= left.limit
}

pub fn validate_device_projection_model_only_v1(
    record: DeviceProjectionRecordV1,
    profile: &DeviceAdmissionProfileV1,
) -> Result<ValidatedDeviceProjectionV1, DeviceProjectionErrorV1> {
    if record.schema_version != DEVICE_PROJECTION_SCHEMA_VERSION_V1 {
        return Err(DeviceProjectionErrorV1::SchemaVersionMismatch);
    }
    if record.profile_id != profile.identity()
        || record.kfd.schema_identity != profile.kfd_schema_identity()
        || record.render.schema_identity != profile.drm_schema_identity()
    {
        return Err(DeviceProjectionErrorV1::ProfileMismatch);
    }
    let source = record.source;
    if source.boot_id == [0; 16]
        || source.topology_file_system_device == 0
        || source.topology_inode == 0
        || source.topology_generation == 0
        || source.process_id == 0
        || source.process_start_time_ticks == 0
        || source.mount_namespace_inode == 0
        || source.amdgpu_module_file_system_device == 0
        || source.amdgpu_module_inode == 0
    {
        return Err(DeviceProjectionErrorV1::SourceIdentityInvalid);
    }
    if source.kernel_release != KernelReleaseObservationV1::Linux6_8_0_124Generic
        || source.amdgpu_module
            != AmdgpuModuleObservationV1::Version6_16_13SourceA6f143bec60c0afc3263226
    {
        return Err(DeviceProjectionErrorV1::UnsupportedPlatform);
    }
    let kfd = record.kfd;
    if kfd.descriptor.inode == 0
        || kfd.descriptor.character_device == 0
        || kfd.descriptor.node.major == 0
        || kfd.descriptor.node.minor != KFD_DEVICE_MINOR_V1
    {
        return Err(DeviceProjectionErrorV1::KfdDescriptorInvalid);
    }
    if kfd.uapi_major != KFD_UAPI_MAJOR_V1
        || kfd.uapi_minor != KFD_UAPI_MINOR_V1
        || kfd.xnack != XnackObservationV1::Disabled
    {
        return Err(DeviceProjectionErrorV1::KfdProfileMismatch);
    }
    let topology = record.topology;
    if topology.kfd_gpu_id == 0
        || topology.gpu_unique_id == 0
        || !topology.pci.is_well_formed()
        || topology.vendor_id != AMD_PCI_VENDOR_ID_V1
        || topology.device_id != MI300X_PCI_DEVICE_ID_V1
        || topology.target != GpuTargetObservationV1::Gfx942
        || topology.compute_partition != ComputePartitionObservationV1::Spx
        || topology.memory_partition != MemoryPartitionObservationV1::Nps1
        || topology.firmware_version != MI300X_KFD_FIRMWARE_VERSION_V1
        || topology.sdma_firmware_version != MI300X_SDMA_FIRMWARE_VERSION_V1
        || topology.wavefront_size != MI300X_WAVEFRONT_SIZE_V1
        || topology.simd_count != MI300X_SPX_SIMD_COUNT_V1
        || topology.xcc_count != MI300X_SPX_XCC_COUNT_V1
    {
        return Err(DeviceProjectionErrorV1::TopologyProfileMismatch);
    }
    if record.inventory.is_empty() || record.inventory.len() > crate::MAX_TOPOLOGY_OBSERVATIONS_V1 {
        return Err(DeviceProjectionErrorV1::InvalidInventory);
    }
    let mut selected_matches = 0;
    for (index, device) in record.inventory.iter().enumerate() {
        if device.kfd_gpu_id == 0
            || device.gpu_unique_id == 0
            || !device.pci.is_well_formed()
            || device.vendor_id != AMD_PCI_VENDOR_ID_V1
            || device.device_id == 0
            || device.drm_render_minor < crate::DRM_RENDER_MIN_MINOR_V1
            || device.target != GpuTargetObservationV1::Gfx942
        {
            return Err(DeviceProjectionErrorV1::InvalidInventory);
        }
        if index > 0 && record.inventory[index - 1].topology_node_id >= device.topology_node_id {
            return Err(DeviceProjectionErrorV1::InvalidInventory);
        }
        for old in &record.inventory[..index] {
            if old.topology_node_id == device.topology_node_id
                || old.kfd_gpu_id == device.kfd_gpu_id
                || old.gpu_unique_id == device.gpu_unique_id
                || old.drm_render_minor == device.drm_render_minor
                || old.pci == device.pci
            {
                return Err(DeviceProjectionErrorV1::InvalidInventory);
            }
        }
        if device.gpu_unique_id == topology.gpu_unique_id {
            selected_matches += 1;
            if device.topology_node_id != topology.node_id
                || device.kfd_gpu_id != topology.kfd_gpu_id
                || device.drm_render_minor != topology.drm_render_minor
                || device.pci != topology.pci
                || device.vendor_id != topology.vendor_id
                || device.device_id != topology.device_id
                || device.pci_revision_id != record.render.pci_revision_id
                || device.target != topology.target
            {
                return Err(DeviceProjectionErrorV1::SelectedInventoryMismatch);
            }
        }
    }
    if selected_matches != 1 {
        return Err(DeviceProjectionErrorV1::SelectedInventoryMismatch);
    }
    let render = record.render;
    if render.descriptor.inode == 0
        || render.descriptor.character_device == 0
        || render.descriptor.node.major != DRM_DEVICE_MAJOR_V1
        || render.descriptor.node.minor < crate::DRM_RENDER_MIN_MINOR_V1
    {
        return Err(DeviceProjectionErrorV1::RenderDescriptorInvalid);
    }
    if !render.pci.is_well_formed()
        || render.vendor_id != AMD_PCI_VENDOR_ID_V1
        || render.device_id != MI300X_PCI_DEVICE_ID_V1
        || render.pci_revision_id != MI300X_PCI_REVISION_V1
        || render.driver_name != DrmDriverNameObservationV1::Amdgpu
        || render.driver_major != DRM_DRIVER_MAJOR_V1
        || render.driver_minor != DRM_DRIVER_MINOR_V1
        || render.driver_patch != DRM_DRIVER_PATCH_V1
        || !render.acceleration_working
        || render.family != DrmFamilyObservationV1::AmdgpuFamilyAi
        || render.family_id != AMDGPU_FAMILY_AI_V1
        || render.chip_revision != MI300X_CHIP_REVISION_V1
        || render.external_revision != MI300X_EXTERNAL_REVISION_V1
    {
        return Err(DeviceProjectionErrorV1::RenderProfileMismatch);
    }
    if topology.gpu_unique_id != render.gpu_unique_id
        || topology.drm_render_minor != render.descriptor.node.minor
        || topology.pci != render.pci
        || topology.vendor_id != render.vendor_id
        || topology.device_id != render.device_id
    {
        return Err(DeviceProjectionErrorV1::CrossSourceIdentityMismatch);
    }
    let fence = record.commit_fence;
    if !fence.process_reobserved_equal
        || !fence.descriptors_revalidated
        || !fence.topology_reobserved_equal
        || !fence.xnack_reobserved_disabled
        || !fence.apertures_reobserved_equal
        || !fence.reset_subscription_established
        || !fence.reset_event_mask_enabled
        || !fence.reset_event_descriptor_cloexec
        || !fence.reset_fence_initially_clear
        || !fence.drm_reobserved_after_subscription_equal
        || !fence.reset_fence_clear_before_commit
    {
        return Err(DeviceProjectionErrorV1::CommitFenceIncomplete);
    }
    if record.apertures.is_empty() || record.apertures.len() > MAX_PROJECTED_APERTURES_V1 {
        return Err(DeviceProjectionErrorV1::InvalidApertureCount);
    }
    let mut selected = false;
    let mut previous_gpu_id = None;
    for aperture in &record.apertures {
        if aperture.kfd_gpu_id == 0
            || !range_valid(aperture.lds)
            || !range_valid(aperture.scratch)
            || !range_valid(aperture.gpuvm)
            || ranges_overlap(aperture.lds, aperture.scratch)
            || ranges_overlap(aperture.lds, aperture.gpuvm)
            || ranges_overlap(aperture.scratch, aperture.gpuvm)
        {
            return Err(DeviceProjectionErrorV1::InvalidAperture(
                aperture.kfd_gpu_id,
            ));
        }
        if previous_gpu_id.is_some_and(|previous| previous >= aperture.kfd_gpu_id) {
            return Err(DeviceProjectionErrorV1::ApertureOrderMismatch);
        }
        previous_gpu_id = Some(aperture.kfd_gpu_id);
        selected |= aperture.kfd_gpu_id == topology.kfd_gpu_id;
    }
    if !selected {
        return Err(DeviceProjectionErrorV1::SelectedApertureMissing);
    }
    if record.apertures.len() != record.inventory.len()
        || record.inventory.iter().any(|device| {
            !record
                .apertures
                .iter()
                .any(|aperture| aperture.kfd_gpu_id == device.kfd_gpu_id)
        })
    {
        return Err(DeviceProjectionErrorV1::InvalidInventory);
    }

    let epoch = ObservationEpochV1(source.topology_generation);
    let inventory = UntrustedDeviceInventoryV1::from_untrusted_observations(
        UntrustedKfdObservationV1 {
            domain_id: record.domain_id,
            epoch,
            node: kfd.descriptor.node,
            uapi_major: kfd.uapi_major,
            uapi_minor: kfd.uapi_minor,
            schema_identity: kfd.schema_identity,
            xnack: kfd.xnack,
        },
        alloc::vec![UntrustedTopologyObservationV1 {
            domain_id: record.domain_id,
            epoch,
            topology_node_id: topology.node_id,
            kfd_gpu_id: topology.kfd_gpu_id,
            gpu_unique_id: topology.gpu_unique_id,
            drm_render_minor: topology.drm_render_minor,
            pci: topology.pci,
            vendor_id: topology.vendor_id,
            device_id: topology.device_id,
            target: topology.target,
            compute_partition: topology.compute_partition,
            memory_partition: topology.memory_partition,
        }],
        alloc::vec![UntrustedRenderObservationV1 {
            domain_id: record.domain_id,
            epoch,
            node: render.descriptor.node,
            gpu_unique_id: render.gpu_unique_id,
            pci: render.pci,
            vendor_id: render.vendor_id,
            device_id: render.device_id,
            pci_revision_id: render.pci_revision_id,
            drm_schema_identity: render.schema_identity,
            driver_name: render.driver_name,
            drm_major: render.driver_major,
            drm_minor: render.driver_minor,
            drm_patch: render.driver_patch,
            acceleration_working: render.acceleration_working,
            family: render.family,
        }],
    )
    .map_err(DeviceProjectionErrorV1::Inventory)?;
    let correlation = correlate_model_only_v1(profile, &inventory)
        .map_err(DeviceProjectionErrorV1::Correlation)?;
    Ok(ValidatedDeviceProjectionV1 {
        record,
        correlation,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceProjectionHistoryEntryV1 {
    pub key: DeviceKeyV1,
    pub predecessor: Option<DeviceKeyV1>,
    pub projection: ValidatedDeviceProjectionV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceProjectionHistoryLinkV1 {
    current: DeviceKeyV1,
    predecessor: Option<DeviceKeyV1>,
}

impl DeviceProjectionHistoryLinkV1 {
    pub const fn current(&self) -> DeviceKeyV1 {
        self.current
    }

    pub const fn predecessor(&self) -> Option<DeviceKeyV1> {
        self.predecessor
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceProjectionHistoryV1 {
    domain_id: DeviceObservationDomainIdV1,
    entries: Vec<DeviceProjectionHistoryEntryV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceProjectionHistoryErrorV1 {
    CapacityExceeded,
    DomainMismatch,
    ZeroGeneration,
    StaleGeneration,
    PhysicalIdentitySubstitution,
    CorrelationSubstitution,
    InvariantViolation,
}

impl DeviceProjectionHistoryV1 {
    pub const fn new(domain_id: DeviceObservationDomainIdV1) -> Self {
        Self {
            domain_id,
            entries: Vec::new(),
        }
    }

    pub const fn domain_id(&self) -> DeviceObservationDomainIdV1 {
        self.domain_id
    }

    pub fn entries(&self) -> &[DeviceProjectionHistoryEntryV1] {
        &self.entries
    }

    pub fn append_model_only(
        &self,
        projection: ValidatedDeviceProjectionV1,
        generation: DeviceGenerationV1,
    ) -> Result<(Self, DeviceProjectionHistoryLinkV1), DeviceProjectionHistoryErrorV1> {
        self.validate_global_invariants()?;
        if self.entries.len() >= MAX_MODEL_DEVICE_ADMISSIONS_V1 {
            return Err(DeviceProjectionHistoryErrorV1::CapacityExceeded);
        }
        if projection.record.domain_id != self.domain_id {
            return Err(DeviceProjectionHistoryErrorV1::DomainMismatch);
        }
        if generation.0 == 0 {
            return Err(DeviceProjectionHistoryErrorV1::ZeroGeneration);
        }
        let physical = projection.correlation.identity().physical_id;
        let current = DeviceKeyV1 {
            physical,
            generation,
        };
        let mut predecessor = None;
        for entry in &self.entries {
            let old = entry.projection.correlation;
            if entry.key.physical == physical {
                if old.identity() != projection.correlation.identity() {
                    return Err(DeviceProjectionHistoryErrorV1::PhysicalIdentitySubstitution);
                }
                if generation <= entry.key.generation {
                    return Err(DeviceProjectionHistoryErrorV1::StaleGeneration);
                }
                predecessor = Some(entry.key);
            } else if old.identity().pci == projection.correlation.identity().pci
                || old.render_node() == projection.correlation.render_node()
                || old.kfd_gpu_id() == projection.correlation.kfd_gpu_id()
            {
                return Err(DeviceProjectionHistoryErrorV1::CorrelationSubstitution);
            }
        }
        let mut next = self.clone();
        next.entries.push(DeviceProjectionHistoryEntryV1 {
            key: current,
            predecessor,
            projection,
        });
        next.validate_global_invariants()?;
        Ok((
            next,
            DeviceProjectionHistoryLinkV1 {
                current,
                predecessor,
            },
        ))
    }

    pub fn validate_global_invariants(&self) -> Result<(), DeviceProjectionHistoryErrorV1> {
        if self.entries.len() > MAX_MODEL_DEVICE_ADMISSIONS_V1 {
            return Err(DeviceProjectionHistoryErrorV1::InvariantViolation);
        }
        for (index, entry) in self.entries.iter().enumerate() {
            if entry.projection.record.domain_id != self.domain_id
                || entry.key.generation.0 == 0
                || entry.key.physical != entry.projection.correlation.identity().physical_id
            {
                return Err(DeviceProjectionHistoryErrorV1::InvariantViolation);
            }
            let previous = self.entries[..index]
                .iter()
                .rev()
                .find(|old| old.key.physical == entry.key.physical);
            if entry.predecessor != previous.map(|old| old.key) {
                return Err(DeviceProjectionHistoryErrorV1::InvariantViolation);
            }
            for old in &self.entries[..index] {
                let old_correlation = old.projection.correlation;
                let correlation = entry.projection.correlation;
                if old.key.physical == entry.key.physical {
                    if old.key.generation >= entry.key.generation
                        || old_correlation.identity() != correlation.identity()
                    {
                        return Err(DeviceProjectionHistoryErrorV1::InvariantViolation);
                    }
                } else if old_correlation.identity().pci == correlation.identity().pci
                    || old_correlation.render_node() == correlation.render_node()
                    || old_correlation.kfd_gpu_id() == correlation.kfd_gpu_id()
                {
                    return Err(DeviceProjectionHistoryErrorV1::InvariantViolation);
                }
            }
        }
        Ok(())
    }
}
