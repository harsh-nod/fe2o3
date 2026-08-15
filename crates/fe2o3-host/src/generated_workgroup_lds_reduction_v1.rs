//! Exact typed host preparation for workgroup LDS reduction V1.

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
pub(crate) const EXPORT_SYMBOL: &str = "lds_publish_read_reduce_i32_v1";
pub(crate) const LANES: usize = 64;
pub(crate) const GRID: [u32; 3] = [1, 1, 1];
pub(crate) const WORKGROUP: [u32; 3] = [64, 1, 1];
pub(crate) const WAVEFRONT_SIZE: u32 = 64;
pub(crate) const EXPLICIT_KERNARG_BYTES: usize = 40;
pub(crate) const COMPLETE_KERNARG_BYTES: usize = 296;
pub(crate) const DESCRIPTOR_KERNARG_ALIGNMENT: u32 = 8;
pub(crate) const RUNTIME_KERNARG_ALIGNMENT: u64 = 16;
pub(crate) const STATIC_GROUP_SEGMENT_BYTES: u32 = 0;
pub(crate) const PRIVATE_SEGMENT_BYTES: u32 = 0;
pub(crate) const DYNAMIC_LDS_BYTES: u32 = 256;
pub(crate) const HIDDEN_DYNAMIC_LDS_OFFSET: usize = 160;
pub(crate) const HIDDEN_DYNAMIC_LDS_VALUE: u32 = 256;

#[repr(C, align(8))]
struct ExactLdsReductionExplicitKernargV1 {
    bytes: [u8; EXPLICIT_KERNARG_BYTES],
}

const _: () = assert!(size_of::<ExactLdsReductionExplicitKernargV1>() == EXPLICIT_KERNARG_BYTES);
const _: () = assert!(align_of::<ExactLdsReductionExplicitKernargV1>() == 8);
const _: () = assert!(COMPLETE_KERNARG_BYTES - EXPLICIT_KERNARG_BYTES == 256);
const _: () = assert!(WAVEFRONT_SIZE == WORKGROUP[0]);
const _: () = assert!(HIDDEN_DYNAMIC_LDS_OFFSET == EXPLICIT_KERNARG_BYTES + 120);

/// One exact pointer role in the LDS reduction ABI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkgroupLdsReductionBufferRoleV1 {
    Values,
    Output,
}

impl fmt::Display for WorkgroupLdsReductionBufferRoleV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Values => "values",
            Self::Output => "output",
        })
    }
}

/// Authority-free rejection while preparing exact LDS-reduction arguments.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum GeneratedWorkgroupLdsReductionV1HostAdapterErrorV1 {
    ObservedTargetMismatch,
    WrongContext {
        role: WorkgroupLdsReductionBufferRoleV1,
    },
    ElementLayout {
        role: WorkgroupLdsReductionBufferRoleV1,
    },
    Length {
        role: WorkgroupLdsReductionBufferRoleV1,
        expected: usize,
        actual: usize,
    },
    NullAddress {
        role: WorkgroupLdsReductionBufferRoleV1,
    },
    Alignment {
        role: WorkgroupLdsReductionBufferRoleV1,
    },
    ByteLengthOverflow {
        role: WorkgroupLdsReductionBufferRoleV1,
    },
    AddressOverflow {
        role: WorkgroupLdsReductionBufferRoleV1,
    },
    InvalidProvenance {
        role: WorkgroupLdsReductionBufferRoleV1,
    },
    AllocationAlias,
    RegionOverlap,
}

