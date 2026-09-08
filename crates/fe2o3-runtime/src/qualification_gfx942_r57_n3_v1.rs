//! Exact gfx942 DeviceLocal R57 three-binding qualification sequence.
//!
//! This module is intentionally behind `hardware-qualification`. It reuses
//! one pinned, repository-owned vecadd object but supplies an independent
//! policy identity and a two-step authority. It grants no general launch or
//! Worker V3 authority.

use core::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use fe2o3_amdhsa_loader::{AdmittedProfile, validate};
use fe2o3_hsaco::{ArgumentAccess, ArgumentAddressSpace, ExplicitValueKind};
use sha2::{Digest, Sha256};

use crate::{
    KfdRuntimeAuthorityAllocationV1, KfdRuntimeAuthorityRequestV1, RuntimeAccessV1,
    RuntimeAllocationIdV1, RuntimeArgumentsV1, RuntimeBindingV1, RuntimeLaunchGeometryV1,
    RuntimeMemoryKindV1, RuntimeMemoryRegionV1,
};

pub const GFX942_R57_N3_QUALIFICATION_PROFILE_ID_V1: &str =
    "fe2o3.runtime.gfx942-r57-n3-qualification.v1";
pub const GFX942_R57_N3_QUALIFICATION_TARGET_V1: &str = "gfx942:xnack-";
pub const GFX942_R57_N3_QUALIFICATION_KERNEL_V1: &str = "vecadd";
pub const GFX942_R57_N3_QUALIFICATION_ELEMENTS_V1: usize = 65_536;
pub const GFX942_R57_N3_QUALIFICATION_BUFFER_BYTES_V1: usize =
    GFX942_R57_N3_QUALIFICATION_ELEMENTS_V1 * size_of::<f32>();
pub const GFX942_R57_N3_QUALIFICATION_KERNARG_BYTES_V1: usize = 48;
pub const GFX942_R57_N3_QUALIFICATION_BUFFER_ALIGNMENT_V1: u64 = 4;
pub const GFX942_R57_N3_QUALIFICATION_C_INITIAL_BITS_V1: u32 = 0x3e80_0000;
pub const GFX942_R57_N3_QUALIFICATION_D_INITIAL_BITS_V1: u32 = 0x3f00_0000;
pub const GFX942_R57_N3_QUALIFICATION_GEOMETRY_V1: RuntimeLaunchGeometryV1 =
    RuntimeLaunchGeometryV1 {
        grid: [GFX942_R57_N3_QUALIFICATION_ELEMENTS_V1 as u32, 1, 1],
        workgroup: [256, 1, 1],
        dynamic_shared_bytes: 0,
    };

pub const GFX942_R57_N3_QUALIFICATION_SOURCE_SHA256_V1: [u8; 32] = [
    0xb3, 0x41, 0x2c, 0x05, 0x0c, 0xe2, 0x18, 0x2f, 0xeb, 0x66, 0x9d, 0x26, 0x7e, 0x3e, 0x72, 0x08,
    0x40, 0x0c, 0x4d, 0x16, 0xf0, 0x86, 0x5e, 0xfb, 0x7a, 0xea, 0xfd, 0x11, 0x8c, 0x8f, 0x7e, 0x51,
];
pub const GFX942_R57_N3_QUALIFICATION_POLICY_SHA256_V1: [u8; 32] = [
    0x70, 0x85, 0xaf, 0xf9, 0x60, 0x7d, 0xea, 0x2a, 0xad, 0x41, 0xef, 0x21, 0xe0, 0x95, 0x35, 0xf6,
    0x55, 0x95, 0x3c, 0xe1, 0x1c, 0x7a, 0x47, 0x9c, 0xa5, 0x9a, 0xc8, 0x4c, 0xe0, 0x17, 0x35, 0x24,
];
pub const GFX942_R57_N3_QUALIFICATION_SIGNATURE_V1: [u8; 32] =
    GFX942_R57_N3_QUALIFICATION_POLICY_SHA256_V1;
