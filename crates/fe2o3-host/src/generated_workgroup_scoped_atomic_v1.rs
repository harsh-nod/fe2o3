//! Exact typed host preparation for scoped global atomic-add V1.

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
pub(crate) const EXPORT_SYMBOL: &str = "scoped_atomic_add_u32_v1";
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
pub(crate) const DYNAMIC_LDS_BYTES: u32 = 0;

#[repr(C, align(8))]
struct ExactScopedAtomicExplicitKernargV1 {
    bytes: [u8; EXPLICIT_KERNARG_BYTES],
}

const _: () = assert!(size_of::<ExactScopedAtomicExplicitKernargV1>() == EXPLICIT_KERNARG_BYTES);
const _: () = assert!(align_of::<ExactScopedAtomicExplicitKernargV1>() == 8);
const _: () = assert!(COMPLETE_KERNARG_BYTES - EXPLICIT_KERNARG_BYTES == 256);
const _: () = assert!(WAVEFRONT_SIZE == WORKGROUP[0]);

/// One exact pointer role in the scoped-atomic ABI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkgroupScopedAtomicBufferRoleV1 {
    Values,
    Eligible,
    Target,
}

impl fmt::Display for WorkgroupScopedAtomicBufferRoleV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Values => "values",
            Self::Eligible => "eligible",
            Self::Target => "atomic target",
        })
    }
}

/// Exact atomic effect admitted by this generated adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkgroupScopedAtomicEffectV1 {
    GlobalU32SystemScopeRelaxedAdd,
}

/// Authority-free rejection while preparing exact scoped-atomic arguments.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum GeneratedWorkgroupScopedAtomicV1HostAdapterErrorV1 {
    ObservedTargetMismatch,
    WrongContext {
        role: WorkgroupScopedAtomicBufferRoleV1,
    },
    ElementLayout {
        role: WorkgroupScopedAtomicBufferRoleV1,
    },
    Length {
        role: WorkgroupScopedAtomicBufferRoleV1,
        expected: usize,
        actual: usize,
    },
    NullAddress {
        role: WorkgroupScopedAtomicBufferRoleV1,
    },
    Alignment {
        role: WorkgroupScopedAtomicBufferRoleV1,
    },
    ByteLengthOverflow {
        role: WorkgroupScopedAtomicBufferRoleV1,
    },
    AddressOverflow {
        role: WorkgroupScopedAtomicBufferRoleV1,
    },
    InvalidProvenance {
        role: WorkgroupScopedAtomicBufferRoleV1,
    },
    AllocationAlias {
        left: WorkgroupScopedAtomicBufferRoleV1,
        right: WorkgroupScopedAtomicBufferRoleV1,
    },
    RegionOverlap {
        left: WorkgroupScopedAtomicBufferRoleV1,
        right: WorkgroupScopedAtomicBufferRoleV1,
    },
}

impl fmt::Display for GeneratedWorkgroupScopedAtomicV1HostAdapterErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ObservedTargetMismatch => {
                formatter.write_str("observed target is not gfx942:xnack-")
            }
            Self::WrongContext { role } => write!(formatter, "{role} belongs to another context"),
            Self::ElementLayout { role } => write!(formatter, "{role} is not aligned u32"),
            Self::Length {
                role,
                expected,
                actual,
            } => write!(
                formatter,
                "{role} requires {expected} elements, got {actual}"
            ),
            Self::NullAddress { role } => write!(formatter, "{role} has a null device address"),
            Self::Alignment { role } => {
                write!(formatter, "{role} device address is not u32-aligned")
            }
            Self::ByteLengthOverflow { role } => write!(formatter, "{role} byte length overflowed"),
            Self::AddressOverflow { role } => write!(formatter, "{role} address range overflowed"),
            Self::InvalidProvenance { role } => {
                write!(formatter, "{role} provenance is inconsistent")
            }
            Self::AllocationAlias { left, right } => write!(
                formatter,
                "{left} and {right} must use distinct allocations"
            ),
            Self::RegionOverlap { left, right } => write!(formatter, "{left} overlaps {right}"),
        }
    }
}

impl Error for GeneratedWorkgroupScopedAtomicV1HostAdapterErrorV1 {}

