use core::fmt;

use crate::feature_capabilities::{
    AdvancedTargetCapabilities, AtomicOrderings, AtomicWidths, DeviceDiagnosticFeature, Fp8Formats,
    LaunchBoundsField, MfmaFamilies, MxFormats, WorkgroupLimits,
};
use crate::{AmdTargetFeature, AmdTargetId};

/// A wavefront width supported by an AMDGPU processor.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WavefrontWidth {
    Wave32,
    Wave64,
}

impl fmt::Display for WavefrontWidth {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Wave32 => "wave32",
            Self::Wave64 => "wave64",
        })
    }
}

/// The AMDGPU synchronization scopes that can be lowered for an atomic.
///
/// System scope still requires runtime evidence that the allocation and its
/// mapping are eligible for system-scope access.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AtomicScope {
    Workgroup,
    Device,
    System,
}

impl fmt::Display for AtomicScope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Workgroup => "workgroup",
            Self::Device => "device",
            Self::System => "system",
        })
    }
}

/// A target-specific matrix instruction family.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MatrixInstructionSet {
    Mfma,
    Wmma128,
    Wmma256,
    Swmma,
}

impl fmt::Display for MatrixInstructionSet {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Mfma => "mfma",
            Self::Wmma128 => "wmma128",
            Self::Wmma256 => "wmma256",
            Self::Swmma => "swmma",
        })
    }
}

/// A target-specific instruction family for moving data between VMEM and LDS.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AsyncCopyInstructionSet {
    VmemToLds,
    AsyncLoadToLds,
    AsyncStoreFromLds,
}

impl fmt::Display for AsyncCopyInstructionSet {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::VmemToLds => "vmem-to-lds",
            Self::AsyncLoadToLds => "async-load-to-lds",
            Self::AsyncStoreFromLds => "async-store-from-lds",
        })
    }
}

/// How a target-level capability claim must be established.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CapabilitySupport {
    Unsupported,
    Supported,
    RequiresRuntimeEvidence,
}

impl fmt::Display for CapabilitySupport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Unsupported => "unsupported",
            Self::Supported => "supported",
            Self::RequiresRuntimeEvidence => "runtime-evidence",
        })
    }
}

/// Why canonical capabilities could not be derived from a target ID.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapabilityDerivationError {
    /// The parser's processor table has an entry absent from this model.
    UnknownProcessor,
    /// A target feature was present on a processor that does not support it.
    UnsupportedTargetFeature(AmdTargetFeature),
    /// The processor profile selected no wave width or selected an invalid
    /// default width.
    ContradictoryWavefrontProfile,
}

impl fmt::Display for CapabilityDerivationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownProcessor => {
                formatter.write_str("no canonical capabilities exist for this processor")
            }
            Self::UnsupportedTargetFeature(feature) => {
                write!(
                    formatter,
                    "processor does not support target feature {feature}"
                )
            }
            Self::ContradictoryWavefrontProfile => {
                formatter.write_str("processor has a contradictory wavefront profile")
            }
        }
    }
}

impl core::error::Error for CapabilityDerivationError {}

/// A compact set of supported wavefront widths.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WavefrontWidths(u8);

impl WavefrontWidths {
    const WAVE32: u8 = 1 << 0;
    const WAVE64: u8 = 1 << 1;

    const fn wave32() -> Self {
        Self(Self::WAVE32)
    }

    const fn wave64() -> Self {
        Self(Self::WAVE64)
    }

    const fn wave32_and_wave64() -> Self {
        Self(Self::WAVE32 | Self::WAVE64)
    }

    /// Returns whether `width` is supported.
    pub const fn contains(self, width: WavefrontWidth) -> bool {
        let flag = match width {
            WavefrontWidth::Wave32 => Self::WAVE32,
            WavefrontWidth::Wave64 => Self::WAVE64,
        };
        self.0 & flag != 0
    }
}

impl fmt::Display for WavefrontWidths {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[")?;
        let mut separator = "";
        for width in [WavefrontWidth::Wave32, WavefrontWidth::Wave64] {
            if self.contains(width) {
                formatter.write_str(separator)?;
                write!(formatter, "{width}")?;
                separator = ",";
            }
        }
        formatter.write_str("]")
    }
}

/// A compact set of supported atomic scopes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AtomicScopes(u8);

