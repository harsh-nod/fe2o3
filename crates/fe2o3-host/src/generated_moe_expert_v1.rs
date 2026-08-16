//! Typed host binding for exact bounded MoE expert compute T8/E4/K2/C4 V1.
//!
//! The binding retains immutable activation tiles, expert weights, routing
//! checked routing offsets/inverse data, and route weights plus unique expert, compact, and
//! combined outputs. It exposes neither device addresses nor packed kernarg
//! bytes and carries no compiler, finalizer, load, copy, or launch authority.
//!
//! This is a host-observed routing binding slice, not an end-to-end GPU MoE
//! pipeline. Offsets and inverse routing are joined by an opaque CPU-checked
//! snapshot, but no router execution or device readback is authenticated. Its
//! kernarg ABI is manually pinned to the reviewed source profile and is not
//! compiler-derived.

use crate::{MoeHostObservedRoutingExpertBridgeV1, ObservedContext};
use fe2o3_amd_target::AmdTargetId;
use fe2o3_core::{
    DeviceBufferIdentity, DeviceBufferRegion, DeviceBufferView, DeviceBufferViewMut, DeviceCopy,
};
use std::{
    error::Error,
    fmt,
    mem::{align_of, size_of},
};

pub(crate) const TARGET: &str = "gfx942:xnack-";
pub(crate) const EXPERTS: usize = 4;
pub(crate) const TILE_ELEMENTS: usize = 256;
pub(crate) const EXPERT_CAPACITY: usize = 4;
pub(crate) const OUTPUT_WIDTH: usize = 16;
pub(crate) const ROUTES: usize = 16;
pub(crate) const EXPERT_OFFSETS: usize = 5;
pub(crate) const COMPACT_OUTPUT_ELEMENTS: usize = 256;
pub(crate) const COMBINED_OUTPUT_ELEMENTS: usize = 128;
pub(crate) const GEMM_EXPLICIT_KERNARG_BYTES: usize = 48;
pub(crate) const COMBINE_EXPLICIT_KERNARG_BYTES: usize = 64;
pub(crate) const GEMM_COMPLETE_KERNARG_BYTES: usize = 304;
pub(crate) const COMBINE_COMPLETE_KERNARG_BYTES: usize = 320;
pub(crate) const KERNARG_ALIGNMENT: usize = 8;
pub(crate) const GEMM_GRID: [u32; 3] = [1, 1, 1];
pub(crate) const GEMM_WORKGROUP: [u32; 3] = [64, 1, 1];
pub(crate) const COMBINE_GRID: [u32; 3] = [2, 1, 1];
pub(crate) const COMBINE_WORKGROUP: [u32; 3] = [64, 1, 1];

const ACTIVATION_ELEMENTS: usize = EXPERTS * TILE_ELEMENTS;
const WEIGHT_ELEMENTS: usize = EXPERTS * TILE_ELEMENTS;
const EXPERT_OUTPUT_ELEMENTS: usize = EXPERTS * TILE_ELEMENTS;

/// Exact retained buffer role used by admission diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoeExpertV1BufferRoleV1 {
    ActivationTiles,
    ExpertWeights,
    ExpertOffsets,
    InverseRouting,
    RouteWeights,
    ExpertOutputTiles,
    CompactOutput,
    CombinedOutput,
}

impl MoeExpertV1BufferRoleV1 {
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
            Self::ActivationTiles => ACTIVATION_ELEMENTS,
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

/// Exact access retained for one admitted buffer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoeExpertV1BufferAccessV1 {
    SharedReadOnly,
    UniqueReadWrite,
}

/// One bounded device-to-device copy from a padded expert tile into slot order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MoeExpertCompactCopyV1 {
    expert: u8,
    source_element_offset: usize,
    compact_element_offset: usize,
    elements: usize,
}

impl MoeExpertCompactCopyV1 {
    pub const fn expert(self) -> u8 {
        self.expert
    }

    pub const fn source_element_offset(self) -> usize {
        self.source_element_offset
    }

