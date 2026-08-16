//! Fixed-profile arithmetic checks for the MoE expert compact plan.
//!
//! This module covers only E4/C4/routes16/output-width16/tile256. Its expected
//! evidence values are inert pins: they authenticate nothing and grant no
//! proof-receipt, copy, address, runtime, artifact, launch, or GPU authority.

use core::fmt;

pub const MOE_EXPERT_COMPACT_EXPERTS_V1: usize = 4;
pub const MOE_EXPERT_COMPACT_CAPACITY_V1: usize = 4;
pub const MOE_EXPERT_COMPACT_ROUTES_V1: usize = 16;
pub const MOE_EXPERT_COMPACT_OUTPUT_WIDTH_V1: usize = 16;
pub const MOE_EXPERT_COMPACT_TILE_ELEMENTS_V1: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MoeExpertCompactRangeV1 {
    pub source_start: usize,
    pub source_end: usize,
    pub destination_start: usize,
    pub destination_end: usize,
}

impl MoeExpertCompactRangeV1 {
    pub const fn is_empty(self) -> bool {
        self.destination_start == self.destination_end
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoeExpertCompactPlanErrorV1 {
    FirstOffset,
    NonMonotone { expert: usize },
    Capacity { expert: usize, count: usize },
    AcceptedPrefix,
    PrefixLength { expected: usize, actual: usize },
}

impl fmt::Display for MoeExpertCompactPlanErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid exact MoE expert compact plan: {self:?}")
    }
}

impl std::error::Error for MoeExpertCompactPlanErrorV1 {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedMoeExpertCompactPlanV1 {
    offsets: [usize; MOE_EXPERT_COMPACT_EXPERTS_V1 + 1],
    ranges: [MoeExpertCompactRangeV1; MOE_EXPERT_COMPACT_EXPERTS_V1],
    accepted_elements: usize,
}

impl CheckedMoeExpertCompactPlanV1 {
    pub const fn offsets(self) -> [usize; MOE_EXPERT_COMPACT_EXPERTS_V1 + 1] {
        self.offsets
    }

    pub const fn ranges(self) -> [MoeExpertCompactRangeV1; MOE_EXPERT_COMPACT_EXPERTS_V1] {
        self.ranges
    }

    pub const fn accepted_routes(self) -> usize {
        self.offsets[MOE_EXPERT_COMPACT_EXPERTS_V1]
    }

    pub const fn accepted_elements(self) -> usize {
        self.accepted_elements
    }

    pub fn every_source_range_is_inside_its_expert_tile(self) -> bool {
        self.ranges.iter().enumerate().all(|(expert, range)| {
            let tile_start = expert * MOE_EXPERT_COMPACT_TILE_ELEMENTS_V1;
            let tile_end = tile_start + MOE_EXPERT_COMPACT_TILE_ELEMENTS_V1;
            range.source_start == tile_start
                && range.source_start <= range.source_end
                && range.source_end <= tile_end
        })
    }

    pub fn every_destination_range_is_inside_compact_tile(self) -> bool {
        self.ranges.iter().all(|range| {
            range.destination_start <= range.destination_end
                && range.destination_end <= MOE_EXPERT_COMPACT_TILE_ELEMENTS_V1
        })
    }

    pub fn nonempty_destination_ranges_are_pairwise_disjoint_and_ordered(self) -> bool {
        for earlier in 0..self.ranges.len() {
            if self.ranges[earlier].is_empty() {
                continue;
            }
            for later in earlier + 1..self.ranges.len() {
                if !self.ranges[later].is_empty()
                    && self.ranges[earlier].destination_end > self.ranges[later].destination_start
                {
                    return false;
                }
            }
        }
        true
    }

    pub fn destination_union_is_exact_accepted_prefix(self) -> bool {
        (0..MOE_EXPERT_COMPACT_TILE_ELEMENTS_V1).all(|index| {
            let memberships = self
                .ranges
                .iter()
                .filter(|range| range.destination_start <= index && index < range.destination_end)
                .count();
            memberships == usize::from(index < self.accepted_elements)
        })
    }

