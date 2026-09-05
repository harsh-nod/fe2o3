//! Exact repository-owned one-buffer transform fixture for gfx942 qualification.
//!
//! This module admits one source-authenticated COV6 object, one fixed launch,
//! and one whole DeviceLocal ReadWrite allocation. Result validation compares
//! every output byte against the deterministic expected image.

use core::fmt;

use fe2o3_amdhsa_loader::{AdmittedProfile, KernelGlobalBufferAbiV1, validate};
use fe2o3_hsaco::{ArgumentAccess, ArgumentAddressSpace, ExplicitValueKind};
use sha2::{Digest, Sha256};

use crate::{
    BackendBindingV1, BackendMemoryRegionV1, KfdRuntimeAuthorityRequestV1, RuntimeAccessV1,
    RuntimeAllocationIdV1, RuntimeArgumentsV1, RuntimeBindingV1, RuntimeLaunchGeometryV1,
    RuntimeMemoryKindV1, RuntimeMemoryRegionV1,
};

pub const GFX942_INPLACE_TRANSFORM_QUALIFICATION_PROFILE_ID_V1: &str =
    "fe2o3.runtime.gfx942-inplace-transform-qualification.v1";
pub const GFX942_INPLACE_TRANSFORM_QUALIFICATION_TARGET_V1: &str = "gfx942:xnack-";
pub const GFX942_INPLACE_TRANSFORM_QUALIFICATION_KERNEL_V1: &str = "inplace_transform";
pub const GFX942_INPLACE_TRANSFORM_QUALIFICATION_ELEMENTS_V1: usize = 262_144;
pub const GFX942_INPLACE_TRANSFORM_QUALIFICATION_BUFFER_BYTES_V1: usize =
    GFX942_INPLACE_TRANSFORM_QUALIFICATION_ELEMENTS_V1 * size_of::<u32>();
pub const GFX942_INPLACE_TRANSFORM_QUALIFICATION_KERNARG_BYTES_V1: usize = 16;
pub const GFX942_INPLACE_TRANSFORM_QUALIFICATION_BUFFER_ALIGNMENT_V1: u64 = 4;
pub const GFX942_INPLACE_TRANSFORM_XOR_V1: u32 = 0x9e37_79b9;

pub const GFX942_INPLACE_TRANSFORM_QUALIFICATION_GEOMETRY_V1: RuntimeLaunchGeometryV1 =
    RuntimeLaunchGeometryV1 {
        grid: [
            GFX942_INPLACE_TRANSFORM_QUALIFICATION_ELEMENTS_V1 as u32,
            1,
            1,
        ],
        workgroup: [256, 1, 1],
        dynamic_shared_bytes: 0,
    };

pub const GFX942_INPLACE_TRANSFORM_QUALIFICATION_SOURCE_SHA256_V1: [u8; 32] = [
    0x11, 0x85, 0xd4, 0xcd, 0x93, 0x1c, 0x1b, 0xb4, 0x3d, 0x11, 0x3e, 0x66, 0x71, 0x4a, 0xf3, 0xd9,
    0x8b, 0xd9, 0x6f, 0x7d, 0x03, 0x6f, 0x5c, 0x61, 0x0a, 0x90, 0x9a, 0xbf, 0x34, 0xba, 0x87, 0xd5,
];
pub const GFX942_INPLACE_TRANSFORM_QUALIFICATION_POLICY_SHA256_V1: [u8; 32] = [
    0xc0, 0x60, 0xc3, 0xc4, 0xa9, 0x60, 0x12, 0xfc, 0x66, 0x61, 0xb0, 0x58, 0x5f, 0x4f, 0xf8, 0xff,
    0xe7, 0xb7, 0xf8, 0x48, 0x3e, 0xb4, 0x02, 0x62, 0xe4, 0xa0, 0x18, 0x13, 0x3c, 0x0e, 0xa5, 0x85,
];
pub const GFX942_INPLACE_TRANSFORM_QUALIFICATION_SIGNATURE_V1: [u8; 32] =
    GFX942_INPLACE_TRANSFORM_QUALIFICATION_POLICY_SHA256_V1;
