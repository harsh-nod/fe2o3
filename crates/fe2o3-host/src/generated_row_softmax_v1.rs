//! Generated typed host binding for the exact protected row-softmax V1 ABI.
//!
//! The binding owns one shared input lease and one unique output lease for
//! exactly 64 `f32` values. It exposes neither device addresses nor packed
//! kernarg bytes and is inert until joined with the protected host token.

use crate::ObservedContext;
use fe2o3_amd_target::AmdTargetId;
use fe2o3_core::{DeviceBufferRegion, DeviceBufferView, DeviceBufferViewMut, DeviceCopy};
use std::{
    error::Error,
    fmt,
    mem::{align_of, size_of},
};

const TARGET: &str = "gfx942:xnack-";
const ELEMENTS: usize = 64;
const EXPLICIT_KERNARG_BYTES: usize = 32;
const COMPLETE_KERNARG_BYTES: u32 = 288;
const KERNARG_ALIGNMENT: u32 = 8;
const GRID: [u32; 3] = [1, 1, 1];
const WORKGROUP: [u32; 3] = [64, 1, 1];

/// Exact argument role reported by generated-binding diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtectedRowSoftmaxV1BufferRoleV1 {
    Input,
    Output,
}

#[repr(C, align(8))]
struct ExplicitKernargV1 {
    bytes: [u8; EXPLICIT_KERNARG_BYTES],
}

const _: () = assert!(size_of::<ExplicitKernargV1>() == EXPLICIT_KERNARG_BYTES);
const _: () = assert!(align_of::<ExplicitKernargV1>() == KERNARG_ALIGNMENT as usize);

/// Exact generated binding for one unmasked, 64-element row.
///
/// This type is linear and cannot expose its raw kernarg representation.
///
/// ```compile_fail
/// use fe2o3_host::GeneratedProtectedRowSoftmaxV1HostAdapterV1;
/// fn replay(value: GeneratedProtectedRowSoftmaxV1HostAdapterV1<'_, '_>) {
///     let _ = value.clone();
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_host::GeneratedProtectedRowSoftmaxV1HostAdapterV1;
/// fn expose(value: &GeneratedProtectedRowSoftmaxV1HostAdapterV1<'_, '_>) {
///     let _ = value.explicit_kernarg_bytes_v1();
/// }
/// ```
#[must_use = "the generated row-softmax binding must enter the protected lifecycle"]
pub struct GeneratedProtectedRowSoftmaxV1HostAdapterV1<'input, 'output> {
    observed: ObservedContext,
    explicit_kernarg: ExplicitKernargV1,
    _input: DeviceBufferView<'input, f32>,
    _output: DeviceBufferViewMut<'output, f32>,
}

impl fmt::Debug for GeneratedProtectedRowSoftmaxV1HostAdapterV1<'_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedProtectedRowSoftmaxV1HostAdapterV1")
            .field("target", &TARGET)
            .field("elements", &ELEMENTS)
            .field("grid", &GRID)
            .field("workgroup", &WORKGROUP)
            .finish_non_exhaustive()
    }
}

impl<'input, 'output> GeneratedProtectedRowSoftmaxV1HostAdapterV1<'input, 'output> {
    /// Validates and retains the exact shared-input/unique-output invocation.
    pub fn prepare(
        observed: &ObservedContext,
        input: DeviceBufferView<'input, f32>,
        output: DeviceBufferViewMut<'output, f32>,
    ) -> Result<Self, GeneratedProtectedRowSoftmaxV1HostAdapterErrorV1> {
        validate_observed_target(observed.device().target())?;
        if !observed.is_for_context(input.context()) {
            return Err(
                GeneratedProtectedRowSoftmaxV1HostAdapterErrorV1::WrongContext {
                    role: ProtectedRowSoftmaxV1BufferRoleV1::Input,
                },
            );
        }
        if !observed.is_for_context(output.context()) {
            return Err(
                GeneratedProtectedRowSoftmaxV1HostAdapterErrorV1::WrongContext {
                    role: ProtectedRowSoftmaxV1BufferRoleV1::Output,
                },
            );
        }

        let input_facts = RegionFacts::from_region(&input);
        let output_facts = RegionFacts::from_region(&output);
        let explicit_kernarg = prepare_regions(input_facts, output_facts)?;
        Ok(Self {
            observed: observed.clone(),
            explicit_kernarg,
            _input: input,
            _output: output,
        })
    }

