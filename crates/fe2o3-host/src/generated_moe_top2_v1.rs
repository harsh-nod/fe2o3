//! Typed host binding for exact deterministic MoE routing T8/E4/K2/C4 V1.
//!
//! The binding retains one shared score lease and seven unique output leases.
//! It exposes neither device addresses nor packed kernarg bytes through its
//! public API and carries no publication, load, or launch authority.

use crate::ObservedContext;
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
pub(crate) const EXPLICIT_KERNARG_BYTES: usize = 128;
pub(crate) const COMPLETE_KERNARG_BYTES: usize = 384;
pub(crate) const KERNARG_ALIGNMENT: usize = 8;
pub(crate) const GRID: [u32; 3] = [1, 1, 1];
pub(crate) const WORKGROUP: [u32; 3] = [64, 1, 1];

const LOGIT_ELEMENTS: usize = 32;
const ROUTES: usize = 16;
const EXPERTS: usize = 4;
const OFFSETS: usize = 5;

/// Exact argument role used by binding diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoeTop2V1BufferRoleV1 {
    Logits,
    Top2Experts,
    RequestedCounts,
    AdmittedCounts,
    ExpertOffsets,
    RouteSlots,
    Permutation,
    Inverse,
}

impl MoeTop2V1BufferRoleV1 {
    const ALL: [Self; 8] = [
        Self::Logits,
        Self::Top2Experts,
        Self::RequestedCounts,
        Self::AdmittedCounts,
        Self::ExpertOffsets,
        Self::RouteSlots,
        Self::Permutation,
        Self::Inverse,
    ];

    const fn elements(self) -> usize {
        match self {
            Self::Logits => LOGIT_ELEMENTS,
            Self::Top2Experts | Self::RouteSlots | Self::Permutation | Self::Inverse => ROUTES,
            Self::RequestedCounts | Self::AdmittedCounts => EXPERTS,
            Self::ExpertOffsets => OFFSETS,
        }
    }

    const fn element_bytes(self) -> usize {
        match self {
            Self::Logits => size_of::<f32>(),
            _ => size_of::<u32>(),
        }
    }

    const fn element_alignment(self) -> usize {
        match self {
            Self::Logits => align_of::<f32>(),
            _ => align_of::<u32>(),
        }
    }
}

/// Exact memory access attached to a retained argument.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoeTop2V1BufferAccessV1 {
    SharedReadOnly,
    UniqueReadWrite,
}

#[repr(C, align(8))]
struct ExplicitKernargV1 {
    bytes: [u8; EXPLICIT_KERNARG_BYTES],
}

const _: () = assert!(size_of::<ExplicitKernargV1>() == EXPLICIT_KERNARG_BYTES);
const _: () = assert!(align_of::<ExplicitKernargV1>() == KERNARG_ALIGNMENT);

/// Linear exact-profile host binding.
///
/// ```compile_fail
/// use fe2o3_host::GeneratedMoeTop2V1HostAdapterV1;
/// fn replay(value: GeneratedMoeTop2V1HostAdapterV1<'_, '_, '_, '_, '_, '_, '_, '_>) {
///     let _ = value.clone();
/// }
/// ```
#[must_use = "the MoE routing binding must enter its one-shot lifecycle"]
pub struct GeneratedMoeTop2V1HostAdapterV1<
    'logits,
    'top2,
    'requested,
    'admitted,
    'offsets,
    'slots,
    'permutation,
    'inverse,
> {
    observed: ObservedContext,
    explicit_kernarg: ExplicitKernargV1,
    _logits: DeviceBufferView<'logits, f32>,
    _top2_experts: DeviceBufferViewMut<'top2, u32>,
    _requested_counts: DeviceBufferViewMut<'requested, u32>,
    _admitted_counts: DeviceBufferViewMut<'admitted, u32>,
    _expert_offsets: DeviceBufferViewMut<'offsets, u32>,
    _route_slots: DeviceBufferViewMut<'slots, u32>,
    _permutation: DeviceBufferViewMut<'permutation, u32>,
    _inverse: DeviceBufferViewMut<'inverse, u32>,
}

impl fmt::Debug for GeneratedMoeTop2V1HostAdapterV1<'_, '_, '_, '_, '_, '_, '_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedMoeTop2V1HostAdapterV1")
            .field("target", &TARGET)
            .field("shape", &"T8/E4/K2/C4")
            .field("grid", &GRID)
            .field("workgroup", &WORKGROUP)
            .finish_non_exhaustive()
    }
}