/// Linear generated arguments for one exact system-scope relaxed atomic add.
#[must_use = "the prepared scoped-atomic arguments must enter the typed lifecycle"]
pub struct GeneratedWorkgroupScopedAtomicV1HostAdapterV1<'values, 'eligible, 'target> {
    observed: ObservedContext,
    explicit_kernarg: ExactScopedAtomicExplicitKernargV1,
    _values: DeviceBufferView<'values, u32>,
    _eligible: DeviceBufferView<'eligible, u32>,
    _target: DeviceBufferViewMut<'target, u32>,
}

impl fmt::Debug for GeneratedWorkgroupScopedAtomicV1HostAdapterV1<'_, '_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedWorkgroupScopedAtomicV1HostAdapterV1")
            .field("target", &TARGET)
            .field(
                "effect",
                &WorkgroupScopedAtomicEffectV1::GlobalU32SystemScopeRelaxedAdd,
            )
            .field("grid", &GRID)
            .field("workgroup", &WORKGROUP)
            .finish_non_exhaustive()
    }
}

impl<'values, 'eligible, 'target>
    GeneratedWorkgroupScopedAtomicV1HostAdapterV1<'values, 'eligible, 'target>
{
    /// Validates and retains two shared inputs and one unique host-visible target.
    pub fn prepare(
        observed: &ObservedContext,
        values: DeviceBufferView<'values, u32>,
        eligible: DeviceBufferView<'eligible, u32>,
        target: DeviceBufferViewMut<'target, u32>,
    ) -> Result<Self, GeneratedWorkgroupScopedAtomicV1HostAdapterErrorV1> {
        validate_target(observed.device().target())?;
        for (role, matches) in [
            (
                WorkgroupScopedAtomicBufferRoleV1::Values,
                observed.is_for_context(values.context()),
            ),
            (
                WorkgroupScopedAtomicBufferRoleV1::Eligible,
                observed.is_for_context(eligible.context()),
            ),
            (
                WorkgroupScopedAtomicBufferRoleV1::Target,
                observed.is_for_context(target.context()),
            ),
        ] {
            if !matches {
                return Err(
                    GeneratedWorkgroupScopedAtomicV1HostAdapterErrorV1::WrongContext { role },
                );
            }
        }
        let regions = [
            checked_region(WorkgroupScopedAtomicBufferRoleV1::Values, LANES, &values)?,
            checked_region(
                WorkgroupScopedAtomicBufferRoleV1::Eligible,
                LANES,
                &eligible,
            )?,
            checked_region(WorkgroupScopedAtomicBufferRoleV1::Target, 1, &target)?,
        ];
        validate_disjoint(regions)?;
        let bytes =
            pack_explicit_kernarg_v1(regions[0].address, regions[1].address, regions[2].address);
        Ok(Self {
            observed: observed.clone(),
            explicit_kernarg: ExactScopedAtomicExplicitKernargV1 { bytes },
            _values: values,
            _eligible: eligible,
            _target: target,
        })
    }

    pub const fn target(&self) -> &'static str {
        TARGET
    }
    pub const fn export_symbol(&self) -> &'static str {
        EXPORT_SYMBOL
    }
    pub const fn effect(&self) -> WorkgroupScopedAtomicEffectV1 {
        WorkgroupScopedAtomicEffectV1::GlobalU32SystemScopeRelaxedAdd
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
    pub const fn host_visible_target_elements(&self) -> usize {
        1
    }
    pub const fn proves_generalized_race_freedom(&self) -> bool {
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
    role: WorkgroupScopedAtomicBufferRoleV1,
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
    role: WorkgroupScopedAtomicBufferRoleV1,
    allocation_identity: I,
    address: u64,
    start: usize,
    end: usize,
}

fn checked_region<T: DeviceCopy, R: DeviceBufferRegion<T> + ?Sized>(
    role: WorkgroupScopedAtomicBufferRoleV1,
    expected_elements: usize,
    region: &R,
) -> Result<CheckedRegionV1<DeviceBufferIdentity>, GeneratedWorkgroupScopedAtomicV1HostAdapterErrorV1>
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
) -> Result<CheckedRegionV1<I>, GeneratedWorkgroupScopedAtomicV1HostAdapterErrorV1> {
    let role = facts.role;
    if facts.element_bytes != size_of::<u32>() || facts.element_alignment != align_of::<u32>() {
        return Err(GeneratedWorkgroupScopedAtomicV1HostAdapterErrorV1::ElementLayout { role });
    }
    if facts.region_elements != expected_elements {
        return Err(GeneratedWorkgroupScopedAtomicV1HostAdapterErrorV1::Length {
            role,
            expected: expected_elements,
            actual: facts.region_elements,
        });
    }
    if facts.allocation_address == 0 || facts.region_address == 0 {
        return Err(GeneratedWorkgroupScopedAtomicV1HostAdapterErrorV1::NullAddress { role });
    }
    if !facts.region_address.is_multiple_of(align_of::<u32>()) {
        return Err(GeneratedWorkgroupScopedAtomicV1HostAdapterErrorV1::Alignment { role });
    }
    let allocation_bytes = facts
        .allocation_elements
        .checked_mul(facts.element_bytes)
        .ok_or(GeneratedWorkgroupScopedAtomicV1HostAdapterErrorV1::ByteLengthOverflow { role })?;
    let region_bytes = facts
        .region_elements
        .checked_mul(facts.element_bytes)
        .ok_or(GeneratedWorkgroupScopedAtomicV1HostAdapterErrorV1::ByteLengthOverflow { role })?;
    let allocation_end = facts
        .allocation_address
        .checked_add(allocation_bytes)
        .ok_or(GeneratedWorkgroupScopedAtomicV1HostAdapterErrorV1::AddressOverflow { role })?;
    let end = facts
        .region_address
        .checked_add(region_bytes)
        .ok_or(GeneratedWorkgroupScopedAtomicV1HostAdapterErrorV1::AddressOverflow { role })?;
    let expected_address = facts
        .allocation_address
        .checked_add(facts.region_byte_start)
        .ok_or(GeneratedWorkgroupScopedAtomicV1HostAdapterErrorV1::AddressOverflow { role })?;
    if facts.region_byte_end > allocation_bytes
        || facts.region_byte_end.checked_sub(facts.region_byte_start) != Some(region_bytes)
        || expected_address != facts.region_address
        || facts.region_address < facts.allocation_address
        || end > allocation_end
    {
        return Err(GeneratedWorkgroupScopedAtomicV1HostAdapterErrorV1::InvalidProvenance { role });
    }
    let address = u64::try_from(facts.region_address).map_err(|_| {
        GeneratedWorkgroupScopedAtomicV1HostAdapterErrorV1::AddressOverflow { role }
    })?;
    Ok(CheckedRegionV1 {
        role,
        allocation_identity: facts.allocation_identity,
        address,
        start: facts.region_address,
        end,
    })
}

