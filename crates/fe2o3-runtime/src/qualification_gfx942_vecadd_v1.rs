//! Exact repository-owned vecadd fixture for gfx942 hardware qualification.
//!
//! This module is intentionally behind `hardware-qualification`. It supplies
//! one reviewable, reproducibly built COV6 object and a crate-private exact
//! KFD qualification gate. It does not authorize arbitrary user code, establish Worker
//! V3 authentication, or generalize beyond the exact policy below.

use core::fmt;

use fe2o3_amdhsa_loader::{AdmittedProfile, validate};
use fe2o3_hsaco::{ArgumentAccess, ArgumentAddressSpace, ExplicitValueKind};
use sha2::{Digest, Sha256};

use crate::{
    BackendBindingV1, BackendMemoryRegionV1, KfdRuntimeAuthorityRequestV1, RuntimeAccessV1,
    RuntimeAllocationIdV1, RuntimeArgumentsV1, RuntimeBindingV1, RuntimeLaunchGeometryV1,
    RuntimeMemoryKindV1, RuntimeMemoryRegionV1,
};

/// Stable identity of this exact qualification-only policy.
pub const GFX942_VECADD_QUALIFICATION_PROFILE_ID_V1: &str =
    "fe2o3.runtime.gfx942-vecadd-qualification.v1";
/// Artifact target declaration admitted by the object and both hardware lanes.
///
/// The omitted SRAM ECC state is compatible with either observed device state;
/// XNACK must be observed disabled.
pub const GFX942_VECADD_QUALIFICATION_TARGET_V1: &str = "gfx942:xnack-";
/// Exact selected AMDHSA metadata kernel name.
pub const GFX942_VECADD_QUALIFICATION_KERNEL_V1: &str = "vecadd";
/// Fixed number of `f32` elements in every qualified buffer.
pub const GFX942_VECADD_QUALIFICATION_ELEMENTS_V1: usize = 1_048_576;
/// Fixed byte length of every qualified buffer.
pub const GFX942_VECADD_QUALIFICATION_BUFFER_BYTES_V1: usize =
    GFX942_VECADD_QUALIFICATION_ELEMENTS_V1 * size_of::<f32>();
/// Complete explicit kernarg length. The object declares no implicit block.
pub const GFX942_VECADD_QUALIFICATION_KERNARG_BYTES_V1: usize = 48;
/// Minimum allocation alignment established by the fixture policy.
pub const GFX942_VECADD_QUALIFICATION_BUFFER_ALIGNMENT_V1: u64 = 4;
/// Quiet-NaN bit pattern required in every output element before dispatch.
pub const GFX942_VECADD_QUALIFICATION_OUTPUT_INITIAL_BITS_V1: u32 = 0x7fc0_0000;

/// Fixed launch shape covered by the qualification policy.
pub const GFX942_VECADD_QUALIFICATION_GEOMETRY_V1: RuntimeLaunchGeometryV1 =
    RuntimeLaunchGeometryV1 {
        grid: [GFX942_VECADD_QUALIFICATION_ELEMENTS_V1 as u32, 1, 1],
        workgroup: [256, 1, 1],
        dynamic_shared_bytes: 0,
    };

/// SHA-256 of the checked LLVM source module.
pub const GFX942_VECADD_QUALIFICATION_SOURCE_SHA256_V1: [u8; 32] = [
    0xb3, 0x41, 0x2c, 0x05, 0x0c, 0xe2, 0x18, 0x2f, 0xeb, 0x66, 0x9d, 0x26, 0x7e, 0x3e, 0x72, 0x08,
    0x40, 0x0c, 0x4d, 0x16, 0xf0, 0x86, 0x5e, 0xfb, 0x7a, 0xea, 0xfd, 0x11, 0x8c, 0x8f, 0x7e, 0x51,
];
/// SHA-256 of `policy-v1.txt`; this is also the typed source-contract identity.
pub const GFX942_VECADD_QUALIFICATION_POLICY_SHA256_V1: [u8; 32] = [
    0x55, 0x88, 0x97, 0xb2, 0xc2, 0x4e, 0xda, 0xcb, 0x9a, 0x0d, 0x83, 0xa6, 0x30, 0xd6, 0xf8, 0x48,
    0x0a, 0x74, 0x09, 0x57, 0x71, 0x21, 0x1f, 0xaf, 0xc0, 0xfd, 0xb8, 0x23, 0xd4, 0x76, 0xc9, 0xa7,
];
/// Nonzero typed signature passed when resolving the exact kernel.
pub const GFX942_VECADD_QUALIFICATION_SIGNATURE_V1: [u8; 32] =
    GFX942_VECADD_QUALIFICATION_POLICY_SHA256_V1;