pub const GFX942_INPLACE_TRANSFORM_QUALIFICATION_HSACO_SHA256_V1: [u8; 32] = [
    0x8f, 0xe1, 0x08, 0xf5, 0x07, 0xde, 0xf3, 0x3e, 0x77, 0x17, 0x13, 0x0a, 0x32, 0x8f, 0xf9, 0x05,
    0x80, 0x67, 0x63, 0x0b, 0x4f, 0xc5, 0xee, 0x78, 0x20, 0x03, 0x0c, 0xc0, 0x7a, 0x3d, 0x98, 0xe9,
];
pub const GFX942_INPLACE_TRANSFORM_INPUT_A_SHA256_V1: [u8; 32] = [
    0xce, 0x96, 0xf8, 0xd8, 0x85, 0x72, 0x64, 0x8c, 0x07, 0xa6, 0xc0, 0x3d, 0x7c, 0xe4, 0x9a, 0xf5,
    0x2c, 0x63, 0x7a, 0xf6, 0x52, 0x67, 0x64, 0x5e, 0xaf, 0xdd, 0x21, 0x93, 0xee, 0x6e, 0x49, 0xb7,
];
pub const GFX942_INPLACE_TRANSFORM_OUTPUT_A_SHA256_V1: [u8; 32] = [
    0x4a, 0x42, 0x77, 0x80, 0x46, 0xc6, 0x0e, 0x35, 0x84, 0x9a, 0xd3, 0x5f, 0xe4, 0xdc, 0x4b, 0xf3,
    0x9a, 0x0a, 0x4d, 0x61, 0x6b, 0x75, 0xc9, 0xe6, 0x2d, 0x14, 0x6d, 0xbd, 0xb4, 0x1e, 0xc9, 0x60,
];
pub const GFX942_INPLACE_TRANSFORM_INPUT_B_SHA256_V1: [u8; 32] = [
    0x06, 0x1c, 0xc0, 0x2d, 0x1e, 0x9f, 0x51, 0x33, 0x66, 0xe2, 0x92, 0x54, 0x47, 0x24, 0xef, 0x65,
    0x92, 0xb6, 0xca, 0x4f, 0x59, 0xcf, 0xb2, 0x46, 0x4a, 0x29, 0xbd, 0x94, 0xff, 0x71, 0x23, 0x6e,
];
pub const GFX942_INPLACE_TRANSFORM_OUTPUT_B_SHA256_V1: [u8; 32] = [
    0x49, 0xf9, 0xda, 0x5c, 0x37, 0xcd, 0x05, 0x16, 0x49, 0xcf, 0x25, 0x7f, 0x52, 0x8b, 0x1b, 0x57,
    0x3b, 0x44, 0xa1, 0x93, 0x7b, 0x86, 0x5b, 0x05, 0x64, 0x38, 0x23, 0x26, 0x75, 0x79, 0xcf, 0x62,
];

const SOURCE_BYTES_V1: &[u8] =
    include_bytes!("../fixtures/trusted-gfx942-inplace-transform-v1/inplace_transform.ll");
const POLICY_BYTES_V1: &[u8] =
    include_bytes!("../fixtures/trusted-gfx942-inplace-transform-v1/policy-v1.txt");
const HSACO_BYTES_V1: &[u8] =
    include_bytes!("../fixtures/trusted-gfx942-inplace-transform-v1/inplace_transform.hsaco");

pub const fn gfx942_inplace_transform_qualification_hsaco_v1() -> &'static [u8] {
    HSACO_BYTES_V1
}

pub const fn gfx942_inplace_transform_qualification_source_v1() -> &'static [u8] {
    SOURCE_BYTES_V1
}

pub const fn gfx942_inplace_transform_qualification_policy_v1() -> &'static [u8] {
    POLICY_BYTES_V1
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Gfx942InplaceTransformQualificationAdmissionErrorV1 {
    Identity,
    Envelope,
    KernelClosure,
    AbiOrEffects,
}

impl fmt::Display for Gfx942InplaceTransformQualificationAdmissionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Identity => formatter.write_str("embedded transform identity mismatch"),
            Self::Envelope => formatter.write_str("embedded transform envelope rejected"),
            Self::KernelClosure => formatter.write_str("embedded transform kernel rejected"),
            Self::AbiOrEffects => formatter.write_str("embedded transform ABI or effect mismatch"),
        }
    }
}

impl std::error::Error for Gfx942InplaceTransformQualificationAdmissionErrorV1 {}

#[derive(Debug)]
pub struct AdmittedGfx942InplaceTransformQualificationV1 {
    initial_buffer_sha256: [[u8; 32]; 2],
    expected_buffer_sha256: [[u8; 32]; 2],
    _private: (),
}

impl AdmittedGfx942InplaceTransformQualificationV1 {
    pub const fn hsaco(&self) -> &'static [u8] {
        HSACO_BYTES_V1
    }

    pub const fn hsaco_sha256(&self) -> [u8; 32] {
        GFX942_INPLACE_TRANSFORM_QUALIFICATION_HSACO_SHA256_V1
    }

    pub const fn kernel_name(&self) -> &'static str {
        GFX942_INPLACE_TRANSFORM_QUALIFICATION_KERNEL_V1
    }

    pub const fn signature(&self) -> [u8; 32] {
        GFX942_INPLACE_TRANSFORM_QUALIFICATION_SIGNATURE_V1
    }

    pub const fn geometry(&self) -> RuntimeLaunchGeometryV1 {
        GFX942_INPLACE_TRANSFORM_QUALIFICATION_GEOMETRY_V1
    }

    pub const fn argument(&self) -> Gfx942InplaceTransformQualificationArgumentV1 {
        GFX942_INPLACE_TRANSFORM_QUALIFICATION_ARGUMENT_V1
    }

    pub fn explicit_kernarg(
        &self,
    ) -> [u8; GFX942_INPLACE_TRANSFORM_QUALIFICATION_KERNARG_BYTES_V1] {
        gfx942_inplace_transform_qualification_explicit_kernarg_v1()
    }

    pub fn host_buffers(
        &self,
    ) -> Result<
        Gfx942InplaceTransformQualificationHostBuffersV1,
        Gfx942InplaceTransformQualificationFixtureErrorV1,
    > {
        gfx942_inplace_transform_qualification_host_buffers_v1()
    }

    pub const fn initial_buffer_sha256(
        &self,
        input: Gfx942InplaceTransformQualificationInputV1,
    ) -> [u8; 32] {
        self.initial_buffer_sha256[input.index()]
    }

    pub const fn expected_buffer_sha256(
        &self,
        input: Gfx942InplaceTransformQualificationInputV1,
    ) -> [u8; 32] {
        self.expected_buffer_sha256[input.index()]
    }

    pub fn validate_output_v1(
        &self,
        input: Gfx942InplaceTransformQualificationInputV1,
        observed: &[u8],
    ) -> Result<(), Gfx942InplaceTransformQualificationValidationErrorV1> {
        validate_gfx942_inplace_transform_output_v1(input, observed)
    }

    // The KFD launch-gate integration is owned by the runtime fast-path lane.
    #[allow(dead_code)]
    pub(crate) fn authorizes_kfd_request_v1(
        &self,
        request: KfdRuntimeAuthorityRequestV1<'_>,
    ) -> bool {
        request.semantic_launch == crate::KfdRuntimeSemanticLaunchV1::Ordinary
            && exact_artifact_v1(&request)
            && exact_kernarg_and_geometry_v1(&request)
            && exact_abi_and_allocation_v1(&request, &self.initial_buffer_sha256)
    }
}

