//! Closed semantic Kernel IR profile for masked Wave64 collectives V1.
//!
//! Frozen generic module encodings do not yet carry reduction and scan
//! operations. This profile is therefore an explicit semantic sidecar, not a
//! hand-authored substitution hidden inside a generic [`crate::Module`]. A
//! compiler frontend may admit it only after authenticating the exact source
//! and its complete reachable MIR closure. Admission proves exact structural
//! equality only; it grants no lowering, artifact, load, or execution authority.

use std::error::Error;
use std::fmt;

use crate::{ScalarType, TargetCapability, WaveWidth, WorkgroupSize};

pub const WAVE64_COLLECTIVES_V1_MODULE_ID: &str = "fe2o3::wave64_collectives_v1";
pub const WAVE64_COLLECTIVES_V1_FUNCTION_ID: &str = "__fe2o3_wave64_collectives_v1_impl";
pub const WAVE64_COLLECTIVES_V1_KERNEL_ID: &str = "wave64_collectives_v1";
pub const WAVE64_COLLECTIVES_V1_DESCRIPTOR_SYMBOL: &str = "wave64_collectives_v1.kd";
pub const WAVE64_COLLECTIVES_V1_LANES: u32 = 64;
pub const WAVE64_COLLECTIVES_V1_EXPLICIT_KERNARG_BYTES: u32 = 72;
pub const WAVE64_COLLECTIVES_V1_COMPLETE_COV6_KERNARG_BYTES: u32 = 328;