    pub const fn compact_element_offset(self) -> usize {
        self.compact_element_offset
    }

    pub const fn elements(self) -> usize {
        self.elements
    }

    pub const fn admitted_rows(self) -> usize {
        self.elements / OUTPUT_WIDTH
    }
}

/// Inert exact compact-materialization plan derived from retained routing offsets.
///
/// A future runtime must zero all 256 compact-output elements before executing
/// these four ordered device-to-device copies. Zero-length expert segments are
/// retained in the plan and publish no copy operation. This descriptor grants
/// no HSA copy, load, or dispatch authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MoeExpertCompactPackPlanV1 {
    expert_offsets: [u32; EXPERT_OFFSETS],
    copies: [MoeExpertCompactCopyV1; EXPERTS],
}

impl MoeExpertCompactPackPlanV1 {
    pub fn from_expert_offsets(
        expert_offsets: [u32; EXPERT_OFFSETS],
    ) -> Result<Self, MoeExpertCompactPackPlanErrorV1> {
        if expert_offsets[0] != 0 {
            return Err(MoeExpertCompactPackPlanErrorV1::FirstOffset);
        }
        let mut copies = [MoeExpertCompactCopyV1 {
            expert: 0,
            source_element_offset: 0,
            compact_element_offset: 0,
            elements: 0,
        }; EXPERTS];
        for expert in 0..EXPERTS {
            let start = expert_offsets[expert] as usize;
            let end = expert_offsets[expert + 1] as usize;
            if start > end || end > ROUTES {
                return Err(MoeExpertCompactPackPlanErrorV1::OffsetOrder { expert });
            }
            let admitted_rows = end - start;
            if admitted_rows > EXPERT_CAPACITY {
                return Err(MoeExpertCompactPackPlanErrorV1::Capacity {
                    expert,
                    admitted_rows,
                });
            }
            copies[expert] = MoeExpertCompactCopyV1 {
                expert: expert as u8,
                source_element_offset: expert * TILE_ELEMENTS,
                compact_element_offset: start * OUTPUT_WIDTH,
                elements: admitted_rows * OUTPUT_WIDTH,
            };
        }
        Ok(Self {
            expert_offsets,
            copies,
        })
    }

    pub const fn expert_offsets(self) -> [u32; EXPERT_OFFSETS] {
        self.expert_offsets
    }

    pub const fn copies(self) -> [MoeExpertCompactCopyV1; EXPERTS] {
        self.copies
    }

    /// The complete compact allocation is initialized to +0 before copies.
    pub const fn zero_fill_elements(self) -> usize {
        COMPACT_OUTPUT_ELEMENTS
    }

    pub const fn accepted_routes(self) -> usize {
        self.expert_offsets[EXPERTS] as usize
    }

    pub const fn defined_tail_elements(self) -> usize {
        COMPACT_OUTPUT_ELEMENTS - self.accepted_routes() * OUTPUT_WIDTH
    }

    pub const fn grants_copy_authority(self) -> bool {
        false
    }
}

/// Invalid host snapshot of the exact routing offset buffer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum MoeExpertCompactPackPlanErrorV1 {
    FirstOffset,
    OffsetOrder { expert: usize },
    Capacity { expert: usize, admitted_rows: usize },
}

impl fmt::Display for MoeExpertCompactPackPlanErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid MoE expert compact pack plan: {self:?}")
    }
}

impl Error for MoeExpertCompactPackPlanErrorV1 {}

#[repr(C, align(8))]
struct GemmExplicitKernargV1 {
    bytes: [u8; GEMM_EXPLICIT_KERNARG_BYTES],
}

#[repr(C, align(8))]
struct CombineExplicitKernargV1 {
    bytes: [u8; COMBINE_EXPLICIT_KERNARG_BYTES],
}