pub const GFX942_R57_N3_QUALIFICATION_HSACO_SHA256_V1: [u8; 32] = [
    0x3a, 0x25, 0xe3, 0x64, 0xdd, 0x1e, 0x19, 0x31, 0xd1, 0xa1, 0x6c, 0x24, 0xb3, 0x7a, 0xa9, 0x98,
    0xdf, 0x2c, 0x6e, 0xf1, 0xcb, 0xcf, 0x0e, 0xc2, 0xaf, 0xb6, 0x37, 0x2c, 0xbc, 0x87, 0x8b, 0xab,
];

const SOURCE_BYTES_V1: &[u8] = include_bytes!("../fixtures/trusted-gfx942-vecadd-v1/vecadd.ll");
const POLICY_BYTES_V1: &[u8] = include_bytes!("../fixtures/trusted-gfx942-r57-n3-v1/policy-v1.txt");
const HSACO_BYTES_V1: &[u8] = include_bytes!("../fixtures/trusted-gfx942-vecadd-v1/vecadd.hsaco");

pub const fn gfx942_r57_n3_qualification_hsaco_v1() -> &'static [u8] {
    HSACO_BYTES_V1
}

pub const fn gfx942_r57_n3_qualification_source_v1() -> &'static [u8] {
    SOURCE_BYTES_V1
}

pub const fn gfx942_r57_n3_qualification_policy_v1() -> &'static [u8] {
    POLICY_BYTES_V1
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Gfx942R57N3QualificationAdmissionErrorV1 {
    Identity,
    Envelope,
    KernelClosure,
    AbiOrEffects,
}

impl fmt::Display for Gfx942R57N3QualificationAdmissionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Identity => {
                formatter.write_str("embedded R57 N3 qualification identity mismatch")
            }
            Self::Envelope => {
                formatter.write_str("embedded R57 N3 qualification envelope rejected")
            }
            Self::KernelClosure => {
                formatter.write_str("embedded R57 N3 qualification kernel closure rejected")
            }
            Self::AbiOrEffects => {
                formatter.write_str("embedded R57 N3 qualification ABI or effect mismatch")
            }
        }
    }
}

impl std::error::Error for Gfx942R57N3QualificationAdmissionErrorV1 {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum QualificationPhaseV1 {
    First,
    Second { a: u64, b: u64, c: u64 },
    Complete,
}

#[derive(Debug)]
struct QualificationAuthorityStateV1 {
    calls: AtomicU64,
    phase: Mutex<QualificationPhaseV1>,
}

/// Read-only observation of the exact qualification authority.
#[derive(Clone, Debug)]
pub struct Gfx942R57N3QualificationAuthorityObservationV1 {
    state: Arc<QualificationAuthorityStateV1>,
}

impl Gfx942R57N3QualificationAuthorityObservationV1 {
    /// Returns the number of final native-authority consultations.
    pub fn authorization_calls_v1(&self) -> u64 {
        self.state.calls.load(Ordering::Acquire)
    }
}

/// Non-cloneable evidence retaining the exact two-launch qualification gate.
#[derive(Debug)]
pub struct AdmittedGfx942R57N3QualificationV1 {
    initial_sha256: [[u8; 32]; 4],
    state: Arc<QualificationAuthorityStateV1>,
    _private: (),
}

impl AdmittedGfx942R57N3QualificationV1 {
    pub const fn hsaco(&self) -> &'static [u8] {
        HSACO_BYTES_V1
    }

