//! Fail-closed host binding for exact bounded MoE expert compute V2.
//!
//! This module is intentionally isolated from the legacy generated V1 module.
//! It accepts only a completed V2 routing bridge and an expert-weight artifact
//! binding tied to the same request/batch. Neither capability has a production
//! issuer, so safe production preparation remains unreachable.

use crate::{
    MoeCompletedRoutingExpertBridgeV2, MoeExpertCompactPackPlanV1,
    MoeExpertWeightArtifactBindingV2, ObservedContext,
    moe_routing_expert_bridge_v2::{
        COMBINED_OUTPUT_ELEMENTS, COMPACT_OUTPUT_ELEMENTS, EXPERT_OFFSETS, EXPERT_OUTPUT_ELEMENTS,
        EXPERTS, ROUTES, TILE_ELEMENTS,
    },
};
use fe2o3_amd_target::AmdTargetId;
use fe2o3_core::{DeviceBufferIdentity, DeviceBufferRegion, DeviceBufferViewMut, DeviceCopy};
use std::{
    error::Error,
    fmt,
    mem::{align_of, size_of},
};

const TARGET: &str = "gfx942:xnack-";
const WEIGHT_ELEMENTS: usize = EXPERTS * TILE_ELEMENTS;
const GEMM_EXPLICIT_KERNARG_BYTES: usize = 48;
const COMBINE_EXPLICIT_KERNARG_BYTES: usize = 64;
const KERNARG_ALIGNMENT: usize = 8;

/// Exact retained V2 buffer role used by admission diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoeExpertV2BufferRoleV2 {
    ActivationTiles,
    ExpertWeights,
    ExpertOffsets,
    InverseRouting,
    RouteWeights,
    ExpertOutputTiles,
    CompactOutput,
    CombinedOutput,
}

impl MoeExpertV2BufferRoleV2 {
    const ALL: [Self; 8] = [
        Self::ActivationTiles,
        Self::ExpertWeights,
        Self::ExpertOffsets,
        Self::InverseRouting,
        Self::RouteWeights,
        Self::ExpertOutputTiles,
        Self::CompactOutput,
        Self::CombinedOutput,
    ];

    const fn elements(self) -> usize {
        match self {
            Self::ActivationTiles => EXPERTS * TILE_ELEMENTS,
            Self::ExpertWeights => WEIGHT_ELEMENTS,
            Self::ExpertOffsets => EXPERT_OFFSETS,
            Self::InverseRouting | Self::RouteWeights => ROUTES,
            Self::ExpertOutputTiles => EXPERT_OUTPUT_ELEMENTS,
            Self::CompactOutput => COMPACT_OUTPUT_ELEMENTS,
            Self::CombinedOutput => COMBINED_OUTPUT_ELEMENTS,
        }
    }

    const fn element_bytes(self) -> usize {
        match self {
            Self::ActivationTiles | Self::ExpertWeights => size_of::<u16>(),
            Self::ExpertOffsets | Self::InverseRouting => size_of::<u32>(),
            Self::RouteWeights
            | Self::ExpertOutputTiles
            | Self::CompactOutput
            | Self::CombinedOutput => size_of::<f32>(),
        }
    }

    const fn element_alignment(self) -> usize {
        match self {
            Self::ActivationTiles | Self::ExpertWeights => align_of::<u16>(),
            Self::ExpertOffsets | Self::InverseRouting => align_of::<u32>(),
            Self::RouteWeights
            | Self::ExpertOutputTiles
            | Self::CompactOutput
            | Self::CombinedOutput => align_of::<f32>(),
        }
    }
}

/// Exact access retained for one admitted V2 buffer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoeExpertV2BufferAccessV2 {
    SharedReadOnly,
    UniqueReadWrite,
}

#[repr(C, align(8))]
struct GemmExplicitKernargV2 {
    bytes: [u8; GEMM_EXPLICIT_KERNARG_BYTES],
}

