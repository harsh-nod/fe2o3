//! Sealed, authority-free admission for exact LDS GEMM compiler imports.
//!
//! The registry recognizes stable profile slots, but only Slice 1 is enabled.
//! Admission retains the complete neutral compiler handoff and grants no
//! compiler-origin, Worker V2, link, publication, load, or launch authority.

use std::{error::Error, fmt, str};

use dialect_amdgcn::lower_tiled_gemm_lds_v1_to_gfx942_llvm_ir;
use fe2o3_compiler_ffi::{
    COMPILER_DESCRIPTOR_SECTION_NAME_V1, CompilerDescriptorSourceErrorV1,
    CompilerDescriptorSourceIdentityV1, CompilerDescriptorSourceV1, CompilerModuleHandoffV2,
    CompilerModuleKindV1, CompilerModuleSymbolManifestV1, CompilerModuleSymbolRoleV1,
};
use fe2o3_kernel_descriptor::{
    AccessMode, AliasSemantics, BlockSizeV1, BuildEvidenceV1, CapabilityV1, CodeObjectVersion,
    DeviceLayoutDescriptorV1, EvidenceDigest, EvidenceIdentity, OwnershipSemantics,
    PhysicalAbiComponentKind, ScalarTypeV1, SourceTypeDescriptorV1,
};
use fe2o3_kernel_ir::{
    Module, TILED_GEMM_LDS_EDGES_V1_KERNEL_ID, TILED_GEMM_LDS_GRID_V1_KERNEL_ID,
    TILED_GEMM_LDS_K32_V2_KERNEL_ID, TILED_GEMM_LDS_V1_ALLOCATION_COUNT,
    TILED_GEMM_LDS_V1_KERNEL_ID, TILED_GEMM_LDS_V1_LANES, TILED_GEMM_LDS_V1_LDS_ALIGNMENT,
    TILED_GEMM_LDS_V1_STATIC_LDS_BYTES, TILED_GEMM_LDS_V1_TILE_BYTES, TiledGemmLdsV1Profile,
    encode_module_v5, tiled_gemm_lds_v1_module,
};
use sha2::{Digest, Sha256};

const EXACT_TARGET: &str = "gfx942:xnack-";
const SLICE1_LOGICAL_NAME: &str = "tiled_gemm_lds_slice1";
const SLICE1_DESCRIPTOR_SYMBOL: &str = "tiled_gemm_lds_v1.kd";
const SLICE1_PRODUCER_VERSION: &str = "typed-tiled-gemm-lds-slice1-gfx942-cov6-v1";
const AUTHORITY_SECTION_V1: &str = ".fe2o3.tiled-lds-slice1-auth.v1";
const RESOURCE_SECTION_V1: &str = ".fe2o3.tiled-lds-slice1-resources.v1";
const RESOURCE_TRANSCRIPT_DOMAIN_V1: &[u8] = b"fe2o3.tiled-gemm-lds-slice1.worker-v2-resources.v1";
const CANONICAL_IR_DOMAIN_V1: &[u8] = b"fe2o3.tiled-gemm-lds-slice1.compiler-structural-ir.v1";
const DESCRIPTOR_IR_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/RUSTC-DESCRIPTOR-IR-IDENTITY/V1\0";
const DESCRIPTOR_IR_DIGEST_DOMAIN_V1: &[u8] = b"FE2O3/RUSTC-DESCRIPTOR-IR-DIGEST/V1\0";
const PROFILE_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/EXACT-LDS-GEMM-PROFILE/V1\0";
const IMPORT_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/EXACT-LDS-GEMM-COMPILER-IMPORT/V1\0";
const CONTENT_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/EXACT-LDS-GEMM-CONTENT/V1\0";
const LENGTH_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/EXACT-LDS-GEMM-LENGTH/V1\0";

/// Stable, disjoint extension slots in the exact LDS GEMM registry.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum ExactLdsGemmProfileIdV1 {
    Slice1M16N16K16 = 1,
    KPhaseM16N16K32 = 2,
    GridM64N48K16 = 3,
    EdgesM17N19K18 = 4,
}

/// Whether a stable registry slot can currently produce a sealed value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExactLdsGemmProfileAvailabilityV1 {
    Enabled,
    Reserved,
}

/// Returns the current fail-closed state of one stable profile slot.
pub const fn exact_lds_gemm_profile_availability_v1(
    profile: ExactLdsGemmProfileIdV1,
) -> ExactLdsGemmProfileAvailabilityV1 {
    match profile {
        ExactLdsGemmProfileIdV1::Slice1M16N16K16 => ExactLdsGemmProfileAvailabilityV1::Enabled,
        ExactLdsGemmProfileIdV1::KPhaseM16N16K32
        | ExactLdsGemmProfileIdV1::GridM64N48K16
        | ExactLdsGemmProfileIdV1::EdgesM17N19K18 => ExactLdsGemmProfileAvailabilityV1::Reserved,
    }
}

/// Stable identity of one typed exact-profile contract.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExactLdsGemmProfileIdentityV1([u8; 32]);

impl ExactLdsGemmProfileIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Digest and length of exact retained content.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExactLdsGemmContentIdentityV1 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl ExactLdsGemmContentIdentityV1 {
    pub const fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }

    fn calculate(bytes: &[u8]) -> Self {
        let mut digest = Sha256::new();
        hash_field(&mut digest, CONTENT_IDENTITY_DOMAIN_V1);
        hash_field(&mut digest, bytes);
        Self {
            sha256: digest.finalize().into(),
            byte_len: bytes.len() as u64,
        }
    }
}

