//! Model-only correlation and generation admission for untrusted device observations.

use alloc::vec::Vec;

use crate::{
    DeviceAdmissionProfileIdV1, DeviceGenerationV1, DeviceKeyV1, DeviceObservationDomainIdV1,
    IdentityDigestV1, PhysicalDeviceIdV1, VmIdV1, VmKeyV1,
};

pub const DEVICE_IDENTITY_SCHEMA_VERSION_V1: u16 = 1;
pub const KFD_DEVICE_MINOR_V1: u32 = 0;
pub const DRM_DEVICE_MAJOR_V1: u32 = 226;
pub const DRM_RENDER_MIN_MINOR_V1: u32 = 128;
pub const AMD_PCI_VENDOR_ID_V1: u16 = 0x1002;
pub const MI300X_PCI_DEVICE_ID_V1: u16 = 0x74a1;
pub const KFD_UAPI_MAJOR_V1: u32 = 1;
pub const KFD_UAPI_MINOR_V1: u32 = 18;
pub const DRM_DRIVER_MAJOR_V1: u32 = 3;
pub const DRM_DRIVER_MINOR_V1: u32 = 64;
pub const DRM_DRIVER_PATCH_V1: u32 = 0;
pub const MAX_TOPOLOGY_OBSERVATIONS_V1: usize = 16;
pub const MAX_RENDER_OBSERVATIONS_V1: usize = 16;
pub const MAX_MODEL_DEVICE_ADMISSIONS_V1: usize = 64;
pub const MAX_MODEL_VM_ADMISSIONS_V1: usize = 256;

/// Marks facts that are useful only inside the executable model.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthorityDomainV1 {
    ModelOnly,
}

mod authority_seal {
    pub trait Sealed {}
}