/// SHA-256 of the checked COV6 object.
pub const GFX942_VECADD_QUALIFICATION_HSACO_SHA256_V1: [u8; 32] = [
    0x3a, 0x25, 0xe3, 0x64, 0xdd, 0x1e, 0x19, 0x31, 0xd1, 0xa1, 0x6c, 0x24, 0xb3, 0x7a, 0xa9, 0x98,
    0xdf, 0x2c, 0x6e, 0xf1, 0xcb, 0xcf, 0x0e, 0xc2, 0xaf, 0xb6, 0x37, 0x2c, 0xbc, 0x87, 0x8b, 0xab,
];

const SOURCE_BYTES_V1: &[u8] = include_bytes!("../fixtures/trusted-gfx942-vecadd-v1/vecadd.ll");
const POLICY_BYTES_V1: &[u8] = include_bytes!("../fixtures/trusted-gfx942-vecadd-v1/policy-v1.txt");
const HSACO_BYTES_V1: &[u8] = include_bytes!("../fixtures/trusted-gfx942-vecadd-v1/vecadd.hsaco");

/// Returns the immutable repository-owned COV6 bytes.
pub const fn gfx942_vecadd_qualification_hsaco_v1() -> &'static [u8] {
    HSACO_BYTES_V1
}

/// Returns the reviewable LLVM source bytes covered by the source digest.
pub const fn gfx942_vecadd_qualification_source_v1() -> &'static [u8] {
    SOURCE_BYTES_V1
}

/// Returns the canonical ABI/effect policy bytes covered by the signature.
pub const fn gfx942_vecadd_qualification_policy_v1() -> &'static [u8] {
    POLICY_BYTES_V1
}

/// Fail-closed stage at which embedded fixture admission stopped.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Gfx942VecaddQualificationAdmissionErrorV1 {
    Identity,
    Envelope,
    KernelClosure,
    AbiOrEffects,
}

impl fmt::Display for Gfx942VecaddQualificationAdmissionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Identity => formatter.write_str("embedded qualification identity mismatch"),
            Self::Envelope => formatter.write_str("embedded qualification envelope rejected"),
            Self::KernelClosure => {
                formatter.write_str("embedded qualification kernel closure rejected")
            }
            Self::AbiOrEffects => {
                formatter.write_str("embedded qualification ABI or effect mismatch")
            }
        }
    }
}

impl std::error::Error for Gfx942VecaddQualificationAdmissionErrorV1 {}

/// Non-cloneable evidence that the embedded fixture was admitted in this process.
///
/// Admission re-hashes the source, policy, and artifact and passes the object
/// through the ordinary COV6 loader before checking the selected kernel's
/// complete explicit ABI, physical resource limits, and compiler-emitted
/// metadata effect declarations.
#[derive(Debug)]
pub struct AdmittedGfx942VecaddQualificationV1 {
    initial_buffer_sha256: [[u8; 32]; 3],
    _private: (),
}

impl AdmittedGfx942VecaddQualificationV1 {
    pub const fn hsaco(&self) -> &'static [u8] {
        HSACO_BYTES_V1
    }

    pub const fn hsaco_sha256(&self) -> [u8; 32] {
        GFX942_VECADD_QUALIFICATION_HSACO_SHA256_V1
    }

    pub const fn kernel_name(&self) -> &'static str {
        GFX942_VECADD_QUALIFICATION_KERNEL_V1
    }

    pub const fn signature(&self) -> [u8; 32] {
        GFX942_VECADD_QUALIFICATION_SIGNATURE_V1
    }

    pub const fn geometry(&self) -> RuntimeLaunchGeometryV1 {
        GFX942_VECADD_QUALIFICATION_GEOMETRY_V1
    }

    pub const fn arguments(&self) -> &'static [Gfx942VecaddQualificationArgumentV1; 3] {
        &GFX942_VECADD_QUALIFICATION_ARGUMENTS_V1
    }

    pub fn explicit_kernarg(&self) -> [u8; GFX942_VECADD_QUALIFICATION_KERNARG_BYTES_V1] {
        gfx942_vecadd_qualification_explicit_kernarg_v1()
    }

    pub fn host_buffers(
        &self,
    ) -> Result<Gfx942VecaddQualificationHostBuffersV1, Gfx942VecaddQualificationFixtureErrorV1>
    {
        gfx942_vecadd_qualification_host_buffers_v1()
    }

    pub(crate) fn authorizes_kfd_request_v1(
        &self,
        request: KfdRuntimeAuthorityRequestV1<'_>,
    ) -> bool {
        request.semantic_launch == crate::KfdRuntimeSemanticLaunchV1::Ordinary
            && exact_artifact_v1(&request)
            && exact_kernarg_and_geometry_v1(&request)
            && exact_abi_and_allocations_v1(&request, &self.initial_buffer_sha256)
    }
}

