use fe2o3_amd_target::{AmdTargetCapabilities, AtomicScope, CapabilitySupport, WavefrontWidth};

use crate::{
    CapabilityV1, DeviceDescriptorTableV1, KernelDescriptorV1, KernelId, MAX_KERNELS,
    ValidationError,
};

/// Exact wavefront width required by one kernel binary.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RequiredWavefrontWidthV2 {
    Wave32,
    Wave64,
}

impl RequiredWavefrontWidthV2 {
    pub(crate) const fn as_target_width(self) -> WavefrontWidth {
        match self {
            Self::Wave32 => WavefrontWidth::Wave32,
            Self::Wave64 => WavefrontWidth::Wave64,
        }
    }
}

/// Bounded LDS allocation requirements for one workgroup.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LdsRequirementsV2 {
    static_bytes: u32,
    max_dynamic_bytes: u32,
}

impl LdsRequirementsV2 {
    pub fn new(static_bytes: u32, max_dynamic_bytes: u32) -> Result<Self, ValidationError> {
        static_bytes
            .checked_add(max_dynamic_bytes)
            .ok_or(ValidationError::Overflow {
                field: "total LDS requirement",
            })?;
        Ok(Self {
            static_bytes,
            max_dynamic_bytes,
        })
    }

    pub const fn static_bytes(self) -> u32 {
        self.static_bytes
    }

    pub const fn max_dynamic_bytes(self) -> u32 {
        self.max_dynamic_bytes
    }

    pub const fn maximum_total_bytes(self) -> u32 {
        // Construction proves this sum cannot overflow.
        self.static_bytes + self.max_dynamic_bytes
    }
}

/// Named synchronization requirements encoded as a closed V2 bit set.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SynchronizationRequirementsV2(u16);

impl SynchronizationRequirementsV2 {
    pub const WAVE_BARRIER: u16 = 1 << 0;
    pub const WORKGROUP_BARRIER: u16 = 1 << 1;
    pub const WORKGROUP_FENCE: u16 = 1 << 2;
    pub const DEVICE_FENCE: u16 = 1 << 3;
    pub const SYSTEM_FENCE: u16 = 1 << 4;
    pub const KNOWN_BITS: u16 = Self::WAVE_BARRIER
        | Self::WORKGROUP_BARRIER
        | Self::WORKGROUP_FENCE
        | Self::DEVICE_FENCE
        | Self::SYSTEM_FENCE;

    pub const fn empty() -> Self {
        Self(0)
    }

    pub fn from_bits(bits: u16) -> Result<Self, ValidationError> {
        if bits & !Self::KNOWN_BITS != 0 {
            return Err(ValidationError::InvalidValue {
                field: "synchronization requirement bits",
            });
        }
        Ok(Self(bits))
    }

    pub const fn bits(self) -> u16 {
        self.0
    }