/// Stable semantic roles for the three exact GEMM buffers.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum ExactLdsGemmBufferRoleV1 {
    A = 1,
    B = 2,
    C = 3,
}

/// Exact element interpretation retained by the host-adapter boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExactLdsGemmElementV1 {
    Bf16BitsU16,
    F32,
}

/// Stable identity of one role-specific exact element count.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExactLdsGemmLengthIdentityV1([u8; 32]);

impl ExactLdsGemmLengthIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Typed exact-length and effect policy for one GEMM buffer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactLdsGemmBufferContractV1 {
    role: ExactLdsGemmBufferRoleV1,
    element: ExactLdsGemmElementV1,
    elements: u64,
    bytes: u64,
    length_identity: ExactLdsGemmLengthIdentityV1,
    ownership: OwnershipSemantics,
    access: AccessMode,
    alias: AliasSemantics,
}

impl ExactLdsGemmBufferContractV1 {
    pub const fn role(self) -> ExactLdsGemmBufferRoleV1 {
        self.role
    }

    pub const fn element(self) -> ExactLdsGemmElementV1 {
        self.element
    }

    pub const fn elements(self) -> u64 {
        self.elements
    }

    pub const fn bytes(self) -> u64 {
        self.bytes
    }

    pub const fn length_identity(self) -> ExactLdsGemmLengthIdentityV1 {
        self.length_identity
    }

    pub const fn ownership(self) -> OwnershipSemantics {
        self.ownership
    }

    pub const fn access(self) -> AccessMode {
        self.access
    }

    pub const fn alias(self) -> AliasSemantics {
        self.alias
    }
}

/// Profile-neutral exact contract consumed by later finalizer and host work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactLdsGemmContractV1 {
    profile: ExactLdsGemmProfileIdV1,
    identity: ExactLdsGemmProfileIdentityV1,
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
    buffers: [ExactLdsGemmBufferContractV1; 3],
}

impl ExactLdsGemmContractV1 {
    pub const fn profile(self) -> ExactLdsGemmProfileIdV1 {
        self.profile
    }

    pub const fn identity(self) -> ExactLdsGemmProfileIdentityV1 {
        self.identity
    }

    pub const fn target(self) -> &'static str {
        EXACT_TARGET
    }

    pub const fn code_object_version(self) -> CodeObjectVersion {
        CodeObjectVersion::V6
    }

    pub const fn grid(self) -> [u32; 3] {
        self.grid
    }

    pub const fn workgroup(self) -> [u32; 3] {
        self.workgroup
    }

    pub const fn wavefront_size(self) -> u32 {
        self.wavefront_size
    }

    pub const fn explicit_kernarg_bytes(self) -> u32 {
        self.explicit_kernarg_bytes
    }

    pub const fn complete_kernarg_bytes(self) -> u32 {
        self.complete_kernarg_bytes
    }

    pub const fn kernarg_alignment(self) -> u32 {
        self.kernarg_alignment
    }

    pub const fn static_lds_bytes(self) -> u32 {
        self.static_lds_bytes
    }

    pub const fn lds_allocations(self) -> u32 {
        self.lds_allocations
    }

    pub const fn lds_bytes_per_allocation(self) -> u32 {
        self.lds_bytes_per_allocation
    }

    pub const fn lds_alignment(self) -> u32 {
        self.lds_alignment
    }

    pub const fn buffers(self) -> [ExactLdsGemmBufferContractV1; 3] {
        self.buffers
    }
}

/// Upstream pins supplied by the source-authenticated compiler route.
///
/// Public construction is structural and grants no authority. Consumers must
/// obtain the values from their own authenticated transaction boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactLdsGemmCompilerImportPinsV1 {
    descriptor_source: CompilerDescriptorSourceIdentityV1,
    source_authority: [u8; 32],
}

impl ExactLdsGemmCompilerImportPinsV1 {
    pub fn new(
        descriptor_source: CompilerDescriptorSourceIdentityV1,
        source_authority: [u8; 32],
    ) -> Result<Self, ExactLdsGemmProfileAdmissionErrorV1> {
        if source_authority == [0; 32] {
            return Err(ExactLdsGemmProfileAdmissionErrorV1::InvalidPins);
        }
        Ok(Self {
            descriptor_source,
            source_authority,
        })
    }

    pub const fn descriptor_source(self) -> CompilerDescriptorSourceIdentityV1 {
        self.descriptor_source
    }

    pub const fn source_authority(&self) -> &[u8; 32] {
        &self.source_authority
    }
}

/// Stable aggregate identity of one fully retained neutral compiler import.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InspectedExactLdsGemmCompilerImportIdentityV1([u8; 32]);

impl InspectedExactLdsGemmCompilerImportIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Sealed exact compiler import. This value is deliberately not `Clone`.
///
/// ```compile_fail
/// use fe2o3_hsaco_finalize::InspectedExactLdsGemmCompilerImportV1;
///
/// fn replay(value: InspectedExactLdsGemmCompilerImportV1) {
///     let _second = value.clone();
/// }
/// ```
#[derive(Debug)]
pub struct InspectedExactLdsGemmCompilerImportV1 {
    identity: InspectedExactLdsGemmCompilerImportIdentityV1,
    contract: ExactLdsGemmContractV1,
    kernel_ir: Module,
    kernel_ir_identity: ExactLdsGemmContentIdentityV1,
    llvm_body: String,
    llvm_body_identity: ExactLdsGemmContentIdentityV1,
    descriptor_source: CompilerDescriptorSourceV1,
    source_authority: [u8; 32],
    resource_transcript_identity: ExactLdsGemmContentIdentityV1,
    handoff: CompilerModuleHandoffV2,
}

