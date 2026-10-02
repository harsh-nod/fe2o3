//! Finite, one-shot gfx942 vecadd shards under an independent qualification policy.
//!
//! The unchanged embedded object's semantics are trusted for this explicit
//! hardware-qualification profile, not proved by hashes or metadata. This is
//! neither general production launch authority nor Worker authority. Device
//! binding is provided by installing each indexed gate on its admitted child;
//! final allocation identities and current bytes come from that child's custody.

use core::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use fe2o3_amdhsa_loader::{AdmittedProfile, validate};
use fe2o3_hsaco::{ArgumentAccess, ArgumentAddressSpace, ExplicitValueKind};
use sha2::{Digest, Sha256};

use crate::{
    KfdRuntimeAuthorityRequestV1, KfdRuntimeSemanticLaunchV1, RuntimeAccessV1,
    RuntimeAllocationIdV1, RuntimeArgumentsV1, RuntimeBindingV1, RuntimeLaunchGeometryV1,
    RuntimeMemoryKindV1, RuntimeMemoryRegionV1,
};

pub const GFX942_SHARDED_VECADD_QUALIFICATION_PROFILE_ID_V1: &str =
    "fe2o3.runtime.gfx942-sharded-vecadd-qualification.v1";
pub const GFX942_SHARDED_VECADD_QUALIFICATION_ELEMENTS_V1: usize = 65_537;
pub const GFX942_SHARDED_VECADD_QUALIFICATION_KERNEL_V1: &str = "vecadd";
pub const GFX942_SHARDED_VECADD_QUALIFICATION_BUFFER_ALIGNMENT_V1: u64 = 4;
pub const GFX942_SHARDED_VECADD_QUALIFICATION_KERNARG_BYTES_V1: usize = 48;
pub const GFX942_SHARDED_VECADD_QUALIFICATION_SOURCE_SHA256_V1: [u8; 32] = [
    0xb3, 0x41, 0x2c, 0x05, 0x0c, 0xe2, 0x18, 0x2f, 0xeb, 0x66, 0x9d, 0x26, 0x7e, 0x3e, 0x72, 0x08,
    0x40, 0x0c, 0x4d, 0x16, 0xf0, 0x86, 0x5e, 0xfb, 0x7a, 0xea, 0xfd, 0x11, 0x8c, 0x8f, 0x7e, 0x51,
];
pub const GFX942_SHARDED_VECADD_QUALIFICATION_HSACO_SHA256_V1: [u8; 32] = [
    0x3a, 0x25, 0xe3, 0x64, 0xdd, 0x1e, 0x19, 0x31, 0xd1, 0xa1, 0x6c, 0x24, 0xb3, 0x7a, 0xa9, 0x98,
    0xdf, 0x2c, 0x6e, 0xf1, 0xcb, 0xcf, 0x0e, 0xc2, 0xaf, 0xb6, 0x37, 0x2c, 0xbc, 0x87, 0x8b, 0xab,
];
pub const GFX942_SHARDED_VECADD_QUALIFICATION_POLICY_SHA256_V1: [u8; 32] = [
    0xe1, 0xd3, 0x5c, 0x9a, 0x26, 0x6d, 0x0c, 0x66, 0x34, 0x7d, 0xf1, 0x89, 0x2c, 0xcf, 0x7d, 0xf8,
    0x81, 0x1b, 0x45, 0x73, 0x56, 0xe7, 0xb0, 0x28, 0xfb, 0x40, 0x6d, 0x71, 0xca, 0x06, 0x10, 0x15,
];
pub const GFX942_SHARDED_VECADD_QUALIFICATION_SIGNATURE_V1: [u8; 32] =
    GFX942_SHARDED_VECADD_QUALIFICATION_POLICY_SHA256_V1;

const SOURCE: &[u8] = include_bytes!("../fixtures/trusted-gfx942-vecadd-v1/vecadd.ll");
const OBJECT: &[u8] = include_bytes!("../fixtures/trusted-gfx942-vecadd-v1/vecadd.hsaco");
const POLICY: &[u8] = include_bytes!("../fixtures/trusted-gfx942-sharded-vecadd-v1/policy-v1.txt");
const NAMES: [&str; 3] = ["arg0.data", "arg1.data", "arg2.data"];
const LENGTH_NAMES: [&str; 3] = ["arg0.len", "arg1.len", "arg2.len"];
const ACCESS: [RuntimeAccessV1; 3] = [
    RuntimeAccessV1::Read,
    RuntimeAccessV1::Read,
    RuntimeAccessV1::Write,
];
const ABI_ACCESS: [ArgumentAccess; 3] = [
    ArgumentAccess::ReadOnly,
    ArgumentAccess::ReadOnly,
    ArgumentAccess::WriteOnly,
];