fn validate_disjoint<I: Eq + Copy>(
    regions: [CheckedRegionV1<I>; 3],
) -> Result<(), GeneratedWorkgroupScopedAtomicV1HostAdapterErrorV1> {
    for left in 0..regions.len() {
        for right in left + 1..regions.len() {
            if regions[left].allocation_identity == regions[right].allocation_identity {
                return Err(
                    GeneratedWorkgroupScopedAtomicV1HostAdapterErrorV1::AllocationAlias {
                        left: regions[left].role,
                        right: regions[right].role,
                    },
                );
            }
            if regions[left].start < regions[right].end && regions[right].start < regions[left].end
            {
                return Err(
                    GeneratedWorkgroupScopedAtomicV1HostAdapterErrorV1::RegionOverlap {
                        left: regions[left].role,
                        right: regions[right].role,
                    },
                );
            }
        }
    }
    Ok(())
}

fn validate_target(target: &str) -> Result<(), GeneratedWorkgroupScopedAtomicV1HostAdapterErrorV1> {
    let expected = AmdTargetId::parse(TARGET).expect("static scoped-atomic target is canonical");
    let actual = AmdTargetId::parse(target)
        .map_err(|_| GeneratedWorkgroupScopedAtomicV1HostAdapterErrorV1::ObservedTargetMismatch)?;
    if expected.is_compatible_with_observed(&actual) {
        Ok(())
    } else {
        Err(GeneratedWorkgroupScopedAtomicV1HostAdapterErrorV1::ObservedTargetMismatch)
    }
}