impl InspectedExactLdsGemmCompilerImportV1 {
    pub const fn identity(&self) -> InspectedExactLdsGemmCompilerImportIdentityV1 {
        self.identity
    }

    pub const fn contract(&self) -> ExactLdsGemmContractV1 {
        self.contract
    }

    pub const fn kernel_ir(&self) -> &Module {
        &self.kernel_ir
    }

    pub const fn kernel_ir_identity(&self) -> ExactLdsGemmContentIdentityV1 {
        self.kernel_ir_identity
    }

    pub fn canonical_llvm_body(&self) -> &str {
        &self.llvm_body
    }

    pub const fn llvm_body_identity(&self) -> ExactLdsGemmContentIdentityV1 {
        self.llvm_body_identity
    }

    pub const fn descriptor_source(&self) -> &CompilerDescriptorSourceV1 {
        &self.descriptor_source
    }

    pub const fn source_authority(&self) -> &[u8; 32] {
        &self.source_authority
    }

    pub const fn resource_transcript_identity(&self) -> ExactLdsGemmContentIdentityV1 {
        self.resource_transcript_identity
    }

    pub const fn handoff(&self) -> &CompilerModuleHandoffV2 {
        &self.handoff
    }

    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }

    pub const fn grants_worker_authority(&self) -> bool {
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

/// Failure to admit one exact compiler import into the sealed registry.
#[derive(Debug)]
#[non_exhaustive]
pub enum ExactLdsGemmProfileAdmissionErrorV1 {
    InvalidPins,
    UnsupportedManifest,
    ReservedProfile(ExactLdsGemmProfileIdV1),
    HandoffField(&'static str),
    SectionLayout(&'static str),
    DescriptorDecode(CompilerDescriptorSourceErrorV1),
    DescriptorPinMismatch,
    DescriptorField(&'static str),
    KernelIr(String),
    LlvmLowering(String),
    LlvmBodyMismatch,
    ExecutableEvidenceMismatch,
    SourceAuthorityMismatch,
    ResourceTranscriptMismatch,
}

impl fmt::Display for ExactLdsGemmProfileAdmissionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPins => formatter.write_str("exact LDS GEMM import pins are invalid"),
            Self::UnsupportedManifest => {
                formatter.write_str("compiler manifest is not a registered exact LDS GEMM profile")
            }
            Self::ReservedProfile(profile) => {
                write!(formatter, "exact LDS GEMM profile {profile:?} is reserved")
            }
            Self::HandoffField(field) => write!(formatter, "compiler handoff {field} drifted"),
            Self::SectionLayout(detail) => {
                write!(
                    formatter,
                    "compiler module section layout rejected: {detail}"
                )
            }
            Self::DescriptorDecode(error) => write!(formatter, "descriptor decode failed: {error}"),
            Self::DescriptorPinMismatch => {
                formatter.write_str("descriptor source differs from its authenticated pin")
            }
            Self::DescriptorField(field) => write!(formatter, "descriptor {field} drifted"),
            Self::KernelIr(error) => write!(formatter, "canonical Kernel IR rejected: {error}"),
            Self::LlvmLowering(error) => {
                write!(formatter, "canonical LLVM lowering failed: {error}")
            }
            Self::LlvmBodyMismatch => formatter
                .write_str("complete pre-section LLVM body differs from the canonical lowering"),
            Self::ExecutableEvidenceMismatch => formatter
                .write_str("descriptor executable evidence does not bind the exact LLVM body"),
            Self::SourceAuthorityMismatch => {
                formatter.write_str("source authority section differs from its authenticated pin")
            }
            Self::ResourceTranscriptMismatch => {
                formatter.write_str("resource transcript is not the exact Slice 1 binding")
            }
        }
    }
}

impl Error for ExactLdsGemmProfileAdmissionErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::DescriptorDecode(error) => Some(error),
            _ => None,
        }
    }
}

