//! Two exact fixed-work fixtures for ordinary native scheduling qualification.
//!
//! This is not general compiler authority, a machine-code refinement proof, or
//! a duration/overlap guarantee. Both variants require one whole HostVisible
//! allocation. The length argument counts payload words, not allocation words.

use core::fmt;
use fe2o3_amdhsa_loader::{AdmittedProfile, KernelGlobalBufferAbiV1, validate};
use fe2o3_hsaco::{ArgumentAccess, ArgumentAddressSpace, ExplicitValueKind};
use sha2::{Digest, Sha256};

use crate::{
    BackendBindingV1, BackendMemoryRegionV1, KfdRuntimeAuthorityRequestV1,
    KfdRuntimeSemanticLaunchV1, RuntimeAccessV1, RuntimeAllocationIdV1, RuntimeArgumentsV1,
    RuntimeBindingV1, RuntimeLaunchGeometryV1, RuntimeMemoryKindV1, RuntimeMemoryRegionV1,
};

pub const GFX942_MIXED_DURATION_QUALIFICATION_PROFILE_ID_V1: &str =
    "fe2o3.runtime.gfx942-mixed-duration-qualification.v1";
pub const GFX942_MIXED_DURATION_QUALIFICATION_ELEMENTS_V1: usize = 64;
pub const GFX942_MIXED_DURATION_QUALIFICATION_GUARD_WORDS_V1: usize = 16;
pub const GFX942_MIXED_DURATION_QUALIFICATION_BUFFER_BYTES_V1: usize = 384;
pub const GFX942_MIXED_DURATION_QUALIFICATION_GEOMETRY_V1: RuntimeLaunchGeometryV1 =
    RuntimeLaunchGeometryV1 {
        grid: [64, 1, 1],
        workgroup: [64, 1, 1],
        dynamic_shared_bytes: 0,
    };
pub const GFX942_MIXED_DURATION_QUALIFICATION_SIGNATURE_V1: [u8; 32] = [
    0xff, 0x19, 0x6c, 0xd4, 0x7e, 0xa2, 0x5c, 0xd5, 0x58, 0xf9, 0xe6, 0xfa, 0x8d, 0xa4, 0xe8, 0x95,
    0x38, 0x0a, 0x23, 0xf5, 0xff, 0x55, 0x05, 0x6a, 0x78, 0xca, 0x0e, 0x45, 0x6c, 0x4e, 0x20, 0xd1,
];

const POLICY: &[u8] = include_bytes!("../fixtures/trusted-gfx942-mixed-duration-v1/policy-v1.txt");
const INITIAL: &[u8; GFX942_MIXED_DURATION_QUALIFICATION_BUFFER_BYTES_V1] =
    include_bytes!("../fixtures/trusted-gfx942-mixed-duration-v1/initial.bin");

struct Fixture {
    source: &'static [u8],
    source_sha256: [u8; 32],
    object: &'static [u8],
    object_sha256: [u8; 32],
    kernel: &'static str,
    iterations: u32,
}