pub const WAVE64_COLLECTIVES_V1_SOURCE_SHA256: [u8; 32] = [
    0x01, 0xac, 0x13, 0x65, 0xb0, 0xfd, 0xfe, 0x91, 0xcd, 0xc8, 0xf7, 0xcf, 0x6a, 0x14, 0xae, 0x5a,
    0xcb, 0xea, 0x41, 0x52, 0x81, 0x03, 0xec, 0x3d, 0xe5, 0xfe, 0x6d, 0x89, 0x52, 0x61, 0x62, 0x5e,
];
pub const WAVE64_COLLECTIVES_V1_NAMESPACE: [u8; 32] = [
    0x28, 0x63, 0x30, 0x4e, 0xbf, 0x7f, 0x50, 0x1a, 0x7f, 0x17, 0x7c, 0x5b, 0x8f, 0x5a, 0x45, 0x62,
    0x61, 0xee, 0x34, 0x76, 0x04, 0x72, 0x72, 0x7b, 0xa3, 0xf0, 0x20, 0x5c, 0xcf, 0x5c, 0xe9, 0xcc,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Wave64CollectiveKindV1 {
    ReduceSum,
    InclusiveScanSum,
    ExclusiveScanSum,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Wave64ArgumentRoleV1 {
    Input,
    ActiveMask,
    ReductionOutput,
    InclusiveOutput,
    ExclusiveOutput,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Wave64ArgumentShapeV1 {
    SharedReadOnlySlice64,
    Scalar,
    LaneOwnedReadWriteSlice64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Wave64ArgumentV1 {
    pub role: Wave64ArgumentRoleV1,
    pub shape: Wave64ArgumentShapeV1,
    pub scalar: ScalarType,
    pub offset: u32,
    pub size: u32,
    pub alignment: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Wave64InactivePolicyV1 {
    ContributeAndPublishPositiveZero,
    PublishCollectiveResult,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Wave64ParticipationV1 {
    AllPhysicalLanesWithLogicalU64Mask,
    DivergentLogicalParticipants,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Wave64CollectiveV1 {
    pub ordinal: u8,
    pub kind: Wave64CollectiveKindV1,
    pub scalar: ScalarType,
    pub participation: Wave64ParticipationV1,
    pub inactive_policy: Wave64InactivePolicyV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Wave64OutputOwnershipV1 {
    PhysicalLaneOwnsSameIndex,
    LaneZeroOwnsEveryIndex,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Wave64OutputV1 {
    pub argument: Wave64ArgumentRoleV1,
    pub source: Wave64CollectiveKindV1,
    pub ownership: Wave64OutputOwnershipV1,
    pub inactive_policy: Wave64InactivePolicyV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Wave64F32PolicyV1 {
    FiniteIntegralMagnitudeAtMost1024,
    FiniteOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Wave64CorrespondenceV1 {
    ReviewedExactSourceAndMirToCanonicalProfileNotRefinementProof,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Wave64DescriptorV1 {
    pub logical_name: String,
    pub export_name: String,
    pub descriptor_symbol: String,
    pub code_object_version: u8,
    pub explicit_kernarg_bytes: u32,
    pub complete_kernarg_bytes: u32,
    pub workgroup_size: WorkgroupSize,
    pub wave_width: WaveWidth,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Wave64CollectivesKernelIrV1 {
    pub module_id: String,
    pub function_id: String,
    pub kernel_id: String,
    pub arguments: Vec<Wave64ArgumentV1>,
    pub collectives: Vec<Wave64CollectiveV1>,
    pub outputs: Vec<Wave64OutputV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Wave64CollectivesProfileV1 {
    pub source_sha256: [u8; 32],
    pub namespace: [u8; 32],
    pub target: TargetCapability,
    pub code_object_version: u8,
    pub wave_width: WaveWidth,
    pub workgroup_size: WorkgroupSize,
    pub grid: [u32; 3],
    pub f32_policy: Wave64F32PolicyV1,
    pub correspondence: Wave64CorrespondenceV1,
    pub descriptor: Wave64DescriptorV1,
}

impl Wave64CollectivesProfileV1 {
    pub fn exact_gfx942_xnack_minus_cov6() -> Self {
        let workgroup_size = WorkgroupSize::new(WAVE64_COLLECTIVES_V1_LANES, 1, 1);
        Self {
            source_sha256: WAVE64_COLLECTIVES_V1_SOURCE_SHA256,
            namespace: WAVE64_COLLECTIVES_V1_NAMESPACE,
            target: crate::gfx942_xnack_minus_target_capability(),
            code_object_version: 6,
            wave_width: WaveWidth::Wave64,
            workgroup_size,
            grid: [1, 1, 1],
            f32_policy: Wave64F32PolicyV1::FiniteIntegralMagnitudeAtMost1024,
            correspondence:
                Wave64CorrespondenceV1::ReviewedExactSourceAndMirToCanonicalProfileNotRefinementProof,
            descriptor: Wave64DescriptorV1 {
                logical_name: WAVE64_COLLECTIVES_V1_KERNEL_ID.to_owned(),
                export_name: WAVE64_COLLECTIVES_V1_KERNEL_ID.to_owned(),
                descriptor_symbol: WAVE64_COLLECTIVES_V1_DESCRIPTOR_SYMBOL.to_owned(),
                code_object_version: 6,
                explicit_kernarg_bytes: WAVE64_COLLECTIVES_V1_EXPLICIT_KERNARG_BYTES,
                complete_kernarg_bytes: WAVE64_COLLECTIVES_V1_COMPLETE_COV6_KERNARG_BYTES,
                workgroup_size,
                wave_width: WaveWidth::Wave64,
            },
        }
    }

    pub fn is_exact(&self) -> bool {
        self == &Self::exact_gfx942_xnack_minus_cov6()
    }
}

pub fn wave64_collectives_v1_kernel_ir() -> Wave64CollectivesKernelIrV1 {
    Wave64CollectivesKernelIrV1 {
        module_id: WAVE64_COLLECTIVES_V1_MODULE_ID.to_owned(),
        function_id: WAVE64_COLLECTIVES_V1_FUNCTION_ID.to_owned(),
        kernel_id: WAVE64_COLLECTIVES_V1_KERNEL_ID.to_owned(),
        arguments: vec![
            argument(
                Wave64ArgumentRoleV1::Input,
                Wave64ArgumentShapeV1::SharedReadOnlySlice64,
                ScalarType::F32,
                0,
                16,
            ),
            argument(
                Wave64ArgumentRoleV1::ActiveMask,
                Wave64ArgumentShapeV1::Scalar,
                ScalarType::U64,
                16,
                8,
            ),
            argument(
                Wave64ArgumentRoleV1::ReductionOutput,
                Wave64ArgumentShapeV1::LaneOwnedReadWriteSlice64,
                ScalarType::F32,
                24,
                16,
            ),
            argument(
                Wave64ArgumentRoleV1::InclusiveOutput,
                Wave64ArgumentShapeV1::LaneOwnedReadWriteSlice64,
                ScalarType::F32,
                40,
                16,
            ),
            argument(
                Wave64ArgumentRoleV1::ExclusiveOutput,
                Wave64ArgumentShapeV1::LaneOwnedReadWriteSlice64,
                ScalarType::F32,
                56,
                16,
            ),
        ],
        collectives: vec![
            collective(0, Wave64CollectiveKindV1::ReduceSum),
            collective(1, Wave64CollectiveKindV1::InclusiveScanSum),
            collective(2, Wave64CollectiveKindV1::ExclusiveScanSum),
        ],
        outputs: vec![
            output(
                Wave64ArgumentRoleV1::ReductionOutput,
                Wave64CollectiveKindV1::ReduceSum,
            ),
            output(
                Wave64ArgumentRoleV1::InclusiveOutput,
                Wave64CollectiveKindV1::InclusiveScanSum,
            ),
            output(
                Wave64ArgumentRoleV1::ExclusiveOutput,
                Wave64CollectiveKindV1::ExclusiveScanSum,
            ),
        ],
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Wave64CollectivesV1Error {
    UnsupportedProfile,
    NonCanonicalKernelIr,
}

impl fmt::Display for Wave64CollectivesV1Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedProfile => formatter.write_str(
                "Wave64 collectives V1 requires the exact source digest/namespace, gfx942:xnack-, COV6, wave64, WG64, one-workgroup grid, strict integral-f32 policy, ABI, effects, and descriptor",
            ),
            Self::NonCanonicalKernelIr => formatter.write_str(
                "semantic Kernel IR differs from the canonical masked Wave64 reduce/inclusive-scan/exclusive-scan profile",
            ),
        }
    }
}

impl Error for Wave64CollectivesV1Error {}

pub fn verify_wave64_collectives_v1(
    ir: &Wave64CollectivesKernelIrV1,
    profile: &Wave64CollectivesProfileV1,
) -> Result<(), Wave64CollectivesV1Error> {
    if !profile.is_exact() {
        return Err(Wave64CollectivesV1Error::UnsupportedProfile);
    }
    if ir != &wave64_collectives_v1_kernel_ir() {
        return Err(Wave64CollectivesV1Error::NonCanonicalKernelIr);
    }
    Ok(())
}

fn argument(
    role: Wave64ArgumentRoleV1,
    shape: Wave64ArgumentShapeV1,
    scalar: ScalarType,
    offset: u32,
    size: u32,
) -> Wave64ArgumentV1 {
    Wave64ArgumentV1 {
        role,
        shape,
        scalar,
        offset,
        size,
        alignment: 8,
    }
}

fn collective(ordinal: u8, kind: Wave64CollectiveKindV1) -> Wave64CollectiveV1 {
    Wave64CollectiveV1 {
        ordinal,
        kind,
        scalar: ScalarType::F32,
        participation: Wave64ParticipationV1::AllPhysicalLanesWithLogicalU64Mask,
        inactive_policy: Wave64InactivePolicyV1::ContributeAndPublishPositiveZero,
    }
}

fn output(argument: Wave64ArgumentRoleV1, source: Wave64CollectiveKindV1) -> Wave64OutputV1 {
    Wave64OutputV1 {
        argument,
        source,
        ownership: Wave64OutputOwnershipV1::PhysicalLaneOwnsSameIndex,
        inactive_policy: Wave64InactivePolicyV1::ContributeAndPublishPositiveZero,
    }
}