/// Admits one exact source-bound Slice 1 compiler handoff.
///
/// The profile is inferred from the complete manifest. Reserved profiles fail
/// before any descriptor, LLVM, worker, linker, or runtime authority can be
/// created.
pub fn inspect_exact_lds_gemm_compiler_import_v1(
    pins: ExactLdsGemmCompilerImportPinsV1,
    handoff: CompilerModuleHandoffV2,
) -> Result<InspectedExactLdsGemmCompilerImportV1, ExactLdsGemmProfileAdmissionErrorV1> {
    let profile_id = infer_profile(handoff.symbol_manifest())?;
    if exact_lds_gemm_profile_availability_v1(profile_id)
        != ExactLdsGemmProfileAvailabilityV1::Enabled
    {
        return Err(ExactLdsGemmProfileAdmissionErrorV1::ReservedProfile(
            profile_id,
        ));
    }
    if handoff.kind() != CompilerModuleKindV1::LlvmTextIr {
        return Err(ExactLdsGemmProfileAdmissionErrorV1::HandoffField(
            "module kind",
        ));
    }
    if handoff.target().to_string() != EXACT_TARGET {
        return Err(ExactLdsGemmProfileAdmissionErrorV1::HandoffField("target"));
    }
    if handoff.code_object_version() != CodeObjectVersion::V6 {
        return Err(ExactLdsGemmProfileAdmissionErrorV1::HandoffField(
            "code-object version",
        ));
    }
    let inspection = handoff.envelope().inspection();
    if handoff.envelope().target() != handoff.target()
        || handoff.envelope().code_object_version() != CodeObjectVersion::V6
        || inspection.import_count() != 0
        || inspection.export_count() != 0
        || inspection.requires_compiler_module_definition_count() != 0
    {
        return Err(ExactLdsGemmProfileAdmissionErrorV1::HandoffField(
            "FFI envelope",
        ));
    }

    let kernel_ir = tiled_gemm_lds_v1_module();
    let kernel_ir_bytes = encode_module_v5(&kernel_ir)
        .map_err(|error| ExactLdsGemmProfileAdmissionErrorV1::KernelIr(error.to_string()))?;
    let canonical_ir_commitment = canonical_ir_commitment(&kernel_ir_bytes);
    let lowering = lower_tiled_gemm_lds_v1_to_gfx942_llvm_ir(
        &kernel_ir,
        TiledGemmLdsV1Profile::exact_gfx942_xnack_minus_cov6(),
    )
    .map_err(|error| ExactLdsGemmProfileAdmissionErrorV1::LlvmLowering(error.to_string()))?;
    let sections = decode_exact_sections(handoff.module_bytes(), lowering.as_str())?;
    let descriptor_source = CompilerDescriptorSourceV1::decode(&sections.descriptor)
        .map_err(ExactLdsGemmProfileAdmissionErrorV1::DescriptorDecode)?;
    if descriptor_source.identity() != pins.descriptor_source {
        return Err(ExactLdsGemmProfileAdmissionErrorV1::DescriptorPinMismatch);
    }

    validate_descriptor(&descriptor_source, handoff.envelope(), &sections.body)?;
    if sections.authority.as_slice() != pins.source_authority {
        return Err(ExactLdsGemmProfileAdmissionErrorV1::SourceAuthorityMismatch);
    }
    validate_resource_transcript(
        &sections.resources,
        &pins.source_authority,
        &canonical_ir_commitment,
        descriptor_source.identity().sha256(),
    )?;

    let contract = exact_slice1_contract();
    let kernel_ir_identity = ExactLdsGemmContentIdentityV1::calculate(&kernel_ir_bytes);
    let llvm_body_identity = ExactLdsGemmContentIdentityV1::calculate(sections.body.as_bytes());
    let resource_transcript_identity =
        ExactLdsGemmContentIdentityV1::calculate(&sections.resources);
    let identity = import_identity(
        contract.identity,
        &kernel_ir_bytes,
        sections.body.as_bytes(),
        descriptor_source.canonical_bytes(),
        handoff.envelope().canonical_bytes(),
        handoff.symbol_manifest().canonical_bytes(),
        &sections.authority,
        &sections.resources,
        handoff.canonical_bytes(),
    );

    Ok(InspectedExactLdsGemmCompilerImportV1 {
        identity,
        contract,
        kernel_ir,
        kernel_ir_identity,
        llvm_body: sections.body,
        llvm_body_identity,
        descriptor_source,
        source_authority: pins.source_authority,
        resource_transcript_identity,
        handoff,
    })
}

fn infer_profile(
    manifest: &CompilerModuleSymbolManifestV1,
) -> Result<ExactLdsGemmProfileIdV1, ExactLdsGemmProfileAdmissionErrorV1> {
    let entries = manifest.entries().collect::<Vec<_>>();
    for (profile, entry) in [
        (
            ExactLdsGemmProfileIdV1::Slice1M16N16K16,
            TILED_GEMM_LDS_V1_KERNEL_ID,
        ),
        (
            ExactLdsGemmProfileIdV1::KPhaseM16N16K32,
            TILED_GEMM_LDS_K32_V2_KERNEL_ID,
        ),
        (
            ExactLdsGemmProfileIdV1::GridM64N48K16,
            TILED_GEMM_LDS_GRID_V1_KERNEL_ID,
        ),
        (
            ExactLdsGemmProfileIdV1::EdgesM17N19K18,
            TILED_GEMM_LDS_EDGES_V1_KERNEL_ID,
        ),
    ] {
        let descriptor = format!("{entry}.kd");
        if entries
            == [
                (CompilerModuleSymbolRoleV1::KernelEntry, entry),
                (
                    CompilerModuleSymbolRoleV1::KernelDescriptor,
                    descriptor.as_str(),
                ),
            ]
        {
            return Ok(profile);
        }
    }
    Err(ExactLdsGemmProfileAdmissionErrorV1::UnsupportedManifest)
}