#[repr(C, align(8))]
struct CombineExplicitKernargV2 {
    bytes: [u8; COMBINE_EXPLICIT_KERNARG_BYTES],
}

const _: () = assert!(size_of::<GemmExplicitKernargV2>() == GEMM_EXPLICIT_KERNARG_BYTES);
const _: () = assert!(size_of::<CombineExplicitKernargV2>() == COMBINE_EXPLICIT_KERNARG_BYTES);
const _: () = assert!(align_of::<GemmExplicitKernargV2>() == KERNARG_ALIGNMENT);
const _: () = assert!(align_of::<CombineExplicitKernargV2>() == KERNARG_ALIGNMENT);

/// Completed V2 binding with a pre-joined expert-weight artifact.
#[must_use = "the V2 MoE expert binding retains every admitted device region"]
pub struct GeneratedMoeExpertV2HostAdapterV2<
    'activations,
    'weights,
    'offsets,
    'inverse,
    'route_weights,
    'expert_output,
    'compact_output,
    'combined_output,
> {
    observed: ObservedContext,
    routing_bridge:
        MoeCompletedRoutingExpertBridgeV2<'activations, 'offsets, 'inverse, 'route_weights>,
    weight_binding: MoeExpertWeightArtifactBindingV2<'weights>,
    _gemm_kernargs: [GemmExplicitKernargV2; EXPERTS],
    _combine_kernarg: CombineExplicitKernargV2,
    _expert_output_tiles: DeviceBufferViewMut<'expert_output, f32>,
    _compact_output: DeviceBufferViewMut<'compact_output, f32>,
    _combined_output: DeviceBufferViewMut<'combined_output, f32>,
}

impl fmt::Debug for GeneratedMoeExpertV2HostAdapterV2<'_, '_, '_, '_, '_, '_, '_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedMoeExpertV2HostAdapterV2")
            .field("target", &TARGET)
            .field("shape", &"T8/E4/K2/C4/I16/O16")
            .field("production_issuer", &"absent")
            .finish_non_exhaustive()
    }
}

impl<
    'activations,
    'weights,
    'offsets,
    'inverse,
    'route_weights,
    'expert_output,
    'compact_output,
    'combined_output,