/// Revalidates and admits the exact embedded qualification fixture.
pub fn admit_gfx942_vecadd_qualification_v1()
-> Result<AdmittedGfx942VecaddQualificationV1, Gfx942VecaddQualificationAdmissionErrorV1> {
    if <[u8; 32]>::from(Sha256::digest(SOURCE_BYTES_V1))
        != GFX942_VECADD_QUALIFICATION_SOURCE_SHA256_V1
        || <[u8; 32]>::from(Sha256::digest(POLICY_BYTES_V1))
            != GFX942_VECADD_QUALIFICATION_POLICY_SHA256_V1
        || <[u8; 32]>::from(Sha256::digest(HSACO_BYTES_V1))
            != GFX942_VECADD_QUALIFICATION_HSACO_SHA256_V1
    {
        return Err(Gfx942VecaddQualificationAdmissionErrorV1::Identity);
    }
    let envelope = validate(HSACO_BYTES_V1, AdmittedProfile::Gfx942XnackOffCov6)
        .map_err(|_| Gfx942VecaddQualificationAdmissionErrorV1::Envelope)?;
    let kernel = envelope
        .bind_kernel(GFX942_VECADD_QUALIFICATION_KERNEL_V1)
        .map_err(|_| Gfx942VecaddQualificationAdmissionErrorV1::KernelClosure)?;
    let resources = kernel.resources();
    let resource_match = resources.kernarg_segment_size()
        == GFX942_VECADD_QUALIFICATION_KERNARG_BYTES_V1 as u64
        && resources.kernarg_segment_alignment() == 8
        && resources.required_workgroup_size() == Some([256, 1, 1])
        && resources.max_flat_workgroup_size() == 256
        && resources.group_segment_fixed_size() == 0
        && resources.private_segment_fixed_size() == 0
        && resources.wavefront_size() == 64
        && kernel.identity_inputs().object_sha256() == GFX942_VECADD_QUALIFICATION_HSACO_SHA256_V1;
    let arguments = kernel.selected_kernel().explicit_arguments();
    let length_names = ["arg0.len", "arg1.len", "arg2.len"];
    let expected_access = [
        ArgumentAccess::ReadOnly,
        ArgumentAccess::ReadOnly,
        ArgumentAccess::WriteOnly,
    ];
    let abi_match = arguments.len() == 6
        && kernel
            .selected_kernel()
            .implicit_argument_offset()
            .is_none()
        && kernel.selected_kernel().implicit_argument_size() == 0
        && GFX942_VECADD_QUALIFICATION_ARGUMENTS_V1
            .iter()
            .enumerate()
            .all(|(index, policy)| {
                let global = &arguments[policy.explicit_argument_index];
                let length = &arguments[policy.explicit_argument_index + 1];
                global.name() == Some(policy.name)
                    && global.offset() == u64::from(policy.pointer_offset)
                    && global.size() == 8
                    && global.value_kind() == ExplicitValueKind::GlobalBuffer
                    && global.address_space() == Some(ArgumentAddressSpace::Global)
                    && global.actual_access() == Some(expected_access[index])
                    && length.name() == Some(length_names[index])
                    && length.offset() == policy.length_offset as u64
                    && length.size() == 8
                    && length.value_kind() == ExplicitValueKind::ByValue
            });
    if !resource_match || !abi_match {
        return Err(Gfx942VecaddQualificationAdmissionErrorV1::AbiOrEffects);
    }
    let buffers = gfx942_vecadd_qualification_host_buffers_v1()
        .map_err(|_| Gfx942VecaddQualificationAdmissionErrorV1::AbiOrEffects)?;
    let initial_buffer_sha256 = [
        Sha256::digest(buffers.left()).into(),
        Sha256::digest(buffers.right()).into(),
        Sha256::digest(buffers.output()).into(),
    ];
    Ok(AdmittedGfx942VecaddQualificationV1 {
        initial_buffer_sha256,
        _private: (),
    })
}

/// One exact global-buffer argument in the qualified explicit ABI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942VecaddQualificationArgumentV1 {
    pub explicit_argument_index: usize,
    pub name: &'static str,
    pub pointer_offset: u32,
    pub length_offset: usize,
    pub access: RuntimeAccessV1,
    /// Loader-side value when upstream metadata omits `.pointee_align`.
    pub reconciled_pointee_alignment: u64,
    /// Minimum alignment independently required of the runtime allocation.
    pub allocation_alignment: u64,
}