impl<'logits, 'top2, 'requested, 'admitted, 'offsets, 'slots, 'permutation, 'inverse>
    GeneratedMoeTop2V1HostAdapterV1<
        'logits,
        'top2,
        'requested,
        'admitted,
        'offsets,
        'slots,
        'permutation,
        'inverse,
    >
{
    /// Validates and retains the exact score and routing-output invocation.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        observed: &ObservedContext,
        logits: DeviceBufferView<'logits, f32>,
        top2_experts: DeviceBufferViewMut<'top2, u32>,
        requested_counts: DeviceBufferViewMut<'requested, u32>,
        admitted_counts: DeviceBufferViewMut<'admitted, u32>,
        expert_offsets: DeviceBufferViewMut<'offsets, u32>,
        route_slots: DeviceBufferViewMut<'slots, u32>,
        permutation: DeviceBufferViewMut<'permutation, u32>,
        inverse: DeviceBufferViewMut<'inverse, u32>,
    ) -> Result<Self, GeneratedMoeTop2V1HostAdapterErrorV1> {
        validate_observed_target(observed.device().target())?;
        let contexts = [
            logits.context(),
            top2_experts.context(),
            requested_counts.context(),
            admitted_counts.context(),
            expert_offsets.context(),
            route_slots.context(),
            permutation.context(),
            inverse.context(),
        ];
        for (role, context) in MoeTop2V1BufferRoleV1::ALL.into_iter().zip(contexts) {
            if !observed.is_for_context(context) {
                return Err(GeneratedMoeTop2V1HostAdapterErrorV1::WrongContext { role });
            }
        }
        let explicit_kernarg = prepare_regions([
            RegionFacts::from_region(&logits),
            RegionFacts::from_region(&top2_experts),
            RegionFacts::from_region(&requested_counts),
            RegionFacts::from_region(&admitted_counts),
            RegionFacts::from_region(&expert_offsets),
            RegionFacts::from_region(&route_slots),
            RegionFacts::from_region(&permutation),
            RegionFacts::from_region(&inverse),
        ])?;
        Ok(Self {
            observed: observed.clone(),
            explicit_kernarg,
            _logits: logits,
            _top2_experts: top2_experts,
            _requested_counts: requested_counts,
            _admitted_counts: admitted_counts,
            _expert_offsets: expert_offsets,
            _route_slots: route_slots,
            _permutation: permutation,
            _inverse: inverse,
        })
    }

    pub const fn target(&self) -> &'static str {
        TARGET
    }
    pub const fn profile(&self) -> [usize; 4] {
        [8, 4, 2, 4]
    }
    pub const fn grid(&self) -> [u32; 3] {
        GRID
    }
    pub const fn workgroup(&self) -> [u32; 3] {
        WORKGROUP
    }
    pub const fn code_object_version(&self) -> u8 {
        6
    }
    pub const fn explicit_kernarg_byte_len(&self) -> usize {
        EXPLICIT_KERNARG_BYTES
    }
    pub const fn complete_kernarg_byte_len(&self) -> usize {
        COMPLETE_KERNARG_BYTES
    }
    pub const fn kernarg_alignment(&self) -> usize {
        KERNARG_ALIGNMENT
    }
    pub const fn is_deterministic_routing_only(&self) -> bool {
        true
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    pub const fn elements_for(&self, role: MoeTop2V1BufferRoleV1) -> usize {
        role.elements()
    }

    pub const fn access_for(&self, role: MoeTop2V1BufferRoleV1) -> MoeTop2V1BufferAccessV1 {
        access_for_role(role)
    }

    pub(crate) const fn observed_context_v1(&self) -> &ObservedContext {
        &self.observed
    }
    pub(crate) const fn explicit_kernarg_bytes_v1(&self) -> &[u8; EXPLICIT_KERNARG_BYTES] {
        &self.explicit_kernarg.bytes
    }
}

const fn access_for_role(role: MoeTop2V1BufferRoleV1) -> MoeTop2V1BufferAccessV1 {
    match role {
        MoeTop2V1BufferRoleV1::Logits => MoeTop2V1BufferAccessV1::SharedReadOnly,
        _ => MoeTop2V1BufferAccessV1::UniqueReadWrite,
    }
}

