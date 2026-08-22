//! Closed semantic Kernel IR sidecar for FlashAttention Phase A V1.
//!
//! The generic module encodings do not model this fused recurrence. A compiler
//! frontend may select this sidecar only after authenticating the exact source,
//! ABI, providers, and complete reachable portable MIR modulo a reviewed
//! semantic-terminal manifest. This is reviewed correspondence, not a
//! compiler-refinement proof or executable authority.

use std::error::Error;
use std::fmt;

use crate::{ScalarType, TargetCapability, WaveWidth, WorkgroupSize};

pub const FLASH_ATTENTION_V1_MODULE_ID: &str = "fe2o3::flash_attention_causal_f32_v1";
pub const FLASH_ATTENTION_V1_FUNCTION_ID: &str =
    "__fe2o3_flash_attention_causal_f32_b1_h1_n8_d16_v1_impl";
pub const FLASH_ATTENTION_V1_KERNEL_ID: &str = "flash_attention_causal_f32_b1_h1_n8_d16_v1";
pub const FLASH_ATTENTION_V1_DESCRIPTOR_SYMBOL: &str =
    "flash_attention_causal_f32_b1_h1_n8_d16_v1.kd";
pub const FLASH_ATTENTION_V1_EXPLICIT_KERNARG_BYTES: u32 = 64;
pub const FLASH_ATTENTION_V1_COMPLETE_COV6_KERNARG_BYTES: u32 = 320;
pub const FLASH_ATTENTION_V1_SOURCE_SHA256: [u8; 32] = [
    0x6d, 0xba, 0xa2, 0xaf, 0x88, 0xfd, 0x5e, 0xdc, 0xdf, 0x04, 0x85, 0xf3, 0xda, 0x47, 0xb1, 0x31,
    0x9c, 0xe2, 0x99, 0x42, 0x2a, 0x77, 0xb9, 0x9a, 0xf5, 0x6f, 0x9a, 0x3e, 0x77, 0xc2, 0xa4, 0x21,
];
pub const FLASH_ATTENTION_V1_NAMESPACE: [u8; 32] = [
    0x4d, 0xfe, 0x87, 0x0b, 0xb7, 0x6d, 0xd3, 0x2b, 0x49, 0x14, 0x4e, 0xe7, 0x0e, 0xc4, 0x92, 0x5e,
    0xab, 0x86, 0x77, 0xb7, 0xcb, 0xd1, 0xa1, 0xbf, 0xe9, 0x9f, 0xa2, 0x29, 0x4f, 0x85, 0xfe, 0xc8,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlashAttentionArgumentRoleV1 {
    Query,
    Key,
    Value,
    Output,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlashAttentionArgumentShapeV1 {
    SharedReadOnlyContiguousF32x128,
    LaneOwnedReadWriteContiguousF32x128,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FlashAttentionArgumentV1 {
    pub role: FlashAttentionArgumentRoleV1,
    pub shape: FlashAttentionArgumentShapeV1,
    pub scalar: ScalarType,
    pub offset: u32,
    pub size: u32,
    pub alignment: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FlashAttentionShapeV1 {
    pub batches: u8,
    pub heads: u8,
    pub sequence_length: u8,
    pub head_dimension: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlashAttentionLayoutV1 {
    RowMajorContiguous,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlashAttentionMaskV1 {
    CausalLowerTriangleDiagonalIncluded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlashAttentionArithmeticV1 {
    StrictSequentialF32NoContraction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlashAttentionExceptionalPolicyV1 {
    FiniteInputsAndIntermediatesOrTrapBeforeOwnedWrites,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlashAttentionRecurrenceStepV1 {
    SequentialDotD16,
    ScaleByExactF32Bits(u32),
    FirstKeyInitializesMaxSumAndNumerator,
    NextMax,
    PreviousWeightExp,
    CurrentWeightExp,
    RescaleDenominator,
    RescaleNumeratorPair,
    CommitMaximum,
    DivideNumeratorPairByDenominator,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlashAttentionOutputOwnershipV1 {
    PhysicalLaneOwnsAdjacentPair,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FlashAttentionOwnershipV1 {
    pub policy: FlashAttentionOutputOwnershipV1,
    pub physical_lanes: u8,
    pub elements_per_lane: u8,
    pub output_elements: u16,
    pub total: bool,
    pub injective: bool,
    pub in_bounds: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FlashAttentionResourcesV1 {
    pub static_lds_bytes: u32,
    pub required_dynamic_lds_bytes: u32,
    pub maximum_dynamic_lds_bytes: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FlashAttentionDescriptorV1 {
    pub logical_name: String,
    pub export_name: String,
    pub descriptor_symbol: String,
    pub code_object_version: u8,
    pub explicit_kernarg_bytes: u32,
    pub complete_kernarg_bytes: u32,
    pub workgroup_size: WorkgroupSize,
    pub wave_width: WaveWidth,
    pub resources: FlashAttentionResourcesV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlashAttentionCorrespondenceV1 {
    ReviewedExactSourceAndMirToCanonicalProfileNotRefinementProof,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FlashAttentionKernelIrV1 {
    pub module_id: String,
    pub function_id: String,
    pub kernel_id: String,
    pub arguments: [FlashAttentionArgumentV1; 4],
    pub shape: FlashAttentionShapeV1,
    pub layout: FlashAttentionLayoutV1,
    pub mask: FlashAttentionMaskV1,
    pub arithmetic: FlashAttentionArithmeticV1,
    pub exceptional_policy: FlashAttentionExceptionalPolicyV1,
    pub recurrence: [FlashAttentionRecurrenceStepV1; 10],
    pub ownership: FlashAttentionOwnershipV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FlashAttentionProfileV1 {
    pub source_sha256: [u8; 32],
    pub namespace: [u8; 32],
    pub target: TargetCapability,
    pub code_object_version: u8,
    pub wave_width: WaveWidth,
    pub workgroup_size: WorkgroupSize,
    pub grid: [u32; 3],
    pub correspondence: FlashAttentionCorrespondenceV1,
    pub descriptor: FlashAttentionDescriptorV1,
}

impl FlashAttentionProfileV1 {
    pub fn exact_gfx942_xnack_minus_cov6() -> Self {
        let workgroup_size = WorkgroupSize::new(64, 1, 1);
        Self {
            source_sha256: FLASH_ATTENTION_V1_SOURCE_SHA256,
            namespace: FLASH_ATTENTION_V1_NAMESPACE,
            target: crate::gfx942_xnack_minus_target_capability(),
            code_object_version: 6,
            wave_width: WaveWidth::Wave64,
            workgroup_size,
            grid: [1, 1, 1],
            correspondence:
                FlashAttentionCorrespondenceV1::ReviewedExactSourceAndMirToCanonicalProfileNotRefinementProof,
            descriptor: FlashAttentionDescriptorV1 {
                logical_name: FLASH_ATTENTION_V1_KERNEL_ID.to_owned(),
                export_name: FLASH_ATTENTION_V1_KERNEL_ID.to_owned(),
                descriptor_symbol: FLASH_ATTENTION_V1_DESCRIPTOR_SYMBOL.to_owned(),
                code_object_version: 6,
                explicit_kernarg_bytes: FLASH_ATTENTION_V1_EXPLICIT_KERNARG_BYTES,
                complete_kernarg_bytes: FLASH_ATTENTION_V1_COMPLETE_COV6_KERNARG_BYTES,
                workgroup_size,
                wave_width: WaveWidth::Wave64,
                resources: FlashAttentionResourcesV1 {
                    static_lds_bytes: 0,
                    required_dynamic_lds_bytes: 0,
                    maximum_dynamic_lds_bytes: 0,
                },
            },
        }
    }

    pub fn is_exact(&self) -> bool {
        self == &Self::exact_gfx942_xnack_minus_cov6()
    }
}

pub fn flash_attention_v1_kernel_ir() -> FlashAttentionKernelIrV1 {
    FlashAttentionKernelIrV1 {
        module_id: FLASH_ATTENTION_V1_MODULE_ID.to_owned(),
        function_id: FLASH_ATTENTION_V1_FUNCTION_ID.to_owned(),
        kernel_id: FLASH_ATTENTION_V1_KERNEL_ID.to_owned(),
        arguments: [
            argument(
                FlashAttentionArgumentRoleV1::Query,
                FlashAttentionArgumentShapeV1::SharedReadOnlyContiguousF32x128,
                0,
            ),
            argument(
                FlashAttentionArgumentRoleV1::Key,
                FlashAttentionArgumentShapeV1::SharedReadOnlyContiguousF32x128,
                16,
            ),
            argument(
                FlashAttentionArgumentRoleV1::Value,
                FlashAttentionArgumentShapeV1::SharedReadOnlyContiguousF32x128,
                32,
            ),
            argument(
                FlashAttentionArgumentRoleV1::Output,
                FlashAttentionArgumentShapeV1::LaneOwnedReadWriteContiguousF32x128,
                48,
            ),
        ],
        shape: FlashAttentionShapeV1 {
            batches: 1,
            heads: 1,
            sequence_length: 8,
            head_dimension: 16,
        },
        layout: FlashAttentionLayoutV1::RowMajorContiguous,
        mask: FlashAttentionMaskV1::CausalLowerTriangleDiagonalIncluded,
        arithmetic: FlashAttentionArithmeticV1::StrictSequentialF32NoContraction,
        exceptional_policy:
            FlashAttentionExceptionalPolicyV1::FiniteInputsAndIntermediatesOrTrapBeforeOwnedWrites,
        recurrence: [
            FlashAttentionRecurrenceStepV1::SequentialDotD16,
            FlashAttentionRecurrenceStepV1::ScaleByExactF32Bits(0x3e80_0000),
            FlashAttentionRecurrenceStepV1::FirstKeyInitializesMaxSumAndNumerator,
            FlashAttentionRecurrenceStepV1::NextMax,
            FlashAttentionRecurrenceStepV1::PreviousWeightExp,
            FlashAttentionRecurrenceStepV1::CurrentWeightExp,
            FlashAttentionRecurrenceStepV1::RescaleDenominator,
            FlashAttentionRecurrenceStepV1::RescaleNumeratorPair,
            FlashAttentionRecurrenceStepV1::CommitMaximum,
            FlashAttentionRecurrenceStepV1::DivideNumeratorPairByDenominator,
        ],
        ownership: FlashAttentionOwnershipV1 {
            policy: FlashAttentionOutputOwnershipV1::PhysicalLaneOwnsAdjacentPair,
            physical_lanes: 64,
            elements_per_lane: 2,
            output_elements: 128,
            total: true,
            injective: true,
            in_bounds: true,
        },
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FlashAttentionV1Error {
    UnsupportedProfile,
    NonCanonicalKernelIr,
}

impl fmt::Display for FlashAttentionV1Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedProfile => formatter.write_str(
                "FlashAttention V1 requires the exact source/namespace, gfx942:xnack-, COV6, Wave64, WG64, one-workgroup grid, four-argument ABI, and zero-LDS resources",
            ),
            Self::NonCanonicalKernelIr => formatter.write_str(
                "FlashAttention semantic Kernel IR differs from the exact B1/H1/N8/D16 causal strict-FP32 online-recurrence sidecar",
            ),
        }
    }
}

impl Error for FlashAttentionV1Error {}

pub fn verify_flash_attention_v1(
    ir: &FlashAttentionKernelIrV1,
    profile: &FlashAttentionProfileV1,
) -> Result<(), FlashAttentionV1Error> {
    if !profile.is_exact() {
        return Err(FlashAttentionV1Error::UnsupportedProfile);
    }
    if ir != &flash_attention_v1_kernel_ir() {
        return Err(FlashAttentionV1Error::NonCanonicalKernelIr);
    }
    Ok(())
}

const fn argument(
    role: FlashAttentionArgumentRoleV1,
    shape: FlashAttentionArgumentShapeV1,
    offset: u32,
) -> FlashAttentionArgumentV1 {
    FlashAttentionArgumentV1 {
        role,
        shape,
        scalar: ScalarType::F32,
        offset,
        size: 16,
        alignment: 8,
    }
}
