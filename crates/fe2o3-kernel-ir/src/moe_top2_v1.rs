//! Closed semantic Kernel IR sidecar for deterministic MoE top-2 routing V1.
//!
//! A compiler frontend may select this inert sidecar only after authenticating
//! the exact attributed source, ABI, provider closure, and complete reachable
//! portable MIR modulo an explicit reviewed semantic-terminal manifest. This
//! is reviewed correspondence, not generic lowering, IEEE-754 refinement, or
//! source-to-model/source-to-Verus refinement authority.

use std::error::Error;
use std::fmt;

use crate::{ScalarType, TargetCapability, WaveWidth, WorkgroupSize};

pub const MOE_TOP2_V1_MODULE_ID: &str = "fe2o3::moe_top2_route_f32_t8_e4_k2_c4_v1";
pub const MOE_TOP2_V1_FUNCTION_ID: &str = "__fe2o3_moe_top2_route_f32_t8_e4_k2_c4_v1_impl";
pub const MOE_TOP2_V1_KERNEL_ID: &str = "moe_top2_route_f32_t8_e4_k2_c4_v1";
pub const MOE_TOP2_V1_DESCRIPTOR_SYMBOL: &str = "moe_top2_route_f32_t8_e4_k2_c4_v1.kd";
pub const MOE_TOP2_V1_EXPLICIT_KERNARG_BYTES: u32 = 128;
pub const MOE_TOP2_V1_COMPLETE_COV6_KERNARG_BYTES: u32 = 384;
pub const MOE_TOP2_V1_DROP_SENTINEL: u32 = u32::MAX;
pub const MOE_TOP2_V1_SOURCE_SHA256: [u8; 32] = [
    0x0e, 0x45, 0x70, 0xbd, 0x52, 0x86, 0x6d, 0xd2, 0x3b, 0x8b, 0x00, 0xd8, 0x39, 0x83, 0xaa, 0xdc,
    0x81, 0x8c, 0x77, 0x58, 0x0d, 0xe8, 0xf7, 0xf5, 0xe2, 0x98, 0x2e, 0x12, 0xa5, 0x7e, 0x20, 0xe2,
];
pub const MOE_TOP2_V1_NAMESPACE: [u8; 32] = [
    0x41, 0x80, 0xef, 0x61, 0x54, 0x56, 0x84, 0xe6, 0x46, 0xbd, 0x52, 0x27, 0x33, 0x3e, 0x75, 0x14,
    0xd2, 0x2a, 0x2d, 0x37, 0x9d, 0x7d, 0x65, 0x73, 0x97, 0xdf, 0x4d, 0x41, 0xf7, 0xa1, 0x92, 0xd1,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoeTop2ArgumentRoleV1 {
    Logits,
    Top2Experts,
    RequestedCounts,
    AdmittedCounts,
    ExpertOffsets,
    RouteSlots,
    Permutation,
    Inverse,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoeTop2ArgumentShapeV1 {
    SharedReadOnlyContiguousF32x32,
    LaneZeroOwnedReadWriteContiguousU32x16,
    LaneZeroOwnedReadWriteContiguousU32x4,
    LaneZeroOwnedReadWriteContiguousU32x5,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MoeTop2ArgumentV1 {
    pub role: MoeTop2ArgumentRoleV1,
    pub shape: MoeTop2ArgumentShapeV1,
    pub scalar: ScalarType,
    pub offset: u32,
    pub size: u32,
    pub alignment: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MoeTop2ShapeV1 {
    pub tokens: u8,
    pub experts: u8,
    pub experts_per_token: u8,
    pub expert_capacity: u8,
    pub logits: u8,
    pub routes: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoeTop2LayoutV1 {
    TokenMajorLogitsAndTokenThenRankRoutes,
    ExpertMajorLogitsUnsupported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoeTop2FiniteInputPolicyV1 {
    AllLogitsFiniteOrTrapBeforeAnyOutputWrite,
    NonFiniteInputsUnsupported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoeTop2TieBreakV1 {
    HigherFiniteF32ScoreThenLowerExpertId,
    HigherExpertIdTieBreakUnsupported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoeTop2OverflowV1 {
    StableRoutePrefixPerExpertDropAfterCapacity,
    ReplaceAcceptedRouteUnsupported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoeTop2RoutingStepV1 {
    ValidateExactExtentsAndFiniteInputsBeforeWrites,
    SelectDistinctTop2DescendingScoreLowerExpertTie,
    CountRequestedRoutesInTokenThenRankOrder,
    ClampAdmittedCountsToCapacityFour,
    ExclusiveScanAdmittedCountsInExpertOrder,
    InitializeSlotsPermutationAndInverseToSentinel,
    ComputeStableRankInIncreasingRouteOrder,
    AssignUniqueBoundedSlotFromExpertOffsetAndStableRank,
    EstablishPermutationAndInverseRoundTrip,
    CommitEveryOutputOnceFromLaneZero,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MoeTop2PackingSemanticsV1 {
    pub requested_counts_exact: bool,
    pub admitted_is_requested_min_capacity: bool,
    pub offsets_are_exclusive_scan: bool,
    pub offsets_start_at_zero: bool,
    pub accepted_slots_unique: bool,
    pub accepted_slots_bounded_by_total_admitted: bool,
    pub permutation_inverse_round_trip: bool,
    pub dropped_slot_and_inverse_are_sentinel: bool,
    pub unused_permutation_tail_is_sentinel: bool,
    pub sentinel: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoeTop2OutputOwnershipPolicyV1 {
    PhysicalLaneZeroOwnsAllOutputElementsOtherLanesInactive,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MoeTop2OutputOwnershipV1 {
    pub policy: MoeTop2OutputOwnershipPolicyV1,
    pub physical_lanes: u8,
    pub active_lanes: u8,
    pub output_lengths: [u8; 7],
    pub every_output_element_written_once: bool,
    pub output_arguments_exclusive: bool,
    pub writes_in_bounds: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MoeTop2ResourcesV1 {
    pub static_lds_bytes: u32,
    pub required_dynamic_lds_bytes: u32,
    pub maximum_dynamic_lds_bytes: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MoeTop2DescriptorV1 {
    pub logical_name: String,
    pub export_name: String,
    pub descriptor_symbol: String,
    pub code_object_version: u8,
    pub explicit_kernarg_bytes: u32,
    pub complete_kernarg_bytes: u32,
    pub workgroup_size: WorkgroupSize,
    pub wave_width: WaveWidth,
    pub resources: MoeTop2ResourcesV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoeTop2CorrespondenceV1 {
    ReviewedExactSourceAndMirToCanonicalProfileNotRefinementProof,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MoeTop2KernelIrV1 {
    pub module_id: String,
    pub function_id: String,
    pub kernel_id: String,
    pub arguments: [MoeTop2ArgumentV1; 8],
    pub shape: MoeTop2ShapeV1,
    pub layout: MoeTop2LayoutV1,
    pub finite_input: MoeTop2FiniteInputPolicyV1,
    pub tie_break: MoeTop2TieBreakV1,
    pub overflow: MoeTop2OverflowV1,
    pub routing: [MoeTop2RoutingStepV1; 10],
    pub packing: MoeTop2PackingSemanticsV1,
    pub ownership: MoeTop2OutputOwnershipV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MoeTop2ProfileV1 {
    pub source_sha256: [u8; 32],
    pub namespace: [u8; 32],
    pub target: TargetCapability,
    pub code_object_version: u8,
    pub wave_width: WaveWidth,
    pub workgroup_size: WorkgroupSize,
    pub grid: [u32; 3],
    pub correspondence: MoeTop2CorrespondenceV1,
    pub descriptor: MoeTop2DescriptorV1,
}

impl MoeTop2ProfileV1 {
    pub fn exact_gfx942_xnack_minus_cov6() -> Self {
        let workgroup_size = WorkgroupSize::new(64, 1, 1);
        Self {
            source_sha256: MOE_TOP2_V1_SOURCE_SHA256,
            namespace: MOE_TOP2_V1_NAMESPACE,
            target: crate::gfx942_xnack_minus_target_capability(),
            code_object_version: 6,
            wave_width: WaveWidth::Wave64,
            workgroup_size,
            grid: [1, 1, 1],
            correspondence:
                MoeTop2CorrespondenceV1::ReviewedExactSourceAndMirToCanonicalProfileNotRefinementProof,
            descriptor: MoeTop2DescriptorV1 {
                logical_name: MOE_TOP2_V1_KERNEL_ID.to_owned(),
                export_name: MOE_TOP2_V1_KERNEL_ID.to_owned(),
                descriptor_symbol: MOE_TOP2_V1_DESCRIPTOR_SYMBOL.to_owned(),
                code_object_version: 6,
                explicit_kernarg_bytes: MOE_TOP2_V1_EXPLICIT_KERNARG_BYTES,
                complete_kernarg_bytes: MOE_TOP2_V1_COMPLETE_COV6_KERNARG_BYTES,
                workgroup_size,
                wave_width: WaveWidth::Wave64,
                resources: MoeTop2ResourcesV1 {
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

pub fn moe_top2_v1_kernel_ir() -> MoeTop2KernelIrV1 {
    MoeTop2KernelIrV1 {
        module_id: MOE_TOP2_V1_MODULE_ID.to_owned(),
        function_id: MOE_TOP2_V1_FUNCTION_ID.to_owned(),
        kernel_id: MOE_TOP2_V1_KERNEL_ID.to_owned(),
        arguments: [
            argument(
                MoeTop2ArgumentRoleV1::Logits,
                MoeTop2ArgumentShapeV1::SharedReadOnlyContiguousF32x32,
                ScalarType::F32,
                0,
            ),
            argument(
                MoeTop2ArgumentRoleV1::Top2Experts,
                MoeTop2ArgumentShapeV1::LaneZeroOwnedReadWriteContiguousU32x16,
                ScalarType::U32,
                16,
            ),
            argument(
                MoeTop2ArgumentRoleV1::RequestedCounts,
                MoeTop2ArgumentShapeV1::LaneZeroOwnedReadWriteContiguousU32x4,
                ScalarType::U32,
                32,
            ),
            argument(
                MoeTop2ArgumentRoleV1::AdmittedCounts,
                MoeTop2ArgumentShapeV1::LaneZeroOwnedReadWriteContiguousU32x4,
                ScalarType::U32,
                48,
            ),
            argument(
                MoeTop2ArgumentRoleV1::ExpertOffsets,
                MoeTop2ArgumentShapeV1::LaneZeroOwnedReadWriteContiguousU32x5,
                ScalarType::U32,
                64,
            ),
            argument(
                MoeTop2ArgumentRoleV1::RouteSlots,
                MoeTop2ArgumentShapeV1::LaneZeroOwnedReadWriteContiguousU32x16,
                ScalarType::U32,
                80,
            ),
            argument(
                MoeTop2ArgumentRoleV1::Permutation,
                MoeTop2ArgumentShapeV1::LaneZeroOwnedReadWriteContiguousU32x16,
                ScalarType::U32,
                96,
            ),
            argument(
                MoeTop2ArgumentRoleV1::Inverse,
                MoeTop2ArgumentShapeV1::LaneZeroOwnedReadWriteContiguousU32x16,
                ScalarType::U32,
                112,
            ),
        ],
        shape: MoeTop2ShapeV1 {
            tokens: 8,
            experts: 4,
            experts_per_token: 2,
            expert_capacity: 4,
            logits: 32,
            routes: 16,
        },
        layout: MoeTop2LayoutV1::TokenMajorLogitsAndTokenThenRankRoutes,
        finite_input: MoeTop2FiniteInputPolicyV1::AllLogitsFiniteOrTrapBeforeAnyOutputWrite,
        tie_break: MoeTop2TieBreakV1::HigherFiniteF32ScoreThenLowerExpertId,
        overflow: MoeTop2OverflowV1::StableRoutePrefixPerExpertDropAfterCapacity,
        routing: [
            MoeTop2RoutingStepV1::ValidateExactExtentsAndFiniteInputsBeforeWrites,
            MoeTop2RoutingStepV1::SelectDistinctTop2DescendingScoreLowerExpertTie,
            MoeTop2RoutingStepV1::CountRequestedRoutesInTokenThenRankOrder,
            MoeTop2RoutingStepV1::ClampAdmittedCountsToCapacityFour,
            MoeTop2RoutingStepV1::ExclusiveScanAdmittedCountsInExpertOrder,
            MoeTop2RoutingStepV1::InitializeSlotsPermutationAndInverseToSentinel,
            MoeTop2RoutingStepV1::ComputeStableRankInIncreasingRouteOrder,
            MoeTop2RoutingStepV1::AssignUniqueBoundedSlotFromExpertOffsetAndStableRank,
            MoeTop2RoutingStepV1::EstablishPermutationAndInverseRoundTrip,
            MoeTop2RoutingStepV1::CommitEveryOutputOnceFromLaneZero,
        ],
        packing: MoeTop2PackingSemanticsV1 {
            requested_counts_exact: true,
            admitted_is_requested_min_capacity: true,
            offsets_are_exclusive_scan: true,
            offsets_start_at_zero: true,
            accepted_slots_unique: true,
            accepted_slots_bounded_by_total_admitted: true,
            permutation_inverse_round_trip: true,
            dropped_slot_and_inverse_are_sentinel: true,
            unused_permutation_tail_is_sentinel: true,
            sentinel: MOE_TOP2_V1_DROP_SENTINEL,
        },
        ownership: MoeTop2OutputOwnershipV1 {
            policy: MoeTop2OutputOwnershipPolicyV1::PhysicalLaneZeroOwnsAllOutputElementsOtherLanesInactive,
            physical_lanes: 64,
            active_lanes: 1,
            output_lengths: [16, 4, 4, 5, 16, 16, 16],
            every_output_element_written_once: true,
            output_arguments_exclusive: true,
            writes_in_bounds: true,
        },
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MoeTop2V1Error {
    UnsupportedProfile,
    NonCanonicalKernelIr,
}

impl fmt::Display for MoeTop2V1Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedProfile => formatter.write_str(
                "MoE top-2 V1 requires the exact source/namespace, gfx942:xnack-, COV6, Wave64, WG64, one-workgroup grid, eight-argument ABI, and zero-LDS resources",
            ),
            Self::NonCanonicalKernelIr => formatter.write_str(
                "MoE top-2 semantic Kernel IR differs from the exact finite-input T8/E4/K2/C4 deterministic stable-capacity routing sidecar",
            ),
        }
    }
}

impl Error for MoeTop2V1Error {}

pub fn verify_moe_top2_v1(
    ir: &MoeTop2KernelIrV1,
    profile: &MoeTop2ProfileV1,
) -> Result<(), MoeTop2V1Error> {
    if !profile.is_exact() {
        return Err(MoeTop2V1Error::UnsupportedProfile);
    }
    if ir != &moe_top2_v1_kernel_ir() {
        return Err(MoeTop2V1Error::NonCanonicalKernelIr);
    }
    Ok(())
}

const fn argument(
    role: MoeTop2ArgumentRoleV1,
    shape: MoeTop2ArgumentShapeV1,
    scalar: ScalarType,
    offset: u32,
) -> MoeTop2ArgumentV1 {
    MoeTop2ArgumentV1 {
        role,
        shape,
        scalar,
        offset,
        size: 16,
        alignment: 8,
    }
}