impl fmt::Display for GeneratedWorkgroupLdsReductionV1HostAdapterErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ObservedTargetMismatch => {
                formatter.write_str("observed target is not gfx942:xnack-")
            }
            Self::WrongContext { role } => write!(formatter, "{role} belongs to another context"),
            Self::ElementLayout { role } => write!(formatter, "{role} is not aligned i32"),
            Self::Length {
                role,
                expected,
                actual,
            } => {
                write!(
                    formatter,
                    "{role} requires {expected} elements, got {actual}"
                )
            }
            Self::NullAddress { role } => write!(formatter, "{role} has a null device address"),
            Self::Alignment { role } => {
                write!(formatter, "{role} device address is not i32-aligned")
            }
            Self::ByteLengthOverflow { role } => write!(formatter, "{role} byte length overflowed"),
            Self::AddressOverflow { role } => write!(formatter, "{role} address range overflowed"),
            Self::InvalidProvenance { role } => {
                write!(formatter, "{role} provenance is inconsistent")
            }
            Self::AllocationAlias => {
                formatter.write_str("values and output must use distinct allocations")
            }
            Self::RegionOverlap => formatter.write_str("values and output regions overlap"),
        }
    }
}

impl Error for GeneratedWorkgroupLdsReductionV1HostAdapterErrorV1 {}

/// Linear generated arguments for one exact LDS-reduction dispatch.
#[must_use = "the prepared LDS-reduction arguments must enter the typed lifecycle"]
pub struct GeneratedWorkgroupLdsReductionV1HostAdapterV1<'values, 'output> {
    observed: ObservedContext,
    epoch: u32,
    explicit_kernarg: ExactLdsReductionExplicitKernargV1,
    _values: DeviceBufferView<'values, i32>,
    _output: DeviceBufferViewMut<'output, i32>,
}

impl fmt::Debug for GeneratedWorkgroupLdsReductionV1HostAdapterV1<'_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedWorkgroupLdsReductionV1HostAdapterV1")
            .field("target", &TARGET)
            .field("epoch", &self.epoch)
            .field("grid", &GRID)
            .field("workgroup", &WORKGROUP)
            .finish_non_exhaustive()
    }
}

impl<'values, 'output> GeneratedWorkgroupLdsReductionV1HostAdapterV1<'values, 'output> {
    /// Validates and retains one shared input and one unique output allocation.
    pub fn prepare(
        observed: &ObservedContext,
        values: DeviceBufferView<'values, i32>,
        epoch: u32,
        output: DeviceBufferViewMut<'output, i32>,
    ) -> Result<Self, GeneratedWorkgroupLdsReductionV1HostAdapterErrorV1> {
        validate_target(observed.device().target())?;
        if !observed.is_for_context(values.context()) {
            return Err(
                GeneratedWorkgroupLdsReductionV1HostAdapterErrorV1::WrongContext {
                    role: WorkgroupLdsReductionBufferRoleV1::Values,
                },
            );
        }
        if !observed.is_for_context(output.context()) {
            return Err(
                GeneratedWorkgroupLdsReductionV1HostAdapterErrorV1::WrongContext {
                    role: WorkgroupLdsReductionBufferRoleV1::Output,
                },
            );
        }
        let values_region =
            checked_region(WorkgroupLdsReductionBufferRoleV1::Values, LANES, &values)?;
        let output_region = checked_region(WorkgroupLdsReductionBufferRoleV1::Output, 1, &output)?;
        validate_disjoint(values_region, output_region)?;
        let bytes = pack_explicit_kernarg_v1(values_region.address, epoch, output_region.address);
        Ok(Self {
            observed: observed.clone(),
            epoch,
            explicit_kernarg: ExactLdsReductionExplicitKernargV1 { bytes },
            _values: values,
            _output: output,
        })
    }