>
    GeneratedMoeExpertV2HostAdapterV2<
        'activations,
        'weights,
        'offsets,
        'inverse,
        'route_weights,
        'expert_output,
        'compact_output,
        'combined_output,
    >
{
    /// Retains a completed V2 routing bridge and exact pre-bound weight artifact.
    pub fn prepare(
        observed: &ObservedContext,
        routing_bridge: MoeCompletedRoutingExpertBridgeV2<
            'activations,
            'offsets,
            'inverse,
            'route_weights,
        >,
        weight_binding: MoeExpertWeightArtifactBindingV2<'weights>,
        expert_output_tiles: DeviceBufferViewMut<'expert_output, f32>,
        compact_output: DeviceBufferViewMut<'compact_output, f32>,
        combined_output: DeviceBufferViewMut<'combined_output, f32>,
    ) -> Result<Self, GeneratedMoeExpertV2HostAdapterErrorV2> {
        validate_observed_target(observed.device().target())?;
        if !weight_binding.binding_matches_transcript() {
            return Err(GeneratedMoeExpertV2HostAdapterErrorV2::WeightArtifactBinding);
        }
        if routing_bridge.request_batch_transcript_sha256()
            != weight_binding.request_batch_transcript_sha256()
            || routing_bridge.model_expert_weight_artifact_identity()
                != weight_binding.model_expert_weight_artifact_identity()
        {
            return Err(GeneratedMoeExpertV2HostAdapterErrorV2::RequestBatchIdentity);
        }
        let contexts = [
            routing_bridge.activation_tiles_view().context(),
            weight_binding.weight_view().context(),
            routing_bridge.offsets_view().context(),
            routing_bridge.inverse_view().context(),
            routing_bridge.route_weights_view().context(),
            expert_output_tiles.context(),
            compact_output.context(),
            combined_output.context(),
        ];
        for (role, context) in MoeExpertV2BufferRoleV2::ALL.into_iter().zip(contexts) {
            if !observed.is_for_context(context) {
                return Err(GeneratedMoeExpertV2HostAdapterErrorV2::WrongContext { role });
            }
        }
        let prepared = prepare_regions([
            RegionFacts::from_region(routing_bridge.activation_tiles_view()),
            RegionFacts::from_region(weight_binding.weight_view()),
            RegionFacts::from_region(routing_bridge.offsets_view()),
            RegionFacts::from_region(routing_bridge.inverse_view()),
            RegionFacts::from_region(routing_bridge.route_weights_view()),
            RegionFacts::from_region(&expert_output_tiles),
            RegionFacts::from_region(&compact_output),
            RegionFacts::from_region(&combined_output),
        ])?;
        Ok(Self {
            observed: observed.clone(),
            routing_bridge,
            weight_binding,
            _gemm_kernargs: prepared.gemm,
            _combine_kernarg: prepared.combine,
            _expert_output_tiles: expert_output_tiles,
            _compact_output: compact_output,
            _combined_output: combined_output,
        })
    }

    pub fn target(&self) -> &str {
        self.observed.device().target()
    }

    pub const fn has_production_issuer(&self) -> bool {
        false
    }

    pub const fn proves_routing_or_expert_semantics(&self) -> bool {
        false
    }

    pub const fn grants_artifact_authority(&self) -> bool {
        false
    }

    pub const fn grants_copy_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_dispatch_authority(&self) -> bool {
        false
    }

    pub const fn compact_pack_plan(&self) -> MoeExpertCompactPackPlanV1 {
        self.routing_bridge.compact_pack_plan()
    }

    pub const fn retains_weight_artifact_binding(&self) -> bool {
        let _ = &self.weight_binding;
        true
    }

    pub const fn access_for(&self, role: MoeExpertV2BufferRoleV2) -> MoeExpertV2BufferAccessV2 {
        access_for_role(role)
    }
}

const fn access_for_role(role: MoeExpertV2BufferRoleV2) -> MoeExpertV2BufferAccessV2 {
    match role {
        MoeExpertV2BufferRoleV2::ActivationTiles
        | MoeExpertV2BufferRoleV2::ExpertWeights
        | MoeExpertV2BufferRoleV2::ExpertOffsets
        | MoeExpertV2BufferRoleV2::InverseRouting
        | MoeExpertV2BufferRoleV2::RouteWeights => MoeExpertV2BufferAccessV2::SharedReadOnly,
        MoeExpertV2BufferRoleV2::ExpertOutputTiles
        | MoeExpertV2BufferRoleV2::CompactOutput
        | MoeExpertV2BufferRoleV2::CombinedOutput => MoeExpertV2BufferAccessV2::UniqueReadWrite,
    }
}