pub const fn gfx942_sharded_vecadd_qualification_source_v1() -> &'static [u8] {
    SOURCE
}

pub const fn gfx942_sharded_vecadd_qualification_hsaco_v1() -> &'static [u8] {
    OBJECT
}

pub const fn gfx942_sharded_vecadd_qualification_policy_v1() -> &'static [u8] {
    POLICY
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Gfx942ShardedVecaddQualificationAdmissionErrorV1 {
    Recipe,
    Identity,
    Envelope,
    KernelClosure,
    AbiOrEffects,
    Capacity,
}

impl fmt::Display for Gfx942ShardedVecaddQualificationAdmissionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Recipe => "sharded vecadd recipe is outside the finite qualification profile",
            Self::Identity => "sharded vecadd source, object or policy identity mismatch",
            Self::Envelope => "sharded vecadd object envelope rejected",
            Self::KernelClosure => "sharded vecadd selected kernel closure rejected",
            Self::AbiOrEffects => "sharded vecadd ABI or effect metadata mismatch",
            Self::Capacity => "sharded vecadd host buffer allocation failed",
        })
    }
}

impl std::error::Error for Gfx942ShardedVecaddQualificationAdmissionErrorV1 {}

/// One of exactly 70 immutable recipes. Logical elements exclude page padding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942ShardedVecaddQualificationRecipeV1 {
    count: usize,
    index: usize,
    round: usize,
    global_offset: usize,
    elements: usize,
    padded_bytes: usize,
    geometry: RuntimeLaunchGeometryV1,
}

impl Gfx942ShardedVecaddQualificationRecipeV1 {
    pub fn new(
        count: usize,
        index: usize,
        round: usize,
    ) -> Result<Self, Gfx942ShardedVecaddQualificationAdmissionErrorV1> {
        use Gfx942ShardedVecaddQualificationAdmissionErrorV1::Recipe;
        if !(2..=8).contains(&count) || index >= count || round > 1 {
            return Err(Recipe);
        }
        let quotient = GFX942_SHARDED_VECADD_QUALIFICATION_ELEMENTS_V1 / count;
        let remainder = GFX942_SHARDED_VECADD_QUALIFICATION_ELEMENTS_V1 % count;
        let elements = quotient
            .checked_add(usize::from(index < remainder))
            .ok_or(Recipe)?;
        let global_offset = index
            .checked_mul(quotient)
            .and_then(|offset| offset.checked_add(index.min(remainder)))
            .ok_or(Recipe)?;
        let padded_bytes = elements
            .checked_mul(size_of::<f32>())
            .and_then(|bytes| bytes.checked_add(4095))
            .map(|bytes| bytes & !4095)
            .ok_or(Recipe)?;
        let grid = elements
            .checked_add(255)
            .map(|elements| elements & !255)
            .ok_or(Recipe)?;
        Ok(Self {
            count,
            index,
            round,
            global_offset,
            elements,
            padded_bytes,
            geometry: RuntimeLaunchGeometryV1 {
                grid: [u32::try_from(grid).map_err(|_| Recipe)?, 1, 1],
                workgroup: [256, 1, 1],
                dynamic_shared_bytes: 0,
            },
        })
    }

    pub const fn count(self) -> usize {
        self.count
    }
    pub const fn index(self) -> usize {
        self.index
    }
    pub const fn round(self) -> usize {
        self.round
    }
    pub const fn global_offset(self) -> usize {
        self.global_offset
    }
    pub const fn elements(self) -> usize {
        self.elements
    }
    pub const fn padded_bytes(self) -> usize {
        self.padded_bytes
    }
    pub const fn geometry(self) -> RuntimeLaunchGeometryV1 {
        self.geometry
    }

    pub fn explicit_kernarg(self) -> [u8; GFX942_SHARDED_VECADD_QUALIFICATION_KERNARG_BYTES_V1] {
        let mut bytes = [0; GFX942_SHARDED_VECADD_QUALIFICATION_KERNARG_BYTES_V1];
        for offset in [8, 24, 40] {
            bytes[offset..offset + 8].copy_from_slice(&(self.elements as u64).to_le_bytes());
        }
        bytes
    }