/// Complete ordered global-buffer roster for the exact kernel.
pub const GFX942_VECADD_QUALIFICATION_ARGUMENTS_V1: [Gfx942VecaddQualificationArgumentV1; 3] = [
    Gfx942VecaddQualificationArgumentV1 {
        explicit_argument_index: 0,
        name: "arg0.data",
        pointer_offset: 0,
        length_offset: 8,
        access: RuntimeAccessV1::Read,
        reconciled_pointee_alignment: 1,
        allocation_alignment: GFX942_VECADD_QUALIFICATION_BUFFER_ALIGNMENT_V1,
    },
    Gfx942VecaddQualificationArgumentV1 {
        explicit_argument_index: 2,
        name: "arg1.data",
        pointer_offset: 16,
        length_offset: 24,
        access: RuntimeAccessV1::Read,
        reconciled_pointee_alignment: 1,
        allocation_alignment: GFX942_VECADD_QUALIFICATION_BUFFER_ALIGNMENT_V1,
    },
    Gfx942VecaddQualificationArgumentV1 {
        explicit_argument_index: 4,
        name: "arg2.data",
        pointer_offset: 32,
        length_offset: 40,
        access: RuntimeAccessV1::Write,
        reconciled_pointee_alignment: 1,
        allocation_alignment: GFX942_VECADD_QUALIFICATION_BUFFER_ALIGNMENT_V1,
    },
];

/// Failure to construct the exact host-side qualification inputs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Gfx942VecaddQualificationFixtureErrorV1 {
    AliasedAllocations,
    Capacity,
}

impl fmt::Display for Gfx942VecaddQualificationFixtureErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AliasedAllocations => formatter
                .write_str("qualification vecadd requires three distinct allocation handles"),
            Self::Capacity => formatter.write_str("qualification host-buffer allocation failed"),
        }
    }
}

impl std::error::Error for Gfx942VecaddQualificationFixtureErrorV1 {}

/// Context-branded typed arguments for the exact qualification launch.
#[derive(Debug)]
pub struct Gfx942VecaddQualificationArgumentsV1 {
    allocations: [RuntimeAllocationIdV1; 3],
}

impl Gfx942VecaddQualificationArgumentsV1 {
    /// Binds distinct left, right, and output allocations in ABI order.
    pub fn new(
        left: RuntimeAllocationIdV1,
        right: RuntimeAllocationIdV1,
        output: RuntimeAllocationIdV1,
    ) -> Result<Self, Gfx942VecaddQualificationFixtureErrorV1> {
        if left == right || left == output || right == output {
            return Err(Gfx942VecaddQualificationFixtureErrorV1::AliasedAllocations);
        }
        Ok(Self {
            allocations: [left, right, output],
        })
    }

    /// Returns left, right, and output allocation IDs in ABI order.
    pub const fn allocations(&self) -> [RuntimeAllocationIdV1; 3] {
        self.allocations
    }
}

impl RuntimeArgumentsV1 for Gfx942VecaddQualificationArgumentsV1 {
    const SIGNATURE_V1: [u8; 32] = GFX942_VECADD_QUALIFICATION_SIGNATURE_V1;

    fn encode_explicit_kernarg_v1(&self) -> Vec<u8> {
        gfx942_vecadd_qualification_explicit_kernarg_v1().to_vec()
    }

    fn bindings_v1(&self) -> Vec<RuntimeBindingV1> {
        GFX942_VECADD_QUALIFICATION_ARGUMENTS_V1
            .iter()
            .enumerate()
            .map(|(index, policy)| RuntimeBindingV1 {
                region: RuntimeMemoryRegionV1 {
                    allocation: self.allocations[index],
                    access: policy.access,
                    byte_offset: 0,
                    byte_len: GFX942_VECADD_QUALIFICATION_BUFFER_BYTES_V1 as u64,
                },
                kernarg_byte_offset: policy.pointer_offset,
            })
            .collect()
    }
}

/// Owned deterministic input, initial-output, and expected-output bytes.
#[derive(Debug)]
pub struct Gfx942VecaddQualificationHostBuffersV1 {
    left: Vec<u8>,
    right: Vec<u8>,
    output: Vec<u8>,
    expected_output: Vec<u8>,
}

impl Gfx942VecaddQualificationHostBuffersV1 {
    pub fn left(&self) -> &[u8] {
        &self.left
    }

    pub fn right(&self) -> &[u8] {
        &self.right
    }

    pub fn output(&self) -> &[u8] {
        &self.output
    }

    pub fn expected_output(&self) -> &[u8] {
        &self.expected_output
    }

    pub fn into_parts(self) -> (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>) {
        (self.left, self.right, self.output, self.expected_output)
    }
}