impl AtomicScopes {
    const WORKGROUP: u8 = 1 << 0;
    const DEVICE: u8 = 1 << 1;
    const SYSTEM: u8 = 1 << 2;

    pub(crate) const NONE: Self = Self(0);
    pub(crate) const ALL: Self = Self(Self::WORKGROUP | Self::DEVICE | Self::SYSTEM);

    /// Returns whether atomics can be lowered at `scope`.
    pub const fn contains(self, scope: AtomicScope) -> bool {
        let flag = match scope {
            AtomicScope::Workgroup => Self::WORKGROUP,
            AtomicScope::Device => Self::DEVICE,
            AtomicScope::System => Self::SYSTEM,
        };
        self.0 & flag != 0
    }

    /// Returns whether no atomic scope has been reviewed.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl fmt::Display for AtomicScopes {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[")?;
        let mut separator = "";
        for scope in [
            AtomicScope::Workgroup,
            AtomicScope::Device,
            AtomicScope::System,
        ] {
            if self.contains(scope) {
                formatter.write_str(separator)?;
                write!(formatter, "{scope}")?;
                separator = ",";
            }
        }
        formatter.write_str("]")
    }
}

/// A compact set of matrix instruction families.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MatrixInstructionSets(u8);

impl MatrixInstructionSets {
    const MFMA: u8 = 1 << 0;
    const WMMA128: u8 = 1 << 1;
    const WMMA256: u8 = 1 << 2;
    const SWMMA: u8 = 1 << 3;

    const NONE: Self = Self(0);
    const MFMA_ONLY: Self = Self(Self::MFMA);
    const WMMA128_AND_SWMMA: Self = Self(Self::WMMA128 | Self::SWMMA);
    const WMMA256_ONLY: Self = Self(Self::WMMA256);
    const SWMMA_ONLY: Self = Self(Self::SWMMA);

    /// Returns whether `instruction_set` is supported.
    pub const fn contains(self, instruction_set: MatrixInstructionSet) -> bool {
        let flag = match instruction_set {
            MatrixInstructionSet::Mfma => Self::MFMA,
            MatrixInstructionSet::Wmma128 => Self::WMMA128,
            MatrixInstructionSet::Wmma256 => Self::WMMA256,
            MatrixInstructionSet::Swmma => Self::SWMMA,
        };
        self.0 & flag != 0
    }

    /// Returns whether no modeled matrix instruction family is supported.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl fmt::Display for MatrixInstructionSets {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[")?;
        let mut separator = "";
        for instruction_set in [
            MatrixInstructionSet::Mfma,
            MatrixInstructionSet::Wmma128,
            MatrixInstructionSet::Wmma256,
            MatrixInstructionSet::Swmma,
        ] {
            if self.contains(instruction_set) {
                formatter.write_str(separator)?;
                write!(formatter, "{instruction_set}")?;
                separator = ",";
            }
        }
        formatter.write_str("]")
    }
}

/// A compact set of VMEM/LDS copy instruction families.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AsyncCopyInstructionSets(u8);

impl AsyncCopyInstructionSets {
    const VMEM_TO_LDS: u8 = 1 << 0;
    const ASYNC_LOAD_TO_LDS: u8 = 1 << 1;
    const ASYNC_STORE_FROM_LDS: u8 = 1 << 2;

    const NONE: Self = Self(0);
    const VMEM_TO_LDS_ONLY: Self = Self(Self::VMEM_TO_LDS);
    const ASYNC_LOAD_ONLY: Self = Self(Self::ASYNC_LOAD_TO_LDS);
    const ASYNC_LOAD_AND_STORE: Self = Self(Self::ASYNC_LOAD_TO_LDS | Self::ASYNC_STORE_FROM_LDS);

    /// Returns whether `instruction_set` is supported.
    pub const fn contains(self, instruction_set: AsyncCopyInstructionSet) -> bool {
        let flag = match instruction_set {
            AsyncCopyInstructionSet::VmemToLds => Self::VMEM_TO_LDS,
            AsyncCopyInstructionSet::AsyncLoadToLds => Self::ASYNC_LOAD_TO_LDS,
            AsyncCopyInstructionSet::AsyncStoreFromLds => Self::ASYNC_STORE_FROM_LDS,
        };
        self.0 & flag != 0
    }