fn exact_slice1_contract() -> ExactLdsGemmContractV1 {
    let buffers = [
        buffer_contract(
            ExactLdsGemmBufferRoleV1::A,
            ExactLdsGemmElementV1::Bf16BitsU16,
            256,
            2,
            OwnershipSemantics::SharedBorrow,
            AccessMode::ReadOnly,
            AliasSemantics::SharedReadOnly,
        ),
        buffer_contract(
            ExactLdsGemmBufferRoleV1::B,
            ExactLdsGemmElementV1::Bf16BitsU16,
            256,
            2,
            OwnershipSemantics::SharedBorrow,
            AccessMode::ReadOnly,
            AliasSemantics::SharedReadOnly,
        ),
        buffer_contract(
            ExactLdsGemmBufferRoleV1::C,
            ExactLdsGemmElementV1::F32,
            256,
            4,
            OwnershipSemantics::UniqueBorrow,
            AccessMode::ReadWrite,
            AliasSemantics::Exclusive,
        ),
    ];
    let mut digest = Sha256::new();
    hash_field(&mut digest, PROFILE_IDENTITY_DOMAIN_V1);
    hash_field(
        &mut digest,
        &[ExactLdsGemmProfileIdV1::Slice1M16N16K16 as u8],
    );
    for field in [
        EXACT_TARGET.as_bytes(),
        &6u16.to_le_bytes(),
        SLICE1_LOGICAL_NAME.as_bytes(),
        TILED_GEMM_LDS_V1_KERNEL_ID.as_bytes(),
        SLICE1_DESCRIPTOR_SYMBOL.as_bytes(),
        &48u32.to_le_bytes(),
        &304u32.to_le_bytes(),
        &8u32.to_le_bytes(),
        &[1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0],
        &[64, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0],
        &64u32.to_le_bytes(),
        &TILED_GEMM_LDS_V1_STATIC_LDS_BYTES.to_le_bytes(),
        &TILED_GEMM_LDS_V1_ALLOCATION_COUNT.to_le_bytes(),
        &TILED_GEMM_LDS_V1_TILE_BYTES.to_le_bytes(),
        &TILED_GEMM_LDS_V1_LDS_ALIGNMENT.to_le_bytes(),
    ] {
        hash_field(&mut digest, field);
    }
    for buffer in buffers {
        hash_field(&mut digest, &[buffer.role as u8]);
        hash_field(&mut digest, &buffer.elements.to_le_bytes());
        hash_field(&mut digest, &buffer.bytes.to_le_bytes());
        hash_field(&mut digest, buffer.length_identity.as_bytes());
        hash_field(&mut digest, &[buffer_effect_tag(buffer)]);
    }
    ExactLdsGemmContractV1 {
        profile: ExactLdsGemmProfileIdV1::Slice1M16N16K16,
        identity: ExactLdsGemmProfileIdentityV1(digest.finalize().into()),
        grid: [1, 1, 1],
        workgroup: [64, 1, 1],
        wavefront_size: 64,
        explicit_kernarg_bytes: 48,
        complete_kernarg_bytes: 304,
        kernarg_alignment: 8,
        static_lds_bytes: TILED_GEMM_LDS_V1_STATIC_LDS_BYTES,
        lds_allocations: TILED_GEMM_LDS_V1_ALLOCATION_COUNT,
        lds_bytes_per_allocation: TILED_GEMM_LDS_V1_TILE_BYTES,
        lds_alignment: TILED_GEMM_LDS_V1_LDS_ALIGNMENT,
        buffers,
    }
}

#[allow(clippy::too_many_arguments)]
fn buffer_contract(
    role: ExactLdsGemmBufferRoleV1,
    element: ExactLdsGemmElementV1,
    elements: u64,
    element_bytes: u64,
    ownership: OwnershipSemantics,
    access: AccessMode,
    alias: AliasSemantics,
) -> ExactLdsGemmBufferContractV1 {
    let bytes = elements * element_bytes;
    let mut digest = Sha256::new();
    hash_field(&mut digest, LENGTH_IDENTITY_DOMAIN_V1);
    hash_field(&mut digest, &[role as u8]);
    hash_field(&mut digest, &[element_tag(element)]);
    hash_field(&mut digest, &elements.to_le_bytes());
    hash_field(&mut digest, &bytes.to_le_bytes());
    ExactLdsGemmBufferContractV1 {
        role,
        element,
        elements,
        bytes,
        length_identity: ExactLdsGemmLengthIdentityV1(digest.finalize().into()),
        ownership,
        access,
        alias,
    }
}

fn element_tag(element: ExactLdsGemmElementV1) -> u8 {
    match element {
        ExactLdsGemmElementV1::Bf16BitsU16 => 1,
        ExactLdsGemmElementV1::F32 => 2,
    }
}

fn buffer_effect_tag(buffer: ExactLdsGemmBufferContractV1) -> u8 {
    match (buffer.ownership, buffer.access, buffer.alias) {
        (
            OwnershipSemantics::SharedBorrow,
            AccessMode::ReadOnly,
            AliasSemantics::SharedReadOnly,
        ) => 1,
        (OwnershipSemantics::UniqueBorrow, AccessMode::ReadWrite, AliasSemantics::Exclusive) => 2,
        _ => 0,
    }
}

struct DecodedSectionsV1 {
    body: String,
    descriptor: Vec<u8>,
    authority: Vec<u8>,
    resources: Vec<u8>,
}