/// Constructs all deterministic whole-buffer byte images without infallible growth.
pub fn gfx942_vecadd_qualification_host_buffers_v1()
-> Result<Gfx942VecaddQualificationHostBuffersV1, Gfx942VecaddQualificationFixtureErrorV1> {
    let mut left = try_zeroed_buffer_v1()?;
    let mut right = try_zeroed_buffer_v1()?;
    let mut output = try_zeroed_buffer_v1()?;
    let mut expected_output = try_zeroed_buffer_v1()?;
    for index in 0..GFX942_VECADD_QUALIFICATION_ELEMENTS_V1 {
        let left_bits = qualification_left_v1(index).to_bits().to_le_bytes();
        let right_bits = qualification_right_v1(index).to_bits().to_le_bytes();
        let expected_bits = qualification_expected_v1(index).to_bits().to_le_bytes();
        let start = index * size_of::<f32>();
        let end = start + size_of::<f32>();
        left[start..end].copy_from_slice(&left_bits);
        right[start..end].copy_from_slice(&right_bits);
        output[start..end]
            .copy_from_slice(&GFX942_VECADD_QUALIFICATION_OUTPUT_INITIAL_BITS_V1.to_le_bytes());
        expected_output[start..end].copy_from_slice(&expected_bits);
    }
    Ok(Gfx942VecaddQualificationHostBuffersV1 {
        left,
        right,
        output,
        expected_output,
    })
}

/// Encodes the address-free explicit kernarg template for the fixed fixture.
pub fn gfx942_vecadd_qualification_explicit_kernarg_v1()
-> [u8; GFX942_VECADD_QUALIFICATION_KERNARG_BYTES_V1] {
    let mut bytes = [0; GFX942_VECADD_QUALIFICATION_KERNARG_BYTES_V1];
    let elements = (GFX942_VECADD_QUALIFICATION_ELEMENTS_V1 as u64).to_le_bytes();
    for argument in GFX942_VECADD_QUALIFICATION_ARGUMENTS_V1 {
        bytes[argument.length_offset..argument.length_offset + size_of::<u64>()]
            .copy_from_slice(&elements);
    }
    bytes
}

/// Constructs the exact whole-allocation binding roster.
pub fn gfx942_vecadd_qualification_bindings_v1(
    allocations: [u64; 3],
) -> Result<[BackendBindingV1; 3], Gfx942VecaddQualificationFixtureErrorV1> {
    if allocations[0] == allocations[1]
        || allocations[0] == allocations[2]
        || allocations[1] == allocations[2]
    {
        return Err(Gfx942VecaddQualificationFixtureErrorV1::AliasedAllocations);
    }
    Ok(core::array::from_fn(|index| BackendBindingV1 {
        region: BackendMemoryRegionV1 {
            allocation: allocations[index],
            access: GFX942_VECADD_QUALIFICATION_ARGUMENTS_V1[index].access,
            byte_offset: 0,
            byte_len: GFX942_VECADD_QUALIFICATION_BUFFER_BYTES_V1 as u64,
        },
        kernarg_byte_offset: GFX942_VECADD_QUALIFICATION_ARGUMENTS_V1[index].pointer_offset,
    }))
}

fn exact_artifact_v1(request: &KfdRuntimeAuthorityRequestV1<'_>) -> bool {
    request.module_image == HSACO_BYTES_V1
        && request.module_sha256 == GFX942_VECADD_QUALIFICATION_HSACO_SHA256_V1
        && <[u8; 32]>::from(Sha256::digest(request.module_image))
            == GFX942_VECADD_QUALIFICATION_HSACO_SHA256_V1
        && request.kernel_name == GFX942_VECADD_QUALIFICATION_KERNEL_V1
        && request.signature == GFX942_VECADD_QUALIFICATION_SIGNATURE_V1
}

fn exact_kernarg_and_geometry_v1(request: &KfdRuntimeAuthorityRequestV1<'_>) -> bool {
    let expected = gfx942_vecadd_qualification_explicit_kernarg_v1();
    request.explicit_kernarg == expected
        && request.complete_kernarg_template == expected
        && request.geometry == GFX942_VECADD_QUALIFICATION_GEOMETRY_V1
}