/// Reserved production-authority boundary.
///
/// This release intentionally has no implementation. A future sealed adapter
/// must supply concrete observation/refinement evidence; model admissions do
/// not implement this trait and cannot be promoted into runtime authority.
pub trait ProductionDeviceAuthorityV1: authority_seal::Sealed {}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct ObservationEpochV1(pub u64);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DeviceNodeV1 {
    pub major: u32,
    pub minor: u32,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PciAddressV1 {
    pub domain: u16,
    pub bus: u8,
    pub device: u8,
    pub function: u8,
}

impl PciAddressV1 {
    pub const fn is_well_formed(self) -> bool {
        self.device < 32 && self.function < 8
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GpuTargetObservationV1 {
    Gfx942,
    Other(u32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XnackObservationV1 {
    Disabled,
    Enabled,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComputePartitionObservationV1 {
    Spx,
    Cpx,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryPartitionObservationV1 {
    Nps1,
    Nps2,
    Nps4,
    Nps8,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PartitionProfileV1 {
    pub compute: ComputePartitionObservationV1,
    pub memory: MemoryPartitionObservationV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DrmDriverNameObservationV1 {
    Amdgpu,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DrmFamilyObservationV1 {
    AmdgpuFamilyAi,
    Other(u32),
}

/// Untrusted observation of the process-wide KFD node and ABI response.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UntrustedKfdObservationV1 {
    pub domain_id: DeviceObservationDomainIdV1,
    pub epoch: ObservationEpochV1,
    pub node: DeviceNodeV1,
    pub uapi_major: u32,
    pub uapi_minor: u32,
    pub schema_identity: IdentityDigestV1,
    pub xnack: XnackObservationV1,
}

/// Untrusted projection of one KFD topology GPU node.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UntrustedTopologyObservationV1 {
    pub domain_id: DeviceObservationDomainIdV1,
    pub epoch: ObservationEpochV1,
    pub topology_node_id: u32,
    pub kfd_gpu_id: u32,
    pub gpu_unique_id: u64,
    pub drm_render_minor: u32,
    pub pci: PciAddressV1,
    pub vendor_id: u16,
    pub device_id: u16,
    pub target: GpuTargetObservationV1,
    pub compute_partition: ComputePartitionObservationV1,
    pub memory_partition: MemoryPartitionObservationV1,
}

/// Untrusted projection of one DRM render node and its correlated PCI device.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UntrustedRenderObservationV1 {
    pub domain_id: DeviceObservationDomainIdV1,
    pub epoch: ObservationEpochV1,
    pub node: DeviceNodeV1,
    pub gpu_unique_id: u64,
    pub pci: PciAddressV1,
    pub vendor_id: u16,
    pub device_id: u16,
    pub pci_revision_id: u8,
    pub drm_schema_identity: IdentityDigestV1,
    pub driver_name: DrmDriverNameObservationV1,
    pub drm_major: u32,
    pub drm_minor: u32,
    pub drm_patch: u32,
    pub acceleration_working: bool,
    pub family: DrmFamilyObservationV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InventoryInputErrorV1 {
    TooManyTopologyObservations { actual: usize, maximum: usize },
    TooManyRenderObservations { actual: usize, maximum: usize },
}

/// Bounded collection of observations. Construction performs no admission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UntrustedDeviceInventoryV1 {
    kfd: UntrustedKfdObservationV1,
    topology: Vec<UntrustedTopologyObservationV1>,
    renders: Vec<UntrustedRenderObservationV1>,
}

impl UntrustedDeviceInventoryV1 {
    pub fn from_untrusted_observations(
        kfd: UntrustedKfdObservationV1,
        topology: Vec<UntrustedTopologyObservationV1>,
        renders: Vec<UntrustedRenderObservationV1>,
    ) -> Result<Self, InventoryInputErrorV1> {
        if topology.len() > MAX_TOPOLOGY_OBSERVATIONS_V1 {
            return Err(InventoryInputErrorV1::TooManyTopologyObservations {
                actual: topology.len(),
                maximum: MAX_TOPOLOGY_OBSERVATIONS_V1,
            });
        }
        if renders.len() > MAX_RENDER_OBSERVATIONS_V1 {
            return Err(InventoryInputErrorV1::TooManyRenderObservations {
                actual: renders.len(),
                maximum: MAX_RENDER_OBSERVATIONS_V1,
            });
        }
        Ok(Self {
            kfd,
            topology,
            renders,
        })
    }

    pub const fn kfd(&self) -> UntrustedKfdObservationV1 {
        self.kfd
    }

    pub fn topology(&self) -> &[UntrustedTopologyObservationV1] {
        &self.topology
    }

    pub fn renders(&self) -> &[UntrustedRenderObservationV1] {
        &self.renders
    }

    /// Correlates the exact single-GPU V1 profile without granting authority.
    pub fn correlate_model_only(
        &self,
        profile: &DeviceAdmissionProfileV1,
    ) -> Result<ModelCorrelatedDeviceV1, DeviceCorrelationErrorV1> {
        correlate_model_only_v1(profile, self)
    }
}

/// Reviewed model profile. Its identity and UAPI schema commitments remain
/// inputs until a future evidence layer authenticates them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeviceAdmissionProfileV1 {
    identity: DeviceAdmissionProfileIdV1,
    kfd_schema_identity: IdentityDigestV1,
    drm_schema_identity: IdentityDigestV1,
}

impl DeviceAdmissionProfileV1 {
    pub const fn gfx942_xnack_minus_spx_nps1_kfd_1_18_drm_3_64_0(
        identity: DeviceAdmissionProfileIdV1,
        kfd_schema_identity: IdentityDigestV1,
        drm_schema_identity: IdentityDigestV1,
    ) -> Self {
        Self {
            identity,
            kfd_schema_identity,
            drm_schema_identity,
        }
    }

    pub const fn identity(self) -> DeviceAdmissionProfileIdV1 {
        self.identity
    }

    pub const fn kfd_schema_identity(self) -> IdentityDigestV1 {
        self.kfd_schema_identity
    }

    pub const fn drm_schema_identity(self) -> IdentityDigestV1 {
        self.drm_schema_identity
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceCorrelationErrorV1 {
    MissingTopologyDevice,
    AmbiguousTopology { actual: usize },
    MissingRenderDevice,
    AmbiguousRender { actual: usize },
    ZeroObservationEpoch,
    ObservationDomainMismatch,
    ObservationEpochMismatch,
    KfdNodeMismatch(DeviceNodeV1),
    KfdUapiMismatch { major: u32, minor: u32 },
    KfdSchemaMismatch,
    DrmSchemaMismatch,
    InvalidPciAddress(PciAddressV1),
    InvalidKfdGpuId,
    InvalidGpuUniqueId,
    InvalidRenderNode(DeviceNodeV1),
    RenderMinorMismatch,
    GpuUniqueIdMismatch,
    VendorMismatch,
    PciMismatch,
    DeviceIdMismatch,
    UnsupportedDeviceId,
    UnsupportedTarget,
    UnsupportedXnack,
    UnsupportedPartition,
    DriverNameMismatch,
    DrmVersionMismatch { major: u32, minor: u32, patch: u32 },
    AccelerationUnavailable,
    UnsupportedFamily,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ModelPhysicalDeviceIdentityV1 {
    pub physical_id: PhysicalDeviceIdV1,
    pub gpu_unique_id: u64,
    pub pci: PciAddressV1,
    pub vendor_id: u16,
    pub device_id: u16,
    pub revision_id: u8,
    pub family: DrmFamilyObservationV1,
    pub partition: PartitionProfileV1,
}

/// Deterministic correlation receipt over explicitly untrusted observations.
///
/// Private fields prevent unchecked construction, but the receipt remains
/// model-only because the observations and profile were not authenticated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ModelCorrelatedDeviceV1 {
    domain_id: DeviceObservationDomainIdV1,
    profile_id: DeviceAdmissionProfileIdV1,
    epoch: ObservationEpochV1,
    identity: ModelPhysicalDeviceIdentityV1,
    topology_node_id: u32,
    kfd_gpu_id: u32,
    render_node: DeviceNodeV1,
    drm_schema_identity: IdentityDigestV1,
}

impl ModelCorrelatedDeviceV1 {
    pub const fn authority_domain(self) -> AuthorityDomainV1 {
        AuthorityDomainV1::ModelOnly
    }

    pub const fn domain_id(self) -> DeviceObservationDomainIdV1 {
        self.domain_id
    }

    pub const fn profile_id(self) -> DeviceAdmissionProfileIdV1 {
        self.profile_id
    }

    pub const fn epoch(self) -> ObservationEpochV1 {
        self.epoch
    }

    pub const fn identity(self) -> ModelPhysicalDeviceIdentityV1 {
        self.identity
    }

    pub const fn topology_node_id(self) -> u32 {
        self.topology_node_id
    }

    pub const fn kfd_gpu_id(self) -> u32 {
        self.kfd_gpu_id
    }

    pub const fn render_node(self) -> DeviceNodeV1 {
        self.render_node
    }

    pub const fn drm_schema_identity(self) -> IdentityDigestV1 {
        self.drm_schema_identity
    }
}

pub fn correlate_model_only_v1(
    profile: &DeviceAdmissionProfileV1,
    inventory: &UntrustedDeviceInventoryV1,
) -> Result<ModelCorrelatedDeviceV1, DeviceCorrelationErrorV1> {
    let topology = match inventory.topology.as_slice() {
        [] => return Err(DeviceCorrelationErrorV1::MissingTopologyDevice),
        [device] => *device,
        devices => {
            return Err(DeviceCorrelationErrorV1::AmbiguousTopology {
                actual: devices.len(),
            });
        }
    };
    let render = match inventory.renders.as_slice() {
        [] => return Err(DeviceCorrelationErrorV1::MissingRenderDevice),
        [device] => *device,
        devices => {
            return Err(DeviceCorrelationErrorV1::AmbiguousRender {
                actual: devices.len(),
            });
        }
    };
    let kfd = inventory.kfd;

    if kfd.epoch.0 == 0 {
        return Err(DeviceCorrelationErrorV1::ZeroObservationEpoch);
    }
    if topology.domain_id != kfd.domain_id || render.domain_id != kfd.domain_id {
        return Err(DeviceCorrelationErrorV1::ObservationDomainMismatch);
    }
    if topology.epoch != kfd.epoch || render.epoch != kfd.epoch {
        return Err(DeviceCorrelationErrorV1::ObservationEpochMismatch);
    }
    // Linux dynamically allocates the KFD character-device major. This model
    // checks only its shape; a sealed adapter must bind the node to its opened
    // file descriptor and the corresponding sysfs device.
    if kfd.node.major == 0 || kfd.node.minor != KFD_DEVICE_MINOR_V1 {
        return Err(DeviceCorrelationErrorV1::KfdNodeMismatch(kfd.node));
    }
    if kfd.uapi_major != KFD_UAPI_MAJOR_V1 || kfd.uapi_minor != KFD_UAPI_MINOR_V1 {
        return Err(DeviceCorrelationErrorV1::KfdUapiMismatch {
            major: kfd.uapi_major,
            minor: kfd.uapi_minor,
        });
    }
    if kfd.schema_identity != profile.kfd_schema_identity {
        return Err(DeviceCorrelationErrorV1::KfdSchemaMismatch);
    }
    if render.drm_schema_identity != profile.drm_schema_identity {
        return Err(DeviceCorrelationErrorV1::DrmSchemaMismatch);
    }
    if !topology.pci.is_well_formed() {
        return Err(DeviceCorrelationErrorV1::InvalidPciAddress(topology.pci));
    }
    if !render.pci.is_well_formed() {
        return Err(DeviceCorrelationErrorV1::InvalidPciAddress(render.pci));
    }
    if topology.kfd_gpu_id == 0 {
        return Err(DeviceCorrelationErrorV1::InvalidKfdGpuId);
    }
    if topology.gpu_unique_id == 0 {
        return Err(DeviceCorrelationErrorV1::InvalidGpuUniqueId);
    }
    if render.node.major != DRM_DEVICE_MAJOR_V1 || render.node.minor < DRM_RENDER_MIN_MINOR_V1 {
        return Err(DeviceCorrelationErrorV1::InvalidRenderNode(render.node));
    }
    if topology.drm_render_minor != render.node.minor {
        return Err(DeviceCorrelationErrorV1::RenderMinorMismatch);
    }
    if topology.gpu_unique_id != render.gpu_unique_id {
        return Err(DeviceCorrelationErrorV1::GpuUniqueIdMismatch);
    }
    if topology.vendor_id != render.vendor_id {
        return Err(DeviceCorrelationErrorV1::VendorMismatch);
    }
    if topology.vendor_id != AMD_PCI_VENDOR_ID_V1 {
        return Err(DeviceCorrelationErrorV1::VendorMismatch);
    }
    if topology.pci != render.pci {
        return Err(DeviceCorrelationErrorV1::PciMismatch);
    }
    if topology.device_id != render.device_id {
        return Err(DeviceCorrelationErrorV1::DeviceIdMismatch);
    }
    if topology.device_id != MI300X_PCI_DEVICE_ID_V1 {
        return Err(DeviceCorrelationErrorV1::UnsupportedDeviceId);
    }
    if topology.target != GpuTargetObservationV1::Gfx942 {
        return Err(DeviceCorrelationErrorV1::UnsupportedTarget);
    }
    if kfd.xnack != XnackObservationV1::Disabled {
        return Err(DeviceCorrelationErrorV1::UnsupportedXnack);
    }
    if topology.compute_partition != ComputePartitionObservationV1::Spx
        || topology.memory_partition != MemoryPartitionObservationV1::Nps1
    {
        return Err(DeviceCorrelationErrorV1::UnsupportedPartition);
    }
    if render.driver_name != DrmDriverNameObservationV1::Amdgpu {
        return Err(DeviceCorrelationErrorV1::DriverNameMismatch);
    }
    if render.drm_major != DRM_DRIVER_MAJOR_V1
        || render.drm_minor != DRM_DRIVER_MINOR_V1
        || render.drm_patch != DRM_DRIVER_PATCH_V1
    {
        return Err(DeviceCorrelationErrorV1::DrmVersionMismatch {
            major: render.drm_major,
            minor: render.drm_minor,
            patch: render.drm_patch,
        });
    }
    if !render.acceleration_working {
        return Err(DeviceCorrelationErrorV1::AccelerationUnavailable);
    }
    if render.family != DrmFamilyObservationV1::AmdgpuFamilyAi {
        return Err(DeviceCorrelationErrorV1::UnsupportedFamily);
    }

    Ok(ModelCorrelatedDeviceV1 {
        domain_id: kfd.domain_id,
        profile_id: profile.identity,
        epoch: kfd.epoch,
        identity: ModelPhysicalDeviceIdentityV1 {
            physical_id: PhysicalDeviceIdV1(topology.gpu_unique_id),
            gpu_unique_id: topology.gpu_unique_id,
            pci: topology.pci,
            vendor_id: topology.vendor_id,
            device_id: topology.device_id,
            revision_id: render.pci_revision_id,
            family: render.family,
            partition: PartitionProfileV1 {
                compute: topology.compute_partition,
                memory: topology.memory_partition,
            },
        },
        topology_node_id: topology.topology_node_id,
        kfd_gpu_id: topology.kfd_gpu_id,
        render_node: render.node,
        drm_schema_identity: render.drm_schema_identity,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelAdmissionStatusV1 {
    Active,
    Retired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ModelDeviceAdmissionRecordV1 {
    pub key: DeviceKeyV1,
    pub domain_id: DeviceObservationDomainIdV1,
    pub profile_id: DeviceAdmissionProfileIdV1,
    pub correlation: ModelCorrelatedDeviceV1,
    pub status: ModelAdmissionStatusV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ModelVmAdmissionRecordV1 {
    pub key: VmKeyV1,
    pub domain_id: DeviceObservationDomainIdV1,
    pub kfd_gpu_id: u32,
    pub render_node: DeviceNodeV1,
    pub pci: PciAddressV1,
    pub status: ModelAdmissionStatusV1,
}

/// Non-forgeable within safe Rust, but explicitly not production authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ModelDeviceAdmissionV1 {
    domain_id: DeviceObservationDomainIdV1,
    profile_id: DeviceAdmissionProfileIdV1,
    key: DeviceKeyV1,
    correlation: ModelCorrelatedDeviceV1,
}

impl ModelDeviceAdmissionV1 {
    pub const fn authority_domain(self) -> AuthorityDomainV1 {
        AuthorityDomainV1::ModelOnly
    }

    pub const fn model_key(self) -> DeviceKeyV1 {
        self.key
    }

    pub const fn domain_id(self) -> DeviceObservationDomainIdV1 {
        self.domain_id
    }

    pub const fn correlation(self) -> ModelCorrelatedDeviceV1 {
        self.correlation
    }
}

/// Model VM admission bound to one exact device generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ModelVmAdmissionV1 {
    domain_id: DeviceObservationDomainIdV1,
    key: VmKeyV1,
}

impl ModelVmAdmissionV1 {
    pub const fn authority_domain(self) -> AuthorityDomainV1 {
        AuthorityDomainV1::ModelOnly
    }

    pub const fn model_key(self) -> VmKeyV1 {
        self.key
    }

    pub const fn domain_id(self) -> DeviceObservationDomainIdV1 {
        self.domain_id
    }
}

/// Untrusted report of a VM/device association after an external operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UntrustedVmObservationV1 {
    pub domain_id: DeviceObservationDomainIdV1,
    pub device: DeviceKeyV1,
    pub vm_id: VmIdV1,
    pub kfd_gpu_id: u32,
    pub render_node: DeviceNodeV1,
    pub pci: PciAddressV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdmissionRecordKindV1 {
    Device,
    Vm,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceAdmissionErrorV1 {
    CapacityExceeded {
        kind: AdmissionRecordKindV1,
        maximum: usize,
    },
    ObservationDomainMismatch,
    ZeroGeneration,
    ActiveDeviceExists(DeviceKeyV1),
    StaleDeviceGeneration {
        requested: DeviceKeyV1,
        newest: DeviceGenerationV1,
    },
    PhysicalIdentitySubstitution(PhysicalDeviceIdV1),
    ActiveCorrelationSubstitution,
    DeviceNotFound(DeviceKeyV1),
    DeviceNotActive(DeviceKeyV1),
    LiveVmPreventsDeviceRetirement(DeviceKeyV1),
    VmObservationMismatch,
    InvalidVmId,
    DuplicateVm(VmKeyV1),
    VmNotFound(VmKeyV1),
    VmNotActive(VmKeyV1),
    SourceInvariant(DeviceIdentityInvariantViolationV1),
    NextInvariant(DeviceIdentityInvariantViolationV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceIdentityInvariantViolationV1 {
    CapacityExceeded(AdmissionRecordKindV1),
    DeviceDomainMismatch(DeviceKeyV1),
    DuplicateDevice(DeviceKeyV1),
    ZeroGeneration(DeviceKeyV1),
    PhysicalIdentityMismatch(PhysicalDeviceIdV1),
    MultipleActiveGenerations(PhysicalDeviceIdV1),
    ActiveGenerationIsNotNewest(PhysicalDeviceIdV1),
    ActiveCorrelationCollision(DeviceKeyV1, DeviceKeyV1),
    VmDomainMismatch(VmKeyV1),
    DuplicateVm(VmKeyV1),
    VmBindingMismatch(VmKeyV1),
    ActiveVmWithoutExactActiveDevice(VmKeyV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceIdentityStateV1 {
    domain_id: DeviceObservationDomainIdV1,
    devices: Vec<ModelDeviceAdmissionRecordV1>,
    vms: Vec<ModelVmAdmissionRecordV1>,
}

impl DeviceIdentityStateV1 {
    pub const fn new(domain_id: DeviceObservationDomainIdV1) -> Self {
        Self {
            domain_id,
            devices: Vec::new(),
            vms: Vec::new(),
        }
    }

    pub const fn authority_domain(&self) -> AuthorityDomainV1 {
        AuthorityDomainV1::ModelOnly
    }

    pub const fn domain_id(&self) -> DeviceObservationDomainIdV1 {
        self.domain_id
    }

    pub fn devices(&self) -> &[ModelDeviceAdmissionRecordV1] {
        &self.devices
    }

    pub fn vms(&self) -> &[ModelVmAdmissionRecordV1] {
        &self.vms
    }

    pub fn register_device_model_only(
        &self,
        correlation: ModelCorrelatedDeviceV1,
        generation: DeviceGenerationV1,
    ) -> Result<(Self, ModelDeviceAdmissionV1), DeviceAdmissionErrorV1> {
        self.validate_global_invariants()
            .map_err(DeviceAdmissionErrorV1::SourceInvariant)?;
        if self.devices.len() >= MAX_MODEL_DEVICE_ADMISSIONS_V1 {
            return Err(DeviceAdmissionErrorV1::CapacityExceeded {
                kind: AdmissionRecordKindV1::Device,
                maximum: MAX_MODEL_DEVICE_ADMISSIONS_V1,
            });
        }
        if correlation.domain_id != self.domain_id {
            return Err(DeviceAdmissionErrorV1::ObservationDomainMismatch);
        }
        if generation.0 == 0 {
            return Err(DeviceAdmissionErrorV1::ZeroGeneration);
        }
        let identity = correlation.identity;
        let key = DeviceKeyV1 {
            physical: identity.physical_id,
            generation,
        };
        let mut newest = None;
        for record in self
            .devices
            .iter()
            .filter(|record| record.key.physical == identity.physical_id)
        {
            if record.correlation.identity != identity {
                return Err(DeviceAdmissionErrorV1::PhysicalIdentitySubstitution(
                    identity.physical_id,
                ));
            }
            if record.status == ModelAdmissionStatusV1::Active {
                return Err(DeviceAdmissionErrorV1::ActiveDeviceExists(record.key));
            }
            newest = Some(
                newest.map_or(record.key.generation, |current: DeviceGenerationV1| {
                    current.max(record.key.generation)
                }),
            );
        }
        if let Some(newest) = newest
            && generation <= newest
        {
            return Err(DeviceAdmissionErrorV1::StaleDeviceGeneration {
                requested: key,
                newest,
            });
        }
        for record in self
            .devices
            .iter()
            .filter(|record| record.status == ModelAdmissionStatusV1::Active)
        {
            let old = record.correlation;
            if old.identity.pci == identity.pci
                || old.render_node == correlation.render_node
                || old.kfd_gpu_id == correlation.kfd_gpu_id
            {
                return Err(DeviceAdmissionErrorV1::ActiveCorrelationSubstitution);
            }
        }

        let token = ModelDeviceAdmissionV1 {
            domain_id: self.domain_id,
            profile_id: correlation.profile_id,
            key,
            correlation,
        };
        let mut next = self.clone();
        next.devices.push(ModelDeviceAdmissionRecordV1 {
            key,
            domain_id: self.domain_id,
            profile_id: correlation.profile_id,
            correlation,
            status: ModelAdmissionStatusV1::Active,
        });
        next.validate_global_invariants()
            .map_err(DeviceAdmissionErrorV1::NextInvariant)?;
        Ok((next, token))
    }

    pub fn register_vm_model_only(
        &self,
        device: ModelDeviceAdmissionV1,
        observation: UntrustedVmObservationV1,
    ) -> Result<(Self, ModelVmAdmissionV1), DeviceAdmissionErrorV1> {
        self.validate_global_invariants()
            .map_err(DeviceAdmissionErrorV1::SourceInvariant)?;
        if self.vms.len() >= MAX_MODEL_VM_ADMISSIONS_V1 {
            return Err(DeviceAdmissionErrorV1::CapacityExceeded {
                kind: AdmissionRecordKindV1::Vm,
                maximum: MAX_MODEL_VM_ADMISSIONS_V1,
            });
        }
        let record = self
            .devices
            .iter()
            .find(|record| record.key == device.key)
            .ok_or(DeviceAdmissionErrorV1::DeviceNotFound(device.key))?;
        if record.status != ModelAdmissionStatusV1::Active {
            return Err(DeviceAdmissionErrorV1::DeviceNotActive(device.key));
        }
        if device.domain_id != self.domain_id
            || device.profile_id != record.profile_id
            || device.correlation != record.correlation
        {
            return Err(DeviceAdmissionErrorV1::VmObservationMismatch);
        }
        if observation.vm_id.0 == 0 {
            return Err(DeviceAdmissionErrorV1::InvalidVmId);
        }
        if observation.domain_id != self.domain_id
            || observation.device != device.key
            || observation.kfd_gpu_id != record.correlation.kfd_gpu_id
            || observation.render_node != record.correlation.render_node
            || observation.pci != record.correlation.identity.pci
        {
            return Err(DeviceAdmissionErrorV1::VmObservationMismatch);
        }
        let key = VmKeyV1 {
            device: observation.device,
            id: observation.vm_id,
        };
        if self.vms.iter().any(|record| record.key == key) {
            return Err(DeviceAdmissionErrorV1::DuplicateVm(key));
        }
        let token = ModelVmAdmissionV1 {
            domain_id: self.domain_id,
            key,
        };
        let mut next = self.clone();
        next.vms.push(ModelVmAdmissionRecordV1 {
            key,
            domain_id: self.domain_id,
            kfd_gpu_id: observation.kfd_gpu_id,
            render_node: observation.render_node,
            pci: observation.pci,
            status: ModelAdmissionStatusV1::Active,
        });
        next.validate_global_invariants()
            .map_err(DeviceAdmissionErrorV1::NextInvariant)?;
        Ok((next, token))
    }

    pub fn retire_vm_model_only(
        &self,
        vm: ModelVmAdmissionV1,
    ) -> Result<Self, DeviceAdmissionErrorV1> {
        self.validate_global_invariants()
            .map_err(DeviceAdmissionErrorV1::SourceInvariant)?;
        if vm.domain_id != self.domain_id {
            return Err(DeviceAdmissionErrorV1::VmObservationMismatch);
        }
        let mut next = self.clone();
        let record = next
            .vms
            .iter_mut()
            .find(|record| record.key == vm.key)
            .ok_or(DeviceAdmissionErrorV1::VmNotFound(vm.key))?;
        if record.status != ModelAdmissionStatusV1::Active {
            return Err(DeviceAdmissionErrorV1::VmNotActive(vm.key));
        }
        record.status = ModelAdmissionStatusV1::Retired;
        next.validate_global_invariants()
            .map_err(DeviceAdmissionErrorV1::NextInvariant)?;
        Ok(next)
    }

    pub fn retire_device_model_only(
        &self,
        device: ModelDeviceAdmissionV1,
    ) -> Result<Self, DeviceAdmissionErrorV1> {
        self.validate_global_invariants()
            .map_err(DeviceAdmissionErrorV1::SourceInvariant)?;
        let stored = self
            .devices
            .iter()
            .find(|record| record.key == device.key)
            .ok_or(DeviceAdmissionErrorV1::DeviceNotFound(device.key))?;
        if stored.status != ModelAdmissionStatusV1::Active {
            return Err(DeviceAdmissionErrorV1::DeviceNotActive(device.key));
        }
        if device.domain_id != self.domain_id
            || device.profile_id != stored.profile_id
            || device.correlation != stored.correlation
        {
            return Err(DeviceAdmissionErrorV1::PhysicalIdentitySubstitution(
                device.key.physical,
            ));
        }
        if self.vms.iter().any(|record| {
            record.key.device == device.key && record.status == ModelAdmissionStatusV1::Active
        }) {
            return Err(DeviceAdmissionErrorV1::LiveVmPreventsDeviceRetirement(
                device.key,
            ));
        }
        let mut next = self.clone();
        next.devices
            .iter_mut()
            .find(|record| record.key == device.key)
            .expect("record was found above")
            .status = ModelAdmissionStatusV1::Retired;
        next.validate_global_invariants()
            .map_err(DeviceAdmissionErrorV1::NextInvariant)?;
        Ok(next)
    }

    pub fn validate_global_invariants(&self) -> Result<(), DeviceIdentityInvariantViolationV1> {
        if self.devices.len() > MAX_MODEL_DEVICE_ADMISSIONS_V1 {
            return Err(DeviceIdentityInvariantViolationV1::CapacityExceeded(
                AdmissionRecordKindV1::Device,
            ));
        }
        if self.vms.len() > MAX_MODEL_VM_ADMISSIONS_V1 {
            return Err(DeviceIdentityInvariantViolationV1::CapacityExceeded(
                AdmissionRecordKindV1::Vm,
            ));
        }
        for (index, device) in self.devices.iter().enumerate() {
            if device.domain_id != self.domain_id || device.correlation.domain_id != self.domain_id
            {
                return Err(DeviceIdentityInvariantViolationV1::DeviceDomainMismatch(
                    device.key,
                ));
            }
            if device.key.generation.0 == 0 {
                return Err(DeviceIdentityInvariantViolationV1::ZeroGeneration(
                    device.key,
                ));
            }
            if device.key.physical != device.correlation.identity.physical_id {
                return Err(
                    DeviceIdentityInvariantViolationV1::PhysicalIdentityMismatch(
                        device.key.physical,
                    ),
                );
            }
            if self.devices[..index]
                .iter()
                .any(|other| other.key == device.key)
            {
                return Err(DeviceIdentityInvariantViolationV1::DuplicateDevice(
                    device.key,
                ));
            }
            for other in self.devices.iter().filter(|other| {
                other.key.physical == device.key.physical && other.key != device.key
            }) {
                if other.correlation.identity != device.correlation.identity {
                    return Err(
                        DeviceIdentityInvariantViolationV1::PhysicalIdentityMismatch(
                            device.key.physical,
                        ),
                    );
                }
                if device.status == ModelAdmissionStatusV1::Active
                    && other.status == ModelAdmissionStatusV1::Active
                {
                    return Err(
                        DeviceIdentityInvariantViolationV1::MultipleActiveGenerations(
                            device.key.physical,
                        ),
                    );
                }
                if device.status == ModelAdmissionStatusV1::Active
                    && other.key.generation > device.key.generation
                {
                    return Err(
                        DeviceIdentityInvariantViolationV1::ActiveGenerationIsNotNewest(
                            device.key.physical,
                        ),
                    );
                }
            }
            if device.status == ModelAdmissionStatusV1::Active {
                for other in self.devices[..index]
                    .iter()
                    .filter(|other| other.status == ModelAdmissionStatusV1::Active)
                {
                    if other.correlation.identity.pci == device.correlation.identity.pci
                        || other.correlation.render_node == device.correlation.render_node
                        || other.correlation.kfd_gpu_id == device.correlation.kfd_gpu_id
                    {
                        return Err(
                            DeviceIdentityInvariantViolationV1::ActiveCorrelationCollision(
                                other.key, device.key,
                            ),
                        );
                    }
                }
            }
        }
        for (index, vm) in self.vms.iter().enumerate() {
            if vm.domain_id != self.domain_id {
                return Err(DeviceIdentityInvariantViolationV1::VmDomainMismatch(vm.key));
            }
            if self.vms[..index].iter().any(|other| other.key == vm.key) {
                return Err(DeviceIdentityInvariantViolationV1::DuplicateVm(vm.key));
            }
            let device = self
                .devices
                .iter()
                .find(|device| device.key == vm.key.device)
                .ok_or(
                    DeviceIdentityInvariantViolationV1::ActiveVmWithoutExactActiveDevice(vm.key),
                )?;
            if vm.kfd_gpu_id != device.correlation.kfd_gpu_id
                || vm.render_node != device.correlation.render_node
                || vm.pci != device.correlation.identity.pci
            {
                return Err(DeviceIdentityInvariantViolationV1::VmBindingMismatch(
                    vm.key,
                ));
            }
            if vm.status == ModelAdmissionStatusV1::Active
                && device.status != ModelAdmissionStatusV1::Active
            {
                return Err(
                    DeviceIdentityInvariantViolationV1::ActiveVmWithoutExactActiveDevice(vm.key),
                );
            }
        }
        Ok(())
    }
}