    pub fn host_buffers(
        self,
    ) -> Result<
        Gfx942ShardedVecaddQualificationHostBuffersV1,
        Gfx942ShardedVecaddQualificationFixtureErrorV1,
    > {
        let mut buffers: [Vec<u8>; 4] = core::array::from_fn(|_| Vec::new());
        for buffer in &mut buffers {
            buffer
                .try_reserve_exact(self.padded_bytes)
                .map_err(|_| Gfx942ShardedVecaddQualificationFixtureErrorV1::Capacity)?;
            buffer.resize(self.padded_bytes, 0);
        }
        for index in 0..self.padded_bytes / size_of::<f32>() {
            let values = if index < self.elements {
                [
                    (self.global_offset + index + 131_072 * self.round) as f32,
                    (3 + self.round) as f32,
                    -1.0,
                    (self.global_offset + index + 3 + 131_073 * self.round) as f32,
                ]
            } else {
                [-7.0, -11.0, -1.0, -1.0]
            };
            for (buffer, value) in buffers.iter_mut().zip(values) {
                buffer[index * 4..index * 4 + 4].copy_from_slice(&value.to_bits().to_le_bytes());
            }
        }
        let [a, b, c_initial, expected_c] = buffers;
        Ok(Gfx942ShardedVecaddQualificationHostBuffersV1 {
            a,
            b,
            c_initial,
            expected_c,
        })
    }
}

#[derive(Debug)]
pub struct Gfx942ShardedVecaddQualificationHostBuffersV1 {
    a: Vec<u8>,
    b: Vec<u8>,
    c_initial: Vec<u8>,
    expected_c: Vec<u8>,
}

impl Gfx942ShardedVecaddQualificationHostBuffersV1 {
    pub fn a(&self) -> &[u8] {
        &self.a
    }
    pub fn b(&self) -> &[u8] {
        &self.b
    }
    pub fn c_initial(&self) -> &[u8] {
        &self.c_initial
    }
    pub fn expected_c(&self) -> &[u8] {
        &self.expected_c
    }
    pub fn into_parts(self) -> [Vec<u8>; 4] {
        [self.a, self.b, self.c_initial, self.expected_c]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Gfx942ShardedVecaddQualificationFixtureErrorV1 {
    AliasedAllocations,
    Capacity,
}

impl fmt::Display for Gfx942ShardedVecaddQualificationFixtureErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::AliasedAllocations => "sharded vecadd requires three distinct allocations",
            Self::Capacity => "sharded vecadd host buffer allocation failed",
        })
    }
}
impl std::error::Error for Gfx942ShardedVecaddQualificationFixtureErrorV1 {}

#[derive(Debug)]
pub struct Gfx942ShardedVecaddQualificationArgumentsV1 {
    recipe: Gfx942ShardedVecaddQualificationRecipeV1,
    allocations: [RuntimeAllocationIdV1; 3],
}

impl Gfx942ShardedVecaddQualificationArgumentsV1 {
    pub fn new(
        recipe: Gfx942ShardedVecaddQualificationRecipeV1,
        left: RuntimeAllocationIdV1,
        right: RuntimeAllocationIdV1,
        output: RuntimeAllocationIdV1,
    ) -> Result<Self, Gfx942ShardedVecaddQualificationFixtureErrorV1> {
        if left == right || left == output || right == output {
            return Err(Gfx942ShardedVecaddQualificationFixtureErrorV1::AliasedAllocations);
        }
        Ok(Self {
            recipe,
            allocations: [left, right, output],
        })
    }
    pub const fn recipe(&self) -> Gfx942ShardedVecaddQualificationRecipeV1 {
        self.recipe
    }
    pub const fn allocations(&self) -> [RuntimeAllocationIdV1; 3] {
        self.allocations
    }
}

impl RuntimeArgumentsV1 for Gfx942ShardedVecaddQualificationArgumentsV1 {
    const SIGNATURE_V1: [u8; 32] = GFX942_SHARDED_VECADD_QUALIFICATION_SIGNATURE_V1;
    fn encode_explicit_kernarg_v1(&self) -> Vec<u8> {
        self.recipe.explicit_kernarg().to_vec()
    }
    fn bindings_v1(&self) -> Vec<RuntimeBindingV1> {
        self.allocations
            .iter()
            .enumerate()
            .map(|(index, allocation)| RuntimeBindingV1 {
                region: RuntimeMemoryRegionV1 {
                    allocation: *allocation,
                    access: ACCESS[index],
                    byte_offset: 0,
                    byte_len: self.recipe.padded_bytes as u64,
                },
                kernarg_byte_offset: (index * 16) as u32,
            })
            .collect()
    }
}

#[derive(Debug)]
struct AuthorityState {
    calls: AtomicU64,
    accepted: AtomicBool,
}