    pub const fn target(&self) -> &'static str {
        TARGET
    }

    pub const fn row_elements(&self) -> usize {
        ELEMENTS
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

    pub const fn complete_kernarg_byte_len(&self) -> u32 {
        COMPLETE_KERNARG_BYTES
    }

    pub const fn kernarg_alignment(&self) -> u32 {
        KERNARG_ALIGNMENT
    }

    pub const fn is_unmasked_all_64_profile(&self) -> bool {
        true
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
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
) -> Result<(), GeneratedProtectedRowSoftmaxV1HostAdapterErrorV1> {
    let expected = AmdTargetId::parse(TARGET)
        .map_err(|_| GeneratedProtectedRowSoftmaxV1HostAdapterErrorV1::ObservedTargetMismatch)?;
    let actual = AmdTargetId::parse(target)
        .map_err(|_| GeneratedProtectedRowSoftmaxV1HostAdapterErrorV1::ObservedTargetMismatch)?;
    if !expected.is_compatible_with_observed(&actual) {
        return Err(GeneratedProtectedRowSoftmaxV1HostAdapterErrorV1::ObservedTargetMismatch);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RegionFacts {
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
struct CheckedRegion {
    address: u64,
    byte_start: usize,
    byte_end: usize,
}

fn validate_region(
    role: ProtectedRowSoftmaxV1BufferRoleV1,
    facts: RegionFacts,
) -> Result<CheckedRegion, GeneratedProtectedRowSoftmaxV1HostAdapterErrorV1> {
    if facts.element_bytes != size_of::<f32>() || facts.element_alignment != align_of::<f32>() {
        return Err(GeneratedProtectedRowSoftmaxV1HostAdapterErrorV1::ElementLayout { role });
    }
    if facts.region_elements != ELEMENTS {
        return Err(GeneratedProtectedRowSoftmaxV1HostAdapterErrorV1::Length {
            role,
            expected: ELEMENTS,
            actual: facts.region_elements,
        });
    }
    if facts.allocation_address == 0 || facts.region_address == 0 {
        return Err(GeneratedProtectedRowSoftmaxV1HostAdapterErrorV1::NullAddress { role });
    }
    if !facts.region_address.is_multiple_of(align_of::<f32>()) {
        return Err(GeneratedProtectedRowSoftmaxV1HostAdapterErrorV1::Alignment { role });
    }

    let allocation_bytes = facts
        .allocation_elements
        .checked_mul(facts.element_bytes)
        .ok_or(GeneratedProtectedRowSoftmaxV1HostAdapterErrorV1::RegionOverflow { role })?;
    let allocation_end = facts
        .allocation_address
        .checked_add(allocation_bytes)
        .ok_or(GeneratedProtectedRowSoftmaxV1HostAdapterErrorV1::RegionOverflow { role })?;
    let region_bytes = facts
        .region_elements
        .checked_mul(facts.element_bytes)
        .ok_or(GeneratedProtectedRowSoftmaxV1HostAdapterErrorV1::RegionOverflow { role })?;
    let region_end = facts
        .region_address
        .checked_add(region_bytes)
        .ok_or(GeneratedProtectedRowSoftmaxV1HostAdapterErrorV1::RegionOverflow { role })?;
    let expected_address = facts
        .allocation_address
        .checked_add(facts.region_byte_start)
        .ok_or(GeneratedProtectedRowSoftmaxV1HostAdapterErrorV1::RegionOverflow { role })?;
    if facts.region_byte_end > allocation_bytes
        || facts.region_byte_end.checked_sub(facts.region_byte_start) != Some(region_bytes)
        || expected_address != facts.region_address
        || facts.region_address < facts.allocation_address
        || region_end > allocation_end
    {
        return Err(GeneratedProtectedRowSoftmaxV1HostAdapterErrorV1::InvalidRegion { role });
    }
    Ok(CheckedRegion {
        address: u64::try_from(facts.region_address)
            .map_err(|_| GeneratedProtectedRowSoftmaxV1HostAdapterErrorV1::PointerWidth { role })?,
        byte_start: facts.region_address,
        byte_end: region_end,
    })
}

fn prepare_regions(
    input: RegionFacts,
    output: RegionFacts,
) -> Result<ExplicitKernargV1, GeneratedProtectedRowSoftmaxV1HostAdapterErrorV1> {
    let input = validate_region(ProtectedRowSoftmaxV1BufferRoleV1::Input, input)?;
    let output = validate_region(ProtectedRowSoftmaxV1BufferRoleV1::Output, output)?;
    if input.byte_start < output.byte_end && output.byte_start < input.byte_end {
        return Err(GeneratedProtectedRowSoftmaxV1HostAdapterErrorV1::InputOutputOverlap);
    }
    let mut bytes = [0_u8; EXPLICIT_KERNARG_BYTES];
    for (slot, region) in bytes.chunks_exact_mut(16).zip([input, output]) {
        slot[..8].copy_from_slice(&region.address.to_le_bytes());
        slot[8..].copy_from_slice(&(ELEMENTS as u64).to_le_bytes());
    }
    Ok(ExplicitKernargV1 { bytes })
}

/// Authority-free rejection while preparing the exact typed binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum GeneratedProtectedRowSoftmaxV1HostAdapterErrorV1 {
    ObservedTargetMismatch,
    WrongContext {
        role: ProtectedRowSoftmaxV1BufferRoleV1,
    },
    ElementLayout {
        role: ProtectedRowSoftmaxV1BufferRoleV1,
    },
    Length {
        role: ProtectedRowSoftmaxV1BufferRoleV1,
        expected: usize,
        actual: usize,
    },
    NullAddress {
        role: ProtectedRowSoftmaxV1BufferRoleV1,
    },
    Alignment {
        role: ProtectedRowSoftmaxV1BufferRoleV1,
    },
    RegionOverflow {
        role: ProtectedRowSoftmaxV1BufferRoleV1,
    },
    InvalidRegion {
        role: ProtectedRowSoftmaxV1BufferRoleV1,
    },
    PointerWidth {
        role: ProtectedRowSoftmaxV1BufferRoleV1,
    },
    InputOutputOverlap,
}

impl fmt::Display for GeneratedProtectedRowSoftmaxV1HostAdapterErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "protected row-softmax host binding rejected: {self:?}"
        )
    }
}