const SHORT: Fixture = Fixture {
    source: include_bytes!("../fixtures/trusted-gfx942-mixed-duration-v1/short.ll"),
    source_sha256: [
        0x95, 0xb3, 0x88, 0xcc, 0xeb, 0x9a, 0x71, 0x74, 0xf4, 0xd5, 0x80, 0x4b, 0x6a, 0xba, 0xb7,
        0xe3, 0xab, 0x22, 0x34, 0x5b, 0x19, 0xd0, 0x27, 0xa9, 0x90, 0x98, 0x0c, 0x0b, 0x29, 0xab,
        0x5d, 0x36,
    ],
    object: include_bytes!("../fixtures/trusted-gfx942-mixed-duration-v1/short.hsaco"),
    object_sha256: [
        0xb3, 0xcf, 0x15, 0x84, 0x58, 0x79, 0xf9, 0x68, 0xb3, 0xe3, 0xb0, 0x9f, 0x4a, 0x5f, 0x4b,
        0xdd, 0x72, 0xd1, 0xf7, 0x74, 0x66, 0xa0, 0xc0, 0x48, 0xd9, 0x72, 0x7d, 0xa0, 0x66, 0x71,
        0x9e, 0xe5,
    ],
    kernel: "mixed_short",
    iterations: 257,
};
const LONG: Fixture = Fixture {
    source: include_bytes!("../fixtures/trusted-gfx942-mixed-duration-v1/long.ll"),
    source_sha256: [
        0x8f, 0x59, 0x61, 0xbc, 0xdf, 0x46, 0x26, 0x36, 0x86, 0x3d, 0x91, 0x69, 0xf7, 0x90, 0x3e,
        0x12, 0x83, 0x05, 0x43, 0x2a, 0xd5, 0x11, 0x33, 0x5a, 0x73, 0x37, 0x9c, 0xca, 0xe6, 0x70,
        0xc8, 0x29,
    ],
    object: include_bytes!("../fixtures/trusted-gfx942-mixed-duration-v1/long.hsaco"),
    object_sha256: [
        0x64, 0x2f, 0x08, 0xb1, 0xce, 0x18, 0xf6, 0xc3, 0x42, 0x8d, 0x3d, 0xfa, 0xc4, 0xc9, 0xe5,
        0x67, 0xb3, 0xd2, 0x3c, 0x11, 0xce, 0xc0, 0x84, 0x9a, 0xb4, 0x7a, 0x6d, 0xe8, 0xa0, 0x62,
        0x74, 0xa9,
    ],
    kernel: "mixed_long",
    iterations: 33_554_433,
};

/// Closed fixture selection; work bounds cannot be supplied at launch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942MixedDurationQualificationVariantV1 {
    Short,
    Long,
}

impl Gfx942MixedDurationQualificationVariantV1 {
    const fn fixture(self) -> &'static Fixture {
        match self {
            Self::Short => &SHORT,
            Self::Long => &LONG,
        }
    }
    pub const fn hsaco(self) -> &'static [u8] {
        self.fixture().object
    }
    pub const fn hsaco_sha256(self) -> [u8; 32] {
        self.fixture().object_sha256
    }
    pub const fn kernel_name(self) -> &'static str {
        self.fixture().kernel
    }
    pub const fn iterations(self) -> u32 {
        self.fixture().iterations
    }

    /// Computes the entire expected image in O(log(iterations) + buffer words).
    pub fn expected_output(self) -> [u8; GFX942_MIXED_DURATION_QUALIFICATION_BUFFER_BYTES_V1] {
        let mut bytes = gfx942_mixed_duration_qualification_initial_v1();
        let (scale, shift) = affine_power(self.iterations());
        for lane in 0..GFX942_MIXED_DURATION_QUALIFICATION_ELEMENTS_V1 {
            let value = initial_payload(lane as u32)
                .wrapping_mul(scale)
                .wrapping_add(shift);
            let offset = (lane + GFX942_MIXED_DURATION_QUALIFICATION_GUARD_WORDS_V1) * 4;
            bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        bytes
    }

    /// Checks every payload and guard byte, never a prefix or sample.
    pub fn validate_output(self, bytes: &[u8]) -> bool {
        bytes == self.expected_output()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Gfx942MixedDurationQualificationAdmissionErrorV1 {
    Identity,
    Envelope,
    KernelClosure,
    AbiOrEffects,
}

impl fmt::Display for Gfx942MixedDurationQualificationAdmissionErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "mixed-duration qualification admission failed: {self:?}")
    }
}
impl std::error::Error for Gfx942MixedDurationQualificationAdmissionErrorV1 {}

/// Non-cloneable evidence for the exact embedded pair, with no production authority.
#[derive(Debug)]
pub struct AdmittedGfx942MixedDurationQualificationV1 {
    initial: &'static [u8; GFX942_MIXED_DURATION_QUALIFICATION_BUFFER_BYTES_V1],
    initial_sha256: [u8; 32],
}

