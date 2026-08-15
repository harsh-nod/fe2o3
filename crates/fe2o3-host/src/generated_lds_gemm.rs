//! Exact, inert host preparation for the enabled LDS GEMM Slice1 profile.
//!
//! This module validates a registry contract and concrete device regions. It
//! does not authenticate an executable, load a code object, initialize COV6
//! hidden arguments, or authorize/submit a launch.

use crate::ObservedContext;
use fe2o3_core::{DeviceBufferRegion, DeviceBufferView, DeviceBufferViewMut, DeviceCopy};
use fe2o3_hsaco_finalize::{
    ExactLdsGemmBufferContractV1, ExactLdsGemmBufferRoleV1, ExactLdsGemmContractV1,
    ExactLdsGemmElementV1, ExactLdsGemmLengthIdentityV1, ExactLdsGemmProfileIdV1,
    ExactLdsGemmProfileIdentityV1, InspectedExactLdsGemmCompilerImportIdentityV1,
    InspectedExactLdsGemmCompilerImportV1,
};
use fe2o3_kernel_descriptor::{AccessMode, AliasSemantics, CodeObjectVersion, OwnershipSemantics};
use sha2::{Digest, Sha256};
use std::{
    error::Error,
    fmt,
    mem::{align_of, size_of},
};

const SLICE1_TARGET: &str = "gfx942:xnack-";
const SLICE1_ELEMENTS: usize = 256;
const SLICE1_EXPLICIT_KERNARG_BYTES: usize = 48;
const SLICE1_COMPLETE_KERNARG_BYTES: u32 = 304;
const SLICE1_KERNARG_ALIGNMENT: u32 = 8;
const SLICE1_GRID: [u32; 3] = [1, 1, 1];
const SLICE1_WORKGROUP: [u32; 3] = [64, 1, 1];
const SLICE1_WAVEFRONT_SIZE: u32 = 64;
const SLICE1_STATIC_LDS_BYTES: u32 = 1_024;
const SLICE1_LDS_ALLOCATIONS: u32 = 2;
const SLICE1_LDS_BYTES_PER_ALLOCATION: u32 = 512;
const SLICE1_LDS_ALIGNMENT: u32 = 16;
const LENGTH_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/EXACT-LDS-GEMM-LENGTH/V1\0";

#[repr(C, align(8))]
struct Slice1ExplicitKernargV1 {
    bytes: [u8; SLICE1_EXPLICIT_KERNARG_BYTES],
}

const _: () = assert!(size_of::<Slice1ExplicitKernargV1>() == SLICE1_EXPLICIT_KERNARG_BYTES);
const _: () = assert!(align_of::<Slice1ExplicitKernargV1>() == SLICE1_KERNARG_ALIGNMENT as usize);

/// Typed copy of the compiler descriptor identity needed by the #100 join.
///
/// `fe2o3-host` deliberately does not depend directly on the compiler-FFI
/// crate. This wrapper preserves the descriptor identity's complete typed
/// hash-and-length value without widening that dependency boundary.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct GeneratedLdsGemmDescriptorSourceIdentityV1 {
    sha256: [u8; 32],
    byte_len: u64,
}

#[allow(dead_code)]
impl GeneratedLdsGemmDescriptorSourceIdentityV1 {
    pub(crate) const fn sha256_v1(&self) -> &[u8; 32] {
        &self.sha256
    }