fn exact_abi_and_allocations_v1(
    request: &KfdRuntimeAuthorityRequestV1<'_>,
    initial_buffer_sha256: &[[u8; 32]; 3],
) -> bool {
    if request.bindings.len() != 3
        || request.dispatch_abi.len() != 3
        || request.allocations.len() != 3
    {
        return false;
    }
    let mut allocation_ids = [0_u64; 3];
    for (index, policy) in GFX942_VECADD_QUALIFICATION_ARGUMENTS_V1.iter().enumerate() {
        let binding = request.bindings[index];
        let abi = request.dispatch_abi[index];
        if binding.kernarg_byte_offset != policy.pointer_offset
            || binding.region.access != policy.access
            || binding.region.byte_offset != 0
            || binding.region.byte_len != GFX942_VECADD_QUALIFICATION_BUFFER_BYTES_V1 as u64
            || abi.explicit_argument_index != policy.explicit_argument_index
            || abi.name != policy.name
            || abi.kernarg_byte_offset != u64::from(policy.pointer_offset)
            || abi.pointee_alignment != policy.reconciled_pointee_alignment
            || abi.access != argument_access_v1(policy.access)
        {
            return false;
        }
        allocation_ids[index] = binding.region.allocation;
    }
    if allocation_ids[0] == allocation_ids[1]
        || allocation_ids[0] == allocation_ids[2]
        || allocation_ids[1] == allocation_ids[2]
    {
        return false;
    }
    allocation_ids.iter().enumerate().all(|(index, id)| {
        request
            .allocations
            .iter()
            .find(|allocation| allocation.allocation == *id)
            .is_some_and(|allocation| {
                allocation.kind == RuntimeMemoryKindV1::HostVisible
                    && allocation.alignment.is_power_of_two()
                    && allocation.alignment >= GFX942_VECADD_QUALIFICATION_BUFFER_ALIGNMENT_V1
                    && allocation.byte_offset == 0
                    && allocation.bytes.len() == GFX942_VECADD_QUALIFICATION_BUFFER_BYTES_V1
                    && allocation.content_sha256 == Some(initial_buffer_sha256[index])
            })
    })
}

const fn argument_access_v1(access: RuntimeAccessV1) -> ArgumentAccess {
    match access {
        RuntimeAccessV1::Read => ArgumentAccess::ReadOnly,
        RuntimeAccessV1::Write => ArgumentAccess::WriteOnly,
        RuntimeAccessV1::ReadWrite => ArgumentAccess::ReadWrite,
    }
}

fn try_zeroed_buffer_v1() -> Result<Vec<u8>, Gfx942VecaddQualificationFixtureErrorV1> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(GFX942_VECADD_QUALIFICATION_BUFFER_BYTES_V1)
        .map_err(|_| Gfx942VecaddQualificationFixtureErrorV1::Capacity)?;
    bytes.resize(GFX942_VECADD_QUALIFICATION_BUFFER_BYTES_V1, 0);
    Ok(bytes)
}

const fn qualification_left_v1(index: usize) -> f32 {
    ((index & 1023) as f32) * 0.5
}

const fn qualification_right_v1(index: usize) -> f32 {
    ((index & 255) as f32) * 0.25
}

const fn qualification_expected_v1(index: usize) -> f32 {
    qualification_left_v1(index) + qualification_right_v1(index)
}

#[cfg(test)]
mod tests {
    use fe2o3_amdhsa_loader::{AdmittedProfile, validate};
    use fe2o3_hsaco::{ArgumentAccess, ExplicitValueKind};

    use super::*;
    use crate::{KfdRuntimeAuthorityAllocationV1, KfdRuntimeAuthorityGlobalBufferV1};