const _: () = assert!(size_of::<GemmExplicitKernargV1>() == GEMM_EXPLICIT_KERNARG_BYTES);
const _: () = assert!(size_of::<CombineExplicitKernargV1>() == COMBINE_EXPLICIT_KERNARG_BYTES);
const _: () = assert!(align_of::<GemmExplicitKernargV1>() == KERNARG_ALIGNMENT);
const _: () = assert!(align_of::<CombineExplicitKernargV1>() == KERNARG_ALIGNMENT);

/// Linear exact-profile host binding without artifact or execution authority.
///
/// ```compile_fail
/// use fe2o3_host::GeneratedMoeExpertV1HostAdapterV1;
/// fn replay(value: GeneratedMoeExpertV1HostAdapterV1<'_, '_, '_, '_, '_, '_, '_, '_>) {
///     let _ = value.clone();
/// }
/// ```
#[must_use = "the MoE expert binding retains all eight admitted device regions"]
pub struct GeneratedMoeExpertV1HostAdapterV1<
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
    routing_bridge: MoeHostObservedRoutingExpertBridgeV1<'offsets, 'inverse>,
    _gemm_kernargs: [GemmExplicitKernargV1; EXPERTS],
    _combine_kernarg: CombineExplicitKernargV1,
    _activation_tiles: DeviceBufferView<'activations, u16>,
    _expert_weights: DeviceBufferView<'weights, u16>,
    _route_weights: DeviceBufferView<'route_weights, f32>,
    _expert_output_tiles: DeviceBufferViewMut<'expert_output, f32>,
    _compact_output: DeviceBufferViewMut<'compact_output, f32>,
    _combined_output: DeviceBufferViewMut<'combined_output, f32>,
}