    pub const fn target(&self) -> &'static str {
        TARGET
    }
    pub const fn export_symbol(&self) -> &'static str {
        EXPORT_SYMBOL
    }
    pub const fn epoch(&self) -> u32 {
        self.epoch
    }
    pub const fn grid(&self) -> [u32; 3] {
        GRID
    }
    pub const fn workgroup(&self) -> [u32; 3] {
        WORKGROUP
    }
    pub const fn wavefront_size(&self) -> u32 {
        WAVEFRONT_SIZE
    }
    pub const fn explicit_kernarg_byte_len(&self) -> usize {
        EXPLICIT_KERNARG_BYTES
    }
    pub const fn complete_kernarg_byte_len(&self) -> usize {
        COMPLETE_KERNARG_BYTES
    }
    pub const fn descriptor_kernarg_alignment(&self) -> u32 {
        DESCRIPTOR_KERNARG_ALIGNMENT
    }
    pub const fn runtime_kernarg_alignment(&self) -> u64 {
        RUNTIME_KERNARG_ALIGNMENT
    }
    pub const fn static_group_segment_bytes(&self) -> u32 {
        STATIC_GROUP_SEGMENT_BYTES
    }
    pub const fn private_segment_bytes(&self) -> u32 {
        PRIVATE_SEGMENT_BYTES
    }
    pub const fn dynamic_lds_bytes(&self) -> u32 {
        DYNAMIC_LDS_BYTES
    }
    pub const fn hidden_dynamic_lds_offset(&self) -> usize {
        HIDDEN_DYNAMIC_LDS_OFFSET
    }
    pub const fn hidden_dynamic_lds_value(&self) -> u32 {
        HIDDEN_DYNAMIC_LDS_VALUE
    }
    pub const fn proves_race_freedom(&self) -> bool {
        false
    }
    pub const fn proves_machine_safety(&self) -> bool {
        false
    }

    #[allow(dead_code)]
    pub(crate) const fn observed_context_v1(&self) -> &ObservedContext {
        &self.observed
    }
    #[allow(dead_code)]
    pub(crate) const fn explicit_kernarg_bytes_v1(&self) -> &[u8; EXPLICIT_KERNARG_BYTES] {
        &self.explicit_kernarg.bytes
    }
}