    pub(crate) const fn byte_len_v1(self) -> u64 {
        self.byte_len
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CopiedCompilerImportIdentitiesV1 {
    compiler_import_identity: InspectedExactLdsGemmCompilerImportIdentityV1,
    profile_identity: ExactLdsGemmProfileIdentityV1,
    contract: ExactLdsGemmContractV1,
    descriptor_source_identity: GeneratedLdsGemmDescriptorSourceIdentityV1,
    length_identities: [ExactLdsGemmLengthIdentityV1; 3],
}

impl CopiedCompilerImportIdentitiesV1 {
    fn from_compiler_import(compiler_import: &InspectedExactLdsGemmCompilerImportV1) -> Self {
        let contract = compiler_import.contract();
        let descriptor_source_identity = compiler_import.descriptor_source().identity();
        let [a, b, c] = contract.buffers();
        Self {
            compiler_import_identity: compiler_import.identity(),
            profile_identity: contract.identity(),
            contract,
            descriptor_source_identity: GeneratedLdsGemmDescriptorSourceIdentityV1 {
                sha256: *descriptor_source_identity.sha256(),
                byte_len: descriptor_source_identity.byte_len(),
            },
            length_identities: [
                a.length_identity(),
                b.length_identity(),
                c.length_identity(),
            ],
        }
    }
}

/// Inert, exact host preparation for one LDS GEMM Slice1 invocation.
///
/// The value copies typed identities from a borrowed sealed compiler import and
/// owns all three device views, preserving the two shared input borrows and
/// exclusive mutable output borrow until it is dropped. It is deliberately not
/// `Clone`, exposes no raw device address or kernarg bytes to applications, and
/// has no load or dispatch operation. It does not retain the compiler import,
/// allowing the finalizer to consume that non-Clone value after preparation.
///
/// ```compile_fail
/// use fe2o3_host::GeneratedLdsGemmSlice1HostAdapterV1;
///
/// fn replay(value: GeneratedLdsGemmSlice1HostAdapterV1<'_, '_, '_>) {
///     let _second = value.clone();
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_host::GeneratedLdsGemmSlice1HostAdapterV1;
///
/// fn bypass(value: &GeneratedLdsGemmSlice1HostAdapterV1<'_, '_, '_>) {
///     let _raw = value.explicit_kernarg_bytes_v1();
/// }
/// ```
#[must_use = "an exact LDS GEMM host preparation is inert until a protected runtime consumes it"]
#[allow(dead_code)]
pub struct GeneratedLdsGemmSlice1HostAdapterV1<'a, 'b, 'c> {
    compiler_import_identities: CopiedCompilerImportIdentitiesV1,
    observed: ObservedContext,
    explicit_kernarg: Slice1ExplicitKernargV1,
    _a: DeviceBufferView<'a, u16>,
    _b: DeviceBufferView<'b, u16>,
    _c: DeviceBufferViewMut<'c, f32>,
}

impl fmt::Debug for GeneratedLdsGemmSlice1HostAdapterV1<'_, '_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedLdsGemmSlice1HostAdapterV1")
            .field("target", &SLICE1_TARGET)
            .field("profile", &ExactLdsGemmProfileIdV1::Slice1M16N16K16)
            .field(
                "profile_identity",
                &self.compiler_import_identities.profile_identity,
            )
            .field("grid", &SLICE1_GRID)
            .field("workgroup", &SLICE1_WORKGROUP)
            .field("static_lds_bytes", &SLICE1_STATIC_LDS_BYTES)
            .finish_non_exhaustive()
    }
}

impl<'a, 'b, 'c> GeneratedLdsGemmSlice1HostAdapterV1<'a, 'b, 'c> {
    /// Temporarily borrows the sealed compiler import, copies its exact Slice1
    /// identities, and validates and retains the concrete regions.
    ///
    /// The typed identities are retained for the later #97/#99 trust join. No
    /// caller-supplied identity can be substituted here, and the returned value
    /// does not borrow `compiler_import`.
    pub fn prepare(
        observed: &ObservedContext,
        compiler_import: &InspectedExactLdsGemmCompilerImportV1,
        a: DeviceBufferView<'a, u16>,
        b: DeviceBufferView<'b, u16>,
        c: DeviceBufferViewMut<'c, f32>,
    ) -> Result<Self, GeneratedLdsGemmSlice1HostAdapterErrorV1> {
        validate_observed_target(observed.device().target())?;
        for (role, matches) in [
            (
                ExactLdsGemmBufferRoleV1::A,
                observed.is_for_context(a.context()),
            ),
            (
                ExactLdsGemmBufferRoleV1::B,
                observed.is_for_context(b.context()),
            ),
            (
                ExactLdsGemmBufferRoleV1::C,
                observed.is_for_context(c.context()),
            ),
        ] {
            if !matches {
                return Err(GeneratedLdsGemmSlice1HostAdapterErrorV1::WrongContext { role });
            }
        }

        let compiler_import_identities =
            CopiedCompilerImportIdentitiesV1::from_compiler_import(compiler_import);
        let contract = compiler_import_identities.contract;
        validate_contract(ContractFacts::from_contract(contract))?;
        let prepared = prepare_regions(
            RegionFacts::from_region(&a),
            RegionFacts::from_region(&b),
            RegionFacts::from_region(&c),
        )?;

        Ok(Self {
            compiler_import_identities,
            observed: observed.clone(),
            explicit_kernarg: prepared.explicit_kernarg,
            _a: a,
            _b: b,
            _c: c,
        })
    }