fn decode_exact_sections(
    module: &[u8],
    expected_body: &str,
) -> Result<DecodedSectionsV1, ExactLdsGemmProfileAdmissionErrorV1> {
    let text = str::from_utf8(module)
        .map_err(|_| ExactLdsGemmProfileAdmissionErrorV1::SectionLayout("module is not UTF-8"))?;
    if !text.ends_with('\n') {
        return Err(ExactLdsGemmProfileAdmissionErrorV1::SectionLayout(
            "module lacks its canonical trailing newline",
        ));
    }
    let suffix = text
        .strip_prefix(expected_body)
        .ok_or(ExactLdsGemmProfileAdmissionErrorV1::LlvmBodyMismatch)?;
    let suffix = suffix
        .strip_prefix('\n')
        .ok_or(ExactLdsGemmProfileAdmissionErrorV1::LlvmBodyMismatch)?;
    let first_header = section_header(COMPILER_DESCRIPTOR_SECTION_NAME_V1);
    if !suffix.starts_with(&first_header) {
        return Err(ExactLdsGemmProfileAdmissionErrorV1::SectionLayout(
            "descriptor section is missing or reordered",
        ));
    }

    let mut lines = suffix.lines().peekable();
    let descriptor = decode_section(
        &mut lines,
        COMPILER_DESCRIPTOR_SECTION_NAME_V1,
        "descriptor",
    )?;
    let authority = decode_section(&mut lines, AUTHORITY_SECTION_V1, "source authority")?;
    let resources = decode_section(&mut lines, RESOURCE_SECTION_V1, "resource transcript")?;
    if lines.next().is_some() {
        return Err(ExactLdsGemmProfileAdmissionErrorV1::SectionLayout(
            "unexpected trailing module text",
        ));
    }
    if authority.len() != 32 {
        return Err(ExactLdsGemmProfileAdmissionErrorV1::SectionLayout(
            "source authority must contain exactly 32 bytes",
        ));
    }
    Ok(DecodedSectionsV1 {
        body: expected_body.to_owned(),
        descriptor,
        authority,
        resources,
    })
}

fn decode_section<'a, I>(
    lines: &mut std::iter::Peekable<I>,
    section: &str,
    description: &'static str,
) -> Result<Vec<u8>, ExactLdsGemmProfileAdmissionErrorV1>
where
    I: Iterator<Item = &'a str>,
{
    if lines.next() != Some(section_header(section).as_str())
        || lines.next() != Some("module asm \".balign 8\"")
    {
        return Err(ExactLdsGemmProfileAdmissionErrorV1::SectionLayout(
            description,
        ));
    }
    let mut bytes = Vec::new();
    let mut chunk_lengths = Vec::new();
    while lines
        .peek()
        .is_some_and(|line| line.starts_with("module asm \".byte "))
    {
        let chunk = decode_byte_line(lines.next().expect("peeked line"))?;
        chunk_lengths.push(chunk.len());
        bytes.extend_from_slice(&chunk);
    }
    if bytes.is_empty()
        || chunk_lengths
            .iter()
            .take(chunk_lengths.len().saturating_sub(1))
            .any(|length| *length != 16)
    {
        return Err(ExactLdsGemmProfileAdmissionErrorV1::SectionLayout(
            description,
        ));
    }
    Ok(bytes)
}

fn decode_byte_line(line: &str) -> Result<Vec<u8>, ExactLdsGemmProfileAdmissionErrorV1> {
    let values = line
        .strip_prefix("module asm \".byte ")
        .and_then(|line| line.strip_suffix('"'))
        .ok_or(ExactLdsGemmProfileAdmissionErrorV1::SectionLayout(
            "noncanonical byte directive",
        ))?;
    let chunks = values.split(", ").collect::<Vec<_>>();
    if chunks.is_empty() || chunks.len() > 16 {
        return Err(ExactLdsGemmProfileAdmissionErrorV1::SectionLayout(
            "invalid byte directive width",
        ));
    }
    chunks
        .into_iter()
        .map(|value| {
            let digits = value.strip_prefix("0x").ok_or(
                ExactLdsGemmProfileAdmissionErrorV1::SectionLayout(
                    "byte directive lacks a lowercase hexadecimal prefix",
                ),
            )?;
            if digits.len() != 2
                || !digits
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            {
                return Err(ExactLdsGemmProfileAdmissionErrorV1::SectionLayout(
                    "byte directive is not canonical lowercase hexadecimal",
                ));
            }
            u8::from_str_radix(digits, 16).map_err(|_| {
                ExactLdsGemmProfileAdmissionErrorV1::SectionLayout("byte directive is outside u8")
            })
        })
        .collect()
}

fn section_header(section: &str) -> String {
    format!("module asm \".section {section},\\22\\22,@progbits\"")
}