impl Error for GeneratedProtectedRowSoftmaxV1HostAdapterErrorV1 {}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;

    pub(super) type TestRegionFactsV1 = RegionFacts;

    pub(super) fn region(
        allocation_address: usize,
        allocation_elements: usize,
        region_address: usize,
        region_elements: usize,
        region_byte_start: usize,
    ) -> TestRegionFactsV1 {
        TestRegionFactsV1 {
            allocation_address,
            allocation_elements,
            region_address,
            region_elements,
            region_byte_start,
            region_byte_end: region_byte_start + region_elements * size_of::<f32>(),
            element_bytes: size_of::<f32>(),
            element_alignment: align_of::<f32>(),
        }
    }

    pub(super) fn prepare_regions_v1(
        input: TestRegionFactsV1,
        output: TestRegionFactsV1,
    ) -> Result<[u8; EXPLICIT_KERNARG_BYTES], GeneratedProtectedRowSoftmaxV1HostAdapterErrorV1>
    {
        Ok(prepare_regions(input, output)?.bytes)
    }

    pub(super) fn validate_target_v1(
        target: &str,
    ) -> Result<(), GeneratedProtectedRowSoftmaxV1HostAdapterErrorV1> {
        validate_observed_target(target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canonical() -> (RegionFacts, RegionFacts) {
        (
            test_support::region(0x1000, 192, 0x1080, ELEMENTS, 0x80),
            test_support::region(0x2000, 192, 0x2080, ELEMENTS, 0x80),
        )
    }

    #[test]
    fn exact_guarded_regions_pack_only_two_fixed_slices() {
        let (input, output) = canonical();
        let bytes = test_support::prepare_regions_v1(input, output).unwrap();
        assert_eq!(bytes.len(), 32);
        for (index, address) in [0x1080_u64, 0x2080].into_iter().enumerate() {
            let offset = index * 16;
            assert_eq!(&bytes[offset..offset + 8], &address.to_le_bytes());
            assert_eq!(&bytes[offset + 8..offset + 16], &64_u64.to_le_bytes());
        }
    }

    #[test]
    fn every_region_boundary_and_layout_substitution_is_rejected() {
        let mutations: &[fn(&mut RegionFacts, &mut RegionFacts)] = &[
            |input, _| input.region_elements = 63,
            |_, output| output.region_elements = 65,
            |input, _| input.allocation_address = 0,
            |_, output| output.region_address = 0,
            |input, _| input.region_address += 1,
            |_, output| output.element_bytes = 8,
            |input, _| input.element_alignment = 8,
            |input, _| input.region_byte_end -= 4,
            |_, output| output.region_byte_start += 4,
            |input, _| input.allocation_elements = 32,
        ];
        for (index, mutate) in mutations.iter().enumerate() {
            let (mut input, mut output) = canonical();
            mutate(&mut input, &mut output);
            assert!(
                test_support::prepare_regions_v1(input, output).is_err(),
                "mutation {index} escaped"
            );
        }
    }

    #[test]
    fn input_and_unique_output_must_be_physically_disjoint() {
        let (input, mut output) = canonical();
        output.allocation_address = input.allocation_address;
        output.allocation_elements = input.allocation_elements;
        output.region_address = input.region_address + 4;
        output.region_byte_start = input.region_byte_start + 4;
        output.region_byte_end = output.region_byte_start + ELEMENTS * size_of::<f32>();
        assert_eq!(
            test_support::prepare_regions_v1(input, output),
            Err(GeneratedProtectedRowSoftmaxV1HostAdapterErrorV1::InputOutputOverlap)
        );
    }

    #[test]
    fn exact_target_is_required() {
        assert!(test_support::validate_target_v1(TARGET).is_ok());
        for target in ["gfx942:xnack+", "gfx1100", "gfx942"] {
            assert!(
                test_support::validate_target_v1(target).is_err(),
                "{target}"
            );
        }
    }
}
