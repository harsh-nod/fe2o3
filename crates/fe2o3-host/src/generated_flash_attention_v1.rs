//! Typed host binding for exact FlashAttention B1/H1/N8/D16 V1.
//!
//! The binding retains three shared input leases and one unique output lease.
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
pub(crate) const ELEMENTS: usize = 128;
pub(crate) const EXPLICIT_KERNARG_BYTES: usize = 64;
pub(crate) const COMPLETE_KERNARG_BYTES: usize = 320;
pub(crate) const KERNARG_ALIGNMENT: usize = 8;
pub(crate) const GRID: [u32; 3] = [1, 1, 1];
pub(crate) const WORKGROUP: [u32; 3] = [64, 1, 1];

/// Exact argument role used by binding diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlashAttentionV1BufferRoleV1 {
    Query,
    Key,
    Value,
    Output,
}

/// Exact memory access attached to a retained argument.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlashAttentionV1BufferAccessV1 {
    SharedReadOnly,
    UniqueWriteOnly,
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
/// use fe2o3_host::GeneratedFlashAttentionV1HostAdapterV1;
/// fn replay(value: GeneratedFlashAttentionV1HostAdapterV1<'_, '_, '_, '_>) {
///     let _ = value.clone();
/// }
/// ```
#[must_use = "the FlashAttention binding must enter its one-shot lifecycle"]
pub struct GeneratedFlashAttentionV1HostAdapterV1<'query, 'key, 'value, 'output> {
    observed: ObservedContext,
    explicit_kernarg: ExplicitKernargV1,
    _query: DeviceBufferView<'query, f32>,
    _key: DeviceBufferView<'key, f32>,
    _value: DeviceBufferView<'value, f32>,
    _output: DeviceBufferViewMut<'output, f32>,
}

impl fmt::Debug for GeneratedFlashAttentionV1HostAdapterV1<'_, '_, '_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedFlashAttentionV1HostAdapterV1")
            .field("target", &TARGET)
            .field("shape", &"B1/H1/N8/D16")
            .field("grid", &GRID)
            .field("workgroup", &WORKGROUP)
            .finish_non_exhaustive()
    }
}