fn validate_observed_target(target: &str) -> Result<(), GeneratedMoeExpertV2HostAdapterErrorV2> {
    let expected = AmdTargetId::parse(TARGET)
        .map_err(|_| GeneratedMoeExpertV2HostAdapterErrorV2::ObservedTargetMismatch)?;
    let actual = AmdTargetId::parse(target)
        .map_err(|_| GeneratedMoeExpertV2HostAdapterErrorV2::ObservedTargetMismatch)?;
    expected
        .is_compatible_with_observed(&actual)
        .then_some(())
        .ok_or(GeneratedMoeExpertV2HostAdapterErrorV2::ObservedTargetMismatch)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RegionFacts<I = DeviceBufferIdentity> {
    allocation_identity: I,
    allocation_address: usize,
    allocation_elements: usize,
    region_address: usize,
    region_elements: usize,
    region_byte_start: usize,
    region_byte_end: usize,
    element_bytes: usize,
    element_alignment: usize,
}

impl RegionFacts {
    fn from_region<T: DeviceCopy, R: DeviceBufferRegion<T> + ?Sized>(region: &R) -> Self {
        let range = region.region_byte_range();
        Self {
            allocation_identity: region.allocation_identity(),
            allocation_address: region.allocation_device_ptr().as_raw().addr(),
            allocation_elements: region.allocation_len(),
            region_address: region.region_device_ptr().as_raw().addr(),
            region_elements: region.region_len(),
            region_byte_start: range.start,
            region_byte_end: range.end,
            element_bytes: size_of::<T>(),
            element_alignment: align_of::<T>(),
        }
    }
}

#[derive(Clone, Copy)]
struct CheckedRegion<I> {
    allocation_identity: I,
    address: u64,
    byte_start: usize,
    byte_end: usize,
}

fn validate_region<I: Copy>(
    role: MoeExpertV2BufferRoleV2,
    facts: RegionFacts<I>,
) -> Result<CheckedRegion<I>, GeneratedMoeExpertV2HostAdapterErrorV2> {
    if facts.element_bytes != role.element_bytes()
        || facts.element_alignment != role.element_alignment()
    {
        return Err(GeneratedMoeExpertV2HostAdapterErrorV2::ElementLayout { role });
    }
    let expected_elements = role.elements();
    if facts.region_elements != expected_elements {
        return Err(GeneratedMoeExpertV2HostAdapterErrorV2::Length {
            role,
            expected: expected_elements,
            actual: facts.region_elements,
        });
    }
    if facts.allocation_address == 0 || facts.region_address == 0 {
        return Err(GeneratedMoeExpertV2HostAdapterErrorV2::NullAddress { role });
    }
    if !facts
        .region_address
        .is_multiple_of(role.element_alignment())
    {
        return Err(GeneratedMoeExpertV2HostAdapterErrorV2::Alignment { role });
    }
    let allocation_bytes = facts
        .allocation_elements
        .checked_mul(facts.element_bytes)
        .ok_or(GeneratedMoeExpertV2HostAdapterErrorV2::RegionOverflow { role })?;
    let allocation_end = facts
        .allocation_address
        .checked_add(allocation_bytes)
        .ok_or(GeneratedMoeExpertV2HostAdapterErrorV2::RegionOverflow { role })?;
    let region_bytes = facts
        .region_elements
        .checked_mul(facts.element_bytes)
        .ok_or(GeneratedMoeExpertV2HostAdapterErrorV2::RegionOverflow { role })?;
    let region_end = facts
        .region_address
        .checked_add(region_bytes)
        .ok_or(GeneratedMoeExpertV2HostAdapterErrorV2::RegionOverflow { role })?;
    let expected_address = facts
        .allocation_address
        .checked_add(facts.region_byte_start)
        .ok_or(GeneratedMoeExpertV2HostAdapterErrorV2::RegionOverflow { role })?;
    if facts.region_byte_start > facts.region_byte_end
        || facts.region_byte_end > allocation_bytes
        || facts.region_byte_end.checked_sub(facts.region_byte_start) != Some(region_bytes)
        || expected_address != facts.region_address
        || facts.region_address < facts.allocation_address
        || region_end > allocation_end
    {
        return Err(GeneratedMoeExpertV2HostAdapterErrorV2::InvalidRegion { role });
    }
    Ok(CheckedRegion {
        allocation_identity: facts.allocation_identity,
        address: u64::try_from(facts.region_address)
            .map_err(|_| GeneratedMoeExpertV2HostAdapterErrorV2::PointerWidth { role })?,
        byte_start: facts.region_address,
        byte_end: region_end,
    })
}

struct PreparedKernargsV2 {
    gemm: [GemmExplicitKernargV2; EXPERTS],
    combine: CombineExplicitKernargV2,
}

fn push_slice(bytes: &mut [u8], slot: usize, address: u64, elements: usize) {
    let offset = slot * 16;
    bytes[offset..offset + 8].copy_from_slice(&address.to_le_bytes());
    bytes[offset + 8..offset + 16].copy_from_slice(&(elements as u64).to_le_bytes());
}

fn prepare_regions<I: Copy + Eq>(
    facts: [RegionFacts<I>; 8],
) -> Result<PreparedKernargsV2, GeneratedMoeExpertV2HostAdapterErrorV2> {
    let roles = MoeExpertV2BufferRoleV2::ALL;
    let checked =
        std::array::from_fn::<_, 8, _>(|index| validate_region(roles[index], facts[index]));
    let checked = [
        checked[0]?,
        checked[1]?,
        checked[2]?,
        checked[3]?,
        checked[4]?,
        checked[5]?,
        checked[6]?,
        checked[7]?,
    ];
    for left in 0..checked.len() {
        for right in left + 1..checked.len() {
            let overlaps = checked[left].byte_start < checked[right].byte_end
                && checked[right].byte_start < checked[left].byte_end;
            if checked[left].allocation_identity == checked[right].allocation_identity || overlaps {
                return Err(GeneratedMoeExpertV2HostAdapterErrorV2::ArgumentsAlias {
                    left: roles[left],
                    right: roles[right],
                });
            }
        }
    }

    let mut gemm = std::array::from_fn(|_| GemmExplicitKernargV2 {
        bytes: [0; GEMM_EXPLICIT_KERNARG_BYTES],
    });
    for (expert, kernarg) in gemm.iter_mut().enumerate() {
        push_slice(
            &mut kernarg.bytes,
            0,
            checked[0].address + (expert * TILE_ELEMENTS * size_of::<u16>()) as u64,
            TILE_ELEMENTS,
        );
        push_slice(
            &mut kernarg.bytes,
            1,
            checked[1].address + (expert * TILE_ELEMENTS * size_of::<u16>()) as u64,
            TILE_ELEMENTS,
        );
        push_slice(
            &mut kernarg.bytes,
            2,
            checked[5].address + (expert * TILE_ELEMENTS * size_of::<f32>()) as u64,
            TILE_ELEMENTS,
        );
    }

    let mut combine = CombineExplicitKernargV2 {
        bytes: [0; COMBINE_EXPLICIT_KERNARG_BYTES],
    };
    push_slice(
        &mut combine.bytes,
        0,
        checked[6].address,
        COMPACT_OUTPUT_ELEMENTS,
    );
    push_slice(&mut combine.bytes, 1, checked[3].address, ROUTES);
    push_slice(&mut combine.bytes, 2, checked[4].address, ROUTES);
    push_slice(
        &mut combine.bytes,
        3,
        checked[7].address,
        COMBINED_OUTPUT_ELEMENTS,
    );
    Ok(PreparedKernargsV2 { gemm, combine })
}

/// Authority-free rejection while preparing the exact V2 host binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum GeneratedMoeExpertV2HostAdapterErrorV2 {
    RequestBatchIdentity,
    WeightArtifactBinding,
    ObservedTargetMismatch,
    WrongContext {
        role: MoeExpertV2BufferRoleV2,
    },
    ElementLayout {
        role: MoeExpertV2BufferRoleV2,
    },
    Length {
        role: MoeExpertV2BufferRoleV2,
        expected: usize,
        actual: usize,
    },
    NullAddress {
        role: MoeExpertV2BufferRoleV2,
    },
    Alignment {
        role: MoeExpertV2BufferRoleV2,
    },
    RegionOverflow {
        role: MoeExpertV2BufferRoleV2,
    },
    InvalidRegion {
        role: MoeExpertV2BufferRoleV2,
    },
    PointerWidth {
        role: MoeExpertV2BufferRoleV2,
    },
    ArgumentsAlias {
        left: MoeExpertV2BufferRoleV2,
        right: MoeExpertV2BufferRoleV2,
    },
}