    pub const fn kernel_name(&self) -> &'static str {
        GFX942_R57_N3_QUALIFICATION_KERNEL_V1
    }

    pub fn observation_v1(&self) -> Gfx942R57N3QualificationAuthorityObservationV1 {
        Gfx942R57N3QualificationAuthorityObservationV1 {
            state: Arc::clone(&self.state),
        }
    }

    pub fn host_buffers(
        &self,
    ) -> Result<Gfx942R57N3QualificationHostBuffersV1, Gfx942R57N3QualificationFixtureErrorV1> {
        gfx942_r57_n3_qualification_host_buffers_v1()
    }

    pub(crate) fn authorizes_kfd_request_v1(
        &self,
        request: KfdRuntimeAuthorityRequestV1<'_>,
    ) -> bool {
        if self
            .state
            .calls
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
                value.checked_add(1)
            })
            .is_err()
            || request.semantic_launch != crate::KfdRuntimeSemanticLaunchV1::Ordinary
            || !exact_artifact_kernarg_geometry_and_abi_v1(&request)
        {
            return false;
        }
        let Ok(mut phase) = self.state.phase.lock() else {
            return false;
        };
        let Some(ids) = exact_device_local_allocations_v1(&request) else {
            return false;
        };
        match *phase {
            QualificationPhaseV1::First => {
                if !initial_content_matches_v1(&request, ids, &self.initial_sha256[..3]) {
                    return false;
                }
                *phase = QualificationPhaseV1::Second {
                    a: ids[0],
                    b: ids[1],
                    c: ids[2],
                };
                true
            }
            QualificationPhaseV1::Second { a, b, c } => {
                if ids[0] != c
                    || ids[1] != b
                    || ids[2] == a
                    || ids[2] == b
                    || ids[2] == c
                    || !allocation_content_matches_v1(&request, ids[1], self.initial_sha256[1])
                    || !allocation_content_matches_v1(&request, ids[2], self.initial_sha256[3])
                    || request
                        .allocations
                        .iter()
                        .find(|allocation| allocation.allocation == c)
                        .is_none_or(|allocation| allocation.content_sha256.is_some())
                {
                    return false;
                }
                *phase = QualificationPhaseV1::Complete;
                true
            }
            QualificationPhaseV1::Complete => false,
        }
    }
}

