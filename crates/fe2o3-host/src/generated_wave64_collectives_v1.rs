//! Exact typed host preparation for masked Wave64 collectives V1.
//!
//! The adapter owns one shared input view and three independent exclusive
//! output views. It validates the fixed physical ABI before a runtime exists
//! and exposes neither addresses nor kernarg bytes to safe callers.

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
pub(crate) const LANES: usize = 64;
pub(crate) const EXPLICIT_KERNARG_BYTES: usize = 72;
pub(crate) const COMPLETE_KERNARG_BYTES: usize = 328;
pub(crate) const DESCRIPTOR_KERNARG_ALIGNMENT: u32 = 8;
pub(crate) const GRID: [u32; 3] = [1, 1, 1];
pub(crate) const WORKGROUP: [u32; 3] = [64, 1, 1];

#[repr(C, align(8))]
struct ExactWave64ExplicitKernargV1 {
    bytes: [u8; EXPLICIT_KERNARG_BYTES],
}

const _: () = assert!(size_of::<ExactWave64ExplicitKernargV1>() == EXPLICIT_KERNARG_BYTES);
const _: () = assert!(align_of::<ExactWave64ExplicitKernargV1>() == 8);

/// One fixed logical buffer in the Wave64 collectives profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Wave64CollectivesBufferRoleV1 {
    Input,
    ReductionOutput,
    InclusiveOutput,
    ExclusiveOutput,
}

impl fmt::Display for Wave64CollectivesBufferRoleV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Input => "input",
            Self::ReductionOutput => "reduction output",
            Self::InclusiveOutput => "inclusive output",
            Self::ExclusiveOutput => "exclusive output",
        })
    }
}

/// Authority-free rejection while preparing exact Wave64 arguments.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum GeneratedWave64CollectivesV1HostAdapterErrorV1 {
    ObservedTargetMismatch,
    WrongContext {
        role: Wave64CollectivesBufferRoleV1,
    },
    ElementLayout {
        role: Wave64CollectivesBufferRoleV1,
    },
    Length {
        role: Wave64CollectivesBufferRoleV1,
        expected: usize,
        actual: usize,
    },
    NullAddress {
        role: Wave64CollectivesBufferRoleV1,
    },
    Alignment {
        role: Wave64CollectivesBufferRoleV1,
    },
    ByteLengthOverflow {
        role: Wave64CollectivesBufferRoleV1,
    },
    AddressOverflow {
        role: Wave64CollectivesBufferRoleV1,
    },
    InvalidProvenance {
        role: Wave64CollectivesBufferRoleV1,
    },
    AllocationAlias {
        left: Wave64CollectivesBufferRoleV1,
        right: Wave64CollectivesBufferRoleV1,
    },
    RegionOverlap {
        left: Wave64CollectivesBufferRoleV1,
        right: Wave64CollectivesBufferRoleV1,
    },
}

impl fmt::Display for GeneratedWave64CollectivesV1HostAdapterErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ObservedTargetMismatch => {
                formatter.write_str("observed target is not gfx942:xnack-")
            }
            Self::WrongContext { role } => write!(formatter, "{role} belongs to another context"),
            Self::ElementLayout { role } => write!(formatter, "{role} is not aligned binary32"),
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
                write!(formatter, "{role} device address is not f32-aligned")
            }
            Self::ByteLengthOverflow { role } => write!(formatter, "{role} byte length overflowed"),
            Self::AddressOverflow { role } => write!(formatter, "{role} address range overflowed"),
            Self::InvalidProvenance { role } => {
                write!(formatter, "{role} provenance is inconsistent")
            }
            Self::AllocationAlias { left, right } => {
                write!(
                    formatter,
                    "{left} and {right} must use distinct allocations"
                )
            }
            Self::RegionOverlap { left, right } => write!(formatter, "{left} overlaps {right}"),
        }
    }
}

impl Error for GeneratedWave64CollectivesV1HostAdapterErrorV1 {}

/// Linear generated arguments for one exact masked Wave64 dispatch.
///
/// The active mask is copied by value. All four device views remain borrowed
/// until this value is dropped by the synchronous protected lifecycle.
#[must_use = "the prepared Wave64 arguments must enter the protected lifecycle"]
pub struct GeneratedWave64CollectivesV1HostAdapterV1<'input, 'reduction, 'inclusive, 'exclusive> {
    observed: ObservedContext,
    active_mask: u64,
    explicit_kernarg: ExactWave64ExplicitKernargV1,
    _input: DeviceBufferView<'input, f32>,
    _reduction: DeviceBufferViewMut<'reduction, f32>,
    _inclusive: DeviceBufferViewMut<'inclusive, f32>,
    _exclusive: DeviceBufferViewMut<'exclusive, f32>,
}