    pub const fn contains(self, bit: u16) -> bool {
        self.0 & bit != 0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// Named atomic-scope requirements encoded as a closed V2 bit set.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AtomicRequirementsV2(u16);

impl AtomicRequirementsV2 {
    pub const WORKGROUP_SCOPE: u16 = 1 << 0;
    pub const DEVICE_SCOPE: u16 = 1 << 1;
    pub const SYSTEM_SCOPE: u16 = 1 << 2;
    pub const KNOWN_BITS: u16 = Self::WORKGROUP_SCOPE | Self::DEVICE_SCOPE | Self::SYSTEM_SCOPE;

    pub const fn empty() -> Self {
        Self(0)
    }

    pub fn from_bits(bits: u16) -> Result<Self, ValidationError> {
        if bits & !Self::KNOWN_BITS != 0 {
            return Err(ValidationError::InvalidValue {
                field: "atomic requirement bits",
            });
        }
        Ok(Self(bits))
    }

    pub const fn bits(self) -> u16 {
        self.0
    }

    pub const fn contains(self, bit: u16) -> bool {
        self.0 & bit != 0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// Target-sensitive declarations for one kernel in a V2 descriptor table.
///
/// These values are requirements to validate. They are not evidence that a
/// physical device, allocation, or launch satisfies those requirements.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KernelTargetRequirementsV2 {
    kernel_id: KernelId,
    lds: LdsRequirementsV2,
    wavefront_width: RequiredWavefrontWidthV2,
    cooperative_launch: bool,
    synchronization: SynchronizationRequirementsV2,
    atomics: AtomicRequirementsV2,
}

impl KernelTargetRequirementsV2 {
    pub const fn new(
        kernel_id: KernelId,
        lds: LdsRequirementsV2,
        wavefront_width: RequiredWavefrontWidthV2,
        cooperative_launch: bool,
        synchronization: SynchronizationRequirementsV2,
        atomics: AtomicRequirementsV2,
    ) -> Self {
        Self {
            kernel_id,
            lds,
            wavefront_width,
            cooperative_launch,
            synchronization,
            atomics,
        }
    }

    pub const fn kernel_id(self) -> KernelId {
        self.kernel_id
    }

    pub const fn lds(self) -> LdsRequirementsV2 {
        self.lds
    }

    pub const fn wavefront_width(self) -> RequiredWavefrontWidthV2 {
        self.wavefront_width
    }

    pub const fn cooperative_launch(self) -> bool {
        self.cooperative_launch
    }

    pub const fn synchronization(self) -> SynchronizationRequirementsV2 {
        self.synchronization
    }

    pub const fn atomics(self) -> AtomicRequirementsV2 {
        self.atomics
    }

    /// Cooperative launch always needs observed runtime and occupancy evidence.
    pub const fn requires_runtime_evidence(self) -> bool {
        self.cooperative_launch
            || self.atomics.contains(AtomicRequirementsV2::SYSTEM_SCOPE)
            || self
                .synchronization
                .contains(SynchronizationRequirementsV2::SYSTEM_FENCE)
    }
}

/// A V2 extension over an unchanged canonical V1 descriptor table.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceDescriptorTableV2 {
    base: DeviceDescriptorTableV1,
    requirements: Vec<KernelTargetRequirementsV2>,
}

impl DeviceDescriptorTableV2 {
    pub fn new(
        base: DeviceDescriptorTableV1,
        mut requirements: Vec<KernelTargetRequirementsV2>,
    ) -> Result<Self, ValidationError> {
        requirements.sort_unstable_by_key(|requirement| requirement.kernel_id);
        let value = Self { base, requirements };
        value.validate()?;
        crate::wire_v2::validate_encoded_size_v2(&value)?;
        Ok(value)
    }

    pub const fn base(&self) -> &DeviceDescriptorTableV1 {
        &self.base
    }

    pub fn requirements(&self) -> &[KernelTargetRequirementsV2] {
        &self.requirements
    }

    pub fn requirement_for(&self, kernel_id: KernelId) -> Option<&KernelTargetRequirementsV2> {
        self.requirements
            .binary_search_by_key(&kernel_id, |requirement| requirement.kernel_id)
            .ok()
            .map(|index| &self.requirements[index])
    }

    pub(crate) fn from_wire(
        base: DeviceDescriptorTableV1,
        requirements: Vec<KernelTargetRequirementsV2>,
    ) -> Result<Self, ValidationError> {
        let value = Self { base, requirements };
        value.validate()?;
        Ok(value)
    }

    fn validate(&self) -> Result<(), ValidationError> {
        if self.requirements.len() > MAX_KERNELS {
            return Err(ValidationError::TooMany {
                field: "kernel target requirements",
                max: MAX_KERNELS,
            });
        }
        for pair in self.requirements.windows(2) {
            if pair[0].kernel_id == pair[1].kernel_id {
                return Err(ValidationError::Duplicate {
                    field: "kernel target requirement",
                });
            }
            if pair[0].kernel_id > pair[1].kernel_id {
                return Err(ValidationError::NonCanonicalOrder {
                    field: "kernel target requirements",
                });
            }
        }
        if self.requirements.len() != self.base.kernels().len() {
            return Err(ValidationError::InvalidValue {
                field: "kernel target requirement closure",
            });
        }

        let target = AmdTargetCapabilities::derive(self.base.device_target().as_amd_target_id())
            .map_err(|_| ValidationError::TargetMismatch {
                field: "target capability profile",
            })?;
        for (kernel, requirement) in self.base.kernels().iter().zip(&self.requirements) {
            if kernel.kernel_id() != requirement.kernel_id {
                return Err(ValidationError::DanglingReference {
                    field: "kernel target requirement",
                });
            }
            validate_kernel_requirement(kernel, *requirement, target)?;
        }
        Ok(())
    }
}

fn validate_kernel_requirement(
    kernel: &KernelDescriptorV1,
    requirement: KernelTargetRequirementsV2,
    target: AmdTargetCapabilities,
) -> Result<(), ValidationError> {
    if requirement.lds.static_bytes != kernel.launch().static_shared_memory_bytes()
        || requirement.lds.max_dynamic_bytes != kernel.launch().max_dynamic_shared_memory_bytes()
    {
        return Err(ValidationError::InvalidValue {
            field: "LDS requirements conflict with V1 launch constraints",
        });
    }
    if requirement.lds.maximum_total_bytes() > target.max_lds_bytes_per_workgroup() {
        return Err(ValidationError::TargetMismatch {
            field: "maximum LDS bytes per workgroup",
        });
    }
    if !target
        .wavefront_widths()
        .contains(requirement.wavefront_width.as_target_width())
    {
        return Err(ValidationError::TargetMismatch {
            field: "exact wavefront width",
        });
    }
    if requirement.cooperative_launch
        && matches!(target.cooperative_launch(), CapabilitySupport::Unsupported)
    {
        return Err(ValidationError::TargetMismatch {
            field: "cooperative launch",
        });
    }

    let capabilities = kernel.capabilities();
    if !capabilities.contains(&CapabilityV1::AmdWave) {
        return Err(ValidationError::InvalidValue {
            field: "exact wavefront width requires the AMD wave capability",
        });
    }
    if (requirement.lds.maximum_total_bytes() != 0
        || requirement
            .synchronization
            .contains(SynchronizationRequirementsV2::WORKGROUP_BARRIER))
        && !capabilities.contains(&CapabilityV1::WorkgroupMemory)
    {
        return Err(ValidationError::InvalidValue {
            field: "LDS or workgroup barrier requires the workgroup-memory capability",
        });
    }
    if requirement
        .synchronization
        .contains(SynchronizationRequirementsV2::WAVE_BARRIER)
        && !capabilities.contains(&CapabilityV1::Subgroup)
    {
        return Err(ValidationError::InvalidValue {
            field: "wave barrier requires the subgroup capability",
        });
    }
    if (!requirement.atomics.is_empty()
        || requirement.synchronization.bits()
            & (SynchronizationRequirementsV2::WORKGROUP_FENCE
                | SynchronizationRequirementsV2::DEVICE_FENCE
                | SynchronizationRequirementsV2::SYSTEM_FENCE)
            != 0)
        && !capabilities.contains(&CapabilityV1::Atomics)
    {
        return Err(ValidationError::InvalidValue {
            field: "atomic or fence requirement requires the atomics capability",
        });
    }

    validate_scope(
        requirement
            .atomics
            .contains(AtomicRequirementsV2::WORKGROUP_SCOPE)
            || requirement
                .synchronization
                .contains(SynchronizationRequirementsV2::WORKGROUP_FENCE),
        AtomicScope::Workgroup,
        target,
    )?;
    validate_scope(
        requirement
            .atomics
            .contains(AtomicRequirementsV2::DEVICE_SCOPE)
            || requirement
                .synchronization
                .contains(SynchronizationRequirementsV2::DEVICE_FENCE),
        AtomicScope::Device,
        target,
    )?;
    validate_scope(
        requirement
            .atomics
            .contains(AtomicRequirementsV2::SYSTEM_SCOPE)
            || requirement
                .synchronization
                .contains(SynchronizationRequirementsV2::SYSTEM_FENCE),
        AtomicScope::System,
        target,
    )
}

fn validate_scope(
    required: bool,
    scope: AtomicScope,
    target: AmdTargetCapabilities,
) -> Result<(), ValidationError> {
    if required && !target.atomic_scopes().contains(scope) {
        Err(ValidationError::TargetMismatch {
            field: "atomic or fence scope",
        })
    } else {
        Ok(())
    }
}
