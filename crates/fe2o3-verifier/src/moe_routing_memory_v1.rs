//! Exact logical memory/effect contract for MoE routing T8/E4/K2/C4.
//!
//! This is an exhaustive checker for one finite logical source model. It is
//! not proof evidence and does not connect Rust indices to LLVM or ISA
//! addresses. The expected Verus values are inert, copyable descriptors that
//! authenticate nothing and grant no artifact, load, launch, or GPU authority.

use core::fmt;

pub const MOE_ROUTING_MEMORY_TOKENS_V1: usize = 8;
pub const MOE_ROUTING_MEMORY_EXPERTS_V1: usize = 4;
pub const MOE_ROUTING_MEMORY_TOP_K_V1: usize = 2;
pub const MOE_ROUTING_MEMORY_CAPACITY_V1: usize = 4;
pub const MOE_ROUTING_MEMORY_ROUTES_V1: usize = 16;
pub const MOE_ROUTING_MEMORY_LANES_V1: usize = 64;
pub const MOE_ROUTING_MEMORY_GLOBAL_ADDRESS_SPACE_V1: u8 = 1;
pub const MOE_ROUTING_MEMORY_ACCESS_WIDTH_V1: u8 = 4;
pub const MOE_ROUTING_MEMORY_DROP_ROUTE_V1: u32 = u32::MAX;