impl fmt::Debug for GeneratedWave64CollectivesV1HostAdapterV1<'_, '_, '_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedWave64CollectivesV1HostAdapterV1")
            .field("target", &TARGET)
            .field("active_mask", &format_args!("{:#018x}", self.active_mask))
            .field("grid", &GRID)
            .field("workgroup", &WORKGROUP)
            .finish_non_exhaustive()
    }
}

impl<'input, 'reduction, 'inclusive, 'exclusive>
    GeneratedWave64CollectivesV1HostAdapterV1<'input, 'reduction, 'inclusive, 'exclusive>
{
    /// Validates and retains the exact input, mask, and three unique outputs.
    pub fn prepare(
        observed: &ObservedContext,
        input: DeviceBufferView<'input, f32>,
        active_mask: u64,
        reduction: DeviceBufferViewMut<'reduction, f32>,
        inclusive: DeviceBufferViewMut<'inclusive, f32>,
        exclusive: DeviceBufferViewMut<'exclusive, f32>,
    ) -> Result<Self, GeneratedWave64CollectivesV1HostAdapterErrorV1> {
        let expected = AmdTargetId::parse(TARGET).expect("static Wave64 target is canonical");
        let actual = AmdTargetId::parse(observed.device().target())
            .map_err(|_| GeneratedWave64CollectivesV1HostAdapterErrorV1::ObservedTargetMismatch)?;
        if !expected.is_compatible_with_observed(&actual) {
            return Err(GeneratedWave64CollectivesV1HostAdapterErrorV1::ObservedTargetMismatch);
        }

        for (role, matches) in [
            (
                Wave64CollectivesBufferRoleV1::Input,
                observed.is_for_context(input.context()),
            ),
            (
                Wave64CollectivesBufferRoleV1::ReductionOutput,
                observed.is_for_context(reduction.context()),
            ),
            (
                Wave64CollectivesBufferRoleV1::InclusiveOutput,
                observed.is_for_context(inclusive.context()),
            ),
            (
                Wave64CollectivesBufferRoleV1::ExclusiveOutput,
                observed.is_for_context(exclusive.context()),
            ),
        ] {
            if !matches {
                return Err(GeneratedWave64CollectivesV1HostAdapterErrorV1::WrongContext { role });
            }
        }

        let regions = [
            checked_region(Wave64CollectivesBufferRoleV1::Input, &input)?,
            checked_region(Wave64CollectivesBufferRoleV1::ReductionOutput, &reduction)?,
            checked_region(Wave64CollectivesBufferRoleV1::InclusiveOutput, &inclusive)?,
            checked_region(Wave64CollectivesBufferRoleV1::ExclusiveOutput, &exclusive)?,
        ];
        for left in 0..regions.len() {
            for right in left + 1..regions.len() {
                if regions[left].allocation_identity == regions[right].allocation_identity {
                    return Err(
                        GeneratedWave64CollectivesV1HostAdapterErrorV1::AllocationAlias {
                            left: regions[left].role,
                            right: regions[right].role,
                        },
                    );
                }
                if regions[left].start < regions[right].end
                    && regions[right].start < regions[left].end
                {
                    return Err(
                        GeneratedWave64CollectivesV1HostAdapterErrorV1::RegionOverlap {
                            left: regions[left].role,
                            right: regions[right].role,
                        },
                    );
                }
            }
        }

        let bytes = pack_explicit_kernarg_v1(regions.map(|region| region.address), active_mask);

        Ok(Self {
            observed: observed.clone(),
            active_mask,
            explicit_kernarg: ExactWave64ExplicitKernargV1 { bytes },
            _input: input,
            _reduction: reduction,
            _inclusive: inclusive,
            _exclusive: exclusive,
        })
    }

    pub const fn target(&self) -> &'static str {
        TARGET
    }
    pub const fn active_mask(&self) -> u64 {
        self.active_mask
    }
    pub const fn grid(&self) -> [u32; 3] {
        GRID
    }
    pub const fn workgroup(&self) -> [u32; 3] {
        WORKGROUP
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
    pub const fn static_lds_bytes(&self) -> u32 {
        0
    }
    pub const fn private_segment_bytes(&self) -> u32 {
        0
    }
    pub const fn proves_functional_collectives(&self) -> bool {
        false
    }
    pub const fn proves_compiler_refinement(&self) -> bool {
        false
    }

    pub(crate) const fn observed_context_v1(&self) -> &ObservedContext {
        &self.observed
    }
    pub(crate) const fn explicit_kernarg_bytes_v1(&self) -> &[u8; EXPLICIT_KERNARG_BYTES] {
        &self.explicit_kernarg.bytes
    }
}