#[derive(Clone, Copy)]
struct RegionFactsV1<I> {
    role: WorkgroupLdsReductionBufferRoleV1,
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

#[derive(Clone, Copy)]
struct CheckedRegionV1<I> {
    allocation_identity: I,
    address: u64,
    start: usize,
    end: usize,
}

fn checked_region<T: DeviceCopy, R: DeviceBufferRegion<T> + ?Sized>(
    role: WorkgroupLdsReductionBufferRoleV1,
    expected_elements: usize,
    region: &R,
) -> Result<CheckedRegionV1<DeviceBufferIdentity>, GeneratedWorkgroupLdsReductionV1HostAdapterErrorV1>
{
    validate_region(
        expected_elements,
        RegionFactsV1 {
            role,
            allocation_identity: region.allocation_identity(),
            allocation_address: region.allocation_device_ptr().as_raw().addr(),
            allocation_elements: region.allocation_len(),
            region_address: region.region_device_ptr().as_raw().addr(),
            region_elements: region.region_len(),
            region_byte_start: region.region_byte_range().start,
            region_byte_end: region.region_byte_range().end,
            element_bytes: size_of::<T>(),
            element_alignment: align_of::<T>(),
        },
    )
}

fn validate_region<I: Copy>(
    expected_elements: usize,
    facts: RegionFactsV1<I>,
) -> Result<CheckedRegionV1<I>, GeneratedWorkgroupLdsReductionV1HostAdapterErrorV1> {
    let role = facts.role;
    if facts.element_bytes != size_of::<i32>() || facts.element_alignment != align_of::<i32>() {
        return Err(GeneratedWorkgroupLdsReductionV1HostAdapterErrorV1::ElementLayout { role });
    }
    if facts.region_elements != expected_elements {
        return Err(GeneratedWorkgroupLdsReductionV1HostAdapterErrorV1::Length {
            role,
            expected: expected_elements,
            actual: facts.region_elements,
        });
    }
    if facts.allocation_address == 0 || facts.region_address == 0 {
        return Err(GeneratedWorkgroupLdsReductionV1HostAdapterErrorV1::NullAddress { role });
    }
    if !facts.region_address.is_multiple_of(align_of::<i32>()) {
        return Err(GeneratedWorkgroupLdsReductionV1HostAdapterErrorV1::Alignment { role });
    }
    let allocation_bytes = facts
        .allocation_elements
        .checked_mul(facts.element_bytes)
        .ok_or(GeneratedWorkgroupLdsReductionV1HostAdapterErrorV1::ByteLengthOverflow { role })?;
    let region_bytes = facts
        .region_elements
        .checked_mul(facts.element_bytes)
        .ok_or(GeneratedWorkgroupLdsReductionV1HostAdapterErrorV1::ByteLengthOverflow { role })?;
    let allocation_end = facts
        .allocation_address
        .checked_add(allocation_bytes)
        .ok_or(GeneratedWorkgroupLdsReductionV1HostAdapterErrorV1::AddressOverflow { role })?;
    let end = facts
        .region_address
        .checked_add(region_bytes)
        .ok_or(GeneratedWorkgroupLdsReductionV1HostAdapterErrorV1::AddressOverflow { role })?;
    let expected_address = facts
        .allocation_address
        .checked_add(facts.region_byte_start)
        .ok_or(GeneratedWorkgroupLdsReductionV1HostAdapterErrorV1::AddressOverflow { role })?;
    if facts.region_byte_end > allocation_bytes
        || facts.region_byte_end.checked_sub(facts.region_byte_start) != Some(region_bytes)
        || expected_address != facts.region_address
        || facts.region_address < facts.allocation_address
        || end > allocation_end
    {
        return Err(GeneratedWorkgroupLdsReductionV1HostAdapterErrorV1::InvalidProvenance { role });
    }
    let address = u64::try_from(facts.region_address).map_err(|_| {
        GeneratedWorkgroupLdsReductionV1HostAdapterErrorV1::AddressOverflow { role }
    })?;
    Ok(CheckedRegionV1 {
        allocation_identity: facts.allocation_identity,
        address,
        start: facts.region_address,
        end,
    })
}

fn validate_disjoint<I: Eq + Copy>(
    values: CheckedRegionV1<I>,
    output: CheckedRegionV1<I>,
) -> Result<(), GeneratedWorkgroupLdsReductionV1HostAdapterErrorV1> {
    if values.allocation_identity == output.allocation_identity {
        return Err(GeneratedWorkgroupLdsReductionV1HostAdapterErrorV1::AllocationAlias);
    }
    if values.start < output.end && output.start < values.end {
        return Err(GeneratedWorkgroupLdsReductionV1HostAdapterErrorV1::RegionOverlap);
    }
    Ok(())
}

fn validate_target(target: &str) -> Result<(), GeneratedWorkgroupLdsReductionV1HostAdapterErrorV1> {
    let expected = AmdTargetId::parse(TARGET).expect("static LDS-reduction target is canonical");
    let actual = AmdTargetId::parse(target)
        .map_err(|_| GeneratedWorkgroupLdsReductionV1HostAdapterErrorV1::ObservedTargetMismatch)?;
    if expected.is_compatible_with_observed(&actual) {
        Ok(())
    } else {
        Err(GeneratedWorkgroupLdsReductionV1HostAdapterErrorV1::ObservedTargetMismatch)
    }
}

fn put_u64(bytes: &mut [u8; EXPLICIT_KERNARG_BYTES], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn pack_explicit_kernarg_v1(values: u64, epoch: u32, output: u64) -> [u8; 40] {
    let mut bytes = [0; EXPLICIT_KERNARG_BYTES];
    put_u64(&mut bytes, 0, values);
    put_u64(&mut bytes, 8, LANES as u64);
    bytes[16..20].copy_from_slice(&epoch.to_le_bytes());
    put_u64(&mut bytes, 24, output);
    put_u64(&mut bytes, 32, 1);
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(
        role: WorkgroupLdsReductionBufferRoleV1,
        identity: u64,
        address: usize,
        len: usize,
    ) -> RegionFactsV1<u64> {
        RegionFactsV1 {
            role,
            allocation_identity: identity,
            allocation_address: address,
            allocation_elements: len,
            region_address: address,
            region_elements: len,
            region_byte_start: 0,
            region_byte_end: len * 4,
            element_bytes: 4,
            element_alignment: 4,
        }
    }

    #[test]
    fn exact_abi_and_resources_are_fixed() {
        let bytes = pack_explicit_kernarg_v1(0x1000, 0x4433_2211, 0x2000);
        assert_eq!(u64::from_le_bytes(bytes[0..8].try_into().unwrap()), 0x1000);
        assert_eq!(u64::from_le_bytes(bytes[8..16].try_into().unwrap()), 64);
        assert_eq!(&bytes[16..20], &0x4433_2211_u32.to_le_bytes());
        assert_eq!(&bytes[20..24], &[0; 4]);
        assert_eq!(
            u64::from_le_bytes(bytes[24..32].try_into().unwrap()),
            0x2000
        );
        assert_eq!(u64::from_le_bytes(bytes[32..40].try_into().unwrap()), 1);
        assert_eq!((EXPLICIT_KERNARG_BYTES, COMPLETE_KERNARG_BYTES), (40, 296));
        assert_eq!((STATIC_GROUP_SEGMENT_BYTES, PRIVATE_SEGMENT_BYTES), (0, 0));
        assert_eq!(
            (
                DYNAMIC_LDS_BYTES,
                HIDDEN_DYNAMIC_LDS_OFFSET,
                HIDDEN_DYNAMIC_LDS_VALUE
            ),
            (256, 160, 256)
        );
    }

    #[test]
    fn exact_extents_provenance_alignment_and_aliasing_fail_closed() {
        let values = validate_region(
            64,
            facts(WorkgroupLdsReductionBufferRoleV1::Values, 1, 0x1000, 64),
        )
        .unwrap();
        let output = validate_region(
            1,
            facts(WorkgroupLdsReductionBufferRoleV1::Output, 2, 0x2000, 1),
        )
        .unwrap();
        assert_eq!(validate_disjoint(values, output), Ok(()));

        let mut wrong = facts(WorkgroupLdsReductionBufferRoleV1::Values, 1, 0x1000, 64);
        wrong.region_elements = 63;
        assert!(matches!(
            validate_region(64, wrong),
            Err(GeneratedWorkgroupLdsReductionV1HostAdapterErrorV1::Length { .. })
        ));
        let mut wrong = facts(WorkgroupLdsReductionBufferRoleV1::Values, 1, 0x1000, 64);
        wrong.region_address += 4;
        assert!(matches!(
            validate_region(64, wrong),
            Err(GeneratedWorkgroupLdsReductionV1HostAdapterErrorV1::InvalidProvenance { .. })
        ));
        let wrong = facts(WorkgroupLdsReductionBufferRoleV1::Values, 1, 0x1002, 64);
        assert!(matches!(
            validate_region(64, wrong),
            Err(GeneratedWorkgroupLdsReductionV1HostAdapterErrorV1::Alignment { .. })
        ));

        let aliased = validate_region(
            1,
            facts(WorkgroupLdsReductionBufferRoleV1::Output, 1, 0x2000, 1),
        )
        .unwrap();
        assert_eq!(
            validate_disjoint(values, aliased),
            Err(GeneratedWorkgroupLdsReductionV1HostAdapterErrorV1::AllocationAlias)
        );
        let overlapping = validate_region(
            1,
            facts(WorkgroupLdsReductionBufferRoleV1::Output, 2, 0x1000, 1),
        )
        .unwrap();
        assert_eq!(
            validate_disjoint(values, overlapping),
            Err(GeneratedWorkgroupLdsReductionV1HostAdapterErrorV1::RegionOverlap)
        );
    }

    #[test]
    fn exact_target_is_closed() {
        assert_eq!(validate_target("gfx942:xnack-"), Ok(()));
        assert_eq!(validate_target("gfx942:sramecc+:xnack-"), Ok(()));
        for target in ["gfx942", "gfx942:xnack+", "gfx950:xnack-", ""] {
            assert_eq!(
                validate_target(target),
                Err(GeneratedWorkgroupLdsReductionV1HostAdapterErrorV1::ObservedTargetMismatch)
            );
        }
    }
}