    /// Returns whether no modeled VMEM/LDS copy instruction is supported.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl fmt::Display for AsyncCopyInstructionSets {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[")?;
        let mut separator = "";
        for instruction_set in [
            AsyncCopyInstructionSet::VmemToLds,
            AsyncCopyInstructionSet::AsyncLoadToLds,
            AsyncCopyInstructionSet::AsyncStoreFromLds,
        ] {
            if self.contains(instruction_set) {
                formatter.write_str(separator)?;
                write!(formatter, "{instruction_set}")?;
                separator = ",";
            }
        }
        formatter.write_str("]")
    }
}

/// Canonical target-derived AMDGPU capabilities.
///
/// This model records ISA/codegen facts only. It does not attest a physical
/// device, memory allocation, runtime version, or occupancy-safe launch.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AmdTargetCapabilities {
    target: AmdTargetId,
    default_wavefront_width: WavefrontWidth,
    wavefront_widths: WavefrontWidths,
    max_lds_bytes_per_workgroup: u32,
    atomic_scopes: AtomicScopes,
    cooperative_launch: CapabilitySupport,
    matrix_instruction_sets: MatrixInstructionSets,
    async_copy_instruction_sets: AsyncCopyInstructionSets,
    advanced: AdvancedTargetCapabilities,
}

impl AmdTargetCapabilities {
    /// Derives capabilities from one validated canonical target ID.
    pub fn derive(target: AmdTargetId) -> Result<Self, CapabilityDerivationError> {
        validate_target_features(target)?;
        let profile = processor_profile(target.processor())
            .ok_or(CapabilityDerivationError::UnknownProcessor)?;
        if !profile
            .wavefront_widths
            .contains(profile.default_wavefront_width)
        {
            return Err(CapabilityDerivationError::ContradictoryWavefrontProfile);
        }

        Ok(Self {
            target,
            default_wavefront_width: profile.default_wavefront_width,
            wavefront_widths: profile.wavefront_widths,
            max_lds_bytes_per_workgroup: profile.max_lds_bytes_per_workgroup,
            atomic_scopes: AtomicScopes::ALL,
            // HIP exposes this as a queried device attribute. A processor
            // spelling alone must never authorize a cooperative launch.
            cooperative_launch: CapabilitySupport::RequiresRuntimeEvidence,
            matrix_instruction_sets: profile.matrix_instruction_sets,
            async_copy_instruction_sets: profile.async_copy_instruction_sets,
            advanced: AdvancedTargetCapabilities::for_processor(target.processor()),
        })
    }

    pub const fn target(&self) -> AmdTargetId {
        self.target
    }

    pub const fn default_wavefront_width(&self) -> WavefrontWidth {
        self.default_wavefront_width
    }

    pub const fn wavefront_widths(&self) -> WavefrontWidths {
        self.wavefront_widths
    }

    pub const fn max_lds_bytes_per_workgroup(&self) -> u32 {
        self.max_lds_bytes_per_workgroup
    }

    pub const fn atomic_scopes(&self) -> AtomicScopes {
        self.atomic_scopes
    }

    pub const fn cooperative_launch(&self) -> CapabilitySupport {
        self.cooperative_launch
    }

    pub const fn matrix_instruction_sets(&self) -> MatrixInstructionSets {
        self.matrix_instruction_sets
    }

    pub const fn async_copy_instruction_sets(&self) -> AsyncCopyInstructionSets {
        self.async_copy_instruction_sets
    }

    /// Returns reviewed workgroup limits, or `None` when this target profile
    /// has not established them.
    pub const fn workgroup_limits(&self) -> Option<WorkgroupLimits> {
        self.advanced.workgroup_limits
    }

    /// Returns the maximum wavefront count implied by reviewed workgroup
    /// limits for `width`.
    pub const fn max_wavefronts_per_workgroup(&self, width: WavefrontWidth) -> Option<u32> {
        if !self.wavefront_widths.contains(width) {
            return None;
        }
        match self.advanced.workgroup_limits {
            Some(limits) => Some(limits.max_wavefronts(width)),
            None => None,
        }
    }

    /// Standard Rust atomic widths with reviewed direct target lowering.
    pub const fn standard_atomic_widths(&self) -> AtomicWidths {
        self.advanced.standard_atomic_widths
    }