#[derive(Clone, Copy)]
struct CheckedRegionV1 {
    role: Wave64CollectivesBufferRoleV1,
    allocation_identity: DeviceBufferIdentity,
    address: u64,
    start: usize,
    end: usize,
}

fn checked_region<T: DeviceCopy, R: DeviceBufferRegion<T> + ?Sized>(
    role: Wave64CollectivesBufferRoleV1,
    region: &R,
) -> Result<CheckedRegionV1, GeneratedWave64CollectivesV1HostAdapterErrorV1> {
    if size_of::<T>() != size_of::<f32>() || align_of::<T>() != align_of::<f32>() {
        return Err(GeneratedWave64CollectivesV1HostAdapterErrorV1::ElementLayout { role });
    }
    if region.region_len() != LANES {
        return Err(GeneratedWave64CollectivesV1HostAdapterErrorV1::Length {
            role,
            expected: LANES,
            actual: region.region_len(),
        });
    }
    let allocation = region.allocation_device_ptr().as_raw().addr();
    let address = region.region_device_ptr().as_raw().addr();
    if allocation == 0 || address == 0 {
        return Err(GeneratedWave64CollectivesV1HostAdapterErrorV1::NullAddress { role });
    }
    if !address.is_multiple_of(align_of::<f32>()) {
        return Err(GeneratedWave64CollectivesV1HostAdapterErrorV1::Alignment { role });
    }
    let allocation_bytes = region
        .allocation_len()
        .checked_mul(size_of::<T>())
        .ok_or(GeneratedWave64CollectivesV1HostAdapterErrorV1::ByteLengthOverflow { role })?;
    let region_bytes = region
        .region_len()
        .checked_mul(size_of::<T>())
        .ok_or(GeneratedWave64CollectivesV1HostAdapterErrorV1::ByteLengthOverflow { role })?;
    let allocation_end = allocation
        .checked_add(allocation_bytes)
        .ok_or(GeneratedWave64CollectivesV1HostAdapterErrorV1::AddressOverflow { role })?;
    let end = address
        .checked_add(region_bytes)
        .ok_or(GeneratedWave64CollectivesV1HostAdapterErrorV1::AddressOverflow { role })?;
    let relative = region.region_byte_range();
    let expected_address = allocation
        .checked_add(relative.start)
        .ok_or(GeneratedWave64CollectivesV1HostAdapterErrorV1::AddressOverflow { role })?;
    if relative.end > allocation_bytes
        || relative.end.checked_sub(relative.start) != Some(region_bytes)
        || expected_address != address
        || address < allocation
        || end > allocation_end
    {
        return Err(GeneratedWave64CollectivesV1HostAdapterErrorV1::InvalidProvenance { role });
    }
    let address_u64 = u64::try_from(address)
        .map_err(|_| GeneratedWave64CollectivesV1HostAdapterErrorV1::AddressOverflow { role })?;
    Ok(CheckedRegionV1 {
        role,
        allocation_identity: region.allocation_identity(),
        address: address_u64,
        start: address,
        end,
    })
}

fn put_u64(bytes: &mut [u8; EXPLICIT_KERNARG_BYTES], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn pack_explicit_kernarg_v1(addresses: [u64; 4], active_mask: u64) -> [u8; 72] {
    let mut bytes = [0_u8; EXPLICIT_KERNARG_BYTES];
    put_u64(&mut bytes, 0, addresses[0]);
    put_u64(&mut bytes, 8, LANES as u64);
    put_u64(&mut bytes, 16, active_mask);
    for (offset, address) in [24, 40, 56].into_iter().zip(addresses[1..].iter().copied()) {
        put_u64(&mut bytes, offset, address);
        put_u64(&mut bytes, offset + 8, LANES as u64);
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packs_exact_nine_component_abi_without_padding() {
        let mask = 0x8000_0000_0000_0001;
        let bytes = pack_explicit_kernarg_v1([0x1000, 0x2000, 0x3000, 0x4000], mask);
        let words: Vec<u64> = bytes
            .chunks_exact(8)
            .map(|word| u64::from_le_bytes(word.try_into().unwrap()))
            .collect();
        assert_eq!(
            words,
            [0x1000, 64, mask, 0x2000, 64, 0x3000, 64, 0x4000, 64]
        );
        assert_eq!(bytes.len(), EXPLICIT_KERNARG_BYTES);
        assert_eq!(COMPLETE_KERNARG_BYTES - bytes.len(), 256);
    }
}