pub const MOE_ROUTING_MEMORY_SOURCE_IDENTITY_V1: [u8; 32] = [
    0x0e, 0x45, 0x70, 0xbd, 0x52, 0x86, 0x6d, 0xd2, 0x3b, 0x8b, 0x00, 0xd8, 0x39, 0x83, 0xaa, 0xdc,
    0x81, 0x8c, 0x77, 0x58, 0x0d, 0xe8, 0xf7, 0xf5, 0xe2, 0x98, 0x2e, 0x12, 0xa5, 0x7e, 0x20, 0xe2,
];
pub const MOE_ROUTING_MEMORY_PROFILE_IDENTITY_V1: [u8; 32] = [
    0x41, 0x80, 0xef, 0x61, 0x54, 0x56, 0x84, 0xe6, 0x46, 0xbd, 0x52, 0x27, 0x33, 0x3e, 0x75, 0x14,
    0xd2, 0x2a, 0x2d, 0x37, 0x9d, 0x7d, 0x65, 0x73, 0x97, 0xdf, 0x4d, 0x41, 0xf7, 0xa1, 0x92, 0xd1,
];
pub const MOE_ROUTING_MEMORY_KIR_IDENTITY_V1: [u8; 32] = [
    0x3d, 0xfa, 0x5d, 0xb9, 0x17, 0x62, 0x40, 0x31, 0x06, 0xe7, 0xd3, 0xa1, 0x58, 0x17, 0x00, 0xb1,
    0xd0, 0x32, 0x82, 0xf5, 0xdd, 0x15, 0x72, 0x77, 0x61, 0xe5, 0xcc, 0x42, 0xc6, 0x37, 0x31, 0xb2,
];
pub const MOE_ROUTING_MEMORY_DESCRIPTOR_IDENTITY_V1: [u8; 32] = [
    0x78, 0x52, 0x33, 0x4c, 0x9d, 0x38, 0xcd, 0x45, 0x44, 0xc5, 0x35, 0x37, 0x76, 0x50, 0x55, 0x43,
    0x44, 0xe8, 0xe5, 0x9d, 0xe2, 0xdc, 0x82, 0x2f, 0x4f, 0x24, 0x92, 0xdf, 0xea, 0x99, 0x87, 0x43,
];
pub const MOE_ROUTING_MEMORY_LAUNCH_IDENTITY_V1: [u8; 32] = [
    0x10, 0x0b, 0xc4, 0x9f, 0x34, 0x62, 0x74, 0x85, 0xa9, 0x59, 0xb7, 0x20, 0x1a, 0x23, 0x8b, 0xbf,
    0x84, 0x21, 0xdf, 0x80, 0x0d, 0x7f, 0x10, 0x28, 0xbb, 0xff, 0xf6, 0xbd, 0x8c, 0x51, 0xed, 0xd1,
];
pub const MOE_ROUTING_MEMORY_EFFECTS_IDENTITY_V1: [u8; 32] = [
    0x49, 0x63, 0x68, 0xf7, 0x0c, 0x21, 0x1b, 0x00, 0x14, 0x17, 0xfb, 0x90, 0x46, 0x22, 0x97, 0x1d,
    0x00, 0x8c, 0xa2, 0x44, 0x42, 0xbe, 0xae, 0xf3, 0xe4, 0xc6, 0xc1, 0x75, 0xb4, 0xf5, 0xf6, 0xba,
];
pub const MOE_ROUTING_MEMORY_ROUTING_IDENTITY_V1: [u8; 32] = [
    0xa9, 0x4a, 0x13, 0xc1, 0xad, 0x0a, 0xc1, 0x49, 0x8e, 0x1c, 0x6c, 0xc6, 0x34, 0x16, 0xdc, 0x1c,
    0xda, 0x2f, 0x7c, 0x14, 0xc5, 0xe4, 0xc1, 0xc4, 0x22, 0xe3, 0x54, 0x82, 0x0f, 0xc0, 0x93, 0x15,
];
pub const MOE_ROUTING_MEMORY_PUBLISHED_MACHINE_BODY_IDENTITY_V1: [u8; 32] = [
    0x47, 0x28, 0x02, 0x8b, 0x85, 0xcc, 0x3f, 0xf4, 0x07, 0x19, 0x0d, 0xe6, 0xa7, 0x0b, 0x9c, 0x84,
    0x44, 0x37, 0xe9, 0xf9, 0x2f, 0xc5, 0x87, 0xe0, 0x61, 0x49, 0x40, 0xbe, 0x89, 0x83, 0x46, 0xcf,
];
pub const MOE_ROUTING_MEMORY_ANALYZER_PROFILE_IDENTITY_V1: [u8; 32] = [
    0x40, 0xbe, 0xa5, 0x76, 0xeb, 0x92, 0xb0, 0xa1, 0x96, 0x91, 0x4b, 0xf5, 0x44, 0xf3, 0x77, 0x0d,
    0x2b, 0x07, 0x57, 0xe3, 0x79, 0xe5, 0x34, 0x2c, 0x95, 0x6c, 0xb2, 0x00, 0xb4, 0x45, 0x40, 0x51,
];
pub const MOE_ROUTING_MEMORY_VERUS_PROOF_IDENTITY_V1: [u8; 32] = [
    0xa1, 0x7f, 0xad, 0x7c, 0x3f, 0x77, 0x4b, 0xa5, 0xd2, 0x75, 0x65, 0x05, 0xa6, 0x51, 0x73, 0x35,
    0x0b, 0x67, 0x06, 0xc5, 0xfa, 0x20, 0x9e, 0x76, 0x55, 0x63, 0x83, 0xce, 0xed, 0x4a, 0x2a, 0xc9,
];
pub const MOE_ROUTING_MEMORY_VERUS_EXECUTABLE_IDENTITY_V1: [u8; 32] = [
    0xd9, 0x75, 0x01, 0xa8, 0x83, 0x93, 0x1d, 0x1d, 0x17, 0x3b, 0x1b, 0xf4, 0xb6, 0xcf, 0x4d, 0x97,
    0x3f, 0x16, 0xd1, 0x05, 0xdb, 0xcb, 0x46, 0x8e, 0x17, 0x7b, 0x52, 0xb2, 0x33, 0x16, 0x12, 0xd2,
];
pub const MOE_ROUTING_MEMORY_VERUS_CLOSURE_IDENTITY_V1: [u8; 32] = [
    0xf0, 0x68, 0x83, 0xe4, 0xce, 0x46, 0x3b, 0xcb, 0x9a, 0x3c, 0x8f, 0x91, 0x10, 0x64, 0xac, 0x85,
    0x05, 0x4c, 0x78, 0x22, 0xdc, 0x33, 0x1d, 0xb1, 0xa7, 0x9f, 0x75, 0xf9, 0xe8, 0x87, 0x8b, 0x01,
];
pub const MOE_ROUTING_MEMORY_VERUS_TRANSCRIPT_IDENTITY_V1: [u8; 32] = [
    0x63, 0x44, 0xa0, 0xde, 0xf7, 0x20, 0x49, 0x69, 0xb6, 0x21, 0x8f, 0x7e, 0x81, 0xa4, 0xed, 0xfb,
    0x65, 0xf2, 0x1f, 0xcb, 0x27, 0x2b, 0xfd, 0x6a, 0xf1, 0xdb, 0x19, 0x91, 0x7c, 0x46, 0xc3, 0xb9,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoeRoutingMemoryBufferV1 {
    Logits,
    Top2Experts,
    RequestedCounts,
    AdmittedCounts,
    ExpertOffsets,
    RouteSlots,
    Permutation,
    Inverse,
}

impl MoeRoutingMemoryBufferV1 {
    pub const ALL: [Self; 8] = [
        Self::Logits,
        Self::Top2Experts,
        Self::RequestedCounts,
        Self::AdmittedCounts,
        Self::ExpertOffsets,
        Self::RouteSlots,
        Self::Permutation,
        Self::Inverse,
    ];

    pub const OUTPUTS: [Self; 7] = [
        Self::Top2Experts,
        Self::RequestedCounts,
        Self::AdmittedCounts,
        Self::ExpertOffsets,
        Self::RouteSlots,
        Self::Permutation,
        Self::Inverse,
    ];

    pub const fn elements(self) -> usize {
        match self {
            Self::Logits => 32,
            Self::Top2Experts | Self::RouteSlots | Self::Permutation | Self::Inverse => 16,
            Self::RequestedCounts | Self::AdmittedCounts => 4,
            Self::ExpertOffsets => 5,
        }
    }

    pub const fn byte_len(self) -> u64 {
        self.elements() as u64 * MOE_ROUTING_MEMORY_ACCESS_WIDTH_V1 as u64
    }

    const fn output_ordinal(self) -> Option<usize> {
        match self {
            Self::Logits => None,
            Self::Top2Experts => Some(0),
            Self::RequestedCounts => Some(1),
            Self::AdmittedCounts => Some(2),
            Self::ExpertOffsets => Some(3),
            Self::RouteSlots => Some(4),
            Self::Permutation => Some(5),
            Self::Inverse => Some(6),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoeRoutingMemoryEffectKindV1 {
    Read,
    Write,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum MoeRoutingMemoryPhaseV1 {
    InputValidation,
    Top2Selection,
    RequestedCount,
    CapacityClamp,
    ExclusiveScan,
    SentinelInitialization,
    StableRank,
    SlotAssignment,
    PermutationInverse,
    OutputCommit,
}

impl MoeRoutingMemoryPhaseV1 {
    const fn successor(self) -> Option<Self> {
        match self {
            Self::InputValidation => Some(Self::Top2Selection),
            Self::Top2Selection => Some(Self::RequestedCount),
            Self::RequestedCount => Some(Self::CapacityClamp),
            Self::CapacityClamp => Some(Self::ExclusiveScan),
            Self::ExclusiveScan => Some(Self::SentinelInitialization),
            Self::SentinelInitialization => Some(Self::StableRank),
            Self::StableRank => Some(Self::SlotAssignment),
            Self::SlotAssignment => Some(Self::PermutationInverse),
            Self::PermutationInverse => Some(Self::OutputCommit),
            Self::OutputCommit => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MoeRoutingLogicalAccessV1 {
    pub lane: usize,
    pub buffer: MoeRoutingMemoryBufferV1,
    pub element_index: usize,
    pub address_space: u8,
    pub byte_width: u8,
    pub phase: MoeRoutingMemoryPhaseV1,
    pub kind: MoeRoutingMemoryEffectKindV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoeRoutingLogicalIndexKindV1 {
    Expert,
    Route,
    RouteSlot,
    PermutationValue,
    InverseValue,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MoeRoutingLogicalIndexV1 {
    pub kind: MoeRoutingLogicalIndexKindV1,
    pub value: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MoeRoutingMemoryRegionV1 {
    pub base: u64,
    pub bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MoeRoutingMemoryRegionsV1 {
    pub logits: MoeRoutingMemoryRegionV1,
    pub top2_experts: MoeRoutingMemoryRegionV1,
    pub requested_counts: MoeRoutingMemoryRegionV1,
    pub admitted_counts: MoeRoutingMemoryRegionV1,
    pub expert_offsets: MoeRoutingMemoryRegionV1,
    pub route_slots: MoeRoutingMemoryRegionV1,
    pub permutation: MoeRoutingMemoryRegionV1,
    pub inverse: MoeRoutingMemoryRegionV1,
}

impl MoeRoutingMemoryRegionsV1 {
    const fn as_array(self) -> [MoeRoutingMemoryRegionV1; 8] {
        [
            self.logits,
            self.top2_experts,
            self.requested_counts,
            self.admitted_counts,
            self.expert_offsets,
            self.route_slots,
            self.permutation,
            self.inverse,
        ]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MoeRoutingMemoryIdentitiesV1 {
    pub source: [u8; 32],
    pub profile: [u8; 32],
    pub kernel_ir: [u8; 32],
    pub descriptor: [u8; 32],
    pub launch: [u8; 32],
    pub effects: [u8; 32],
    pub routing: [u8; 32],
}

impl MoeRoutingMemoryIdentitiesV1 {
    pub const fn exact() -> Self {
        Self {
            source: MOE_ROUTING_MEMORY_SOURCE_IDENTITY_V1,
            profile: MOE_ROUTING_MEMORY_PROFILE_IDENTITY_V1,
            kernel_ir: MOE_ROUTING_MEMORY_KIR_IDENTITY_V1,
            descriptor: MOE_ROUTING_MEMORY_DESCRIPTOR_IDENTITY_V1,
            launch: MOE_ROUTING_MEMORY_LAUNCH_IDENTITY_V1,
            effects: MOE_ROUTING_MEMORY_EFFECTS_IDENTITY_V1,
            routing: MOE_ROUTING_MEMORY_ROUTING_IDENTITY_V1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoeRoutingMemoryContractErrorV1 {
    Identity,
    Extent,
    Alignment,
    AddressOverflow,
    RegionAlias,
    Lane,
    AddressSpace,
    AccessWidth,
    EffectKind,
    EffectOrdering,
    OutputOwnership,
    InvalidExpert,
    InvalidRoute,
    InvalidRouteValue,
    DuplicateWriteOwnership,
}

impl fmt::Display for MoeRoutingMemoryContractErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid exact MoE routing memory contract: {self:?}"
        )
    }
}

impl std::error::Error for MoeRoutingMemoryContractErrorV1 {}

/// Inert exhaustive-check result for the complete fixed logical source model.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedMoeRoutingMemoryContractV1 {
    identities: MoeRoutingMemoryIdentitiesV1,
}

/// Inert expectations consumed only by fail-closed evidence tests.
///
/// This descriptor is deliberately copyable. It authenticates nothing and
/// cannot mint or join an authenticated Verus execution receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MoeRoutingMemoryVerusExpectedEvidenceV1 {
    pub proof_source: [u8; 32],
    pub published_machine_body: [u8; 32],
    pub analyzer_profile: [u8; 32],
    pub verus_executable: [u8; 32],
    pub verus_closure_manifest: [u8; 32],
    pub transcript: [u8; 32],
}

impl MoeRoutingMemoryVerusExpectedEvidenceV1 {
    pub const fn exact() -> Self {
        Self {
            proof_source: MOE_ROUTING_MEMORY_VERUS_PROOF_IDENTITY_V1,
            published_machine_body: MOE_ROUTING_MEMORY_PUBLISHED_MACHINE_BODY_IDENTITY_V1,
            analyzer_profile: MOE_ROUTING_MEMORY_ANALYZER_PROFILE_IDENTITY_V1,
            verus_executable: MOE_ROUTING_MEMORY_VERUS_EXECUTABLE_IDENTITY_V1,
            verus_closure_manifest: MOE_ROUTING_MEMORY_VERUS_CLOSURE_IDENTITY_V1,
            transcript: MOE_ROUTING_MEMORY_VERUS_TRANSCRIPT_IDENTITY_V1,
        }
    }

    pub const fn authenticates_anything(self) -> bool {
        false
    }
}

impl CheckedMoeRoutingMemoryContractV1 {
    pub const fn identities(self) -> MoeRoutingMemoryIdentitiesV1 {
        self.identities
    }

    pub const fn exhaustively_checks_fixed_source_index_bounds(self) -> bool {
        true
    }

    pub const fn exhaustively_checks_fixed_source_output_disjointness(self) -> bool {
        true
    }

    pub const fn exhaustively_checks_fixed_source_write_ownership(self) -> bool {
        true
    }

    pub const fn has_identity_bound_verus_receipt(self) -> bool {
        false
    }

    pub const fn proves_compiler_refinement(self) -> bool {
        false
    }

    pub const fn proves_kernel_ir_refinement(self) -> bool {
        false
    }

    pub const fn proves_llvm_refinement(self) -> bool {
        false
    }

    pub const fn proves_isa_refinement(self) -> bool {
        false
    }

    pub const fn proves_logical_to_machine_address_refinement(self) -> bool {
        false
    }

    pub const fn proves_machine_memory_safety(self) -> bool {
        false
    }

    pub const fn proves_generalized_race_freedom(self) -> bool {
        false
    }

    pub const fn grants_artifact_authority(self) -> bool {
        false
    }

    pub const fn proves_gpu_execution(self) -> bool {
        false
    }
}

pub fn check_moe_routing_memory_contract_v1(
    identities: MoeRoutingMemoryIdentitiesV1,
    regions: MoeRoutingMemoryRegionsV1,
) -> Result<CheckedMoeRoutingMemoryContractV1, MoeRoutingMemoryContractErrorV1> {
    if identities != MoeRoutingMemoryIdentitiesV1::exact() {
        return Err(MoeRoutingMemoryContractErrorV1::Identity);
    }
    validate_regions(regions)?;

    for index in 0..MoeRoutingMemoryBufferV1::Logits.elements() {
        for phase in [
            MoeRoutingMemoryPhaseV1::InputValidation,
            MoeRoutingMemoryPhaseV1::Top2Selection,
        ] {
            validate_access(MoeRoutingLogicalAccessV1 {
                lane: 0,
                buffer: MoeRoutingMemoryBufferV1::Logits,
                element_index: index,
                address_space: MOE_ROUTING_MEMORY_GLOBAL_ADDRESS_SPACE_V1,
                byte_width: MOE_ROUTING_MEMORY_ACCESS_WIDTH_V1,
                phase,
                kind: MoeRoutingMemoryEffectKindV1::Read,
            })?;
        }
    }

    for buffer in MoeRoutingMemoryBufferV1::OUTPUTS {
        for index in 0..buffer.elements() {
            validate_access(MoeRoutingLogicalAccessV1 {
                lane: 0,
                buffer,
                element_index: index,
                address_space: MOE_ROUTING_MEMORY_GLOBAL_ADDRESS_SPACE_V1,
                byte_width: MOE_ROUTING_MEMORY_ACCESS_WIDTH_V1,
                phase: MoeRoutingMemoryPhaseV1::OutputCommit,
                kind: MoeRoutingMemoryEffectKindV1::Write,
            })?;
        }
    }

    for left_buffer in MoeRoutingMemoryBufferV1::OUTPUTS {
        for left_index in 0..left_buffer.elements() {
            let left = write_owner_key(left_buffer, left_index)?;
            for right_buffer in MoeRoutingMemoryBufferV1::OUTPUTS {
                for right_index in 0..right_buffer.elements() {
                    let right = write_owner_key(right_buffer, right_index)?;
                    if (left_buffer, left_index) != (right_buffer, right_index) && left == right {
                        return Err(MoeRoutingMemoryContractErrorV1::DuplicateWriteOwnership);
                    }
                }
            }
        }
    }

    for expert in 0..MOE_ROUTING_MEMORY_EXPERTS_V1 as u32 {
        validate_index(MoeRoutingLogicalIndexV1 {
            kind: MoeRoutingLogicalIndexKindV1::Expert,
            value: expert,
        })?;
    }
    for route in 0..MOE_ROUTING_MEMORY_ROUTES_V1 as u32 {
        validate_index(MoeRoutingLogicalIndexV1 {
            kind: MoeRoutingLogicalIndexKindV1::Route,
            value: route,
        })?;
        for kind in [
            MoeRoutingLogicalIndexKindV1::RouteSlot,
            MoeRoutingLogicalIndexKindV1::PermutationValue,
            MoeRoutingLogicalIndexKindV1::InverseValue,
        ] {
            validate_index(MoeRoutingLogicalIndexV1 { kind, value: route })?;
        }
    }
    for kind in [
        MoeRoutingLogicalIndexKindV1::RouteSlot,
        MoeRoutingLogicalIndexKindV1::PermutationValue,
        MoeRoutingLogicalIndexKindV1::InverseValue,
    ] {
        validate_index(MoeRoutingLogicalIndexV1 {
            kind,
            value: MOE_ROUTING_MEMORY_DROP_ROUTE_V1,
        })?;
    }

    Ok(CheckedMoeRoutingMemoryContractV1 { identities })
}

pub fn validate_moe_routing_logical_access_v1(
    access: MoeRoutingLogicalAccessV1,
) -> Result<(), MoeRoutingMemoryContractErrorV1> {
    validate_access(access)
}

pub fn validate_moe_routing_logical_index_v1(
    index: MoeRoutingLogicalIndexV1,
) -> Result<(), MoeRoutingMemoryContractErrorV1> {
    validate_index(index)
}

pub fn validate_moe_routing_phase_transition_v1(
    previous: MoeRoutingMemoryPhaseV1,
    next: MoeRoutingMemoryPhaseV1,
) -> Result<(), MoeRoutingMemoryContractErrorV1> {
    if previous.successor() == Some(next) {
        Ok(())
    } else {
        Err(MoeRoutingMemoryContractErrorV1::EffectOrdering)
    }
}

fn validate_regions(
    regions: MoeRoutingMemoryRegionsV1,
) -> Result<(), MoeRoutingMemoryContractErrorV1> {
    let regions = regions.as_array();
    let mut ends = [0_u64; 8];
    for (index, (buffer, region)) in MoeRoutingMemoryBufferV1::ALL
        .into_iter()
        .zip(regions)
        .enumerate()
    {
        if region.bytes != buffer.byte_len() {
            return Err(MoeRoutingMemoryContractErrorV1::Extent);
        }
        if region.base % MOE_ROUTING_MEMORY_ACCESS_WIDTH_V1 as u64 != 0 {
            return Err(MoeRoutingMemoryContractErrorV1::Alignment);
        }
        ends[index] = region
            .base
            .checked_add(region.bytes)
            .ok_or(MoeRoutingMemoryContractErrorV1::AddressOverflow)?;
    }
    for left in 0..regions.len() {
        for right in left + 1..regions.len() {
            if regions[left].base < ends[right] && regions[right].base < ends[left] {
                return Err(MoeRoutingMemoryContractErrorV1::RegionAlias);
            }
        }
    }
    Ok(())
}

fn validate_access(
    access: MoeRoutingLogicalAccessV1,
) -> Result<(), MoeRoutingMemoryContractErrorV1> {
    if access.lane >= MOE_ROUTING_MEMORY_LANES_V1 {
        return Err(MoeRoutingMemoryContractErrorV1::Lane);
    }
    if access.address_space != MOE_ROUTING_MEMORY_GLOBAL_ADDRESS_SPACE_V1 {
        return Err(MoeRoutingMemoryContractErrorV1::AddressSpace);
    }
    if access.byte_width != MOE_ROUTING_MEMORY_ACCESS_WIDTH_V1 {
        return Err(MoeRoutingMemoryContractErrorV1::AccessWidth);
    }
    if access.element_index >= access.buffer.elements() {
        return Err(MoeRoutingMemoryContractErrorV1::Extent);
    }
    match (access.buffer, access.kind, access.phase) {
        (
            MoeRoutingMemoryBufferV1::Logits,
            MoeRoutingMemoryEffectKindV1::Read,
            MoeRoutingMemoryPhaseV1::InputValidation | MoeRoutingMemoryPhaseV1::Top2Selection,
        ) => Ok(()),
        (MoeRoutingMemoryBufferV1::Logits, MoeRoutingMemoryEffectKindV1::Write, _) => {
            Err(MoeRoutingMemoryContractErrorV1::EffectKind)
        }
        (_, MoeRoutingMemoryEffectKindV1::Write, MoeRoutingMemoryPhaseV1::OutputCommit)
            if access.lane == 0 =>
        {
            Ok(())
        }
        (_, MoeRoutingMemoryEffectKindV1::Write, MoeRoutingMemoryPhaseV1::OutputCommit) => {
            Err(MoeRoutingMemoryContractErrorV1::OutputOwnership)
        }
        (_, MoeRoutingMemoryEffectKindV1::Read, _) => {
            Err(MoeRoutingMemoryContractErrorV1::EffectKind)
        }
        _ => Err(MoeRoutingMemoryContractErrorV1::EffectOrdering),
    }
}

fn validate_index(index: MoeRoutingLogicalIndexV1) -> Result<(), MoeRoutingMemoryContractErrorV1> {
    match index.kind {
        MoeRoutingLogicalIndexKindV1::Expert
            if index.value < MOE_ROUTING_MEMORY_EXPERTS_V1 as u32 =>
        {
            Ok(())
        }
        MoeRoutingLogicalIndexKindV1::Expert => Err(MoeRoutingMemoryContractErrorV1::InvalidExpert),
        MoeRoutingLogicalIndexKindV1::Route
            if index.value < MOE_ROUTING_MEMORY_ROUTES_V1 as u32 =>
        {
            Ok(())
        }
        MoeRoutingLogicalIndexKindV1::Route => Err(MoeRoutingMemoryContractErrorV1::InvalidRoute),
        MoeRoutingLogicalIndexKindV1::RouteSlot
        | MoeRoutingLogicalIndexKindV1::PermutationValue
        | MoeRoutingLogicalIndexKindV1::InverseValue
            if index.value < MOE_ROUTING_MEMORY_ROUTES_V1 as u32
                || index.value == MOE_ROUTING_MEMORY_DROP_ROUTE_V1 =>
        {
            Ok(())
        }
        _ => Err(MoeRoutingMemoryContractErrorV1::InvalidRouteValue),
    }
}

fn write_owner_key(
    buffer: MoeRoutingMemoryBufferV1,
    index: usize,
) -> Result<usize, MoeRoutingMemoryContractErrorV1> {
    let ordinal = buffer
        .output_ordinal()
        .ok_or(MoeRoutingMemoryContractErrorV1::EffectKind)?;
    if index >= buffer.elements() {
        return Err(MoeRoutingMemoryContractErrorV1::Extent);
    }
    Ok(ordinal * 32 + index)
}