impl fmt::Debug for GeneratedMoeExpertV1HostAdapterV1<'_, '_, '_, '_, '_, '_, '_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedMoeExpertV1HostAdapterV1")
            .field("target", &TARGET)
            .field("shape", &"T8/E4/K2/C4/I16/O16")
            .field("expert_dispatches", &EXPERTS)
            .field("combine_grid", &COMBINE_GRID)
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
    GeneratedMoeExpertV1HostAdapterV1<
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
    /// Validates and retains every exact input, routing, and output region.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        observed: &ObservedContext,
        activation_tiles: DeviceBufferView<'activations, u16>,
        expert_weights: DeviceBufferView<'weights, u16>,
        routing_bridge: MoeHostObservedRoutingExpertBridgeV1<'offsets, 'inverse>,
        route_weights: DeviceBufferView<'route_weights, f32>,
        expert_output_tiles: DeviceBufferViewMut<'expert_output, f32>,
        compact_output: DeviceBufferViewMut<'compact_output, f32>,
        combined_output: DeviceBufferViewMut<'combined_output, f32>,
    ) -> Result<Self, GeneratedMoeExpertV1HostAdapterErrorV1> {
        validate_observed_target(observed.device().target())?;
        let contexts = [
            activation_tiles.context(),
            expert_weights.context(),
            routing_bridge.offsets_view().context(),
            routing_bridge.inverse_view().context(),
            route_weights.context(),
            expert_output_tiles.context(),
            compact_output.context(),
            combined_output.context(),
        ];
        for (role, context) in MoeExpertV1BufferRoleV1::ALL.into_iter().zip(contexts) {
            if !observed.is_for_context(context) {
                return Err(GeneratedMoeExpertV1HostAdapterErrorV1::WrongContext { role });
            }
        }
        let prepared = prepare_regions([
            RegionFacts::from_region(&activation_tiles),
            RegionFacts::from_region(&expert_weights),
            RegionFacts::from_region(routing_bridge.offsets_view()),
            RegionFacts::from_region(routing_bridge.inverse_view()),
            RegionFacts::from_region(&route_weights),
            RegionFacts::from_region(&expert_output_tiles),
            RegionFacts::from_region(&compact_output),
            RegionFacts::from_region(&combined_output),
        ])?;
        Ok(Self {
            observed: observed.clone(),
            routing_bridge,
            _gemm_kernargs: prepared.gemm,
            _combine_kernarg: prepared.combine,
            _activation_tiles: activation_tiles,
            _expert_weights: expert_weights,
            _route_weights: route_weights,
            _expert_output_tiles: expert_output_tiles,
            _compact_output: compact_output,
            _combined_output: combined_output,
        })
    }

    pub fn target(&self) -> &str {
        self.observed.device().target()
    }
    pub const fn profile(&self) -> [usize; 6] {
        [8, 4, 2, 4, 16, 16]
    }
    pub const fn expert_dispatch_count(&self) -> usize {
        EXPERTS
    }
    pub const fn gemm_grid(&self) -> [u32; 3] {
        GEMM_GRID
    }
    pub const fn gemm_workgroup(&self) -> [u32; 3] {
        GEMM_WORKGROUP
    }
    pub const fn combine_grid(&self) -> [u32; 3] {
        COMBINE_GRID
    }
    pub const fn combine_workgroup(&self) -> [u32; 3] {
        COMBINE_WORKGROUP
    }
    pub const fn gemm_explicit_kernarg_byte_len(&self) -> usize {
        GEMM_EXPLICIT_KERNARG_BYTES
    }
    pub const fn gemm_complete_kernarg_byte_len(&self) -> usize {
        GEMM_COMPLETE_KERNARG_BYTES
    }
    pub const fn combine_explicit_kernarg_byte_len(&self) -> usize {
        COMBINE_EXPLICIT_KERNARG_BYTES
    }
    pub const fn combine_complete_kernarg_byte_len(&self) -> usize {
        COMBINE_COMPLETE_KERNARG_BYTES
    }
    pub const fn kernarg_alignment(&self) -> usize {
        KERNARG_ALIGNMENT
    }
    pub const fn grants_artifact_authority(&self) -> bool {
        false
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
    /// Offsets and inverse routing came from one CPU-checked host snapshot.
    ///
    /// This does not authenticate router execution or device readback.
    pub const fn has_routing_consistency_witness(&self) -> bool {
        true
    }
    pub const fn elements_for(&self, role: MoeExpertV1BufferRoleV1) -> usize {
        role.elements()
    }
    pub const fn access_for(&self, role: MoeExpertV1BufferRoleV1) -> MoeExpertV1BufferAccessV1 {
        access_for_role(role)
    }

    /// Returns inert compact-plan arithmetic for the checked host snapshot.
    ///
    /// The plan is consistency-joined to inverse routing, but not to a router
    /// execution or packed activations, and grants no HSA operation authority.
    pub const fn compact_pack_plan(&self) -> MoeExpertCompactPackPlanV1 {
        self.routing_bridge.compact_pack_plan()
    }

    pub const fn routing_snapshot_payload_sha256(&self) -> [u8; 32] {
        self.routing_bridge.payload_sha256()
    }
}

const fn access_for_role(role: MoeExpertV1BufferRoleV1) -> MoeExpertV1BufferAccessV1 {
    match role {
        MoeExpertV1BufferRoleV1::ActivationTiles
        | MoeExpertV1BufferRoleV1::ExpertWeights
        | MoeExpertV1BufferRoleV1::ExpertOffsets
        | MoeExpertV1BufferRoleV1::InverseRouting
        | MoeExpertV1BufferRoleV1::RouteWeights => MoeExpertV1BufferAccessV1::SharedReadOnly,
        MoeExpertV1BufferRoleV1::ExpertOutputTiles
        | MoeExpertV1BufferRoleV1::CompactOutput
        | MoeExpertV1BufferRoleV1::CombinedOutput => MoeExpertV1BufferAccessV1::UniqueReadWrite,
    }
}