impl<'query, 'key, 'value, 'output>
    GeneratedFlashAttentionV1HostAdapterV1<'query, 'key, 'value, 'output>
{
    /// Validates and retains the exact Q/K/V/output invocation.
    pub fn prepare(
        observed: &ObservedContext,
        query: DeviceBufferView<'query, f32>,
        key: DeviceBufferView<'key, f32>,
        value: DeviceBufferView<'value, f32>,
        output: DeviceBufferViewMut<'output, f32>,
    ) -> Result<Self, GeneratedFlashAttentionV1HostAdapterErrorV1> {
        validate_observed_target(observed.device().target())?;
        for (role, context) in [
            (FlashAttentionV1BufferRoleV1::Query, query.context()),
            (FlashAttentionV1BufferRoleV1::Key, key.context()),
            (FlashAttentionV1BufferRoleV1::Value, value.context()),
            (FlashAttentionV1BufferRoleV1::Output, output.context()),
        ] {
            if !observed.is_for_context(context) {
                return Err(GeneratedFlashAttentionV1HostAdapterErrorV1::WrongContext { role });
            }
        }
        let explicit_kernarg = prepare_regions([
            RegionFacts::from_region(&query),
            RegionFacts::from_region(&key),
            RegionFacts::from_region(&value),
            RegionFacts::from_region(&output),
        ])?;
        Ok(Self {
            observed: observed.clone(),
            explicit_kernarg,
            _query: query,
            _key: key,
            _value: value,
            _output: output,
        })
    }

    pub const fn target(&self) -> &'static str {
        TARGET
    }
    pub const fn shape(&self) -> [usize; 4] {
        [1, 1, 8, 16]
    }
    pub const fn argument_elements(&self) -> usize {
        ELEMENTS
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
    pub const fn is_causal(&self) -> bool {
        true
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    pub const fn access_for(
        &self,
        role: FlashAttentionV1BufferRoleV1,
    ) -> FlashAttentionV1BufferAccessV1 {
        match role {
            FlashAttentionV1BufferRoleV1::Query
            | FlashAttentionV1BufferRoleV1::Key
            | FlashAttentionV1BufferRoleV1::Value => FlashAttentionV1BufferAccessV1::SharedReadOnly,
            FlashAttentionV1BufferRoleV1::Output => FlashAttentionV1BufferAccessV1::UniqueWriteOnly,
        }
    }

    pub(crate) const fn observed_context_v1(&self) -> &ObservedContext {
        &self.observed
    }
    pub(crate) const fn explicit_kernarg_bytes_v1(&self) -> &[u8; EXPLICIT_KERNARG_BYTES] {
        &self.explicit_kernarg.bytes
    }
}

fn validate_observed_target(
    target: &str,
) -> Result<(), GeneratedFlashAttentionV1HostAdapterErrorV1> {
    let expected = AmdTargetId::parse(TARGET)
        .map_err(|_| GeneratedFlashAttentionV1HostAdapterErrorV1::ObservedTargetMismatch)?;
    let actual = AmdTargetId::parse(target)
        .map_err(|_| GeneratedFlashAttentionV1HostAdapterErrorV1::ObservedTargetMismatch)?;
    expected
        .is_compatible_with_observed(&actual)
        .then_some(())
        .ok_or(GeneratedFlashAttentionV1HostAdapterErrorV1::ObservedTargetMismatch)
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
    role: FlashAttentionV1BufferRoleV1,
    facts: RegionFacts<I>,
) -> Result<CheckedRegion<I>, GeneratedFlashAttentionV1HostAdapterErrorV1> {
    if facts.element_bytes != size_of::<f32>() || facts.element_alignment != align_of::<f32>() {
        return Err(GeneratedFlashAttentionV1HostAdapterErrorV1::ElementLayout { role });
    }
    if facts.region_elements != ELEMENTS {
        return Err(GeneratedFlashAttentionV1HostAdapterErrorV1::Length {
            role,
            expected: ELEMENTS,
            actual: facts.region_elements,
        });
    }
    if facts.allocation_address == 0 || facts.region_address == 0 {
        return Err(GeneratedFlashAttentionV1HostAdapterErrorV1::NullAddress { role });
    }
    if !facts.region_address.is_multiple_of(align_of::<f32>()) {
        return Err(GeneratedFlashAttentionV1HostAdapterErrorV1::Alignment { role });
    }
    let allocation_bytes = facts
        .allocation_elements
        .checked_mul(facts.element_bytes)
        .ok_or(GeneratedFlashAttentionV1HostAdapterErrorV1::RegionOverflow { role })?;
    let allocation_end = facts
        .allocation_address
        .checked_add(allocation_bytes)
        .ok_or(GeneratedFlashAttentionV1HostAdapterErrorV1::RegionOverflow { role })?;
    let region_bytes = facts
        .region_elements
        .checked_mul(facts.element_bytes)
        .ok_or(GeneratedFlashAttentionV1HostAdapterErrorV1::RegionOverflow { role })?;
    let region_end = facts
        .region_address
        .checked_add(region_bytes)
        .ok_or(GeneratedFlashAttentionV1HostAdapterErrorV1::RegionOverflow { role })?;
    let expected_address = facts
        .allocation_address
        .checked_add(facts.region_byte_start)
        .ok_or(GeneratedFlashAttentionV1HostAdapterErrorV1::RegionOverflow { role })?;
    if facts.region_byte_start > facts.region_byte_end
        || facts.region_byte_end > allocation_bytes
        || facts.region_byte_end.checked_sub(facts.region_byte_start) != Some(region_bytes)
        || expected_address != facts.region_address
        || facts.region_address < facts.allocation_address
        || region_end > allocation_end
    {
        return Err(GeneratedFlashAttentionV1HostAdapterErrorV1::InvalidRegion { role });
    }
    Ok(CheckedRegion {
        allocation_identity: facts.allocation_identity,
        address: u64::try_from(facts.region_address)
            .map_err(|_| GeneratedFlashAttentionV1HostAdapterErrorV1::PointerWidth { role })?,
        byte_start: facts.region_address,
        byte_end: region_end,
    })
}

fn prepare_regions<I: Copy + Eq>(
    facts: [RegionFacts<I>; 4],
) -> Result<ExplicitKernargV1, GeneratedFlashAttentionV1HostAdapterErrorV1> {
    let roles = [
        FlashAttentionV1BufferRoleV1::Query,
        FlashAttentionV1BufferRoleV1::Key,
        FlashAttentionV1BufferRoleV1::Value,
        FlashAttentionV1BufferRoleV1::Output,
    ];
    let checked = [
        validate_region(roles[0], facts[0])?,
        validate_region(roles[1], facts[1])?,
        validate_region(roles[2], facts[2])?,
        validate_region(roles[3], facts[3])?,
    ];
    let output = checked[3];
    for (input_index, input) in checked[..3].iter().copied().enumerate() {
        if input.allocation_identity == output.allocation_identity
            && input.byte_start < output.byte_end
            && output.byte_start < input.byte_end
        {
            return Err(
                GeneratedFlashAttentionV1HostAdapterErrorV1::OutputAliasesInput {
                    input: roles[input_index],
                },
            );
        }
        // Allocation identities are authoritative provenance. The physical
        // interval check additionally fails closed if a reviewed allocator
        // ever reports distinct identities for overlapping live allocations.
        if input.byte_start < output.byte_end && output.byte_start < input.byte_end {
            return Err(
                GeneratedFlashAttentionV1HostAdapterErrorV1::OutputAliasesInput {
                    input: roles[input_index],
                },
            );
        }
    }
    let mut bytes = [0_u8; EXPLICIT_KERNARG_BYTES];
    for (slot, region) in bytes.chunks_exact_mut(16).zip(checked) {
        slot[..8].copy_from_slice(&region.address.to_le_bytes());
        slot[8..].copy_from_slice(&(ELEMENTS as u64).to_le_bytes());
    }
    Ok(ExplicitKernargV1 { bytes })
}

/// Authority-free rejection while preparing the exact binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum GeneratedFlashAttentionV1HostAdapterErrorV1 {
    ObservedTargetMismatch,
    WrongContext {
        role: FlashAttentionV1BufferRoleV1,
    },
    ElementLayout {
        role: FlashAttentionV1BufferRoleV1,
    },
    Length {
        role: FlashAttentionV1BufferRoleV1,
        expected: usize,
        actual: usize,
    },
    NullAddress {
        role: FlashAttentionV1BufferRoleV1,
    },
    Alignment {
        role: FlashAttentionV1BufferRoleV1,
    },
    RegionOverflow {
        role: FlashAttentionV1BufferRoleV1,
    },
    InvalidRegion {
        role: FlashAttentionV1BufferRoleV1,
    },
    PointerWidth {
        role: FlashAttentionV1BufferRoleV1,
    },
    OutputAliasesInput {
        input: FlashAttentionV1BufferRoleV1,
    },
}