pub fn admit_gfx942_inplace_transform_qualification_v1() -> Result<
    AdmittedGfx942InplaceTransformQualificationV1,
    Gfx942InplaceTransformQualificationAdmissionErrorV1,
> {
    if <[u8; 32]>::from(Sha256::digest(SOURCE_BYTES_V1))
        != GFX942_INPLACE_TRANSFORM_QUALIFICATION_SOURCE_SHA256_V1
        || <[u8; 32]>::from(Sha256::digest(POLICY_BYTES_V1))
            != GFX942_INPLACE_TRANSFORM_QUALIFICATION_POLICY_SHA256_V1
        || <[u8; 32]>::from(Sha256::digest(HSACO_BYTES_V1))
            != GFX942_INPLACE_TRANSFORM_QUALIFICATION_HSACO_SHA256_V1
    {
        return Err(Gfx942InplaceTransformQualificationAdmissionErrorV1::Identity);
    }
    let envelope = validate(HSACO_BYTES_V1, AdmittedProfile::Gfx942XnackOffCov6)
        .map_err(|_| Gfx942InplaceTransformQualificationAdmissionErrorV1::Envelope)?;
    let kernel = envelope
        .bind_kernel(GFX942_INPLACE_TRANSFORM_QUALIFICATION_KERNEL_V1)
        .map_err(|_| Gfx942InplaceTransformQualificationAdmissionErrorV1::KernelClosure)?;
    let resources = kernel.resources();
    let resource_match = resources.kernarg_segment_size()
        == GFX942_INPLACE_TRANSFORM_QUALIFICATION_KERNARG_BYTES_V1 as u64
        && resources.kernarg_segment_alignment() == 8
        && resources.required_workgroup_size() == Some([256, 1, 1])
        && resources.max_flat_workgroup_size() == 256
        && resources.group_segment_fixed_size() == 0
        && resources.private_segment_fixed_size() == 0
        && resources.wavefront_size() == 64
        && kernel.identity_inputs().object_sha256()
            == GFX942_INPLACE_TRANSFORM_QUALIFICATION_HSACO_SHA256_V1;
    let arguments = kernel.selected_kernel().explicit_arguments();
    let global = arguments.first();
    let length = arguments.get(1);
    let physical_abi_match = arguments.len() == 2
        && kernel
            .selected_kernel()
            .implicit_argument_offset()
            .is_none()
        && kernel.selected_kernel().implicit_argument_size() == 0
        && global.is_some_and(|argument| {
            argument.name() == Some(GFX942_INPLACE_TRANSFORM_QUALIFICATION_ARGUMENT_V1.name)
                && argument.offset()
                    == u64::from(GFX942_INPLACE_TRANSFORM_QUALIFICATION_ARGUMENT_V1.pointer_offset)
                && argument.size() == 8
                && argument.value_kind() == ExplicitValueKind::GlobalBuffer
                && argument.address_space() == Some(ArgumentAddressSpace::Global)
        })
        && length.is_some_and(|argument| {
            argument.name() == Some("data.len")
                && argument.offset()
                    == GFX942_INPLACE_TRANSFORM_QUALIFICATION_ARGUMENT_V1.length_offset as u64
                && argument.size() == 8
                && argument.value_kind() == ExplicitValueKind::ByValue
        });
    if !resource_match || !physical_abi_match {
        return Err(Gfx942InplaceTransformQualificationAdmissionErrorV1::AbiOrEffects);
    }
    let kernel = kernel
        .reconcile_dispatch_abi(
            GFX942_INPLACE_TRANSFORM_QUALIFICATION_SIGNATURE_V1,
            &[KernelGlobalBufferAbiV1::new(
                GFX942_INPLACE_TRANSFORM_QUALIFICATION_ARGUMENT_V1.explicit_argument_index,
                GFX942_INPLACE_TRANSFORM_QUALIFICATION_ARGUMENT_V1.name,
                u64::from(GFX942_INPLACE_TRANSFORM_QUALIFICATION_ARGUMENT_V1.pointer_offset),
                GFX942_INPLACE_TRANSFORM_QUALIFICATION_ARGUMENT_V1.reconciled_pointee_alignment,
                ArgumentAccess::ReadWrite,
            )],
        )
        .map_err(|_| Gfx942InplaceTransformQualificationAdmissionErrorV1::AbiOrEffects)?;
    if kernel.dispatch_actual_access(0) != Some(ArgumentAccess::ReadWrite)
        || kernel.dispatch_pointee_alignment(0)
            != Some(GFX942_INPLACE_TRANSFORM_QUALIFICATION_ARGUMENT_V1.reconciled_pointee_alignment)
    {
        return Err(Gfx942InplaceTransformQualificationAdmissionErrorV1::AbiOrEffects);
    }
    let buffers = gfx942_inplace_transform_qualification_host_buffers_v1()
        .map_err(|_| Gfx942InplaceTransformQualificationAdmissionErrorV1::AbiOrEffects)?;
    let initial_buffer_sha256 = [
        Sha256::digest(buffers.initial(Gfx942InplaceTransformQualificationInputV1::A)).into(),
        Sha256::digest(buffers.initial(Gfx942InplaceTransformQualificationInputV1::B)).into(),
    ];
    let expected_buffer_sha256 = [
        Sha256::digest(buffers.expected(Gfx942InplaceTransformQualificationInputV1::A)).into(),
        Sha256::digest(buffers.expected(Gfx942InplaceTransformQualificationInputV1::B)).into(),
    ];
    if initial_buffer_sha256
        != [
            GFX942_INPLACE_TRANSFORM_INPUT_A_SHA256_V1,
            GFX942_INPLACE_TRANSFORM_INPUT_B_SHA256_V1,
        ]
        || expected_buffer_sha256
            != [
                GFX942_INPLACE_TRANSFORM_OUTPUT_A_SHA256_V1,
                GFX942_INPLACE_TRANSFORM_OUTPUT_B_SHA256_V1,
            ]
    {
        return Err(Gfx942InplaceTransformQualificationAdmissionErrorV1::AbiOrEffects);
    }
    Ok(AdmittedGfx942InplaceTransformQualificationV1 {
        initial_buffer_sha256,
        expected_buffer_sha256,
        _private: (),
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942InplaceTransformQualificationInputV1 {
    A,
    B,
}

impl Gfx942InplaceTransformQualificationInputV1 {
    pub const fn for_global_iteration(iteration: u64) -> Self {
        if iteration & 1 == 0 { Self::A } else { Self::B }
    }

    const fn index(self) -> usize {
        match self {
            Self::A => 0,
            Self::B => 1,
        }
    }
}

pub const fn gfx942_inplace_transform_initial_value_v1(
    input: Gfx942InplaceTransformQualificationInputV1,
    index: usize,
) -> Option<u32> {
    if index >= GFX942_INPLACE_TRANSFORM_QUALIFICATION_ELEMENTS_V1 {
        return None;
    }
    Some(qualification_initial_v1(input, index))
}

pub const fn gfx942_inplace_transform_value_v1(value: u32, index: u32) -> u32 {
    (value.rotate_left(13) ^ GFX942_INPLACE_TRANSFORM_XOR_V1).wrapping_add(index)
}

pub const fn gfx942_inplace_transform_expected_value_v1(
    input: Gfx942InplaceTransformQualificationInputV1,
    index: usize,
) -> Option<u32> {
    if index >= GFX942_INPLACE_TRANSFORM_QUALIFICATION_ELEMENTS_V1 {
        return None;
    }
    Some(qualification_expected_v1(input, index))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942InplaceTransformQualificationArgumentV1 {
    pub explicit_argument_index: usize,
    pub name: &'static str,
    pub pointer_offset: u32,
    pub length_offset: usize,
    pub access: RuntimeAccessV1,
    pub reconciled_pointee_alignment: u64,
    pub allocation_alignment: u64,
}

pub const GFX942_INPLACE_TRANSFORM_QUALIFICATION_ARGUMENT_V1:
    Gfx942InplaceTransformQualificationArgumentV1 = Gfx942InplaceTransformQualificationArgumentV1 {
    explicit_argument_index: 0,
    name: "data",
    pointer_offset: 0,
    length_offset: 8,
    access: RuntimeAccessV1::ReadWrite,
    reconciled_pointee_alignment: 1,
    allocation_alignment: GFX942_INPLACE_TRANSFORM_QUALIFICATION_BUFFER_ALIGNMENT_V1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Gfx942InplaceTransformQualificationFixtureErrorV1 {
    Capacity,
}

impl fmt::Display for Gfx942InplaceTransformQualificationFixtureErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("qualification transform host-buffer allocation failed")
    }
}

impl std::error::Error for Gfx942InplaceTransformQualificationFixtureErrorV1 {}

#[derive(Debug)]
pub struct Gfx942InplaceTransformQualificationArgumentsV1 {
    allocation: RuntimeAllocationIdV1,
}

impl Gfx942InplaceTransformQualificationArgumentsV1 {
    pub const fn new(allocation: RuntimeAllocationIdV1) -> Self {
        Self { allocation }
    }

    pub const fn allocation(&self) -> RuntimeAllocationIdV1 {
        self.allocation
    }
}

impl RuntimeArgumentsV1 for Gfx942InplaceTransformQualificationArgumentsV1 {
    const SIGNATURE_V1: [u8; 32] = GFX942_INPLACE_TRANSFORM_QUALIFICATION_SIGNATURE_V1;

    fn encode_explicit_kernarg_v1(&self) -> Vec<u8> {
        gfx942_inplace_transform_qualification_explicit_kernarg_v1().to_vec()
    }

    fn bindings_v1(&self) -> Vec<RuntimeBindingV1> {
        vec![RuntimeBindingV1 {
            region: RuntimeMemoryRegionV1 {
                allocation: self.allocation,
                access: RuntimeAccessV1::ReadWrite,
                byte_offset: 0,
                byte_len: GFX942_INPLACE_TRANSFORM_QUALIFICATION_BUFFER_BYTES_V1 as u64,
            },
            kernarg_byte_offset: 0,
        }]
    }
}

#[derive(Debug)]
pub struct Gfx942InplaceTransformQualificationHostBuffersV1 {
    initial: [Vec<u8>; 2],
    expected: [Vec<u8>; 2],
}

impl Gfx942InplaceTransformQualificationHostBuffersV1 {
    pub fn initial(&self, input: Gfx942InplaceTransformQualificationInputV1) -> &[u8] {
        &self.initial[input.index()]
    }

    pub fn expected(&self, input: Gfx942InplaceTransformQualificationInputV1) -> &[u8] {
        &self.expected[input.index()]
    }

    pub fn into_parts(self) -> ([Vec<u8>; 2], [Vec<u8>; 2]) {
        (self.initial, self.expected)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Gfx942InplaceTransformQualificationValidationErrorV1 {
    ByteLength { observed: usize, expected: usize },
    Mismatch { byte_index: usize },
}

impl fmt::Display for Gfx942InplaceTransformQualificationValidationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ByteLength { observed, expected } => write!(
                formatter,
                "transform output byte length {observed} does not equal {expected}"
            ),
            Self::Mismatch { byte_index } => {
                write!(formatter, "transform output mismatch at byte {byte_index}")
            }
        }
    }
}

impl std::error::Error for Gfx942InplaceTransformQualificationValidationErrorV1 {}

pub fn gfx942_inplace_transform_qualification_host_buffers_v1() -> Result<
    Gfx942InplaceTransformQualificationHostBuffersV1,
    Gfx942InplaceTransformQualificationFixtureErrorV1,
> {
    let mut initial = [try_zeroed_buffer_v1()?, try_zeroed_buffer_v1()?];
    let mut expected = [try_zeroed_buffer_v1()?, try_zeroed_buffer_v1()?];
    for index in 0..GFX942_INPLACE_TRANSFORM_QUALIFICATION_ELEMENTS_V1 {
        let start = index * size_of::<u32>();
        let end = start + size_of::<u32>();
        for input in [
            Gfx942InplaceTransformQualificationInputV1::A,
            Gfx942InplaceTransformQualificationInputV1::B,
        ] {
            initial[input.index()][start..end]
                .copy_from_slice(&qualification_initial_v1(input, index).to_le_bytes());
            expected[input.index()][start..end]
                .copy_from_slice(&qualification_expected_v1(input, index).to_le_bytes());
        }
    }
    Ok(Gfx942InplaceTransformQualificationHostBuffersV1 { initial, expected })
}

pub fn validate_gfx942_inplace_transform_output_v1(
    input: Gfx942InplaceTransformQualificationInputV1,
    observed: &[u8],
) -> Result<(), Gfx942InplaceTransformQualificationValidationErrorV1> {
    if observed.len() != GFX942_INPLACE_TRANSFORM_QUALIFICATION_BUFFER_BYTES_V1 {
        return Err(
            Gfx942InplaceTransformQualificationValidationErrorV1::ByteLength {
                observed: observed.len(),
                expected: GFX942_INPLACE_TRANSFORM_QUALIFICATION_BUFFER_BYTES_V1,
            },
        );
    }
    for (index, chunk) in observed.chunks_exact(size_of::<u32>()).enumerate() {
        let expected = qualification_expected_v1(input, index).to_le_bytes();
        if chunk != expected {
            let byte_in_element = chunk
                .iter()
                .zip(expected)
                .position(|(observed, expected)| *observed != expected)
                .expect("unequal u32 bytes have one unequal byte");
            return Err(
                Gfx942InplaceTransformQualificationValidationErrorV1::Mismatch {
                    byte_index: index * size_of::<u32>() + byte_in_element,
                },
            );
        }
    }
    Ok(())
}

pub fn gfx942_inplace_transform_qualification_explicit_kernarg_v1()
-> [u8; GFX942_INPLACE_TRANSFORM_QUALIFICATION_KERNARG_BYTES_V1] {
    let mut bytes = [0; GFX942_INPLACE_TRANSFORM_QUALIFICATION_KERNARG_BYTES_V1];
    bytes[8..16].copy_from_slice(
        &(GFX942_INPLACE_TRANSFORM_QUALIFICATION_ELEMENTS_V1 as u64).to_le_bytes(),
    );
    bytes
}

pub const fn gfx942_inplace_transform_qualification_bindings_v1(
    allocation: u64,
) -> [BackendBindingV1; 1] {
    [BackendBindingV1 {
        region: BackendMemoryRegionV1 {
            allocation,
            access: RuntimeAccessV1::ReadWrite,
            byte_offset: 0,
            byte_len: GFX942_INPLACE_TRANSFORM_QUALIFICATION_BUFFER_BYTES_V1 as u64,
        },
        kernarg_byte_offset: 0,
    }]
}

#[allow(dead_code)]
fn exact_artifact_v1(request: &KfdRuntimeAuthorityRequestV1<'_>) -> bool {
    request.module_image == HSACO_BYTES_V1
        && request.module_sha256 == GFX942_INPLACE_TRANSFORM_QUALIFICATION_HSACO_SHA256_V1
        && <[u8; 32]>::from(Sha256::digest(request.module_image))
            == GFX942_INPLACE_TRANSFORM_QUALIFICATION_HSACO_SHA256_V1
        && request.kernel_name == GFX942_INPLACE_TRANSFORM_QUALIFICATION_KERNEL_V1
        && request.signature == GFX942_INPLACE_TRANSFORM_QUALIFICATION_SIGNATURE_V1
}

#[allow(dead_code)]
fn exact_kernarg_and_geometry_v1(request: &KfdRuntimeAuthorityRequestV1<'_>) -> bool {
    let expected = gfx942_inplace_transform_qualification_explicit_kernarg_v1();
    request.explicit_kernarg == expected
        && request.complete_kernarg_template == expected
        && request.geometry == GFX942_INPLACE_TRANSFORM_QUALIFICATION_GEOMETRY_V1
}

#[allow(dead_code)]
fn exact_abi_and_allocation_v1(
    request: &KfdRuntimeAuthorityRequestV1<'_>,
    initial_buffer_sha256: &[[u8; 32]; 2],
) -> bool {
    if request.bindings.len() != 1
        || request.dispatch_abi.len() != 1
        || request.allocations.len() != 1
    {
        return false;
    }
    let policy = GFX942_INPLACE_TRANSFORM_QUALIFICATION_ARGUMENT_V1;
    let binding = request.bindings[0];
    let abi = request.dispatch_abi[0];
    let allocation = request.allocations[0];
    binding.kernarg_byte_offset == policy.pointer_offset
        && binding.region.access == RuntimeAccessV1::ReadWrite
        && binding.region.byte_offset == 0
        && binding.region.byte_len == GFX942_INPLACE_TRANSFORM_QUALIFICATION_BUFFER_BYTES_V1 as u64
        && abi.explicit_argument_index == 0
        && abi.name == policy.name
        && abi.kernarg_byte_offset == u64::from(policy.pointer_offset)
        && abi.pointee_alignment == policy.reconciled_pointee_alignment
        && abi.access == ArgumentAccess::ReadWrite
        && allocation.allocation == binding.region.allocation
        && allocation.kind == RuntimeMemoryKindV1::DeviceLocal
        && allocation.alignment.is_power_of_two()
        && allocation.alignment >= GFX942_INPLACE_TRANSFORM_QUALIFICATION_BUFFER_ALIGNMENT_V1
        && allocation.byte_offset == 0
        && allocation.bytes.len() == GFX942_INPLACE_TRANSFORM_QUALIFICATION_BUFFER_BYTES_V1
        // `content_sha256` is maintained with the runtime-private shadow: a
        // complete host write sets it, while partial or device writes clear it.
        // The persistent H2D-ready path additionally authenticates the same
        // digest before publication. Rehashing the shadow here would duplicate
        // an O(n) pass without strengthening those custody invariants.
        && initial_buffer_sha256
            .iter()
            .any(|digest| allocation.content_sha256 == Some(*digest))
}

fn try_zeroed_buffer_v1() -> Result<Vec<u8>, Gfx942InplaceTransformQualificationFixtureErrorV1> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(GFX942_INPLACE_TRANSFORM_QUALIFICATION_BUFFER_BYTES_V1)
        .map_err(|_| Gfx942InplaceTransformQualificationFixtureErrorV1::Capacity)?;
    bytes.resize(GFX942_INPLACE_TRANSFORM_QUALIFICATION_BUFFER_BYTES_V1, 0);
    Ok(bytes)
}

const fn qualification_initial_v1(
    input: Gfx942InplaceTransformQualificationInputV1,
    index: usize,
) -> u32 {
    match input {
        Gfx942InplaceTransformQualificationInputV1::A => {
            (index as u32).wrapping_mul(0x045d_9f3b) ^ 0xa5a5_5a5a
        }
        Gfx942InplaceTransformQualificationInputV1::B => {
            (index as u32).wrapping_mul(0x27d4_eb2d) ^ 0x5a5a_a5a5
        }
    }
}

const fn qualification_expected_v1(
    input: Gfx942InplaceTransformQualificationInputV1,
    index: usize,
) -> u32 {
    gfx942_inplace_transform_value_v1(qualification_initial_v1(input, index), index as u32)
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
            GFX942_INPLACE_TRANSFORM_QUALIFICATION_SOURCE_SHA256_V1
        );
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(POLICY_BYTES_V1)),
            GFX942_INPLACE_TRANSFORM_QUALIFICATION_POLICY_SHA256_V1
        );
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(HSACO_BYTES_V1)),
            GFX942_INPLACE_TRANSFORM_QUALIFICATION_HSACO_SHA256_V1
        );
    }

    #[test]
    fn checked_artifact_matches_one_readwrite_buffer_contract() {
        let admitted = admit_gfx942_inplace_transform_qualification_v1().unwrap();
        assert_eq!(admitted.hsaco(), HSACO_BYTES_V1);
        let kernel = validate(HSACO_BYTES_V1, AdmittedProfile::Gfx942XnackOffCov6)
            .unwrap()
            .bind_kernel(GFX942_INPLACE_TRANSFORM_QUALIFICATION_KERNEL_V1)
            .unwrap();
        assert_eq!(kernel.resources().kernarg_segment_size(), 16);
        assert_eq!(
            kernel.resources().required_workgroup_size(),
            Some([256, 1, 1])
        );
        let arguments = kernel.selected_kernel().explicit_arguments();
        assert_eq!(arguments.len(), 2);
        assert_eq!(arguments[0].name(), Some("data"));
        assert_eq!(arguments[0].value_kind(), ExplicitValueKind::GlobalBuffer);
        assert_eq!(arguments[0].access(), None);
        assert_eq!(arguments[0].actual_access(), None);
        assert_eq!(arguments[1].name(), Some("data.len"));
        let reconciled = kernel
            .reconcile_dispatch_abi(
                GFX942_INPLACE_TRANSFORM_QUALIFICATION_SIGNATURE_V1,
                &[KernelGlobalBufferAbiV1::new(
                    0,
                    "data",
                    0,
                    GFX942_INPLACE_TRANSFORM_QUALIFICATION_ARGUMENT_V1.reconciled_pointee_alignment,
                    ArgumentAccess::ReadWrite,
                )],
            )
            .unwrap();
        assert_eq!(
            reconciled.dispatch_actual_access(0),
            Some(ArgumentAccess::ReadWrite)
        );
    }

    #[test]
    fn deterministic_images_and_full_validator_are_exact() {
        let buffers = gfx942_inplace_transform_qualification_host_buffers_v1().unwrap();
        for input in [
            Gfx942InplaceTransformQualificationInputV1::A,
            Gfx942InplaceTransformQualificationInputV1::B,
        ] {
            for index in [
                0,
                1,
                255,
                256,
                GFX942_INPLACE_TRANSFORM_QUALIFICATION_ELEMENTS_V1 - 1,
            ] {
                let start = index * 4;
                assert_eq!(
                    &buffers.initial(input)[start..start + 4],
                    &qualification_initial_v1(input, index).to_le_bytes()
                );
                assert_eq!(
                    &buffers.expected(input)[start..start + 4],
                    &gfx942_inplace_transform_value_v1(
                        qualification_initial_v1(input, index),
                        index as u32,
                    )
                    .to_le_bytes()
                );
            }
            validate_gfx942_inplace_transform_output_v1(input, buffers.expected(input)).unwrap();
            assert!(matches!(
                validate_gfx942_inplace_transform_output_v1(input, &buffers.expected(input)[..4]),
                Err(Gfx942InplaceTransformQualificationValidationErrorV1::ByteLength { .. })
            ));
            for byte_index in [
                0,
                buffers.expected(input).len() / 2,
                buffers.expected(input).len() - 1,
            ] {
                let mut hostile = buffers.expected(input).to_vec();
                hostile[byte_index] ^= 1;
                assert_eq!(
                    validate_gfx942_inplace_transform_output_v1(input, &hostile),
                    Err(
                        Gfx942InplaceTransformQualificationValidationErrorV1::Mismatch {
                            byte_index
                        }
                    )
                );
            }
        }
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(
                buffers.initial(Gfx942InplaceTransformQualificationInputV1::A)
            )),
            GFX942_INPLACE_TRANSFORM_INPUT_A_SHA256_V1
        );
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(
                buffers.expected(Gfx942InplaceTransformQualificationInputV1::A)
            )),
            GFX942_INPLACE_TRANSFORM_OUTPUT_A_SHA256_V1
        );
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(
                buffers.initial(Gfx942InplaceTransformQualificationInputV1::B)
            )),
            GFX942_INPLACE_TRANSFORM_INPUT_B_SHA256_V1
        );
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(
                buffers.expected(Gfx942InplaceTransformQualificationInputV1::B)
            )),
            GFX942_INPLACE_TRANSFORM_OUTPUT_B_SHA256_V1
        );
        assert_eq!(
            Gfx942InplaceTransformQualificationInputV1::for_global_iteration(0),
            Gfx942InplaceTransformQualificationInputV1::A
        );
        assert_eq!(
            Gfx942InplaceTransformQualificationInputV1::for_global_iteration(1),
            Gfx942InplaceTransformQualificationInputV1::B
        );
    }

    #[test]
    fn exact_gate_accepts_only_complete_device_local_invocation() {
        let mut invocation = TestInvocationV1::new();
        assert!(invocation.authorize_v1(HSACO_BYTES_V1));
        let buffers = gfx942_inplace_transform_qualification_host_buffers_v1().unwrap();
        invocation.initial = buffers
            .initial(Gfx942InplaceTransformQualificationInputV1::B)
            .to_vec();
        invocation.content_sha256 = Some(GFX942_INPLACE_TRANSFORM_INPUT_B_SHA256_V1);
        assert!(invocation.authorize_v1(HSACO_BYTES_V1));
        invocation.initial = buffers
            .initial(Gfx942InplaceTransformQualificationInputV1::A)
            .to_vec();
        invocation.content_sha256 = Some(GFX942_INPLACE_TRANSFORM_INPUT_A_SHA256_V1);

        let mut image = HSACO_BYTES_V1.to_vec();
        image[1024] ^= 1;
        assert!(!invocation.authorize_v1(&image));
        invocation.module_sha256[0] ^= 1;
        assert!(!invocation.authorize_v1(HSACO_BYTES_V1));
        invocation.module_sha256 = GFX942_INPLACE_TRANSFORM_QUALIFICATION_HSACO_SHA256_V1;
        invocation.signature[0] ^= 1;
        assert!(!invocation.authorize_v1(HSACO_BYTES_V1));
        invocation.signature = GFX942_INPLACE_TRANSFORM_QUALIFICATION_SIGNATURE_V1;
        invocation.kernarg[8] ^= 1;
        assert!(!invocation.authorize_v1(HSACO_BYTES_V1));
        invocation.kernarg = gfx942_inplace_transform_qualification_explicit_kernarg_v1();
        invocation.geometry.grid[0] -= 256;
        assert!(!invocation.authorize_v1(HSACO_BYTES_V1));
        invocation.geometry = GFX942_INPLACE_TRANSFORM_QUALIFICATION_GEOMETRY_V1;
        invocation.binding.region.access = RuntimeAccessV1::Read;
        assert!(!invocation.authorize_v1(HSACO_BYTES_V1));
        invocation.binding.region.access = RuntimeAccessV1::ReadWrite;
        invocation.kind = RuntimeMemoryKindV1::HostVisible;
        assert!(!invocation.authorize_v1(HSACO_BYTES_V1));
        invocation.kind = RuntimeMemoryKindV1::DeviceLocal;
        invocation.alignment = 2;
        assert!(!invocation.authorize_v1(HSACO_BYTES_V1));
        invocation.alignment = GFX942_INPLACE_TRANSFORM_QUALIFICATION_BUFFER_ALIGNMENT_V1;
        invocation.byte_offset = 4;
        assert!(!invocation.authorize_v1(HSACO_BYTES_V1));
        invocation.byte_offset = 0;
        invocation.initial[17] ^= 1;
        invocation.content_sha256 = None;
        assert!(!invocation.authorize_v1(HSACO_BYTES_V1));
    }

    struct TestInvocationV1 {
        initial: Vec<u8>,
        binding: BackendBindingV1,
        abi: KfdRuntimeAuthorityGlobalBufferV1<'static>,
        kernarg: [u8; GFX942_INPLACE_TRANSFORM_QUALIFICATION_KERNARG_BYTES_V1],
        module_sha256: [u8; 32],
        signature: [u8; 32],
        geometry: RuntimeLaunchGeometryV1,
        kind: RuntimeMemoryKindV1,
        alignment: u64,
        byte_offset: u64,
        content_sha256: Option<[u8; 32]>,
    }

    impl TestInvocationV1 {
        fn new() -> Self {
            let buffers = gfx942_inplace_transform_qualification_host_buffers_v1().unwrap();
            Self {
                initial: buffers
                    .initial(Gfx942InplaceTransformQualificationInputV1::A)
                    .to_vec(),
                binding: gfx942_inplace_transform_qualification_bindings_v1(10)[0],
                abi: KfdRuntimeAuthorityGlobalBufferV1 {
                    explicit_argument_index: 0,
                    name: "data",
                    kernarg_byte_offset: 0,
                    pointee_alignment: 1,
                    access: ArgumentAccess::ReadWrite,
                },
                kernarg: gfx942_inplace_transform_qualification_explicit_kernarg_v1(),
                module_sha256: GFX942_INPLACE_TRANSFORM_QUALIFICATION_HSACO_SHA256_V1,
                signature: GFX942_INPLACE_TRANSFORM_QUALIFICATION_SIGNATURE_V1,
                geometry: GFX942_INPLACE_TRANSFORM_QUALIFICATION_GEOMETRY_V1,
                kind: RuntimeMemoryKindV1::DeviceLocal,
                alignment: GFX942_INPLACE_TRANSFORM_QUALIFICATION_BUFFER_ALIGNMENT_V1,
                byte_offset: 0,
                content_sha256: Some(GFX942_INPLACE_TRANSFORM_INPUT_A_SHA256_V1),
            }
        }

        fn authorize_v1(&self, module_image: &[u8]) -> bool {
            let allocation = KfdRuntimeAuthorityAllocationV1 {
                allocation: 10,
                kind: self.kind,
                alignment: self.alignment,
                byte_offset: self.byte_offset,
                bytes: &self.initial,
                content_sha256: self.content_sha256,
            };
            admit_gfx942_inplace_transform_qualification_v1()
                .unwrap()
                .authorizes_kfd_request_v1(KfdRuntimeAuthorityRequestV1 {
                    module_image,
                    module_sha256: self.module_sha256,
                    kernel_name: GFX942_INPLACE_TRANSFORM_QUALIFICATION_KERNEL_V1,
                    signature: self.signature,
                    explicit_kernarg: &self.kernarg,
                    complete_kernarg_template: &self.kernarg,
                    bindings: core::slice::from_ref(&self.binding),
                    dispatch_abi: core::slice::from_ref(&self.abi),
                    allocations: core::slice::from_ref(&allocation),
                    geometry: self.geometry,
                    semantic_launch: crate::KfdRuntimeSemanticLaunchV1::Ordinary,
                })
        }
    }
}