impl AdmittedGfx942MixedDurationQualificationV1 {
    pub(crate) fn authorizes_kfd_request_v1(
        &self,
        request: KfdRuntimeAuthorityRequestV1<'_>,
    ) -> bool {
        let fixture = match request.kernel_name {
            "mixed_short" => &SHORT,
            "mixed_long" => &LONG,
            _ => return false,
        };
        let expected = gfx942_mixed_duration_qualification_explicit_kernarg_v1();
        if request.semantic_launch != KfdRuntimeSemanticLaunchV1::Ordinary
            || request.module_image != fixture.object
            || request.module_sha256 != fixture.object_sha256
            || request.signature != GFX942_MIXED_DURATION_QUALIFICATION_SIGNATURE_V1
            || request.explicit_kernarg != expected
            || request.complete_kernarg_template != expected
            || request.geometry != GFX942_MIXED_DURATION_QUALIFICATION_GEOMETRY_V1
            || request.bindings.len() != 1
            || request.dispatch_abi.len() != 1
            || request.allocations.len() != 1
        {
            return false;
        }
        let binding = request.bindings[0];
        let abi = request.dispatch_abi[0];
        let allocation = &request.allocations[0];
        binding.kernarg_byte_offset == 0
            && binding.region.access == RuntimeAccessV1::ReadWrite
            && binding.region.byte_offset == 0
            && binding.region.byte_len == GFX942_MIXED_DURATION_QUALIFICATION_BUFFER_BYTES_V1 as u64
            && abi.explicit_argument_index == 0
            && abi.name == "data"
            && abi.kernarg_byte_offset == 0
            && abi.pointee_alignment == 1
            && abi.access == ArgumentAccess::ReadWrite
            && allocation.allocation == binding.region.allocation
            && allocation.kind == RuntimeMemoryKindV1::HostVisible
            && allocation.alignment.is_power_of_two()
            && allocation.alignment >= 4
            && allocation.byte_offset == 0
            && allocation.bytes == self.initial
            && allocation.content_sha256 == Some(self.initial_sha256)
    }
}

pub fn admit_gfx942_mixed_duration_qualification_v1() -> Result<
    AdmittedGfx942MixedDurationQualificationV1,
    Gfx942MixedDurationQualificationAdmissionErrorV1,
> {
    use Gfx942MixedDurationQualificationAdmissionErrorV1 as Error;
    if <[u8; 32]>::from(Sha256::digest(POLICY)) != GFX942_MIXED_DURATION_QUALIFICATION_SIGNATURE_V1
    {
        return Err(Error::Identity);
    }
    for fixture in [&SHORT, &LONG] {
        if <[u8; 32]>::from(Sha256::digest(fixture.source)) != fixture.source_sha256
            || <[u8; 32]>::from(Sha256::digest(fixture.object)) != fixture.object_sha256
        {
            return Err(Error::Identity);
        }
        let envelope = validate(fixture.object, AdmittedProfile::Gfx942XnackOffCov6)
            .map_err(|_| Error::Envelope)?;
        let kernel = envelope
            .bind_kernel(fixture.kernel)
            .map_err(|_| Error::KernelClosure)?;
        let resources = kernel.resources();
        let args = kernel.selected_kernel().explicit_arguments();
        if resources.kernarg_segment_size() != 16
            || resources.kernarg_segment_alignment() != 8
            || resources.required_workgroup_size() != Some([64, 1, 1])
            || resources.max_flat_workgroup_size() != 64
            || resources.group_segment_fixed_size() != 0
            || resources.private_segment_fixed_size() != 0
            || resources.wavefront_size() != 64
            || kernel.identity_inputs().object_sha256() != fixture.object_sha256
            || kernel
                .selected_kernel()
                .implicit_argument_offset()
                .is_some()
            || kernel.selected_kernel().implicit_argument_size() != 0
            || args.len() != 2
        {
            return Err(Error::AbiOrEffects);
        }
        if args[0].name() != Some("data")
            || args[0].offset() != 0
            || args[0].size() != 8
            || args[0].value_kind() != ExplicitValueKind::GlobalBuffer
            || args[0].address_space() != Some(ArgumentAddressSpace::Global)
            || args[1].name() != Some("data.len")
            || args[1].offset() != 8
            || args[1].size() != 8
            || args[1].value_kind() != ExplicitValueKind::ByValue
        {
            return Err(Error::AbiOrEffects);
        }
        let kernel = kernel
            .reconcile_dispatch_abi(
                GFX942_MIXED_DURATION_QUALIFICATION_SIGNATURE_V1,
                &[KernelGlobalBufferAbiV1::new(
                    0,
                    "data",
                    0,
                    1,
                    ArgumentAccess::ReadWrite,
                )],
            )
            .map_err(|_| Error::AbiOrEffects)?;
        if kernel.dispatch_actual_access(0) != Some(ArgumentAccess::ReadWrite)
            || kernel.dispatch_pointee_alignment(0) != Some(1)
        {
            return Err(Error::AbiOrEffects);
        }
    }
    let initial = INITIAL;
    if *initial != gfx942_mixed_duration_qualification_initial_v1() {
        return Err(Error::Identity);
    }
    Ok(AdmittedGfx942MixedDurationQualificationV1 {
        initial_sha256: Sha256::digest(initial).into(),
        initial,
    })
}