    /// Standard Rust atomic scopes with reviewed target lowering.
    ///
    /// System scope still requires runtime evidence for the allocation and
    /// mapping. Address-space compatibility is a separate verifier check.
    pub const fn standard_atomic_scopes(&self) -> AtomicScopes {
        self.advanced.standard_atomic_scopes
    }

    /// Standard Rust atomic orderings with reviewed target lowering.
    pub const fn standard_atomic_orderings(&self) -> AtomicOrderings {
        self.advanced.standard_atomic_orderings
    }

    /// Whether the target has a reviewed native split-barrier mechanism.
    pub const fn native_split_barriers(&self) -> CapabilitySupport {
        self.advanced.native_split_barriers
    }

    /// Reviewed scalar FP8 encodings for this exact target.
    pub const fn fp8_formats(&self) -> Fp8Formats {
        self.advanced.fp8_formats
    }

    /// Reviewed native microscaling formats for this exact target.
    pub const fn mx_formats(&self) -> MxFormats {
        self.advanced.mx_formats
    }

    /// Reviewed MFMA numerical families for this exact target.
    pub const fn mfma_families(&self) -> MfmaFamilies {
        self.advanced.mfma_families
    }

    /// Returns the target-level support state for a device diagnostic feature.
    pub const fn device_diagnostic_support(
        &self,
        feature: DeviceDiagnosticFeature,
    ) -> CapabilitySupport {
        match feature {
            DeviceDiagnosticFeature::Printf => self.advanced.device_printf,
            DeviceDiagnosticFeature::Trap => self.advanced.device_trap,
            DeviceDiagnosticFeature::DebugTrap => self.advanced.device_debug_trap,
            DeviceDiagnosticFeature::ClockCounter => self.advanced.device_clock_counter,
            DeviceDiagnosticFeature::ProfilingMarker => self.advanced.device_profiling_marker,
        }
    }

    /// Returns support for one source-level launch-bounds metadata field.
    pub const fn launch_bounds_support(&self, field: LaunchBoundsField) -> CapabilitySupport {
        match field {
            LaunchBoundsField::MaxWorkgroupSize => self.advanced.max_workgroup_size_metadata,
            LaunchBoundsField::MinWorkgroupsPerComputeUnit => {
                self.advanced.min_workgroups_per_compute_unit_metadata
            }
        }
    }

    /// Writes the deterministic V1 canonical text encoding.
    pub fn encode_canonical(&self, writer: &mut impl fmt::Write) -> fmt::Result {
        write!(
            writer,
            "amd-target-capabilities-v1{{target={};default-wave={};waves={};max-lds-per-workgroup={};atomic-scopes={};cooperative-launch={};matrix={};async-copy={}}}",
            self.target,
            self.default_wavefront_width,
            self.wavefront_widths,
            self.max_lds_bytes_per_workgroup,
            self.atomic_scopes,
            self.cooperative_launch,
            self.matrix_instruction_sets,
            self.async_copy_instruction_sets,
        )
    }
}

impl fmt::Display for AmdTargetCapabilities {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.encode_canonical(formatter)
    }
}

#[derive(Clone, Copy)]
struct ProcessorProfile {
    default_wavefront_width: WavefrontWidth,
    wavefront_widths: WavefrontWidths,
    max_lds_bytes_per_workgroup: u32,
    matrix_instruction_sets: MatrixInstructionSets,
    async_copy_instruction_sets: AsyncCopyInstructionSets,
}