fn validate_descriptor(
    source: &CompilerDescriptorSourceV1,
    envelope: &fe2o3_compiler_ffi::CompilerFfiEnvelopeV1,
    llvm_body: &str,
) -> Result<(), ExactLdsGemmProfileAdmissionErrorV1> {
    let table = source.table();
    if table.canonical_code_object_digest().as_bytes() != &[0; 32] {
        return Err(ExactLdsGemmProfileAdmissionErrorV1::DescriptorField(
            "canonical code-object digest",
        ));
    }
    if table.device_target().to_string() != EXACT_TARGET
        || table.code_object_version() != CodeObjectVersion::V6
        || table.compiler().name().as_str() != "rustc-codegen-fe2o3"
        || table.compiler().release().as_str() != env!("CARGO_PKG_VERSION")
        || table.compiler().commit() != &[0; 20]
        || table.producer().name().as_str() != "rustc-codegen-fe2o3-worker-v2"
        || table.producer().version().as_str() != SLICE1_PRODUCER_VERSION
    {
        return Err(ExactLdsGemmProfileAdmissionErrorV1::DescriptorField(
            "target, COV6, compiler, or producer identity",
        ));
    }
    let [kernel] = table.kernels() else {
        return Err(ExactLdsGemmProfileAdmissionErrorV1::DescriptorField(
            "one-kernel closure",
        ));
    };
    if kernel.logical_name().as_str() != SLICE1_LOGICAL_NAME
        || kernel.entry_name().as_str() != TILED_GEMM_LDS_V1_KERNEL_ID
        || kernel.descriptor_symbol().as_str() != SLICE1_DESCRIPTOR_SYMBOL
        || kernel.source_evidence().identity().as_bytes() == &[0; 32]
        || kernel.source_evidence().digest().as_bytes() == &[0; 32]
    {
        return Err(ExactLdsGemmProfileAdmissionErrorV1::DescriptorField(
            "symbols or source evidence",
        ));
    }
    let expected_evidence = executable_evidence(
        envelope,
        llvm_body,
        kernel.kernel_id().as_bytes(),
        TILED_GEMM_LDS_V1_KERNEL_ID,
    );
    if kernel.executable_ir_evidence() != expected_evidence {
        return Err(ExactLdsGemmProfileAdmissionErrorV1::ExecutableEvidenceMismatch);
    }
    if kernel.capabilities()
        != [
            CapabilityV1::Subgroup,
            CapabilityV1::WorkgroupMemory,
            CapabilityV1::MatrixMultiply,
            CapabilityV1::AmdWave,
            CapabilityV1::AmdMfma,
        ]
    {
        return Err(ExactLdsGemmProfileAdmissionErrorV1::DescriptorField(
            "capability closure",
        ));
    }
    let abi = kernel.abi_layout();
    let launch = kernel.launch();
    let BlockSizeV1::Exact(block) = launch.block_size() else {
        return Err(ExactLdsGemmProfileAdmissionErrorV1::DescriptorField(
            "workgroup geometry",
        ));
    };
    if abi.explicit_argument_size() != 48
        || abi.kernarg_segment_size() != 304
        || abi.kernarg_segment_alignment() != 8
        || launch.rank() != 1
        || (block.x(), block.y(), block.z()) != (TILED_GEMM_LDS_V1_LANES, 1, 1)
        || (
            launch.max_grid().x(),
            launch.max_grid().y(),
            launch.max_grid().z(),
        ) != (1, 1, 1)
        || launch.max_flat_workgroup_size() != TILED_GEMM_LDS_V1_LANES
        || launch.static_shared_memory_bytes() != TILED_GEMM_LDS_V1_STATIC_LDS_BYTES
        || launch.max_dynamic_shared_memory_bytes() != 0
    {
        return Err(ExactLdsGemmProfileAdmissionErrorV1::DescriptorField(
            "ABI, grid, WG64, or LDS contract",
        ));
    }
    let [a, b, c] = kernel.arguments() else {
        return Err(ExactLdsGemmProfileAdmissionErrorV1::DescriptorField(
            "A/B/C argument closure",
        ));
    };
    for (index, argument) in [a, b, c].into_iter().enumerate() {
        let (source_descriptor, layout_descriptor, pointer_offset) = match index {
            0 => (
                SourceTypeDescriptorV1::shared_slice(ScalarTypeV1::U16),
                DeviceLayoutDescriptorV1::shared_slice(ScalarTypeV1::U16),
                0,
            ),
            1 => (
                SourceTypeDescriptorV1::shared_slice(ScalarTypeV1::U16),
                DeviceLayoutDescriptorV1::shared_slice(ScalarTypeV1::U16),
                16,
            ),
            2 => (
                SourceTypeDescriptorV1::disjoint_slice(ScalarTypeV1::F32),
                DeviceLayoutDescriptorV1::disjoint_slice(ScalarTypeV1::F32),
                32,
            ),
            _ => unreachable!("exact three-argument closure"),
        };
        let exact_source = table.type_records().iter().any(|record| {
            record.identity() == argument.source_type() && record.descriptor() == &source_descriptor
        });
        let exact_layout = table.layout_records().iter().any(|record| {
            record.identity() == argument.device_layout()
                && record.descriptor() == &layout_descriptor
        });
        let exact_effect = if index < 2 {
            argument.ownership() == OwnershipSemantics::SharedBorrow
                && argument.access() == AccessMode::ReadOnly
                && argument.alias() == AliasSemantics::SharedReadOnly
        } else {
            argument.ownership() == OwnershipSemantics::UniqueBorrow
                && argument.access() == AccessMode::ReadWrite
                && argument.alias() == AliasSemantics::Exclusive
        };
        if argument.source_index() != index as u16
            || argument.name().as_str() != format!("arg{index}")
            || !exact_source
            || !exact_layout
            || !exact_effect
            || argument.physical_components().collect::<Vec<_>>()
                != [
                    (
                        PhysicalAbiComponentKind::GlobalPointer,
                        pointer_offset,
                        8,
                        8,
                    ),
                    (
                        PhysicalAbiComponentKind::SliceLengthU64,
                        pointer_offset + 8,
                        8,
                        8,
                    ),
                ]
        {
            return Err(ExactLdsGemmProfileAdmissionErrorV1::DescriptorField(
                "A/B/C type, effect, alias, or physical ABI",
            ));
        }
    }
    if table.type_records().len() != 2 || table.layout_records().len() != 2 {
        return Err(ExactLdsGemmProfileAdmissionErrorV1::DescriptorField(
            "type/layout record closure",
        ));
    }
    Ok(())
}