/// Stored authority observations, not completion or GPU execution evidence.
#[derive(Clone, Debug)]
pub struct Gfx942ShardedVecaddQualificationAuthorityObservationV1 {
    state: Arc<AuthorityState>,
}
impl Gfx942ShardedVecaddQualificationAuthorityObservationV1 {
    pub fn authorization_calls_v1(&self) -> u64 {
        self.state.calls.load(Ordering::Acquire)
    }
    pub fn accepted_v1(&self) -> bool {
        self.state.accepted.load(Ordering::Acquire)
    }
}

/// Non-cloneable one-shot admission. Acceptance never resets, including when
/// subsequent publication or completion is ambiguous or unsuccessful.
#[derive(Debug)]
pub struct AdmittedGfx942ShardedVecaddQualificationV1 {
    recipe: Gfx942ShardedVecaddQualificationRecipeV1,
    initial_sha256: [[u8; 32]; 3],
    state: Arc<AuthorityState>,
}

impl AdmittedGfx942ShardedVecaddQualificationV1 {
    pub const fn recipe(&self) -> Gfx942ShardedVecaddQualificationRecipeV1 {
        self.recipe
    }
    pub const fn hsaco(&self) -> &'static [u8] {
        OBJECT
    }
    pub const fn hsaco_sha256(&self) -> [u8; 32] {
        GFX942_SHARDED_VECADD_QUALIFICATION_HSACO_SHA256_V1
    }
    pub const fn kernel_name(&self) -> &'static str {
        GFX942_SHARDED_VECADD_QUALIFICATION_KERNEL_V1
    }
    pub const fn signature(&self) -> [u8; 32] {
        GFX942_SHARDED_VECADD_QUALIFICATION_SIGNATURE_V1
    }
    pub const fn geometry(&self) -> RuntimeLaunchGeometryV1 {
        self.recipe.geometry()
    }
    pub fn explicit_kernarg(&self) -> [u8; GFX942_SHARDED_VECADD_QUALIFICATION_KERNARG_BYTES_V1] {
        self.recipe.explicit_kernarg()
    }
    pub fn host_buffers(
        &self,
    ) -> Result<
        Gfx942ShardedVecaddQualificationHostBuffersV1,
        Gfx942ShardedVecaddQualificationFixtureErrorV1,
    > {
        self.recipe.host_buffers()
    }
    pub fn observation_v1(&self) -> Gfx942ShardedVecaddQualificationAuthorityObservationV1 {
        Gfx942ShardedVecaddQualificationAuthorityObservationV1 {
            state: Arc::clone(&self.state),
        }
    }

    pub(crate) fn authorizes_kfd_request_v1(
        &self,
        request: KfdRuntimeAuthorityRequestV1<'_>,
    ) -> bool {
        if self
            .state
            .calls
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |calls| {
                calls.checked_add(1)
            })
            .is_err()
            || self.state.accepted.load(Ordering::Acquire)
            || request.semantic_launch != KfdRuntimeSemanticLaunchV1::Ordinary
            || !self.exact_request_v1(&request)
        {
            return false;
        }
        self.state
            .accepted
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    fn exact_request_v1(&self, request: &KfdRuntimeAuthorityRequestV1<'_>) -> bool {
        let kernarg = self.explicit_kernarg();
        if request.module_image != OBJECT
            || request.module_sha256 != self.hsaco_sha256()
            || <[u8; 32]>::from(Sha256::digest(request.module_image)) != self.hsaco_sha256()
            || request.kernel_name != self.kernel_name()
            || request.signature != self.signature()
            || request.explicit_kernarg != kernarg
            || request.complete_kernarg_template != kernarg
            || request.geometry != self.geometry()
            || request.bindings.len() != 3
            || request.dispatch_abi.len() != 3
            || request.allocations.len() != 3
        {
            return false;
        }
        let ids: [u64; 3] = core::array::from_fn(|index| request.bindings[index].region.allocation);
        if ids.contains(&0) || ids[0] == ids[1] || ids[0] == ids[2] || ids[1] == ids[2] {
            return false;
        }
        (0..3).all(|index| {
            let binding = request.bindings[index];
            let abi = request.dispatch_abi[index];
            binding.kernarg_byte_offset == (index * 16) as u32
                && binding.region.access == ACCESS[index]
                && binding.region.byte_offset == 0
                && binding.region.byte_len == self.recipe.padded_bytes as u64
                && abi.explicit_argument_index == index * 2
                && abi.name == NAMES[index]
                && abi.kernarg_byte_offset == (index * 16) as u64
                && abi.pointee_alignment == 1
                && abi.access == ABI_ACCESS[index]
                && request
                    .allocations
                    .iter()
                    .filter(|allocation| allocation.allocation == ids[index])
                    .count()
                    == 1
                && request
                    .allocations
                    .iter()
                    .find(|allocation| allocation.allocation == ids[index])
                    .is_some_and(|allocation| {
                        allocation.kind == RuntimeMemoryKindV1::DeviceLocal
                            && allocation.alignment.is_power_of_two()
                            && allocation.alignment
                                >= GFX942_SHARDED_VECADD_QUALIFICATION_BUFFER_ALIGNMENT_V1
                            && allocation.byte_offset == 0
                            && allocation.bytes.len() == self.recipe.padded_bytes
                            && allocation.content_sha256 == Some(self.initial_sha256[index])
                            && <[u8; 32]>::from(Sha256::digest(allocation.bytes))
                                == self.initial_sha256[index]
                    })
        })
    }
}