/// Re-hashes and admits the independent policy, shared source, and shared object.
pub fn admit_gfx942_r57_n3_qualification_v1()
-> Result<AdmittedGfx942R57N3QualificationV1, Gfx942R57N3QualificationAdmissionErrorV1> {
    if <[u8; 32]>::from(Sha256::digest(SOURCE_BYTES_V1))
        != GFX942_R57_N3_QUALIFICATION_SOURCE_SHA256_V1
        || <[u8; 32]>::from(Sha256::digest(POLICY_BYTES_V1))
            != GFX942_R57_N3_QUALIFICATION_POLICY_SHA256_V1
        || <[u8; 32]>::from(Sha256::digest(HSACO_BYTES_V1))
            != GFX942_R57_N3_QUALIFICATION_HSACO_SHA256_V1
    {
        return Err(Gfx942R57N3QualificationAdmissionErrorV1::Identity);
    }
    let envelope = validate(HSACO_BYTES_V1, AdmittedProfile::Gfx942XnackOffCov6)
        .map_err(|_| Gfx942R57N3QualificationAdmissionErrorV1::Envelope)?;
    let kernel = envelope
        .bind_kernel(GFX942_R57_N3_QUALIFICATION_KERNEL_V1)
        .map_err(|_| Gfx942R57N3QualificationAdmissionErrorV1::KernelClosure)?;
    let resources = kernel.resources();
    let arguments = kernel.selected_kernel().explicit_arguments();
    let expected_access = [
        ArgumentAccess::ReadOnly,
        ArgumentAccess::ReadOnly,
        ArgumentAccess::WriteOnly,
    ];
    let length_names = ["arg0.len", "arg1.len", "arg2.len"];
    let valid = resources.kernarg_segment_size()
        == GFX942_R57_N3_QUALIFICATION_KERNARG_BYTES_V1 as u64
        && resources.kernarg_segment_alignment() == 8
        && resources.required_workgroup_size() == Some([256, 1, 1])
        && resources.max_flat_workgroup_size() == 256
        && resources.group_segment_fixed_size() == 0
        && resources.private_segment_fixed_size() == 0
        && resources.wavefront_size() == 64
        && kernel.identity_inputs().object_sha256() == GFX942_R57_N3_QUALIFICATION_HSACO_SHA256_V1
        && arguments.len() == 6
        && kernel
            .selected_kernel()
            .implicit_argument_offset()
            .is_none()
        && kernel.selected_kernel().implicit_argument_size() == 0
        && GFX942_R57_N3_QUALIFICATION_ARGUMENTS_V1
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
    if !valid {
        return Err(Gfx942R57N3QualificationAdmissionErrorV1::AbiOrEffects);
    }
    let buffers = gfx942_r57_n3_qualification_host_buffers_v1()
        .map_err(|_| Gfx942R57N3QualificationAdmissionErrorV1::AbiOrEffects)?;
    Ok(AdmittedGfx942R57N3QualificationV1 {
        initial_sha256: [
            Sha256::digest(buffers.a()).into(),
            Sha256::digest(buffers.b()).into(),
            Sha256::digest(buffers.c_initial()).into(),
            Sha256::digest(buffers.d_initial()).into(),
        ],
        state: Arc::new(QualificationAuthorityStateV1 {
            calls: AtomicU64::new(0),
            phase: Mutex::new(QualificationPhaseV1::First),
        }),
        _private: (),
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942R57N3QualificationArgumentV1 {
    pub explicit_argument_index: usize,
    pub name: &'static str,
    pub pointer_offset: u32,
    pub length_offset: usize,
    pub access: RuntimeAccessV1,
    pub reconciled_pointee_alignment: u64,
}

pub const GFX942_R57_N3_QUALIFICATION_ARGUMENTS_V1: [Gfx942R57N3QualificationArgumentV1; 3] = [
    Gfx942R57N3QualificationArgumentV1 {
        explicit_argument_index: 0,
        name: "arg0.data",
        pointer_offset: 0,
        length_offset: 8,
        access: RuntimeAccessV1::Read,
        reconciled_pointee_alignment: 1,
    },
    Gfx942R57N3QualificationArgumentV1 {
        explicit_argument_index: 2,
        name: "arg1.data",
        pointer_offset: 16,
        length_offset: 24,
        access: RuntimeAccessV1::Read,
        reconciled_pointee_alignment: 1,
    },
    Gfx942R57N3QualificationArgumentV1 {
        explicit_argument_index: 4,
        name: "arg2.data",
        pointer_offset: 32,
        length_offset: 40,
        access: RuntimeAccessV1::Write,
        reconciled_pointee_alignment: 1,
    },
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Gfx942R57N3QualificationFixtureErrorV1 {
    AliasedAllocations,
    Capacity,
}

impl fmt::Display for Gfx942R57N3QualificationFixtureErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AliasedAllocations => {
                formatter.write_str("R57 N3 qualification requires three distinct allocations")
            }
            Self::Capacity => formatter.write_str("R57 N3 qualification buffer allocation failed"),
        }
    }
}

impl std::error::Error for Gfx942R57N3QualificationFixtureErrorV1 {}

#[derive(Debug)]
pub struct Gfx942R57N3QualificationArgumentsV1 {
    allocations: [RuntimeAllocationIdV1; 3],
}

impl Gfx942R57N3QualificationArgumentsV1 {
    pub fn new(
        left: RuntimeAllocationIdV1,
        right: RuntimeAllocationIdV1,
        output: RuntimeAllocationIdV1,
    ) -> Result<Self, Gfx942R57N3QualificationFixtureErrorV1> {
        if left == right || left == output || right == output {
            return Err(Gfx942R57N3QualificationFixtureErrorV1::AliasedAllocations);
        }
        Ok(Self {
            allocations: [left, right, output],
        })
    }
}

impl RuntimeArgumentsV1 for Gfx942R57N3QualificationArgumentsV1 {
    const SIGNATURE_V1: [u8; 32] = GFX942_R57_N3_QUALIFICATION_SIGNATURE_V1;

    fn encode_explicit_kernarg_v1(&self) -> Vec<u8> {
        gfx942_r57_n3_qualification_explicit_kernarg_v1().to_vec()
    }

    fn bindings_v1(&self) -> Vec<RuntimeBindingV1> {
        GFX942_R57_N3_QUALIFICATION_ARGUMENTS_V1
            .iter()
            .enumerate()
            .map(|(index, policy)| RuntimeBindingV1 {
                region: RuntimeMemoryRegionV1 {
                    allocation: self.allocations[index],
                    access: policy.access,
                    byte_offset: 0,
                    byte_len: GFX942_R57_N3_QUALIFICATION_BUFFER_BYTES_V1 as u64,
                },
                kernarg_byte_offset: policy.pointer_offset,
            })
            .collect()
    }
}

#[derive(Debug)]
pub struct Gfx942R57N3QualificationHostBuffersV1 {
    a: Vec<u8>,
    b: Vec<u8>,
    c_initial: Vec<u8>,
    d_initial: Vec<u8>,
    expected_c: Vec<u8>,
    expected_d: Vec<u8>,
}

impl Gfx942R57N3QualificationHostBuffersV1 {
    pub fn a(&self) -> &[u8] {
        &self.a
    }
    pub fn b(&self) -> &[u8] {
        &self.b
    }
    pub fn c_initial(&self) -> &[u8] {
        &self.c_initial
    }
    pub fn d_initial(&self) -> &[u8] {
        &self.d_initial
    }
    pub fn expected_c(&self) -> &[u8] {
        &self.expected_c
    }
    pub fn expected_d(&self) -> &[u8] {
        &self.expected_d
    }
    pub fn into_parts(self) -> [Vec<u8>; 6] {
        [
            self.a,
            self.b,
            self.c_initial,
            self.d_initial,
            self.expected_c,
            self.expected_d,
        ]
    }
}

pub fn gfx942_r57_n3_qualification_host_buffers_v1()
-> Result<Gfx942R57N3QualificationHostBuffersV1, Gfx942R57N3QualificationFixtureErrorV1> {
    let mut buffers: [Vec<u8>; 6] = core::array::from_fn(|_| Vec::new());
    for buffer in &mut buffers {
        buffer
            .try_reserve_exact(GFX942_R57_N3_QUALIFICATION_BUFFER_BYTES_V1)
            .map_err(|_| Gfx942R57N3QualificationFixtureErrorV1::Capacity)?;
        buffer.resize(GFX942_R57_N3_QUALIFICATION_BUFFER_BYTES_V1, 0);
    }
    for index in 0..GFX942_R57_N3_QUALIFICATION_ELEMENTS_V1 {
        let start = index * size_of::<f32>();
        let end = start + size_of::<f32>();
        let a = ((index & 63) as f32) * 0.25;
        let b = ((index & 31) as f32) * 0.5;
        buffers[0][start..end].copy_from_slice(&a.to_bits().to_le_bytes());
        buffers[1][start..end].copy_from_slice(&b.to_bits().to_le_bytes());
        buffers[2][start..end]
            .copy_from_slice(&GFX942_R57_N3_QUALIFICATION_C_INITIAL_BITS_V1.to_le_bytes());
        buffers[3][start..end]
            .copy_from_slice(&GFX942_R57_N3_QUALIFICATION_D_INITIAL_BITS_V1.to_le_bytes());
        buffers[4][start..end].copy_from_slice(&(a + b).to_bits().to_le_bytes());
        buffers[5][start..end].copy_from_slice(&(a + b + b).to_bits().to_le_bytes());
    }
    let [a, b, c_initial, d_initial, expected_c, expected_d] = buffers;
    Ok(Gfx942R57N3QualificationHostBuffersV1 {
        a,
        b,
        c_initial,
        d_initial,
        expected_c,
        expected_d,
    })
}

pub fn gfx942_r57_n3_qualification_explicit_kernarg_v1()
-> [u8; GFX942_R57_N3_QUALIFICATION_KERNARG_BYTES_V1] {
    let mut bytes = [0; GFX942_R57_N3_QUALIFICATION_KERNARG_BYTES_V1];
    let elements = (GFX942_R57_N3_QUALIFICATION_ELEMENTS_V1 as u64).to_le_bytes();
    for argument in GFX942_R57_N3_QUALIFICATION_ARGUMENTS_V1 {
        bytes[argument.length_offset..argument.length_offset + size_of::<u64>()]
            .copy_from_slice(&elements);
    }
    bytes
}

fn exact_artifact_kernarg_geometry_and_abi_v1(request: &KfdRuntimeAuthorityRequestV1<'_>) -> bool {
    let expected_kernarg = gfx942_r57_n3_qualification_explicit_kernarg_v1();
    request.module_image == HSACO_BYTES_V1
        && request.module_sha256 == GFX942_R57_N3_QUALIFICATION_HSACO_SHA256_V1
        && <[u8; 32]>::from(Sha256::digest(request.module_image))
            == GFX942_R57_N3_QUALIFICATION_HSACO_SHA256_V1
        && request.kernel_name == GFX942_R57_N3_QUALIFICATION_KERNEL_V1
        && request.signature == GFX942_R57_N3_QUALIFICATION_SIGNATURE_V1
        && request.explicit_kernarg == expected_kernarg
        && request.complete_kernarg_template == expected_kernarg
        && request.geometry == GFX942_R57_N3_QUALIFICATION_GEOMETRY_V1
        && request.bindings.len() == 3
        && request.dispatch_abi.len() == 3
        && request.allocations.len() == 3
        && GFX942_R57_N3_QUALIFICATION_ARGUMENTS_V1
            .iter()
            .enumerate()
            .all(|(index, policy)| {
                let binding = request.bindings[index];
                let abi = request.dispatch_abi[index];
                binding.kernarg_byte_offset == policy.pointer_offset
                    && binding.region.access == policy.access
                    && binding.region.byte_offset == 0
                    && binding.region.byte_len == GFX942_R57_N3_QUALIFICATION_BUFFER_BYTES_V1 as u64
                    && abi.explicit_argument_index == policy.explicit_argument_index
                    && abi.name == policy.name
                    && abi.kernarg_byte_offset == u64::from(policy.pointer_offset)
                    && abi.pointee_alignment == policy.reconciled_pointee_alignment
                    && abi.access == argument_access_v1(policy.access)
            })
}

fn exact_device_local_allocations_v1(
    request: &KfdRuntimeAuthorityRequestV1<'_>,
) -> Option<[u64; 3]> {
    let ids = core::array::from_fn(|index| request.bindings[index].region.allocation);
    if ids[0] == ids[1] || ids[0] == ids[2] || ids[1] == ids[2] {
        return None;
    }
    ids.iter()
        .all(|id| {
            request
                .allocations
                .iter()
                .filter(|allocation| allocation.allocation == *id)
                .count()
                == 1
                && request
                    .allocations
                    .iter()
                    .find(|allocation| allocation.allocation == *id)
                    .is_some_and(exact_allocation_shape_v1)
        })
        .then_some(ids)
}

fn exact_allocation_shape_v1(allocation: &KfdRuntimeAuthorityAllocationV1<'_>) -> bool {
    allocation.kind == RuntimeMemoryKindV1::DeviceLocal
        && allocation.alignment.is_power_of_two()
        && allocation.alignment >= GFX942_R57_N3_QUALIFICATION_BUFFER_ALIGNMENT_V1
        && allocation.byte_offset == 0
        && allocation.bytes.len() == GFX942_R57_N3_QUALIFICATION_BUFFER_BYTES_V1
}

fn initial_content_matches_v1(
    request: &KfdRuntimeAuthorityRequestV1<'_>,
    ids: [u64; 3],
    expected: &[[u8; 32]],
) -> bool {
    ids.into_iter()
        .zip(expected.iter().copied())
        .all(|(id, digest)| allocation_content_matches_v1(request, id, digest))
}

fn allocation_content_matches_v1(
    request: &KfdRuntimeAuthorityRequestV1<'_>,
    id: u64,
    digest: [u8; 32],
) -> bool {
    request
        .allocations
        .iter()
        .find(|allocation| allocation.allocation == id)
        .is_some_and(|allocation| {
            allocation.content_sha256 == Some(digest)
                && <[u8; 32]>::from(Sha256::digest(allocation.bytes)) == digest
        })
}

const fn argument_access_v1(access: RuntimeAccessV1) -> ArgumentAccess {
    match access {
        RuntimeAccessV1::Read => ArgumentAccess::ReadOnly,
        RuntimeAccessV1::Write => ArgumentAccess::WriteOnly,
        RuntimeAccessV1::ReadWrite => ArgumentAccess::ReadWrite,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BackendBindingV1, BackendMemoryRegionV1, KfdRuntimeAuthorityGlobalBufferV1};

    #[test]
    fn independent_policy_and_shared_artifact_identities_are_exact() {
        assert_eq!(
            Sha256::digest(SOURCE_BYTES_V1).as_slice(),
            GFX942_R57_N3_QUALIFICATION_SOURCE_SHA256_V1
        );
        assert_eq!(
            Sha256::digest(POLICY_BYTES_V1).as_slice(),
            GFX942_R57_N3_QUALIFICATION_POLICY_SHA256_V1
        );
        assert_eq!(
            Sha256::digest(HSACO_BYTES_V1).as_slice(),
            GFX942_R57_N3_QUALIFICATION_HSACO_SHA256_V1
        );
        assert_ne!(
            GFX942_R57_N3_QUALIFICATION_SIGNATURE_V1,
            crate::qualification_gfx942_vecadd_v1::GFX942_VECADD_QUALIFICATION_SIGNATURE_V1
        );
        assert_ne!(
            POLICY_BYTES_V1,
            crate::qualification_gfx942_vecadd_v1::gfx942_vecadd_qualification_policy_v1()
        );
        admit_gfx942_r57_n3_qualification_v1().unwrap();
    }

    #[test]
    fn deterministic_images_encode_both_exact_equations() {
        let buffers = gfx942_r57_n3_qualification_host_buffers_v1().unwrap();
        for index in [
            0,
            1,
            31,
            32,
            63,
            64,
            GFX942_R57_N3_QUALIFICATION_ELEMENTS_V1 - 1,
        ] {
            let read = |bytes: &[u8]| {
                let start = index * 4;
                f32::from_bits(u32::from_le_bytes(
                    bytes[start..start + 4].try_into().unwrap(),
                ))
            };
            assert_eq!(
                read(buffers.expected_c()),
                read(buffers.a()) + read(buffers.b())
            );
            assert_eq!(
                read(buffers.expected_d()),
                read(buffers.expected_c()) + read(buffers.b())
            );
        }
        assert_ne!(buffers.c_initial(), buffers.expected_c());
        assert_ne!(buffers.d_initial(), buffers.expected_d());
    }

    #[test]
    fn authority_is_exactly_two_phase_and_counts_every_consultation() {
        let admitted = admit_gfx942_r57_n3_qualification_v1().unwrap();
        let observation = admitted.observation_v1();
        let buffers = admitted.host_buffers().unwrap();
        let kernarg = gfx942_r57_n3_qualification_explicit_kernarg_v1();
        let abi = authority_abi_v1();
        let first_bindings = bindings_v1([10, 20, 30]);
        let first_allocations = [
            authority_allocation_v1(10, buffers.a(), Some(Sha256::digest(buffers.a()).into())),
            authority_allocation_v1(20, buffers.b(), Some(Sha256::digest(buffers.b()).into())),
            authority_allocation_v1(
                30,
                buffers.c_initial(),
                Some(Sha256::digest(buffers.c_initial()).into()),
            ),
        ];
        assert!(admitted.authorizes_kfd_request_v1(request_v1(
            &kernarg,
            &abi,
            &first_bindings,
            &first_allocations
        )));
        assert_eq!(observation.authorization_calls_v1(), 1);

        let second_bindings = bindings_v1([30, 20, 40]);
        let second_allocations = [
            authority_allocation_v1(30, buffers.c_initial(), None),
            authority_allocation_v1(20, buffers.b(), Some(Sha256::digest(buffers.b()).into())),
            authority_allocation_v1(
                40,
                buffers.d_initial(),
                Some(Sha256::digest(buffers.d_initial()).into()),
            ),
        ];
        assert!(admitted.authorizes_kfd_request_v1(request_v1(
            &kernarg,
            &abi,
            &second_bindings,
            &second_allocations
        )));
        assert!(!admitted.authorizes_kfd_request_v1(request_v1(
            &kernarg,
            &abi,
            &second_bindings,
            &second_allocations
        )));
        assert_eq!(observation.authorization_calls_v1(), 3);
    }

    #[test]
    fn authority_rejects_second_phase_before_first_and_malformed_first() {
        let buffers = gfx942_r57_n3_qualification_host_buffers_v1().unwrap();
        let kernarg = gfx942_r57_n3_qualification_explicit_kernarg_v1();
        let abi = authority_abi_v1();
        let bindings = bindings_v1([30, 20, 40]);
        let allocations = [
            authority_allocation_v1(30, buffers.c_initial(), None),
            authority_allocation_v1(20, buffers.b(), Some(Sha256::digest(buffers.b()).into())),
            authority_allocation_v1(
                40,
                buffers.d_initial(),
                Some(Sha256::digest(buffers.d_initial()).into()),
            ),
        ];
        let admitted = admit_gfx942_r57_n3_qualification_v1().unwrap();
        assert!(!admitted.authorizes_kfd_request_v1(request_v1(
            &kernarg,
            &abi,
            &bindings,
            &allocations
        )));

        let admitted = admit_gfx942_r57_n3_qualification_v1().unwrap();
        let bindings = bindings_v1([10, 20, 30]);
        let mut allocations = [
            authority_allocation_v1(10, buffers.a(), Some(Sha256::digest(buffers.a()).into())),
            authority_allocation_v1(20, buffers.b(), Some(Sha256::digest(buffers.b()).into())),
            authority_allocation_v1(
                30,
                buffers.c_initial(),
                Some(Sha256::digest(buffers.c_initial()).into()),
            ),
        ];
        allocations[2].kind = RuntimeMemoryKindV1::HostVisible;
        assert!(!admitted.authorizes_kfd_request_v1(request_v1(
            &kernarg,
            &abi,
            &bindings,
            &allocations
        )));
    }

    fn bindings_v1(ids: [u64; 3]) -> [BackendBindingV1; 3] {
        core::array::from_fn(|index| BackendBindingV1 {
            region: BackendMemoryRegionV1 {
                allocation: ids[index],
                access: GFX942_R57_N3_QUALIFICATION_ARGUMENTS_V1[index].access,
                byte_offset: 0,
                byte_len: GFX942_R57_N3_QUALIFICATION_BUFFER_BYTES_V1 as u64,
            },
            kernarg_byte_offset: GFX942_R57_N3_QUALIFICATION_ARGUMENTS_V1[index].pointer_offset,
        })
    }

    fn authority_abi_v1() -> [KfdRuntimeAuthorityGlobalBufferV1<'static>; 3] {
        core::array::from_fn(|index| {
            let policy = GFX942_R57_N3_QUALIFICATION_ARGUMENTS_V1[index];
            KfdRuntimeAuthorityGlobalBufferV1 {
                explicit_argument_index: policy.explicit_argument_index,
                name: policy.name,
                kernarg_byte_offset: u64::from(policy.pointer_offset),
                pointee_alignment: policy.reconciled_pointee_alignment,
                access: argument_access_v1(policy.access),
            }
        })
    }

    fn authority_allocation_v1<'a>(
        allocation: u64,
        bytes: &'a [u8],
        content_sha256: Option<[u8; 32]>,
    ) -> KfdRuntimeAuthorityAllocationV1<'a> {
        KfdRuntimeAuthorityAllocationV1 {
            allocation,
            kind: RuntimeMemoryKindV1::DeviceLocal,
            alignment: GFX942_R57_N3_QUALIFICATION_BUFFER_ALIGNMENT_V1,
            byte_offset: 0,
            bytes,
            content_sha256,
        }
    }

    fn request_v1<'a>(
        kernarg: &'a [u8],
        abi: &'a [KfdRuntimeAuthorityGlobalBufferV1<'a>],
        bindings: &'a [BackendBindingV1],
        allocations: &'a [KfdRuntimeAuthorityAllocationV1<'a>],
    ) -> KfdRuntimeAuthorityRequestV1<'a> {
        KfdRuntimeAuthorityRequestV1 {
            module_image: HSACO_BYTES_V1,
            module_sha256: GFX942_R57_N3_QUALIFICATION_HSACO_SHA256_V1,
            kernel_name: GFX942_R57_N3_QUALIFICATION_KERNEL_V1,
            signature: GFX942_R57_N3_QUALIFICATION_SIGNATURE_V1,
            explicit_kernarg: kernarg,
            complete_kernarg_template: kernarg,
            bindings,
            dispatch_abi: abi,
            allocations,
            geometry: GFX942_R57_N3_QUALIFICATION_GEOMETRY_V1,
            semantic_launch: crate::KfdRuntimeSemanticLaunchV1::Ordinary,
        }
    }
}