fn executable_evidence(
    envelope: &fe2o3_compiler_ffi::CompilerFfiEnvelopeV1,
    llvm_body: &str,
    binding: &[u8; 32],
    entry: &str,
) -> BuildEvidenceV1 {
    let envelope_identity = envelope.identity().as_bytes();
    let target = envelope.target().to_string();
    BuildEvidenceV1::new(
        EvidenceIdentity::from_opaque_bytes(domain_hash(
            DESCRIPTOR_IR_IDENTITY_DOMAIN_V1,
            &[
                binding.as_slice(),
                envelope_identity.as_slice(),
                target.as_bytes(),
                entry.as_bytes(),
            ],
        )),
        EvidenceDigest::from_sha256_bytes(domain_hash(
            DESCRIPTOR_IR_DIGEST_DOMAIN_V1,
            &[
                envelope.canonical_bytes(),
                llvm_body.as_bytes(),
                entry.as_bytes(),
            ],
        )),
    )
}

fn validate_resource_transcript(
    transcript: &[u8],
    authority: &[u8; 32],
    canonical_ir: &[u8; 32],
    descriptor: &[u8; 32],
) -> Result<(), ExactLdsGemmProfileAdmissionErrorV1> {
    let mut cursor = TranscriptCursor::new(transcript);
    let geometry = [64, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0];
    let expected: [&[u8]; 12] = [
        RESOURCE_TRANSCRIPT_DOMAIN_V1,
        authority,
        EXACT_TARGET.as_bytes(),
        &6u16.to_le_bytes(),
        canonical_ir,
        descriptor,
        &geometry,
        &0u32.to_le_bytes(),
        &TILED_GEMM_LDS_V1_STATIC_LDS_BYTES.to_le_bytes(),
        &TILED_GEMM_LDS_V1_ALLOCATION_COUNT.to_le_bytes(),
        &TILED_GEMM_LDS_V1_TILE_BYTES.to_le_bytes(),
        &TILED_GEMM_LDS_V1_LDS_ALIGNMENT.to_le_bytes(),
    ];
    for field in expected {
        if cursor.field() != Some(field) {
            return Err(ExactLdsGemmProfileAdmissionErrorV1::ResourceTranscriptMismatch);
        }
    }
    if !cursor.finished() {
        return Err(ExactLdsGemmProfileAdmissionErrorV1::ResourceTranscriptMismatch);
    }
    Ok(())
}

struct TranscriptCursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> TranscriptCursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn field(&mut self) -> Option<&'a [u8]> {
        let length_bytes: [u8; 8] = self
            .bytes
            .get(self.offset..self.offset + 8)?
            .try_into()
            .ok()?;
        self.offset += 8;
        let length = usize::try_from(u64::from_le_bytes(length_bytes)).ok()?;
        let end = self.offset.checked_add(length)?;
        let field = self.bytes.get(self.offset..end)?;
        self.offset = end;
        Some(field)
    }

    fn finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

fn canonical_ir_commitment(bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    hash_field(&mut digest, CANONICAL_IR_DOMAIN_V1);
    hash_field(&mut digest, bytes);
    digest.finalize().into()
}

#[allow(clippy::too_many_arguments)]
fn import_identity(
    profile: ExactLdsGemmProfileIdentityV1,
    kernel_ir: &[u8],
    llvm_body: &[u8],
    descriptor: &[u8],
    envelope: &[u8],
    manifest: &[u8],
    authority: &[u8],
    resources: &[u8],
    handoff: &[u8],
) -> InspectedExactLdsGemmCompilerImportIdentityV1 {
    let mut digest = Sha256::new();
    hash_field(&mut digest, IMPORT_IDENTITY_DOMAIN_V1);
    for field in [
        profile.as_bytes().as_slice(),
        kernel_ir,
        llvm_body,
        descriptor,
        envelope,
        manifest,
        authority,
        resources,
        handoff,
    ] {
        hash_field(&mut digest, field);
    }
    InspectedExactLdsGemmCompilerImportIdentityV1(digest.finalize().into())
}

fn domain_hash(domain: &[u8], frames: &[&[u8]]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((frames.len() as u64).to_le_bytes());
    for frame in frames {
        hash_field(&mut digest, frame);
    }
    digest.finalize().into()
}

fn hash_field(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_enables_only_slice1() {
        assert_eq!(
            exact_lds_gemm_profile_availability_v1(ExactLdsGemmProfileIdV1::Slice1M16N16K16),
            ExactLdsGemmProfileAvailabilityV1::Enabled
        );
        for profile in [
            ExactLdsGemmProfileIdV1::KPhaseM16N16K32,
            ExactLdsGemmProfileIdV1::GridM64N48K16,
            ExactLdsGemmProfileIdV1::EdgesM17N19K18,
        ] {
            assert_eq!(
                exact_lds_gemm_profile_availability_v1(profile),
                ExactLdsGemmProfileAvailabilityV1::Reserved
            );
        }
    }

    #[test]
    fn exact_contract_has_role_separated_length_identities() {
        let contract = exact_slice1_contract();
        let [a, b, c] = contract.buffers();
        assert_eq!(contract.grid(), [1, 1, 1]);
        assert_eq!(contract.workgroup(), [64, 1, 1]);
        assert_eq!(contract.static_lds_bytes(), 1024);
        assert_eq!((a.elements(), b.elements(), c.elements()), (256, 256, 256));
        assert_ne!(a.length_identity(), b.length_identity());
        assert_ne!(b.length_identity(), c.length_identity());
    }
}