    #[test]
    fn checked_source_policy_and_artifact_have_exact_identities() {
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(SOURCE_BYTES_V1)),
            GFX942_VECADD_QUALIFICATION_SOURCE_SHA256_V1
        );
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(POLICY_BYTES_V1)),
            GFX942_VECADD_QUALIFICATION_POLICY_SHA256_V1
        );
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(HSACO_BYTES_V1)),
            GFX942_VECADD_QUALIFICATION_HSACO_SHA256_V1
        );
    }

    #[test]
    fn checked_artifact_matches_the_reviewed_loader_contract() {
        let admitted = admit_gfx942_vecadd_qualification_v1().unwrap();
        assert_eq!(admitted.hsaco(), HSACO_BYTES_V1);
        assert_eq!(
            admitted.kernel_name(),
            GFX942_VECADD_QUALIFICATION_KERNEL_V1
        );
        let kernel = validate(HSACO_BYTES_V1, AdmittedProfile::Gfx942XnackOffCov6)
            .unwrap()
            .bind_kernel(GFX942_VECADD_QUALIFICATION_KERNEL_V1)
            .unwrap();
        let resources = kernel.resources();
        assert_eq!(resources.kernarg_segment_size(), 48);
        assert_eq!(resources.kernarg_segment_alignment(), 8);
        assert_eq!(resources.required_workgroup_size(), Some([256, 1, 1]));
        assert_eq!(resources.group_segment_fixed_size(), 0);
        assert_eq!(resources.private_segment_fixed_size(), 0);
        let arguments = kernel.selected_kernel().explicit_arguments();
        assert_eq!(arguments.len(), 6);
        for (policy, access) in GFX942_VECADD_QUALIFICATION_ARGUMENTS_V1.iter().zip([
            ArgumentAccess::ReadOnly,
            ArgumentAccess::ReadOnly,
            ArgumentAccess::WriteOnly,
        ]) {
            let argument = &arguments[policy.explicit_argument_index];
            assert_eq!(argument.name(), Some(policy.name));
            assert_eq!(argument.offset(), u64::from(policy.pointer_offset));
            assert_eq!(argument.size(), 8);
            assert_eq!(argument.value_kind(), ExplicitValueKind::GlobalBuffer);
            assert_eq!(argument.actual_access(), Some(access));
        }
    }

    #[test]
    fn exact_qualification_gate_accepts_the_complete_fixed_invocation() {
        let buffers = gfx942_vecadd_qualification_host_buffers_v1().unwrap();
        let bindings = gfx942_vecadd_qualification_bindings_v1([10, 20, 30]).unwrap();
        let allocations = [
            authority_allocation_v1(10, buffers.left()),
            authority_allocation_v1(20, buffers.right()),
            authority_allocation_v1(30, buffers.output()),
        ];
        let dispatch_abi = authority_abi_v1();
        let kernarg = gfx942_vecadd_qualification_explicit_kernarg_v1();
        let mut request = KfdRuntimeAuthorityRequestV1 {
            module_image: HSACO_BYTES_V1,
            module_sha256: GFX942_VECADD_QUALIFICATION_HSACO_SHA256_V1,
            kernel_name: GFX942_VECADD_QUALIFICATION_KERNEL_V1,
            signature: GFX942_VECADD_QUALIFICATION_SIGNATURE_V1,
            explicit_kernarg: &kernarg,
            complete_kernarg_template: &kernarg,
            bindings: &bindings,
            dispatch_abi: &dispatch_abi,
            allocations: &allocations,
            geometry: GFX942_VECADD_QUALIFICATION_GEOMETRY_V1,
            semantic_launch: crate::KfdRuntimeSemanticLaunchV1::Ordinary,
        };
        assert!(
            admit_gfx942_vecadd_qualification_v1()
                .unwrap()
                .authorizes_kfd_request_v1(request)
        );
        request.semantic_launch =
            crate::KfdRuntimeSemanticLaunchV1::Atomic(crate::RuntimeAtomicLaunchContractV1 {
                operation: crate::RuntimeAtomicOperationV1::Add,
                scope: crate::RuntimeMemoryScopeV1::Workgroup,
                order: crate::RuntimeMemoryOrderV1::Relaxed,
                failure_order: None,
                weak: false,
                geometry: GFX942_VECADD_QUALIFICATION_GEOMETRY_V1,
            });
        assert!(
            !admit_gfx942_vecadd_qualification_v1()
                .unwrap()
                .authorizes_kfd_request_v1(request)
        );
    }

    #[test]
    fn qualification_gate_rejects_each_policy_dimension_independently() {
        let mut invocation = TestInvocationV1::new();
        assert!(invocation.authorize_v1(HSACO_BYTES_V1));

        let mut image = HSACO_BYTES_V1.to_vec();
        image[1024] ^= 1;
        assert!(!invocation.authorize_v1(&image), "module bytes");

        invocation.module_sha256[0] ^= 1;
        assert!(!invocation.authorize_v1(HSACO_BYTES_V1), "module digest");
        invocation.module_sha256 = GFX942_VECADD_QUALIFICATION_HSACO_SHA256_V1;

        invocation.signature[0] ^= 1;
        assert!(!invocation.authorize_v1(HSACO_BYTES_V1), "signature");
        invocation.signature = GFX942_VECADD_QUALIFICATION_SIGNATURE_V1;

        invocation.kernarg[8] ^= 1;
        assert!(!invocation.authorize_v1(HSACO_BYTES_V1), "kernarg length");
        invocation.kernarg = gfx942_vecadd_qualification_explicit_kernarg_v1();

        invocation.geometry.grid[0] -= 256;
        assert!(!invocation.authorize_v1(HSACO_BYTES_V1), "geometry");
        invocation.geometry = GFX942_VECADD_QUALIFICATION_GEOMETRY_V1;

        invocation.bindings[0].region.access = RuntimeAccessV1::ReadWrite;
        assert!(!invocation.authorize_v1(HSACO_BYTES_V1), "binding effect");
        invocation.bindings[0].region.access = RuntimeAccessV1::Read;

        invocation.bindings[0].kernarg_byte_offset = 4;
        assert!(!invocation.authorize_v1(HSACO_BYTES_V1), "binding offset");
        invocation.bindings[0].kernarg_byte_offset = 0;

        invocation.bindings[1].region.allocation = 10;
        assert!(!invocation.authorize_v1(HSACO_BYTES_V1), "allocation alias");
        invocation.bindings[1].region.allocation = 20;

        invocation.kinds[0] = RuntimeMemoryKindV1::DeviceLocal;
        assert!(!invocation.authorize_v1(HSACO_BYTES_V1), "allocation kind");
        invocation.kinds[0] = RuntimeMemoryKindV1::HostVisible;

        invocation.alignments[0] = 2;
        assert!(
            !invocation.authorize_v1(HSACO_BYTES_V1),
            "allocation alignment"
        );
        invocation.alignments[0] = GFX942_VECADD_QUALIFICATION_BUFFER_ALIGNMENT_V1;

        invocation.byte_offsets[0] = 4;
        assert!(
            !invocation.authorize_v1(HSACO_BYTES_V1),
            "allocation window"
        );
        invocation.byte_offsets[0] = 0;

        invocation.buffers.left[17] ^= 1;
        assert!(
            !invocation.authorize_v1(HSACO_BYTES_V1),
            "allocation content"
        );
    }

    fn authority_allocation_v1(
        allocation: u64,
        bytes: &[u8],
    ) -> KfdRuntimeAuthorityAllocationV1<'_> {
        KfdRuntimeAuthorityAllocationV1 {
            allocation,
            kind: RuntimeMemoryKindV1::HostVisible,
            alignment: GFX942_VECADD_QUALIFICATION_BUFFER_ALIGNMENT_V1,
            byte_offset: 0,
            bytes,
            content_sha256: Some(Sha256::digest(bytes).into()),
        }
    }

    fn authority_abi_v1() -> [KfdRuntimeAuthorityGlobalBufferV1<'static>; 3] {
        core::array::from_fn(|index| {
            let policy = GFX942_VECADD_QUALIFICATION_ARGUMENTS_V1[index];
            KfdRuntimeAuthorityGlobalBufferV1 {
                explicit_argument_index: policy.explicit_argument_index,
                name: policy.name,
                kernarg_byte_offset: u64::from(policy.pointer_offset),
                pointee_alignment: policy.reconciled_pointee_alignment,
                access: argument_access_v1(policy.access),
            }
        })
    }

    struct TestInvocationV1 {
        buffers: Gfx942VecaddQualificationHostBuffersV1,
        bindings: [BackendBindingV1; 3],
        dispatch_abi: [KfdRuntimeAuthorityGlobalBufferV1<'static>; 3],
        kernarg: [u8; GFX942_VECADD_QUALIFICATION_KERNARG_BYTES_V1],
        module_sha256: [u8; 32],
        signature: [u8; 32],
        geometry: RuntimeLaunchGeometryV1,
        kinds: [RuntimeMemoryKindV1; 3],
        alignments: [u64; 3],
        byte_offsets: [u64; 3],
    }

    impl TestInvocationV1 {
        fn new() -> Self {
            Self {
                buffers: gfx942_vecadd_qualification_host_buffers_v1().unwrap(),
                bindings: gfx942_vecadd_qualification_bindings_v1([10, 20, 30]).unwrap(),
                dispatch_abi: authority_abi_v1(),
                kernarg: gfx942_vecadd_qualification_explicit_kernarg_v1(),
                module_sha256: GFX942_VECADD_QUALIFICATION_HSACO_SHA256_V1,
                signature: GFX942_VECADD_QUALIFICATION_SIGNATURE_V1,
                geometry: GFX942_VECADD_QUALIFICATION_GEOMETRY_V1,
                kinds: [RuntimeMemoryKindV1::HostVisible; 3],
                alignments: [GFX942_VECADD_QUALIFICATION_BUFFER_ALIGNMENT_V1; 3],
                byte_offsets: [0; 3],
            }
        }

        fn authorize_v1(&self, module_image: &[u8]) -> bool {
            let bytes = [
                self.buffers.left(),
                self.buffers.right(),
                self.buffers.output(),
            ];
            let allocations: [KfdRuntimeAuthorityAllocationV1<'_>; 3] =
                core::array::from_fn(|index| KfdRuntimeAuthorityAllocationV1 {
                    allocation: [10, 20, 30][index],
                    kind: self.kinds[index],
                    alignment: self.alignments[index],
                    byte_offset: self.byte_offsets[index],
                    bytes: bytes[index],
                    content_sha256: Some(Sha256::digest(bytes[index]).into()),
                });
            admit_gfx942_vecadd_qualification_v1()
                .unwrap()
                .authorizes_kfd_request_v1(KfdRuntimeAuthorityRequestV1 {
                    module_image,
                    module_sha256: self.module_sha256,
                    kernel_name: GFX942_VECADD_QUALIFICATION_KERNEL_V1,
                    signature: self.signature,
                    explicit_kernarg: &self.kernarg,
                    complete_kernarg_template: &self.kernarg,
                    bindings: &self.bindings,
                    dispatch_abi: &self.dispatch_abi,
                    allocations: &allocations,
                    geometry: self.geometry,
                    semantic_launch: crate::KfdRuntimeSemanticLaunchV1::Ordinary,
                })
        }
    }
}