fn validate_observed_target(target: &str) -> Result<(), GeneratedMoeExpertV1HostAdapterErrorV1> {
    let expected = AmdTargetId::parse(TARGET)
        .map_err(|_| GeneratedMoeExpertV1HostAdapterErrorV1::ObservedTargetMismatch)?;
    let actual = AmdTargetId::parse(target)
        .map_err(|_| GeneratedMoeExpertV1HostAdapterErrorV1::ObservedTargetMismatch)?;
    expected
        .is_compatible_with_observed(&actual)
        .then_some(())
        .ok_or(GeneratedMoeExpertV1HostAdapterErrorV1::ObservedTargetMismatch)
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
    role: MoeExpertV1BufferRoleV1,
    facts: RegionFacts<I>,
) -> Result<CheckedRegion<I>, GeneratedMoeExpertV1HostAdapterErrorV1> {
    if facts.element_bytes != role.element_bytes()
        || facts.element_alignment != role.element_alignment()
    {
        return Err(GeneratedMoeExpertV1HostAdapterErrorV1::ElementLayout { role });
    }
    let expected_elements = role.elements();
    if facts.region_elements != expected_elements {
        return Err(GeneratedMoeExpertV1HostAdapterErrorV1::Length {
            role,
            expected: expected_elements,
            actual: facts.region_elements,
        });
    }
    if facts.allocation_address == 0 || facts.region_address == 0 {
        return Err(GeneratedMoeExpertV1HostAdapterErrorV1::NullAddress { role });
    }
    if !facts
        .region_address
        .is_multiple_of(role.element_alignment())
    {
        return Err(GeneratedMoeExpertV1HostAdapterErrorV1::Alignment { role });
    }
    let allocation_bytes = facts
        .allocation_elements
        .checked_mul(facts.element_bytes)
        .ok_or(GeneratedMoeExpertV1HostAdapterErrorV1::RegionOverflow { role })?;
    let allocation_end = facts
        .allocation_address
        .checked_add(allocation_bytes)
        .ok_or(GeneratedMoeExpertV1HostAdapterErrorV1::RegionOverflow { role })?;
    let region_bytes = facts
        .region_elements
        .checked_mul(facts.element_bytes)
        .ok_or(GeneratedMoeExpertV1HostAdapterErrorV1::RegionOverflow { role })?;
    let region_end = facts
        .region_address
        .checked_add(region_bytes)
        .ok_or(GeneratedMoeExpertV1HostAdapterErrorV1::RegionOverflow { role })?;
    let expected_address = facts
        .allocation_address
        .checked_add(facts.region_byte_start)
        .ok_or(GeneratedMoeExpertV1HostAdapterErrorV1::RegionOverflow { role })?;
    if facts.region_byte_start > facts.region_byte_end
        || facts.region_byte_end > allocation_bytes
        || facts.region_byte_end.checked_sub(facts.region_byte_start) != Some(region_bytes)
        || expected_address != facts.region_address
        || facts.region_address < facts.allocation_address
        || region_end > allocation_end
    {
        return Err(GeneratedMoeExpertV1HostAdapterErrorV1::InvalidRegion { role });
    }
    Ok(CheckedRegion {
        allocation_identity: facts.allocation_identity,
        address: u64::try_from(facts.region_address)
            .map_err(|_| GeneratedMoeExpertV1HostAdapterErrorV1::PointerWidth { role })?,
        byte_start: facts.region_address,
        byte_end: region_end,
    })
}

struct PreparedKernargsV1 {
    gemm: [GemmExplicitKernargV1; EXPERTS],
    combine: CombineExplicitKernargV1,
}

fn push_slice(bytes: &mut [u8], slot: usize, address: u64, elements: usize) {
    let offset = slot * 16;
    bytes[offset..offset + 8].copy_from_slice(&address.to_le_bytes());
    bytes[offset + 8..offset + 16].copy_from_slice(&(elements as u64).to_le_bytes());
}