#[derive(Debug)]
pub struct Gfx942MixedDurationQualificationArgumentsV1 {
    allocation: RuntimeAllocationIdV1,
}
impl Gfx942MixedDurationQualificationArgumentsV1 {
    pub const fn new(allocation: RuntimeAllocationIdV1) -> Self {
        Self { allocation }
    }
}
impl RuntimeArgumentsV1 for Gfx942MixedDurationQualificationArgumentsV1 {
    const SIGNATURE_V1: [u8; 32] = GFX942_MIXED_DURATION_QUALIFICATION_SIGNATURE_V1;
    fn encode_explicit_kernarg_v1(&self) -> Vec<u8> {
        gfx942_mixed_duration_qualification_explicit_kernarg_v1().to_vec()
    }
    fn bindings_v1(&self) -> Vec<RuntimeBindingV1> {
        vec![RuntimeBindingV1 {
            region: RuntimeMemoryRegionV1 {
                allocation: self.allocation,
                access: RuntimeAccessV1::ReadWrite,
                byte_offset: 0,
                byte_len: GFX942_MIXED_DURATION_QUALIFICATION_BUFFER_BYTES_V1 as u64,
            },
            kernarg_byte_offset: 0,
        }]
    }
}

pub fn gfx942_mixed_duration_qualification_bindings_v1(allocation: u64) -> [BackendBindingV1; 1] {
    [BackendBindingV1 {
        region: BackendMemoryRegionV1 {
            allocation,
            access: RuntimeAccessV1::ReadWrite,
            byte_offset: 0,
            byte_len: GFX942_MIXED_DURATION_QUALIFICATION_BUFFER_BYTES_V1 as u64,
        },
        kernarg_byte_offset: 0,
    }]
}

pub fn gfx942_mixed_duration_qualification_explicit_kernarg_v1() -> [u8; 16] {
    let mut bytes = [0; 16];
    bytes[8..]
        .copy_from_slice(&(GFX942_MIXED_DURATION_QUALIFICATION_ELEMENTS_V1 as u64).to_le_bytes());
    bytes
}

pub fn gfx942_mixed_duration_qualification_initial_v1()
-> [u8; GFX942_MIXED_DURATION_QUALIFICATION_BUFFER_BYTES_V1] {
    let mut bytes = [0; GFX942_MIXED_DURATION_QUALIFICATION_BUFFER_BYTES_V1];
    for (index, word) in bytes.chunks_exact_mut(4).enumerate() {
        let value = if (16..80).contains(&index) {
            initial_payload((index - 16) as u32)
        } else {
            0xd15e_a5ed ^ (index as u32).wrapping_mul(0x0101_0101)
        };
        word.copy_from_slice(&value.to_le_bytes());
    }
    bytes
}

fn initial_payload(lane: u32) -> u32 {
    lane.wrapping_mul(0x045d_9f3b) ^ 0xa5a5_5a5a
}

// Compose affine maps modulo 2^32 by repeated squaring, without modular division.
fn affine_power(mut iterations: u32) -> (u32, u32) {
    let (mut scale, mut shift) = (1u32, 0u32);
    let (mut factor, mut offset) = (1_664_525u32, 1_013_904_223u32);
    while iterations != 0 {
        if iterations & 1 != 0 {
            scale = scale.wrapping_mul(factor);
            shift = shift.wrapping_mul(factor).wrapping_add(offset);
        }
        offset = offset.wrapping_mul(factor.wrapping_add(1));
        factor = factor.wrapping_mul(factor);
        iterations >>= 1;
    }
    (scale, shift)
}

#[cfg(test)]
mod tests;