    pub fn zero_fill(
        self,
        accepted_values: &[i32],
    ) -> Result<[i32; MOE_EXPERT_COMPACT_TILE_ELEMENTS_V1], MoeExpertCompactPlanErrorV1> {
        if accepted_values.len() != self.accepted_elements {
            return Err(MoeExpertCompactPlanErrorV1::PrefixLength {
                expected: self.accepted_elements,
                actual: accepted_values.len(),
            });
        }
        let mut compact = [0_i32; MOE_EXPERT_COMPACT_TILE_ELEMENTS_V1];
        compact[..self.accepted_elements].copy_from_slice(accepted_values);
        Ok(compact)
    }
}

pub fn check_moe_expert_compact_plan_v1(
    offsets: [usize; MOE_EXPERT_COMPACT_EXPERTS_V1 + 1],
) -> Result<CheckedMoeExpertCompactPlanV1, MoeExpertCompactPlanErrorV1> {
    if offsets[0] != 0 {
        return Err(MoeExpertCompactPlanErrorV1::FirstOffset);
    }

    let empty = MoeExpertCompactRangeV1 {
        source_start: 0,
        source_end: 0,
        destination_start: 0,
        destination_end: 0,
    };
    let mut ranges = [empty; MOE_EXPERT_COMPACT_EXPERTS_V1];
    for expert in 0..MOE_EXPERT_COMPACT_EXPERTS_V1 {
        let count = offsets[expert + 1]
            .checked_sub(offsets[expert])
            .ok_or(MoeExpertCompactPlanErrorV1::NonMonotone { expert })?;
        if count > MOE_EXPERT_COMPACT_CAPACITY_V1 {
            return Err(MoeExpertCompactPlanErrorV1::Capacity { expert, count });
        }
        let source_start = expert * MOE_EXPERT_COMPACT_TILE_ELEMENTS_V1;
        ranges[expert] = MoeExpertCompactRangeV1 {
            source_start,
            source_end: source_start + count * MOE_EXPERT_COMPACT_OUTPUT_WIDTH_V1,
            destination_start: offsets[expert] * MOE_EXPERT_COMPACT_OUTPUT_WIDTH_V1,
            destination_end: offsets[expert + 1] * MOE_EXPERT_COMPACT_OUTPUT_WIDTH_V1,
        };
    }

    let accepted_elements = offsets[MOE_EXPERT_COMPACT_EXPERTS_V1]
        .checked_mul(MOE_EXPERT_COMPACT_OUTPUT_WIDTH_V1)
        .filter(|elements| *elements <= MOE_EXPERT_COMPACT_TILE_ELEMENTS_V1)
        .ok_or(MoeExpertCompactPlanErrorV1::AcceptedPrefix)?;
    Ok(CheckedMoeExpertCompactPlanV1 {
        offsets,
        ranges,
        accepted_elements,
    })
}

/// Inert expected values for the fixed-profile proof test only.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MoeExpertCompactPlanExpectedEvidenceV1 {
    pub proof_source: [u8; 32],
    pub runner_source: [u8; 32],
    pub negative_manifest: [u8; 32],
    pub verus_executable: [u8; 32],
    pub verus_closure_manifest: [u8; 32],
    pub transcript: [u8; 32],
}

impl MoeExpertCompactPlanExpectedEvidenceV1 {
    pub const fn exact() -> Self {
        Self {
            proof_source: [
                0x96, 0xea, 0x63, 0xd2, 0xd3, 0x99, 0xb1, 0x59, 0x29, 0x13, 0x90, 0x1a, 0x52, 0x94,
                0x9a, 0x39, 0x28, 0x71, 0x85, 0x52, 0x8d, 0xd0, 0xda, 0xb2, 0xe2, 0xe5, 0xef, 0xce,
                0xdc, 0xcd, 0x02, 0x3f,
            ],
            runner_source: [
                0x94, 0x9d, 0x00, 0xda, 0x1f, 0x0e, 0x7c, 0x73, 0xe3, 0xc0, 0xe5, 0x89, 0x17, 0xe6,
                0xeb, 0x66, 0xa5, 0x83, 0x2b, 0xd5, 0x8b, 0x01, 0x63, 0x7c, 0xa6, 0x27, 0x34, 0xa7,
                0x8f, 0x05, 0xd3, 0x56,
            ],
            negative_manifest: [
                0x44, 0x16, 0x17, 0x66, 0x18, 0x17, 0x1b, 0xb2, 0x23, 0x89, 0x31, 0x25, 0x51, 0xaa,
                0x31, 0x2d, 0x20, 0x1b, 0x66, 0xb2, 0x55, 0x9d, 0x77, 0x4f, 0xf1, 0xb4, 0xdc, 0x58,
                0xbf, 0xe7, 0x46, 0xc7,
            ],
            verus_executable: [
                0xad, 0x26, 0x69, 0xf5, 0x79, 0xd8, 0x98, 0xed, 0xe5, 0x3f, 0x2b, 0xf8, 0x4e, 0x80,
                0xa1, 0xda, 0xf4, 0xe3, 0x57, 0x87, 0x39, 0xb0, 0xf5, 0x80, 0x7e, 0xf2, 0x09, 0xa0,
                0xc9, 0xf3, 0x82, 0xdd,
            ],
            verus_closure_manifest: [
                0xd2, 0x8d, 0xf3, 0xfb, 0x5e, 0x0d, 0x74, 0x76, 0x37, 0x54, 0x39, 0x33, 0xdf, 0xc3,
                0x8c, 0xff, 0x45, 0x57, 0x6d, 0xa9, 0xb9, 0x20, 0xd7, 0x55, 0xb4, 0xb7, 0xe9, 0x19,
                0xe4, 0x7a, 0x60, 0x19,
            ],
            transcript: [
                0x5e, 0x0e, 0x01, 0x3f, 0x1f, 0xf0, 0x03, 0xaa, 0x85, 0x3a, 0x54, 0x70, 0x14, 0x26,
                0x29, 0xef, 0xae, 0xb2, 0x86, 0x38, 0x06, 0x40, 0x06, 0xcc, 0xb6, 0xe9, 0x34, 0x4c,
                0x92, 0x25, 0x72, 0xf1,
            ],
        }
    }

    pub const fn authenticates_anything(self) -> bool {
        false
    }

    pub const fn has_authenticated_proof_receipt(self) -> bool {
        false
    }

    pub const fn proves_hsa_copy(self) -> bool {
        false
    }

    pub const fn proves_machine_addresses(self) -> bool {
        false
    }

    pub const fn proves_runtime_execution(self) -> bool {
        false
    }

    pub const fn proves_gpu_execution(self) -> bool {
        false
    }

    pub const fn proves_generalized_profile(self) -> bool {
        false
    }
}