impl fmt::Display for GeneratedFlashAttentionV1HostAdapterErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "FlashAttention host binding rejected: {self:?}")
    }
}

impl Error for GeneratedFlashAttentionV1HostAdapterErrorV1 {}

#[cfg(test)]
mod tests {
    use super::*;

    fn region(identity: u64, address: usize) -> RegionFacts<u64> {
        RegionFacts {
            allocation_identity: identity,
            allocation_address: address - 0x80,
            allocation_elements: 192,
            region_address: address,
            region_elements: ELEMENTS,
            region_byte_start: 0x80,
            region_byte_end: 0x80 + ELEMENTS * size_of::<f32>(),
            element_bytes: size_of::<f32>(),
            element_alignment: align_of::<f32>(),
        }
    }

    fn canonical() -> [RegionFacts<u64>; 4] {
        [
            region(1, 0x1080),
            region(2, 0x2080),
            region(3, 0x3080),
            region(4, 0x4080),
        ]
    }

    #[test]
    fn exact_four_slice_abi_is_packed() {
        let bytes = prepare_regions(canonical()).unwrap().bytes;
        for (index, address) in [0x1080_u64, 0x2080, 0x3080, 0x4080].into_iter().enumerate() {
            let offset = index * 16;
            assert_eq!(&bytes[offset..offset + 8], &address.to_le_bytes());
            assert_eq!(&bytes[offset + 8..offset + 16], &128_u64.to_le_bytes());
        }
    }

    #[test]
    fn hostile_extent_layout_and_provenance_substitutions_fail_closed() {
        let mutations: &[fn(&mut [RegionFacts<u64>; 4])] = &[
            |r| r[0].region_elements = 127,
            |r| r[1].region_elements = 129,
            |r| r[2].allocation_address = 0,
            |r| r[3].region_address = 0,
            |r| r[0].region_address += 1,
            |r| r[1].element_bytes = 8,
            |r| r[2].element_alignment = 8,
            |r| r[3].region_byte_end -= 4,
            |r| r[0].region_byte_start += 4,
            |r| r[1].allocation_elements = 32,
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
    fn shared_inputs_may_alias_but_output_never_may() {
        let mut inputs_alias = canonical();
        inputs_alias[1] = inputs_alias[0];
        assert!(prepare_regions(inputs_alias).is_ok());

        for input in 0..3 {
            let mut regions = canonical();
            regions[3] = regions[input];
            assert_eq!(
                prepare_regions(regions).map(|_| ()),
                Err(
                    GeneratedFlashAttentionV1HostAdapterErrorV1::OutputAliasesInput {
                        input: [
                            FlashAttentionV1BufferRoleV1::Query,
                            FlashAttentionV1BufferRoleV1::Key,
                            FlashAttentionV1BufferRoleV1::Value,
                        ][input],
                    }
                )
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