fn processor_profile(processor: &str) -> Option<ProcessorProfile> {
    let bytes = processor.as_bytes();
    let is_gfx6 = matches!(bytes, b"gfx600" | b"gfx601" | b"gfx602");
    let is_gfx7 = matches!(
        bytes,
        b"gfx700" | b"gfx701" | b"gfx702" | b"gfx703" | b"gfx704" | b"gfx705"
    );
    let is_gfx8 = matches!(
        bytes,
        b"gfx801" | b"gfx802" | b"gfx803" | b"gfx805" | b"gfx810"
    );
    let is_gfx9 = matches!(
        bytes,
        b"gfx900"
            | b"gfx902"
            | b"gfx904"
            | b"gfx906"
            | b"gfx908"
            | b"gfx909"
            | b"gfx90a"
            | b"gfx90c"
            | b"gfx942"
            | b"gfx950"
    );
    let is_gfx10 = matches!(
        bytes,
        b"gfx1010"
            | b"gfx1011"
            | b"gfx1012"
            | b"gfx1013"
            | b"gfx1030"
            | b"gfx1031"
            | b"gfx1032"
            | b"gfx1033"
            | b"gfx1034"
            | b"gfx1035"
            | b"gfx1036"
    );
    let is_gfx11 = matches!(
        bytes,
        b"gfx1100"
            | b"gfx1101"
            | b"gfx1102"
            | b"gfx1103"
            | b"gfx1150"
            | b"gfx1151"
            | b"gfx1152"
            | b"gfx1153"
            | b"gfx1154"
            | b"gfx1170"
            | b"gfx1171"
            | b"gfx1172"
    );
    let is_gfx12 = matches!(bytes, b"gfx1200" | b"gfx1201");
    let is_gfx125 = matches!(bytes, b"gfx1250" | b"gfx1251");
    let is_gfx13 = matches!(bytes, b"gfx1310");
    if !(is_gfx6
        || is_gfx7
        || is_gfx8
        || is_gfx9
        || is_gfx10
        || is_gfx11
        || is_gfx12
        || is_gfx125
        || is_gfx13)
    {
        return None;
    }

    let default_wavefront_width = if is_gfx6 || is_gfx7 || is_gfx8 || is_gfx9 {
        WavefrontWidth::Wave64
    } else {
        WavefrontWidth::Wave32
    };
    let wavefront_widths = if is_gfx6 || is_gfx7 || is_gfx8 || is_gfx9 {
        WavefrontWidths::wave64()
    } else if is_gfx125 {
        WavefrontWidths::wave32()
    } else {
        WavefrontWidths::wave32_and_wave64()
    };
    let max_lds_bytes_per_workgroup = if is_gfx6 {
        32 * 1024
    } else if matches!(bytes, b"gfx950") {
        160 * 1024
    } else if is_gfx125 {
        320 * 1024
    } else {
        64 * 1024
    };
    let matrix_instruction_sets = if matches!(bytes, b"gfx908" | b"gfx90a" | b"gfx942" | b"gfx950")
    {
        MatrixInstructionSets::MFMA_ONLY
    } else if matches!(
        bytes,
        b"gfx1100"
            | b"gfx1101"
            | b"gfx1102"
            | b"gfx1103"
            | b"gfx1150"
            | b"gfx1151"
            | b"gfx1152"
            | b"gfx1153"
            | b"gfx1154"
    ) {
        MatrixInstructionSets::WMMA256_ONLY
    } else if matches!(
        bytes,
        b"gfx1170" | b"gfx1171" | b"gfx1172" | b"gfx1200" | b"gfx1201"
    ) {
        MatrixInstructionSets::WMMA128_AND_SWMMA
    } else if is_gfx125 {
        MatrixInstructionSets::SWMMA_ONLY
    } else {
        MatrixInstructionSets::NONE
    };
    let async_copy_instruction_sets = if is_gfx9 || is_gfx10 {
        AsyncCopyInstructionSets::VMEM_TO_LDS_ONLY
    } else if is_gfx125 {
        AsyncCopyInstructionSets::ASYNC_LOAD_AND_STORE
    } else if is_gfx13 {
        AsyncCopyInstructionSets::ASYNC_LOAD_ONLY
    } else {
        AsyncCopyInstructionSets::NONE
    };

    Some(ProcessorProfile {
        default_wavefront_width,
        wavefront_widths,
        max_lds_bytes_per_workgroup,
        matrix_instruction_sets,
        async_copy_instruction_sets,
    })
}

fn validate_target_features(target: AmdTargetId) -> Result<(), CapabilityDerivationError> {
    for feature in [AmdTargetFeature::SramEcc, AmdTargetFeature::Xnack] {
        if target.feature(feature).is_some()
            && !crate::processor_supports_feature(target.processor(), feature)
        {
            return Err(CapabilityDerivationError::UnsupportedTargetFeature(feature));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn internal_profile_validation_rejects_an_invalid_default() {
        let mut profile = processor_profile("gfx942").unwrap();
        profile.default_wavefront_width = WavefrontWidth::Wave32;
        assert!(
            !profile
                .wavefront_widths
                .contains(profile.default_wavefront_width)
        );
    }
}