/// Independently rechecks source, policy, object, envelope and selected ABI,
/// then constructs exactly one of the finite recipe authorities.
pub fn admit_gfx942_sharded_vecadd_qualification_v1(
    count: usize,
    index: usize,
    round: usize,
) -> Result<
    AdmittedGfx942ShardedVecaddQualificationV1,
    Gfx942ShardedVecaddQualificationAdmissionErrorV1,
> {
    let recipe = Gfx942ShardedVecaddQualificationRecipeV1::new(count, index, round)?;
    validate_artifacts_v1(SOURCE, POLICY, OBJECT)?;
    let buffers = recipe
        .host_buffers()
        .map_err(|_| Gfx942ShardedVecaddQualificationAdmissionErrorV1::Capacity)?;
    Ok(AdmittedGfx942ShardedVecaddQualificationV1 {
        recipe,
        initial_sha256: [buffers.a(), buffers.b(), buffers.c_initial()]
            .map(|bytes| Sha256::digest(bytes).into()),
        state: Arc::new(AuthorityState {
            calls: AtomicU64::new(0),
            accepted: AtomicBool::new(false),
        }),
    })
}

fn validate_artifacts_v1(
    source: &[u8],
    policy: &[u8],
    object: &[u8],
) -> Result<(), Gfx942ShardedVecaddQualificationAdmissionErrorV1> {
    use Gfx942ShardedVecaddQualificationAdmissionErrorV1 as E;
    if <[u8; 32]>::from(Sha256::digest(source))
        != GFX942_SHARDED_VECADD_QUALIFICATION_SOURCE_SHA256_V1
        || <[u8; 32]>::from(Sha256::digest(policy))
            != GFX942_SHARDED_VECADD_QUALIFICATION_POLICY_SHA256_V1
        || <[u8; 32]>::from(Sha256::digest(object))
            != GFX942_SHARDED_VECADD_QUALIFICATION_HSACO_SHA256_V1
    {
        return Err(E::Identity);
    }
    let envelope =
        validate(object, AdmittedProfile::Gfx942XnackOffCov6).map_err(|_| E::Envelope)?;
    let kernel = envelope
        .bind_kernel(GFX942_SHARDED_VECADD_QUALIFICATION_KERNEL_V1)
        .map_err(|_| E::KernelClosure)?;
    let resources = kernel.resources();
    let arguments = kernel.selected_kernel().explicit_arguments();
    if resources.kernarg_segment_size()
        != GFX942_SHARDED_VECADD_QUALIFICATION_KERNARG_BYTES_V1 as u64
        || resources.kernarg_segment_alignment() != 8
        || resources.required_workgroup_size() != Some([256, 1, 1])
        || resources.max_flat_workgroup_size() != 256
        || resources.group_segment_fixed_size() != 0
        || resources.private_segment_fixed_size() != 0
        || resources.wavefront_size() != 64
        || kernel.identity_inputs().object_sha256()
            != GFX942_SHARDED_VECADD_QUALIFICATION_HSACO_SHA256_V1
        || arguments.len() != 6
        || kernel
            .selected_kernel()
            .implicit_argument_offset()
            .is_some()
        || kernel.selected_kernel().implicit_argument_size() != 0
        || !(0..3).all(|index| {
            let global = &arguments[index * 2];
            let length = &arguments[index * 2 + 1];
            global.name() == Some(NAMES[index])
                && global.offset() == (index * 16) as u64
                && global.size() == 8
                && global.value_kind() == ExplicitValueKind::GlobalBuffer
                && global.address_space() == Some(ArgumentAddressSpace::Global)
                && global.actual_access() == Some(ABI_ACCESS[index])
                && length.name() == Some(LENGTH_NAMES[index])
                && length.offset() == (index * 16 + 8) as u64
                && length.size() == 8
                && length.value_kind() == ExplicitValueKind::ByValue
        })
    {
        return Err(E::AbiOrEffects);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