impl fmt::Display for GeneratedMoeExpertV2HostAdapterErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "MoE expert V2 preparation rejected: {self:?}")
    }
}

impl Error for GeneratedMoeExpertV2HostAdapterErrorV2 {}

#[cfg(test)]
mod tests {
    use super::*;

    fn region(identity: u64, address: usize, role: MoeExpertV2BufferRoleV2) -> RegionFacts<u64> {
        let elements = role.elements();
        let bytes = role.element_bytes();
        RegionFacts {
            allocation_identity: identity,
            allocation_address: address - 0x100,
            allocation_elements: elements + 256,
            region_address: address,
            region_elements: elements,
            region_byte_start: 0x100,
            region_byte_end: 0x100 + elements * bytes,
            element_bytes: bytes,
            element_alignment: role.element_alignment(),
        }
    }

    fn exact_regions() -> [RegionFacts<u64>; 8] {
        std::array::from_fn(|index| {
            region(
                (index + 1) as u64,
                0x2100 + index * 0x4000,
                MoeExpertV2BufferRoleV2::ALL[index],
            )
        })
    }

    fn slice(bytes: &[u8], slot: usize) -> (u64, u64) {
        let offset = slot * 16;
        (
            u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap()),
            u64::from_le_bytes(bytes[offset + 8..offset + 16].try_into().unwrap()),
        )
    }

    #[test]
    fn exact_v2_regions_prepare_four_gemm_and_one_combine_record() {
        let prepared = prepare_regions(exact_regions()).unwrap();
        assert_eq!(prepared.gemm.len(), EXPERTS);
        assert_eq!(slice(&prepared.gemm[0].bytes, 0), (0x2100, 256));
        assert_eq!(slice(&prepared.gemm[3].bytes, 0), (0x2700, 256));
        assert_eq!(slice(&prepared.combine.bytes, 0), (0x1a100, 256));
        assert_eq!(slice(&prepared.combine.bytes, 1), (0xe100, 16));
        assert_eq!(slice(&prepared.combine.bytes, 2), (0x12100, 16));
        assert_eq!(slice(&prepared.combine.bytes, 3), (0x1e100, 128));
    }

    #[test]
    fn v2_region_extent_layout_and_every_alias_pair_fail_closed() {
        let mut short = exact_regions();
        short[0].region_elements -= 1;
        assert!(matches!(
            prepare_regions(short),
            Err(GeneratedMoeExpertV2HostAdapterErrorV2::Length { .. })
        ));

        let mut misaligned = exact_regions();
        misaligned[1].region_address += 1;
        assert!(matches!(
            prepare_regions(misaligned),
            Err(GeneratedMoeExpertV2HostAdapterErrorV2::Alignment { .. })
                | Err(GeneratedMoeExpertV2HostAdapterErrorV2::InvalidRegion { .. })
        ));

        for left in 0..8 {
            for right in left + 1..8 {
                let mut aliased = exact_regions();
                aliased[right].allocation_identity = aliased[left].allocation_identity;
                assert_eq!(
                    prepare_regions(aliased).map(|_| ()),
                    Err(GeneratedMoeExpertV2HostAdapterErrorV2::ArgumentsAlias {
                        left: MoeExpertV2BufferRoleV2::ALL[left],
                        right: MoeExpertV2BufferRoleV2::ALL[right],
                    })
                );
            }
        }
    }

    #[test]
    fn v2_target_and_access_roles_are_exact() {
        assert!(validate_observed_target(TARGET).is_ok());
        for target in ["gfx942:xnack+", "gfx942", "gfx1100"] {
            assert!(validate_observed_target(target).is_err(), "{target}");
        }
        for role in MoeExpertV2BufferRoleV2::ALL.into_iter().take(5) {
            assert_eq!(
                access_for_role(role),
                MoeExpertV2BufferAccessV2::SharedReadOnly
            );
        }
        for role in MoeExpertV2BufferRoleV2::ALL.into_iter().skip(5) {
            assert_eq!(
                access_for_role(role),
                MoeExpertV2BufferAccessV2::UniqueReadWrite
            );
        }
    }
}