    pub const fn target(&self) -> &'static str {
        SLICE1_TARGET
    }

    pub const fn profile(&self) -> ExactLdsGemmProfileIdV1 {
        ExactLdsGemmProfileIdV1::Slice1M16N16K16
    }

    pub const fn profile_identity(&self) -> ExactLdsGemmProfileIdentityV1 {
        self.compiler_import_identities.profile_identity
    }

    /// Exact sealed-import identity retained by this host preparation.
    ///
    /// This typed value is descriptive and grants no compiler, finalizer, load,
    /// or launch authority.
    pub const fn compiler_import_identity(&self) -> InspectedExactLdsGemmCompilerImportIdentityV1 {
        self.compiler_import_identities.compiler_import_identity
    }

    pub const fn grid(&self) -> [u32; 3] {
        SLICE1_GRID
    }

    pub const fn workgroup(&self) -> [u32; 3] {
        SLICE1_WORKGROUP
    }

    pub const fn static_lds_bytes(&self) -> u32 {
        SLICE1_STATIC_LDS_BYTES
    }

    pub const fn dynamic_lds_bytes(&self) -> u32 {
        0
    }

    pub const fn explicit_kernarg_byte_len(&self) -> usize {
        SLICE1_EXPLICIT_KERNARG_BYTES
    }

    pub const fn complete_kernarg_byte_len(&self) -> u32 {
        SLICE1_COMPLETE_KERNARG_BYTES
    }

    pub const fn kernarg_alignment(&self) -> u32 {
        SLICE1_KERNARG_ALIGNMENT
    }

    /// Crate-private handoff for the protected runtime join. These bytes are
    /// inert and remain borrowed from this non-Clone request.
    #[allow(dead_code)]
    pub(crate) const fn explicit_kernarg_bytes_v1(&self) -> &[u8; SLICE1_EXPLICIT_KERNARG_BYTES] {
        &self.explicit_kernarg.bytes
    }

    #[allow(dead_code)]
    pub(crate) const fn contract_v1(&self) -> ExactLdsGemmContractV1 {
        self.compiler_import_identities.contract
    }

    /// Typed #99 side of the protected #97/#99 runtime join.
    #[allow(dead_code)]
    pub(crate) const fn compiler_import_identity_v1(
        &self,
    ) -> InspectedExactLdsGemmCompilerImportIdentityV1 {
        self.compiler_import_identities.compiler_import_identity
    }

    #[allow(dead_code)]
    pub(crate) const fn descriptor_source_identity_v1(
        &self,
    ) -> GeneratedLdsGemmDescriptorSourceIdentityV1 {
        self.compiler_import_identities.descriptor_source_identity
    }

    #[allow(dead_code)]
    pub(crate) const fn length_identities_v1(&self) -> [ExactLdsGemmLengthIdentityV1; 3] {
        self.compiler_import_identities.length_identities
    }

    #[allow(dead_code)]
    pub(crate) const fn observed_context_v1(&self) -> &ObservedContext {
        &self.observed
    }

    pub const fn authenticates_artifact(&self) -> bool {
        false
    }

    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }

    pub const fn grants_link_authority(&self) -> bool {
        false
    }

    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    pub const fn proves_verus_verification(&self) -> bool {
        false
    }
}