fn put_u64(bytes: &mut [u8; EXPLICIT_KERNARG_BYTES], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn pack_explicit_kernarg_v1(values: u64, eligible: u64, target: u64) -> [u8; 40] {
    let mut bytes = [0; EXPLICIT_KERNARG_BYTES];
    put_u64(&mut bytes, 0, values);
    put_u64(&mut bytes, 8, LANES as u64);
    put_u64(&mut bytes, 16, eligible);
    put_u64(&mut bytes, 24, LANES as u64);
    put_u64(&mut bytes, 32, target);
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(
        role: WorkgroupScopedAtomicBufferRoleV1,
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
    fn exact_abi_effect_and_resources_are_fixed() {
        let bytes = pack_explicit_kernarg_v1(0x1000, 0x2000, 0x3000);
        let words: Vec<u64> = bytes
            .chunks_exact(8)
            .map(|word| u64::from_le_bytes(word.try_into().unwrap()))
            .collect();
        assert_eq!(words, [0x1000, 64, 0x2000, 64, 0x3000]);
        assert_eq!((EXPLICIT_KERNARG_BYTES, COMPLETE_KERNARG_BYTES), (40, 296));
        assert_eq!(
            (
                STATIC_GROUP_SEGMENT_BYTES,
                PRIVATE_SEGMENT_BYTES,
                DYNAMIC_LDS_BYTES
            ),
            (0, 0, 0)
        );
        assert_eq!(
            WorkgroupScopedAtomicEffectV1::GlobalU32SystemScopeRelaxedAdd,
            WorkgroupScopedAtomicEffectV1::GlobalU32SystemScopeRelaxedAdd
        );
    }

    #[test]
    fn exact_extents_provenance_alignment_and_all_aliases_fail_closed() {
        let values = validate_region(
            64,
            facts(WorkgroupScopedAtomicBufferRoleV1::Values, 1, 0x1000, 64),
        )
        .unwrap();
        let eligible = validate_region(
            64,
            facts(WorkgroupScopedAtomicBufferRoleV1::Eligible, 2, 0x2000, 64),
        )
        .unwrap();
        let target = validate_region(
            1,
            facts(WorkgroupScopedAtomicBufferRoleV1::Target, 3, 0x3000, 1),
        )
        .unwrap();
        assert_eq!(validate_disjoint([values, eligible, target]), Ok(()));

        let mut wrong = facts(WorkgroupScopedAtomicBufferRoleV1::Target, 3, 0x3000, 1);
        wrong.region_elements = 2;
        assert!(matches!(
            validate_region(1, wrong),
            Err(GeneratedWorkgroupScopedAtomicV1HostAdapterErrorV1::Length { .. })
        ));
        let mut wrong = facts(WorkgroupScopedAtomicBufferRoleV1::Values, 1, 0x1000, 64);
        wrong.region_byte_end -= 4;
        assert!(matches!(
            validate_region(64, wrong),
            Err(GeneratedWorkgroupScopedAtomicV1HostAdapterErrorV1::InvalidProvenance { .. })
        ));
        let wrong = facts(WorkgroupScopedAtomicBufferRoleV1::Target, 3, 0x3002, 1);
        assert!(matches!(
            validate_region(1, wrong),
            Err(GeneratedWorkgroupScopedAtomicV1HostAdapterErrorV1::Alignment { .. })
        ));

        let alias = validate_region(
            1,
            facts(WorkgroupScopedAtomicBufferRoleV1::Target, 1, 0x3000, 1),
        )
        .unwrap();
        assert!(matches!(
            validate_disjoint([values, eligible, alias]),
            Err(GeneratedWorkgroupScopedAtomicV1HostAdapterErrorV1::AllocationAlias { .. })
        ));
        let overlap = validate_region(
            1,
            facts(WorkgroupScopedAtomicBufferRoleV1::Target, 3, 0x1000, 1),
        )
        .unwrap();
        assert!(matches!(
            validate_disjoint([values, eligible, overlap]),
            Err(GeneratedWorkgroupScopedAtomicV1HostAdapterErrorV1::RegionOverlap { .. })
        ));
    }

    #[test]
    fn exact_target_is_closed() {
        assert_eq!(validate_target("gfx942:xnack-"), Ok(()));
        assert_eq!(validate_target("gfx942:sramecc-:xnack-"), Ok(()));
        for target in ["gfx942", "gfx942:xnack+", "gfx950:xnack-", ""] {
            assert_eq!(
                validate_target(target),
                Err(GeneratedWorkgroupScopedAtomicV1HostAdapterErrorV1::ObservedTargetMismatch)
            );
        }
    }
}