fn prepare_regions<I: Copy + Eq>(
    facts: [RegionFacts<I>; 8],
) -> Result<PreparedKernargsV1, GeneratedMoeExpertV1HostAdapterErrorV1> {
    let roles = MoeExpertV1BufferRoleV1::ALL;
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
                return Err(GeneratedMoeExpertV1HostAdapterErrorV1::ArgumentsAlias {
                    left: roles[left],
                    right: roles[right],
                });
            }
        }
    }

    let mut gemm = std::array::from_fn(|_| GemmExplicitKernargV1 {
        bytes: [0; GEMM_EXPLICIT_KERNARG_BYTES],
    });
    for (expert, kernarg) in gemm.iter_mut().enumerate() {
        let activation_offset = expert * TILE_ELEMENTS * size_of::<u16>();
        let weight_offset = expert * TILE_ELEMENTS * size_of::<u16>();
        let output_offset = expert * TILE_ELEMENTS * size_of::<f32>();
        push_slice(
            &mut kernarg.bytes,
            0,
            checked[0].address + activation_offset as u64,
            TILE_ELEMENTS,
        );
        push_slice(
            &mut kernarg.bytes,
            1,
            checked[1].address + weight_offset as u64,
            TILE_ELEMENTS,
        );
        push_slice(
            &mut kernarg.bytes,
            2,
            checked[5].address + output_offset as u64,
            TILE_ELEMENTS,
        );
    }

    let mut combine = CombineExplicitKernargV1 {
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
    Ok(PreparedKernargsV1 { gemm, combine })
}

/// Authority-free rejection while preparing the exact host binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum GeneratedMoeExpertV1HostAdapterErrorV1 {
    ObservedTargetMismatch,
    WrongContext {
        role: MoeExpertV1BufferRoleV1,
    },
    ElementLayout {
        role: MoeExpertV1BufferRoleV1,
    },
    Length {
        role: MoeExpertV1BufferRoleV1,
        expected: usize,
        actual: usize,
    },
    NullAddress {
        role: MoeExpertV1BufferRoleV1,
    },
    Alignment {
        role: MoeExpertV1BufferRoleV1,
    },
    RegionOverflow {
        role: MoeExpertV1BufferRoleV1,
    },
    InvalidRegion {
        role: MoeExpertV1BufferRoleV1,
    },
    PointerWidth {
        role: MoeExpertV1BufferRoleV1,
    },
    ArgumentsAlias {
        left: MoeExpertV1BufferRoleV1,
        right: MoeExpertV1BufferRoleV1,
    },
}

impl fmt::Display for GeneratedMoeExpertV1HostAdapterErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "MoE expert host binding rejected: {self:?}")
    }
}

impl Error for GeneratedMoeExpertV1HostAdapterErrorV1 {}

#[cfg(test)]
mod tests {
    use super::*;