fn validate_observed_target(target: &str) -> Result<(), GeneratedLdsGemmSlice1HostAdapterErrorV1> {
    if target != SLICE1_TARGET {
        return Err(GeneratedLdsGemmSlice1HostAdapterErrorV1::ObservedTargetMismatch);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BufferContractFacts {
    role: ExactLdsGemmBufferRoleV1,
    element: ExactLdsGemmElementV1,
    elements: u64,
    bytes: u64,
    length_identity: [u8; 32],
    ownership: OwnershipSemantics,
    access: AccessMode,
    alias: AliasSemantics,
}

impl BufferContractFacts {
    fn from_contract(contract: ExactLdsGemmBufferContractV1) -> Self {
        Self {
            role: contract.role(),
            element: contract.element(),
            elements: contract.elements(),
            bytes: contract.bytes(),
            length_identity: *contract.length_identity().as_bytes(),
            ownership: contract.ownership(),
            access: contract.access(),
            alias: contract.alias(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ContractFacts {
    profile: ExactLdsGemmProfileIdV1,
    target: &'static str,
    code_object_version: CodeObjectVersion,
    grid: [u32; 3],
    workgroup: [u32; 3],
    wavefront_size: u32,
    explicit_kernarg_bytes: u32,
    complete_kernarg_bytes: u32,
    kernarg_alignment: u32,
    static_lds_bytes: u32,
    lds_allocations: u32,
    lds_bytes_per_allocation: u32,
    lds_alignment: u32,
    buffers: [BufferContractFacts; 3],
}

impl ContractFacts {
    fn from_contract(contract: ExactLdsGemmContractV1) -> Self {
        Self {
            profile: contract.profile(),
            target: contract.target(),
            code_object_version: contract.code_object_version(),
            grid: contract.grid(),
            workgroup: contract.workgroup(),
            wavefront_size: contract.wavefront_size(),
            explicit_kernarg_bytes: contract.explicit_kernarg_bytes(),
            complete_kernarg_bytes: contract.complete_kernarg_bytes(),
            kernarg_alignment: contract.kernarg_alignment(),
            static_lds_bytes: contract.static_lds_bytes(),
            lds_allocations: contract.lds_allocations(),
            lds_bytes_per_allocation: contract.lds_bytes_per_allocation(),
            lds_alignment: contract.lds_alignment(),
            buffers: contract.buffers().map(BufferContractFacts::from_contract),
        }
    }
}

fn validate_contract(facts: ContractFacts) -> Result<(), GeneratedLdsGemmSlice1HostAdapterErrorV1> {
    for (matches, field) in [
        (
            facts.profile == ExactLdsGemmProfileIdV1::Slice1M16N16K16,
            "profile",
        ),
        (facts.target == SLICE1_TARGET, "target"),
        (
            facts.code_object_version == CodeObjectVersion::V6,
            "code-object version",
        ),
        (facts.grid == SLICE1_GRID, "grid"),
        (facts.workgroup == SLICE1_WORKGROUP, "workgroup"),
        (
            facts.wavefront_size == SLICE1_WAVEFRONT_SIZE,
            "wavefront size",
        ),
        (
            facts.explicit_kernarg_bytes == SLICE1_EXPLICIT_KERNARG_BYTES as u32,
            "explicit kernarg bytes",
        ),
        (
            facts.complete_kernarg_bytes == SLICE1_COMPLETE_KERNARG_BYTES,
            "complete kernarg bytes",
        ),
        (
            facts.kernarg_alignment == SLICE1_KERNARG_ALIGNMENT,
            "kernarg alignment",
        ),
        (
            facts.static_lds_bytes == SLICE1_STATIC_LDS_BYTES,
            "static LDS bytes",
        ),
        (
            facts.lds_allocations == SLICE1_LDS_ALLOCATIONS,
            "LDS allocation count",
        ),
        (
            facts.lds_bytes_per_allocation == SLICE1_LDS_BYTES_PER_ALLOCATION,
            "LDS bytes per allocation",
        ),
        (facts.lds_alignment == SLICE1_LDS_ALIGNMENT, "LDS alignment"),
    ] {
        if !matches {
            return Err(GeneratedLdsGemmSlice1HostAdapterErrorV1::ContractField { field });
        }
    }

    let expected = [
        BufferContractFacts {
            role: ExactLdsGemmBufferRoleV1::A,
            element: ExactLdsGemmElementV1::Bf16BitsU16,
            elements: 256,
            bytes: 512,
            length_identity: length_identity(
                ExactLdsGemmBufferRoleV1::A,
                ExactLdsGemmElementV1::Bf16BitsU16,
                256,
                512,
            ),
            ownership: OwnershipSemantics::SharedBorrow,
            access: AccessMode::ReadOnly,
            alias: AliasSemantics::SharedReadOnly,
        },
        BufferContractFacts {
            role: ExactLdsGemmBufferRoleV1::B,
            element: ExactLdsGemmElementV1::Bf16BitsU16,
            elements: 256,
            bytes: 512,
            length_identity: length_identity(
                ExactLdsGemmBufferRoleV1::B,
                ExactLdsGemmElementV1::Bf16BitsU16,
                256,
                512,
            ),
            ownership: OwnershipSemantics::SharedBorrow,
            access: AccessMode::ReadOnly,
            alias: AliasSemantics::SharedReadOnly,
        },
        BufferContractFacts {
            role: ExactLdsGemmBufferRoleV1::C,
            element: ExactLdsGemmElementV1::F32,
            elements: 256,
            bytes: 1_024,
            length_identity: length_identity(
                ExactLdsGemmBufferRoleV1::C,
                ExactLdsGemmElementV1::F32,
                256,
                1_024,
            ),
            ownership: OwnershipSemantics::UniqueBorrow,
            access: AccessMode::ReadWrite,
            alias: AliasSemantics::Exclusive,
        },
    ];
    for (index, (actual, expected)) in facts.buffers.into_iter().zip(expected).enumerate() {
        if actual != expected {
            return Err(GeneratedLdsGemmSlice1HostAdapterErrorV1::BufferContract { index });
        }
    }
    Ok(())
}

fn length_identity(
    role: ExactLdsGemmBufferRoleV1,
    element: ExactLdsGemmElementV1,
    elements: u64,
    bytes: u64,
) -> [u8; 32] {
    let element_tag = match element {
        ExactLdsGemmElementV1::Bf16BitsU16 => 1,
        ExactLdsGemmElementV1::F32 => 2,
    };
    let mut digest = Sha256::new();
    for field in [
        LENGTH_IDENTITY_DOMAIN_V1,
        &[role as u8],
        &[element_tag],
        &elements.to_le_bytes(),
        &bytes.to_le_bytes(),
    ] {
        digest.update((field.len() as u64).to_le_bytes());
        digest.update(field);
    }
    digest.finalize().into()
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CheckedRegion {
    address: u64,
    elements: u64,
    byte_start: usize,
    byte_end: usize,
}

fn validate_region(
    role: ExactLdsGemmBufferRoleV1,
    facts: RegionFacts,
    expected_element_bytes: usize,
    expected_alignment: usize,
) -> Result<CheckedRegion, GeneratedLdsGemmSlice1HostAdapterErrorV1> {
    if facts.element_bytes != expected_element_bytes
        || facts.element_alignment != expected_alignment
    {
        return Err(GeneratedLdsGemmSlice1HostAdapterErrorV1::ElementLayout { role });
    }
    if facts.region_elements != SLICE1_ELEMENTS {
        return Err(GeneratedLdsGemmSlice1HostAdapterErrorV1::Length {
            role,
            expected: SLICE1_ELEMENTS,
            actual: facts.region_elements,
        });
    }
    if facts.allocation_address == 0 || facts.region_address == 0 {
        return Err(GeneratedLdsGemmSlice1HostAdapterErrorV1::NullAddress { role });
    }
    if !facts.region_address.is_multiple_of(expected_alignment) {
        return Err(GeneratedLdsGemmSlice1HostAdapterErrorV1::Alignment {
            role,
            required: expected_alignment,
            address: facts.region_address,
        });
    }

    let allocation_bytes = facts
        .allocation_elements
        .checked_mul(facts.element_bytes)
        .ok_or(GeneratedLdsGemmSlice1HostAdapterErrorV1::ByteLengthOverflow { role })?;
    let allocation_end = facts
        .allocation_address
        .checked_add(allocation_bytes)
        .ok_or(GeneratedLdsGemmSlice1HostAdapterErrorV1::AllocationAddressOverflow { role })?;
    let region_bytes = facts
        .region_elements
        .checked_mul(facts.element_bytes)
        .ok_or(GeneratedLdsGemmSlice1HostAdapterErrorV1::ByteLengthOverflow { role })?;
    let region_end = facts
        .region_address
        .checked_add(region_bytes)
        .ok_or(GeneratedLdsGemmSlice1HostAdapterErrorV1::RegionAddressOverflow { role })?;
    let relative_bytes = facts
        .region_byte_end
        .checked_sub(facts.region_byte_start)
        .ok_or(GeneratedLdsGemmSlice1HostAdapterErrorV1::InvalidRegionRange { role })?;
    let expected_region_address = facts
        .allocation_address
        .checked_add(facts.region_byte_start)
        .ok_or(GeneratedLdsGemmSlice1HostAdapterErrorV1::AllocationAddressOverflow { role })?;

    if facts.region_byte_end > allocation_bytes
        || relative_bytes != region_bytes
        || expected_region_address != facts.region_address
        || facts.region_address < facts.allocation_address
        || region_end > allocation_end
    {
        return Err(GeneratedLdsGemmSlice1HostAdapterErrorV1::InvalidRegionRange { role });
    }

    let address = u64::try_from(facts.region_address)
        .map_err(|_| GeneratedLdsGemmSlice1HostAdapterErrorV1::PointerWidth { role })?;
    Ok(CheckedRegion {
        address,
        elements: SLICE1_ELEMENTS as u64,
        byte_start: facts.region_address,
        byte_end: region_end,
    })
}

struct PreparedRegions {
    explicit_kernarg: Slice1ExplicitKernargV1,
}

fn prepare_regions(
    a: RegionFacts,
    b: RegionFacts,
    c: RegionFacts,
) -> Result<PreparedRegions, GeneratedLdsGemmSlice1HostAdapterErrorV1> {
    let a = validate_region(ExactLdsGemmBufferRoleV1::A, a, 2, 2)?;
    let b = validate_region(ExactLdsGemmBufferRoleV1::B, b, 2, 2)?;
    let c = validate_region(ExactLdsGemmBufferRoleV1::C, c, 4, 4)?;

    // A and B are shared read-only and may overlap. C is an exclusive
    // read-write region and therefore must be disjoint from both inputs.
    for (input, region) in [
        (ExactLdsGemmBufferRoleV1::A, a),
        (ExactLdsGemmBufferRoleV1::B, b),
    ] {
        if ranges_overlap(region, c) {
            return Err(GeneratedLdsGemmSlice1HostAdapterErrorV1::OutputOverlap { input });
        }
    }

    let mut explicit_kernarg = [0_u8; SLICE1_EXPLICIT_KERNARG_BYTES];
    for (slot, region) in explicit_kernarg.chunks_exact_mut(16).zip([a, b, c]) {
        slot[..8].copy_from_slice(&region.address.to_le_bytes());
        slot[8..].copy_from_slice(&region.elements.to_le_bytes());
    }
    Ok(PreparedRegions {
        explicit_kernarg: Slice1ExplicitKernargV1 {
            bytes: explicit_kernarg,
        },
    })
}

fn ranges_overlap(left: CheckedRegion, right: CheckedRegion) -> bool {
    left.byte_start < right.byte_end && right.byte_start < left.byte_end
}

/// Exact preparation failures. Every failure is authority-free and occurs
/// before a runtime can see a launch request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum GeneratedLdsGemmSlice1HostAdapterErrorV1 {
    ObservedTargetMismatch,
    ContractField {
        field: &'static str,
    },
    BufferContract {
        index: usize,
    },
    WrongContext {
        role: ExactLdsGemmBufferRoleV1,
    },
    ElementLayout {
        role: ExactLdsGemmBufferRoleV1,
    },
    Length {
        role: ExactLdsGemmBufferRoleV1,
        expected: usize,
        actual: usize,
    },
    NullAddress {
        role: ExactLdsGemmBufferRoleV1,
    },
    Alignment {
        role: ExactLdsGemmBufferRoleV1,
        required: usize,
        address: usize,
    },
    ByteLengthOverflow {
        role: ExactLdsGemmBufferRoleV1,
    },
    AllocationAddressOverflow {
        role: ExactLdsGemmBufferRoleV1,
    },
    RegionAddressOverflow {
        role: ExactLdsGemmBufferRoleV1,
    },
    InvalidRegionRange {
        role: ExactLdsGemmBufferRoleV1,
    },
    PointerWidth {
        role: ExactLdsGemmBufferRoleV1,
    },
    OutputOverlap {
        input: ExactLdsGemmBufferRoleV1,
    },
}

impl fmt::Display for GeneratedLdsGemmSlice1HostAdapterErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "exact LDS GEMM Slice1 host preparation rejected: {self:?}"
        )
    }
}

impl Error for GeneratedLdsGemmSlice1HostAdapterErrorV1 {}

#[cfg(test)]
#[allow(dead_code)]
pub(crate) mod test_support {
    use super::*;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(crate) struct TestRegionFactsV1 {
        pub(crate) allocation_address: usize,
        pub(crate) allocation_elements: usize,
        pub(crate) region_address: usize,
        pub(crate) region_elements: usize,
        pub(crate) region_byte_start: usize,
        pub(crate) region_byte_end: usize,
        pub(crate) element_bytes: usize,
        pub(crate) element_alignment: usize,
    }

    impl From<TestRegionFactsV1> for RegionFacts {
        fn from(value: TestRegionFactsV1) -> Self {
            Self {
                allocation_address: value.allocation_address,
                allocation_elements: value.allocation_elements,
                region_address: value.region_address,
                region_elements: value.region_elements,
                region_byte_start: value.region_byte_start,
                region_byte_end: value.region_byte_end,
                element_bytes: value.element_bytes,
                element_alignment: value.element_alignment,
            }
        }
    }

    pub(crate) fn prepare_regions_v1(
        a: TestRegionFactsV1,
        b: TestRegionFactsV1,
        c: TestRegionFactsV1,
    ) -> Result<[u8; SLICE1_EXPLICIT_KERNARG_BYTES], GeneratedLdsGemmSlice1HostAdapterErrorV1> {
        Ok(prepare_regions(a.into(), b.into(), c.into())?
            .explicit_kernarg
            .bytes)
    }

    pub(crate) const fn explicit_kernarg_layout_v1() -> (usize, usize) {
        (
            size_of::<Slice1ExplicitKernargV1>(),
            align_of::<Slice1ExplicitKernargV1>(),
        )
    }

    pub(crate) fn validate_observed_target_v1(
        target: &str,
    ) -> Result<(), GeneratedLdsGemmSlice1HostAdapterErrorV1> {
        validate_observed_target(target)
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(crate) enum TestContractMutationV1 {
        None,
        Profile,
        Target,
        CodeObjectVersion,
        Grid,
        Workgroup,
        Wavefront,
        ExplicitKernarg,
        CompleteKernarg,
        KernargAlignment,
        StaticLds,
        LdsAllocations,
        LdsBytesPerAllocation,
        LdsAlignment,
        BufferRole,
        BufferElement,
        BufferLength,
        BufferBytes,
        BufferLengthIdentity,
        BufferOwnership,
        BufferAccess,
        BufferAlias,
    }

    pub(crate) fn validate_contract_mutation_v1(
        mutation: TestContractMutationV1,
    ) -> Result<(), GeneratedLdsGemmSlice1HostAdapterErrorV1> {
        let mut facts = ContractFacts {
            profile: ExactLdsGemmProfileIdV1::Slice1M16N16K16,
            target: SLICE1_TARGET,
            code_object_version: CodeObjectVersion::V6,
            grid: SLICE1_GRID,
            workgroup: SLICE1_WORKGROUP,
            wavefront_size: SLICE1_WAVEFRONT_SIZE,
            explicit_kernarg_bytes: SLICE1_EXPLICIT_KERNARG_BYTES as u32,
            complete_kernarg_bytes: SLICE1_COMPLETE_KERNARG_BYTES,
            kernarg_alignment: SLICE1_KERNARG_ALIGNMENT,
            static_lds_bytes: SLICE1_STATIC_LDS_BYTES,
            lds_allocations: SLICE1_LDS_ALLOCATIONS,
            lds_bytes_per_allocation: SLICE1_LDS_BYTES_PER_ALLOCATION,
            lds_alignment: SLICE1_LDS_ALIGNMENT,
            buffers: [
                BufferContractFacts {
                    role: ExactLdsGemmBufferRoleV1::A,
                    element: ExactLdsGemmElementV1::Bf16BitsU16,
                    elements: 256,
                    bytes: 512,
                    length_identity: length_identity(
                        ExactLdsGemmBufferRoleV1::A,
                        ExactLdsGemmElementV1::Bf16BitsU16,
                        256,
                        512,
                    ),
                    ownership: OwnershipSemantics::SharedBorrow,
                    access: AccessMode::ReadOnly,
                    alias: AliasSemantics::SharedReadOnly,
                },
                BufferContractFacts {
                    role: ExactLdsGemmBufferRoleV1::B,
                    element: ExactLdsGemmElementV1::Bf16BitsU16,
                    elements: 256,
                    bytes: 512,
                    length_identity: length_identity(
                        ExactLdsGemmBufferRoleV1::B,
                        ExactLdsGemmElementV1::Bf16BitsU16,
                        256,
                        512,
                    ),
                    ownership: OwnershipSemantics::SharedBorrow,
                    access: AccessMode::ReadOnly,
                    alias: AliasSemantics::SharedReadOnly,
                },
                BufferContractFacts {
                    role: ExactLdsGemmBufferRoleV1::C,
                    element: ExactLdsGemmElementV1::F32,
                    elements: 256,
                    bytes: 1_024,
                    length_identity: length_identity(
                        ExactLdsGemmBufferRoleV1::C,
                        ExactLdsGemmElementV1::F32,
                        256,
                        1_024,
                    ),
                    ownership: OwnershipSemantics::UniqueBorrow,
                    access: AccessMode::ReadWrite,
                    alias: AliasSemantics::Exclusive,
                },
            ],
        };
        match mutation {
            TestContractMutationV1::None => {}
            TestContractMutationV1::Profile => {
                facts.profile = ExactLdsGemmProfileIdV1::KPhaseM16N16K32
            }
            TestContractMutationV1::Target => facts.target = "gfx942:xnack+",
            TestContractMutationV1::CodeObjectVersion => {
                facts.code_object_version = CodeObjectVersion::V5
            }
            TestContractMutationV1::Grid => facts.grid = [2, 1, 1],
            TestContractMutationV1::Workgroup => facts.workgroup = [32, 1, 1],
            TestContractMutationV1::Wavefront => facts.wavefront_size = 32,
            TestContractMutationV1::ExplicitKernarg => facts.explicit_kernarg_bytes = 56,
            TestContractMutationV1::CompleteKernarg => facts.complete_kernarg_bytes = 312,
            TestContractMutationV1::KernargAlignment => facts.kernarg_alignment = 16,
            TestContractMutationV1::StaticLds => facts.static_lds_bytes = 512,
            TestContractMutationV1::LdsAllocations => facts.lds_allocations = 1,
            TestContractMutationV1::LdsBytesPerAllocation => facts.lds_bytes_per_allocation = 256,
            TestContractMutationV1::LdsAlignment => facts.lds_alignment = 8,
            TestContractMutationV1::BufferRole => {
                facts.buffers[0].role = ExactLdsGemmBufferRoleV1::B
            }
            TestContractMutationV1::BufferElement => {
                facts.buffers[0].element = ExactLdsGemmElementV1::F32
            }
            TestContractMutationV1::BufferLength => facts.buffers[0].elements = 255,
            TestContractMutationV1::BufferBytes => facts.buffers[0].bytes = 1_024,
            TestContractMutationV1::BufferLengthIdentity => {
                facts.buffers[0].length_identity = [0; 32]
            }
            TestContractMutationV1::BufferOwnership => {
                facts.buffers[0].ownership = OwnershipSemantics::UniqueBorrow
            }
            TestContractMutationV1::BufferAccess => facts.buffers[0].access = AccessMode::ReadWrite,
            TestContractMutationV1::BufferAlias => {
                facts.buffers[0].alias = AliasSemantics::Exclusive
            }
        }
        validate_contract(facts)
    }
}