fn validate_observed_target(target: &str) -> Result<(), GeneratedMoeTop2V1HostAdapterErrorV1> {
    let expected = AmdTargetId::parse(TARGET)
        .map_err(|_| GeneratedMoeTop2V1HostAdapterErrorV1::ObservedTargetMismatch)?;
    let actual = AmdTargetId::parse(target)
        .map_err(|_| GeneratedMoeTop2V1HostAdapterErrorV1::ObservedTargetMismatch)?;
    expected
        .is_compatible_with_observed(&actual)
        .then_some(())
        .ok_or(GeneratedMoeTop2V1HostAdapterErrorV1::ObservedTargetMismatch)
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
    elements: usize,
}

fn validate_region<I: Copy>(
    role: MoeTop2V1BufferRoleV1,
    facts: RegionFacts<I>,
) -> Result<CheckedRegion<I>, GeneratedMoeTop2V1HostAdapterErrorV1> {
    if facts.element_bytes != role.element_bytes()
        || facts.element_alignment != role.element_alignment()
    {
        return Err(GeneratedMoeTop2V1HostAdapterErrorV1::ElementLayout { role });
    }
    let expected_elements = role.elements();
    if facts.region_elements != expected_elements {
        return Err(GeneratedMoeTop2V1HostAdapterErrorV1::Length {
            role,
            expected: expected_elements,
            actual: facts.region_elements,
        });
    }
    if facts.allocation_address == 0 || facts.region_address == 0 {
        return Err(GeneratedMoeTop2V1HostAdapterErrorV1::NullAddress { role });
    }
    if !facts
        .region_address
        .is_multiple_of(role.element_alignment())
    {
        return Err(GeneratedMoeTop2V1HostAdapterErrorV1::Alignment { role });
    }
    let allocation_bytes = facts
        .allocation_elements
        .checked_mul(facts.element_bytes)
        .ok_or(GeneratedMoeTop2V1HostAdapterErrorV1::RegionOverflow { role })?;
    let allocation_end = facts
        .allocation_address
        .checked_add(allocation_bytes)
        .ok_or(GeneratedMoeTop2V1HostAdapterErrorV1::RegionOverflow { role })?;
    let region_bytes = facts
        .region_elements
        .checked_mul(facts.element_bytes)
        .ok_or(GeneratedMoeTop2V1HostAdapterErrorV1::RegionOverflow { role })?;
    let region_end = facts
        .region_address
        .checked_add(region_bytes)
        .ok_or(GeneratedMoeTop2V1HostAdapterErrorV1::RegionOverflow { role })?;
    let expected_address = facts
        .allocation_address
        .checked_add(facts.region_byte_start)
        .ok_or(GeneratedMoeTop2V1HostAdapterErrorV1::RegionOverflow { role })?;
    if facts.region_byte_start > facts.region_byte_end
        || facts.region_byte_end > allocation_bytes
        || facts.region_byte_end.checked_sub(facts.region_byte_start) != Some(region_bytes)
        || expected_address != facts.region_address
        || facts.region_address < facts.allocation_address
        || region_end > allocation_end
    {
        return Err(GeneratedMoeTop2V1HostAdapterErrorV1::InvalidRegion { role });
    }
    Ok(CheckedRegion {
        allocation_identity: facts.allocation_identity,
        address: u64::try_from(facts.region_address)
            .map_err(|_| GeneratedMoeTop2V1HostAdapterErrorV1::PointerWidth { role })?,
        byte_start: facts.region_address,
        byte_end: region_end,
        elements: expected_elements,
    })
}

fn prepare_regions<I: Copy + Eq>(
    facts: [RegionFacts<I>; 8],
) -> Result<ExplicitKernargV1, GeneratedMoeTop2V1HostAdapterErrorV1> {
    let roles = MoeTop2V1BufferRoleV1::ALL;
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
                return Err(GeneratedMoeTop2V1HostAdapterErrorV1::ArgumentsAlias {
                    left: roles[left],
                    right: roles[right],
                });
            }
        }
    }
    let mut bytes = [0_u8; EXPLICIT_KERNARG_BYTES];
    for (slot, region) in bytes.chunks_exact_mut(16).zip(checked) {
        slot[..8].copy_from_slice(&region.address.to_le_bytes());
        slot[8..].copy_from_slice(&(region.elements as u64).to_le_bytes());
    }
    Ok(ExplicitKernargV1 { bytes })
}