    fn region(identity: u64, address: usize, role: MoeExpertV1BufferRoleV1) -> RegionFacts<u64> {
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

    fn canonical() -> [RegionFacts<u64>; 8] {
        std::array::from_fn(|index| {
            region(
                (index + 1) as u64,
                0x2100 + index * 0x4000,
                MoeExpertV1BufferRoleV1::ALL[index],
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
    fn exactly_four_expert_kernargs_select_disjoint_tiles() {
        let prepared = prepare_regions(canonical()).unwrap();
        for expert in 0..EXPERTS {
            let bytes = &prepared.gemm[expert].bytes;
            assert_eq!(
                slice(bytes, 0),
                ((0x2100 + expert * TILE_ELEMENTS * 2) as u64, 256)
            );
            assert_eq!(
                slice(bytes, 1),
                ((0x6100 + expert * TILE_ELEMENTS * 2) as u64, 256)
            );
            assert_eq!(
                slice(bytes, 2),
                ((0x16100 + expert * TILE_ELEMENTS * 4) as u64, 256)
            );
        }
    }

    #[test]
    fn combine_kernarg_uses_compact_inverse_weight_and_unique_output() {
        let prepared = prepare_regions(canonical()).unwrap();
        assert_eq!(slice(&prepared.combine.bytes, 0), (0x1a100, 256));
        assert_eq!(slice(&prepared.combine.bytes, 1), (0xe100, 16));
        assert_eq!(slice(&prepared.combine.bytes, 2), (0x12100, 16));
        assert_eq!(slice(&prepared.combine.bytes, 3), (0x1e100, 128));
    }

    #[test]
    fn hostile_extent_layout_and_provenance_substitutions_fail_closed() {
        let mutations: &[fn(&mut [RegionFacts<u64>; 8])] = &[
            |r| r[0].region_elements -= 1,
            |r| r[1].region_elements += 1,
            |r| r[2].region_elements = 4,
            |r| r[3].region_elements = 15,
            |r| r[4].element_bytes = 8,
            |r| r[5].region_byte_end -= 4,
            |r| r[6].region_byte_start += 4,
            |r| r[7].allocation_elements = 1,
            |r| r[0].region_address += 1,
            |r| r[1].allocation_address = 0,
        ];
        for (index, mutate) in mutations.iter().enumerate() {
            let mut regions = canonical();
            mutate(&mut regions);
            assert!(
                prepare_regions(regions).is_err(),
                "mutation {index} escaped"
            );
        }
    }

    #[test]
    fn every_cross_role_alias_is_rejected() {
        for left in 0..8 {
            for right in left + 1..8 {
                let mut regions = canonical();
                regions[right].allocation_identity = regions[left].allocation_identity;
                assert_eq!(
                    prepare_regions(regions).map(|_| ()),
                    Err(GeneratedMoeExpertV1HostAdapterErrorV1::ArgumentsAlias {
                        left: MoeExpertV1BufferRoleV1::ALL[left],
                        right: MoeExpertV1BufferRoleV1::ALL[right],
                    })
                );
            }
        }
    }

    #[test]
    fn access_roles_retain_five_immutable_and_three_unique_leases() {
        for role in MoeExpertV1BufferRoleV1::ALL.into_iter().take(5) {
            assert_eq!(
                access_for_role(role),
                MoeExpertV1BufferAccessV1::SharedReadOnly
            );
        }
        for role in MoeExpertV1BufferRoleV1::ALL.into_iter().skip(5) {
            assert_eq!(
                access_for_role(role),
                MoeExpertV1BufferAccessV1::UniqueReadWrite
            );
        }
    }

    #[test]
    fn exact_target_is_required() {
        assert!(validate_observed_target(TARGET).is_ok());
        for target in ["gfx942:xnack+", "gfx942", "gfx1100"] {
            assert!(validate_observed_target(target).is_err(), "{target}");
        }
    }

    #[test]
    fn compact_pack_zero_fills_then_copies_only_admitted_rows() {
        let plan = MoeExpertCompactPackPlanV1::from_expert_offsets([0, 4, 4, 7, 9]).unwrap();
        assert_eq!(plan.zero_fill_elements(), 256);
        assert_eq!(plan.accepted_routes(), 9);
        assert_eq!(plan.defined_tail_elements(), 112);
        assert_eq!(
            plan.copies(),
            [
                MoeExpertCompactCopyV1 {
                    expert: 0,
                    source_element_offset: 0,
                    compact_element_offset: 0,
                    elements: 64,
                },
                MoeExpertCompactCopyV1 {
                    expert: 1,
                    source_element_offset: 256,
                    compact_element_offset: 64,
                    elements: 0,
                },
                MoeExpertCompactCopyV1 {
                    expert: 2,
                    source_element_offset: 512,
                    compact_element_offset: 64,
                    elements: 48,
                },
                MoeExpertCompactCopyV1 {
                    expert: 3,
                    source_element_offset: 768,
                    compact_element_offset: 112,
                    elements: 32,
                },
            ]
        );
        assert!(!plan.grants_copy_authority());
    }

    #[test]
    fn compact_pack_rejects_offset_capacity_and_route_bound_mutations() {
        for offsets in [
            [1, 1, 1, 1, 1],
            [0, 4, 3, 3, 3],
            [0, 5, 5, 5, 5],
            [0, 4, 8, 12, 17],
        ] {
            assert!(
                MoeExpertCompactPackPlanV1::from_expert_offsets(offsets).is_err(),
                "{offsets:?}"
            );
        }
    }
}