/// Authority-free rejection while preparing the exact binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum GeneratedMoeTop2V1HostAdapterErrorV1 {
    ObservedTargetMismatch,
    WrongContext {
        role: MoeTop2V1BufferRoleV1,
    },
    ElementLayout {
        role: MoeTop2V1BufferRoleV1,
    },
    Length {
        role: MoeTop2V1BufferRoleV1,
        expected: usize,
        actual: usize,
    },
    NullAddress {
        role: MoeTop2V1BufferRoleV1,
    },
    Alignment {
        role: MoeTop2V1BufferRoleV1,
    },
    RegionOverflow {
        role: MoeTop2V1BufferRoleV1,
    },
    InvalidRegion {
        role: MoeTop2V1BufferRoleV1,
    },
    PointerWidth {
        role: MoeTop2V1BufferRoleV1,
    },
    ArgumentsAlias {
        left: MoeTop2V1BufferRoleV1,
        right: MoeTop2V1BufferRoleV1,
    },
}

impl fmt::Display for GeneratedMoeTop2V1HostAdapterErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "MoE routing host binding rejected: {self:?}")
    }
}

impl Error for GeneratedMoeTop2V1HostAdapterErrorV1 {}

#[cfg(test)]
mod tests {
    use super::*;

    fn region(identity: u64, address: usize, role: MoeTop2V1BufferRoleV1) -> RegionFacts<u64> {
        let elements = role.elements();
        let bytes = role.element_bytes();
        RegionFacts {
            allocation_identity: identity,
            allocation_address: address - 0x80,
            allocation_elements: elements + 32,
            region_address: address,
            region_elements: elements,
            region_byte_start: 0x80,
            region_byte_end: 0x80 + elements * bytes,
            element_bytes: bytes,
            element_alignment: role.element_alignment(),
        }
    }

    fn canonical() -> [RegionFacts<u64>; 8] {
        std::array::from_fn(|index| {
            region(
                (index + 1) as u64,
                0x1080 + index * 0x1000,
                MoeTop2V1BufferRoleV1::ALL[index],
            )
        })
    }

    #[test]
    fn exact_eight_slice_abi_is_packed() {
        let bytes = prepare_regions(canonical()).unwrap().bytes;
        for (index, role) in MoeTop2V1BufferRoleV1::ALL.into_iter().enumerate() {
            let offset = index * 16;
            let address = (0x1080 + index * 0x1000) as u64;
            assert_eq!(&bytes[offset..offset + 8], &address.to_le_bytes());
            assert_eq!(
                &bytes[offset + 8..offset + 16],
                &(role.elements() as u64).to_le_bytes()
            );
        }
    }

    #[test]
    fn hostile_extent_layout_and_provenance_substitutions_fail_closed() {
        let mutations: &[fn(&mut [RegionFacts<u64>; 8])] = &[
            |r| r[0].region_elements = 31,
            |r| r[1].region_elements = 15,
            |r| r[2].region_elements = 5,
            |r| r[4].region_elements = 4,
            |r| r[7].allocation_address = 0,
            |r| r[0].region_address += 1,
            |r| r[1].element_bytes = 8,
            |r| r[4].region_byte_end -= 4,
            |r| r[5].region_byte_start += 4,
            |r| r[6].allocation_elements = 1,
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
    fn every_argument_pair_must_be_disjoint() {
        for left in 0..8 {
            for right in left + 1..8 {
                let mut regions = canonical();
                regions[right].allocation_identity = regions[left].allocation_identity;
                assert_eq!(
                    prepare_regions(regions).map(|_| ()),
                    Err(GeneratedMoeTop2V1HostAdapterErrorV1::ArgumentsAlias {
                        left: MoeTop2V1BufferRoleV1::ALL[left],
                        right: MoeTop2V1BufferRoleV1::ALL[right],
                    })
                );
            }
        }
    }

    #[test]
    fn access_roles_match_the_pinned_descriptor() {
        assert_eq!(
            access_for_role(MoeTop2V1BufferRoleV1::Logits),
            MoeTop2V1BufferAccessV1::SharedReadOnly
        );
        for role in MoeTop2V1BufferRoleV1::ALL.into_iter().skip(1) {
            assert_eq!(
                access_for_role(role),
                MoeTop2V1BufferAccessV1::UniqueReadWrite
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
}
