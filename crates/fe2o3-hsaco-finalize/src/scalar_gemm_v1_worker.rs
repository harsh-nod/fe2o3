//! Exact Worker V2 validation for the scalar GEMM V1 profile.
//!
//! This module starts at an already sealed Worker V2 exchange. It does not
//! authenticate the rustc frontend that selected the canonical Kernel IR.

use std::{error::Error, fmt};

use dialect_amdgcn::{ScalarGemmLoweringErrorV1, lower_scalar_gemm_v1_to_gfx942_llvm_ir};
use fe2o3_compiler_ffi::{
    COMPILER_DESCRIPTOR_SECTION_NAME_V1, CompilerDescriptorSourceV1, CompilerFfiEnvelopeV1,
    CompilerModuleSymbolManifestV1, CompilerModuleSymbolRoleV1,
};
use fe2o3_kernel_descriptor::{
    AccessMode, AliasSemantics, BlockSizeV1, CapabilityV1, CodeObjectVersion, DeviceTargetV1,
    KernelDescriptorV1, OwnershipSemantics, PhysicalAbiComponentKind, ScalarTypeV1,
};
use fe2o3_kernel_ir::{
    SCALAR_GEMM_V1_KERNEL_ID, ScalarGemmTargetRequirementsV1, scalar_gemm_v1_module,
};
use sha2::{Digest, Sha256};

use crate::{
    ContentIdentityV1, InertDecodedWorkerExchangeV2, InertFirstBuildWorkerV2EvidenceV1,
    InspectedRawWorkerV2HsacoV1, WorkerCompilerFfiEnvelopeIdentityV2, WorkerInputKindV1,
    WorkerOptimizationLevelV1, WorkerOptionsV1, WorkerProtocolError, WorkerRequestV2,
    WorkerResponseV2, WorkerStageV1, WorkerV2RawHsacoInspectionError,
    inspect_worker_v2_raw_hsaco_v1,
};

const SCALAR_GEMM_V1_TARGET: &str = "gfx942:xnack-";
const SCALAR_GEMM_V1_DESCRIPTOR: &str = "scalar_gemm_v1.kd";
const SCALAR_GEMM_V1_EXPLICIT_KERNARG_BYTES: u64 = 64;
const SCALAR_GEMM_V1_TOTAL_KERNARG_BYTES: u64 = 320;
const SCALAR_GEMM_V1_FRONTEND_AUTHORITY_SECTION: &str = ".fe2o3.scalar-auth.v1";
const SCALAR_GEMM_V1_FRONTEND_AUTHORITY_BYTES: usize = 32;
const SCALAR_GEMM_V1_KERNEL_BINDING: [u8; 32] = [
    0x78, 0x9a, 0xde, 0xdf, 0xdc, 0x3b, 0xe1, 0xfb, 0x60, 0x51, 0x8d, 0xd2, 0xc7, 0x46, 0x0c, 0x3e,
    0xf8, 0xe6, 0xb9, 0x00, 0x52, 0x7d, 0x1b, 0xcb, 0x22, 0x89, 0xba, 0xa1, 0xe0, 0x14, 0x69, 0x3e,
];
const SCALAR_GEMM_V1_SUCCESS_DIAGNOSTICS: [&str; 5] = [
    "post_link.check=exports status=ok symbols=[scalar_gemm_v1,scalar_gemm_v1.kd]",
    "post_link.check=metadata status=ok kernels=1 target=amdgcn-amd-amdhsa--gfx942%3Axnack-",
    "post_link.check=target status=ok arch=gfx942 code_object_version=6 e_flags=0x64c",
    "post_link.check=unresolved status=ok symbols=[]",
    "post_link.kernel name=scalar_gemm_v1 symbol=scalar_gemm_v1.kd kernarg_size=320 group_size=0 private_size=0 kernarg_align=8 wavefront_size=64 max_workgroup_size=256 reqd_workgroup_size=[256,1,1]",
];
const EXCHANGE_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/SCALAR-GEMM-V1/WORKER-EXCHANGE/V1\0";

/// Identity of one exact request/response exchange admitted by the scalar GEMM V1 worker profile.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ScalarGemmV1WorkerExchangeIdentityV1([u8; 32]);

impl ScalarGemmV1WorkerExchangeIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Inert validation of the exact scalar GEMM V1 Worker V2 request and response.
///
/// This proves that the request contains the canonical lowering, one exact
/// scalar descriptor profile, one embedded nonzero frontend commitment, and a
/// response bound to that request. It does not inspect the output as a code
/// object and does not authenticate the producer of the frontend commitment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidatedScalarGemmV1WorkerExchangeV1 {
    identity: ScalarGemmV1WorkerExchangeIdentityV1,
    compiler_module: ContentIdentityV1,
    linked_output: ContentIdentityV1,
    embedded_frontend_authority_commitment: [u8; 32],
}

impl ValidatedScalarGemmV1WorkerExchangeV1 {
    pub const fn identity(&self) -> ScalarGemmV1WorkerExchangeIdentityV1 {
        self.identity
    }

    pub const fn compiler_module_identity(&self) -> ContentIdentityV1 {
        self.compiler_module
    }

    pub const fn linked_output_identity(&self) -> ContentIdentityV1 {
        self.linked_output
    }

    /// Returns the frontend commitment observed inside the exact compiler module.
    ///
    /// This is lineage data, not proof that the frontend was trusted. A later protected compiler
    /// transaction must authenticate the producer and compare this exact value.
    pub const fn embedded_frontend_authority_commitment(&self) -> &[u8; 32] {
        &self.embedded_frontend_authority_commitment
    }

    pub const fn requested_code_object_version(&self) -> CodeObjectVersion {
        CodeObjectVersion::V6
    }

    pub const fn code_object_version_was_inspected(&self) -> bool {
        false
    }

    pub const fn authenticates_frontend_origin(&self) -> bool {
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
}

/// Inert scalar GEMM V1 evidence whose exact Worker V2 output passed raw-HSACO inspection.
///
/// COV6, `gfx942:xnack-`, the one exact kernel/descriptor pair, workgroup size,
/// and wavefront size have been observed in the raw artifact. This still grants
/// no publication, loading, or launch authority.
#[derive(Debug, Eq, PartialEq)]
pub struct InspectedScalarGemmV1WorkerV2HsacoV1 {
    exchange: ValidatedScalarGemmV1WorkerExchangeV1,
    raw: InspectedRawWorkerV2HsacoV1,
}

impl InspectedScalarGemmV1WorkerV2HsacoV1 {
    pub const fn exchange(&self) -> ValidatedScalarGemmV1WorkerExchangeV1 {
        self.exchange
    }

    pub const fn target(&self) -> DeviceTargetV1 {
        self.raw.target()
    }

    pub const fn code_object_version(&self) -> CodeObjectVersion {
        self.raw.code_object_version()
    }

    pub const fn code_object_version_was_inspected(&self) -> bool {
        true
    }

    pub fn exact_bytes(&self) -> &[u8] {
        self.raw.exact_bytes()
    }

    pub const fn raw_inspection(&self) -> &InspectedRawWorkerV2HsacoV1 {
        &self.raw
    }

    /// Consumes exact scalar-profile inspection and transfers its retained raw-HSACO lineage.
    ///
    /// This is the only ownership bridge into generic Worker V2 finalization. It does not create
    /// authority from bytes, and the consumed scalar capability cannot be reused.
    pub fn into_raw(self) -> InspectedRawWorkerV2HsacoV1 {
        self.raw
    }

    pub const fn authenticates_frontend_origin(&self) -> bool {
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
}

#[derive(Debug)]
#[non_exhaustive]
pub enum ScalarGemmV1WorkerValidationErrorV1 {
    CanonicalLowering(ScalarGemmLoweringErrorV1),
    WorkerProtocol(WorkerProtocolError),
    ProfileMismatch(&'static str),
    RawHsaco(WorkerV2RawHsacoInspectionError),
}

impl fmt::Display for ScalarGemmV1WorkerValidationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CanonicalLowering(error) => {
                write!(
                    formatter,
                    "canonical scalar GEMM V1 lowering failed: {error}"
                )
            }
            Self::WorkerProtocol(error) => {
                write!(
                    formatter,
                    "scalar GEMM V1 Worker V2 exchange is invalid: {error}"
                )
            }
            Self::ProfileMismatch(field) => {
                write!(
                    formatter,
                    "scalar GEMM V1 Worker V2 profile mismatch: {field}"
                )
            }
            Self::RawHsaco(error) => {
                write!(
                    formatter,
                    "scalar GEMM V1 raw-HSACO inspection failed: {error}"
                )
            }
        }
    }
}

impl Error for ScalarGemmV1WorkerValidationErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CanonicalLowering(error) => Some(error),
            Self::WorkerProtocol(error) => Some(error),
            Self::RawHsaco(error) => Some(error),
            Self::ProfileMismatch(_) => None,
        }
    }
}

/// Validates the exact scalar GEMM V1 module and sealed Worker V2 exchange.
///
/// The canonical Kernel IR and LLVM input are reconstructed internally. No
/// symbol, target, LLVM module, provider, option, or output identity is accepted
/// from the caller. The retained first-build evidence remains inert.
pub fn validate_scalar_gemm_v1_worker_exchange_v1(
    source: &InertFirstBuildWorkerV2EvidenceV1,
) -> Result<ValidatedScalarGemmV1WorkerExchangeV1, ScalarGemmV1WorkerValidationErrorV1> {
    let exchange = InertDecodedWorkerExchangeV2::decode(
        source.authorized_request_bytes(),
        source.authorized().response().canonical_bytes(),
    )
    .map_err(ScalarGemmV1WorkerValidationErrorV1::WorkerProtocol)?;
    let expected_envelope = exact_compiler_envelope()?;
    let expected_manifest = exact_symbol_manifest()?;
    let validated = validate_exchange_parts(&exchange, &expected_envelope, &expected_manifest)?;

    if source.compiler_envelope() != &expected_envelope {
        return Err(profile_mismatch("compiler FFI envelope"));
    }
    if source.symbol_manifest() != &expected_manifest {
        return Err(profile_mismatch("compiler symbol manifest"));
    }
    if source.plan().target() != exchange.request().target() {
        return Err(profile_mismatch("link-plan target"));
    }
    if source.worker_measurement().executable() != exchange.request().worker_executable()
        || source.worker_measurement().worker_build_identity()
            != exchange.request().worker_build_identity()
        || source.worker_measurement().llvm_build_identity()
            != exchange.request().llvm_build_identity()
    {
        return Err(profile_mismatch("measured Worker V2 identity"));
    }
    if source.output_identity() != validated.linked_output_identity()
        || !source.output_identity().matches(source.output_bytes())
    {
        return Err(profile_mismatch("linked output identity"));
    }
    Ok(validated)
}

/// Validates the exact exchange, then consumes it through raw-HSACO inspection.
///
/// This is the first scalar GEMM V1 API in this module that reports COV6 as an
/// observed artifact property. It deliberately exposes no HSA launch operation.
pub fn inspect_scalar_gemm_v1_worker_v2_hsaco_v1(
    source: InertFirstBuildWorkerV2EvidenceV1,
) -> Result<InspectedScalarGemmV1WorkerV2HsacoV1, ScalarGemmV1WorkerValidationErrorV1> {
    let exchange = validate_scalar_gemm_v1_worker_exchange_v1(&source)?;
    let raw = inspect_worker_v2_raw_hsaco_v1(source)
        .map_err(ScalarGemmV1WorkerValidationErrorV1::RawHsaco)?;
    let expected_target = exact_target();
    if raw.target() != expected_target {
        return Err(profile_mismatch("inspected target"));
    }
    if raw.code_object_version() != CodeObjectVersion::V6 {
        return Err(profile_mismatch("inspected code-object version"));
    }
    if raw.policy().observed_kernels().len() != 1
        || raw.policy().observed_kernels()[0].entry() != SCALAR_GEMM_V1_KERNEL_ID
        || raw.policy().observed_kernels()[0].descriptor() != SCALAR_GEMM_V1_DESCRIPTOR
    {
        return Err(profile_mismatch("inspected kernel symbol pair"));
    }
    validate_scalar_gemm_v1_kernarg_layout(raw.exact_bytes())?;
    Ok(InspectedScalarGemmV1WorkerV2HsacoV1 { exchange, raw })
}

fn validate_scalar_gemm_v1_kernarg_layout(
    bytes: &[u8],
) -> Result<(), ScalarGemmV1WorkerValidationErrorV1> {
    let inspected = fe2o3_hsaco::inspect(bytes)
        .map_err(|_| profile_mismatch("inspected scalar GEMM metadata"))?;
    let [kernel] = inspected.kernels() else {
        return Err(profile_mismatch("inspected scalar GEMM kernel count"));
    };
    if kernel.kernarg_segment_size() != SCALAR_GEMM_V1_TOTAL_KERNARG_BYTES
        || kernel.kernarg_segment_alignment() != 8
        || kernel.implicit_argument_offset() != Some(SCALAR_GEMM_V1_EXPLICIT_KERNARG_BYTES)
        || kernel.implicit_argument_size()
            != SCALAR_GEMM_V1_TOTAL_KERNARG_BYTES - SCALAR_GEMM_V1_EXPLICIT_KERNARG_BYTES
    {
        return Err(profile_mismatch("inspected scalar GEMM kernarg span"));
    }

    const EXPLICIT_FIELDS: [(u64, u64); 9] = [
        (0, 8),
        (8, 8),
        (16, 8),
        (24, 8),
        (32, 8),
        (40, 8),
        (48, 4),
        (52, 4),
        (56, 4),
    ];
    let actual_fields = kernel
        .explicit_arguments()
        .iter()
        .map(|argument| (argument.offset(), argument.size()))
        .collect::<Vec<_>>();
    if actual_fields != EXPLICIT_FIELDS {
        return Err(profile_mismatch("inspected scalar GEMM explicit ABI"));
    }
    Ok(())
}

fn validate_exchange_parts(
    exchange: &InertDecodedWorkerExchangeV2,
    expected_envelope: &CompilerFfiEnvelopeV1,
    expected_manifest: &CompilerModuleSymbolManifestV1,
) -> Result<ValidatedScalarGemmV1WorkerExchangeV1, ScalarGemmV1WorkerValidationErrorV1> {
    let request = exchange.request();
    let response = exchange.response();
    let embedded_frontend_authority_commitment =
        validate_request(request, expected_envelope, expected_manifest)?;
    validate_response(request, response)?;
    let output = response
        .output()
        .ok_or_else(|| profile_mismatch("completed response output"))?;
    let identity = calculate_exchange_identity(request, response);
    Ok(ValidatedScalarGemmV1WorkerExchangeV1 {
        identity,
        compiler_module: request.compiler_module().identity(),
        linked_output: output.identity(),
        embedded_frontend_authority_commitment,
    })
}

fn validate_request(
    request: &WorkerRequestV2,
    expected_envelope: &CompilerFfiEnvelopeV1,
    expected_manifest: &CompilerModuleSymbolManifestV1,
) -> Result<[u8; 32], ScalarGemmV1WorkerValidationErrorV1> {
    let canonical = lower_scalar_gemm_v1_to_gfx942_llvm_ir(
        &scalar_gemm_v1_module(),
        ScalarGemmTargetRequirementsV1::gfx942_xnack_minus_cov6(),
    )
    .map_err(ScalarGemmV1WorkerValidationErrorV1::CanonicalLowering)?;
    if request.target() != exact_target() {
        return Err(profile_mismatch("request target"));
    }
    if request.code_object_version() != CodeObjectVersion::V6 {
        return Err(profile_mismatch("requested code-object version"));
    }
    if request.options() != WorkerOptionsV1::new(WorkerOptimizationLevelV1::O0, true, true) {
        return Err(profile_mismatch("worker options"));
    }
    if request.compiler_envelope_identity()
        != WorkerCompilerFfiEnvelopeIdentityV2::from_compiler_identity(expected_envelope.identity())
    {
        return Err(profile_mismatch("request compiler-envelope identity"));
    }
    if expected_manifest != &exact_symbol_manifest()? {
        return Err(profile_mismatch("expected compiler symbol manifest"));
    }
    if request.compiler_module().kind() != WorkerInputKindV1::LlvmTextIr {
        return Err(profile_mismatch("compiler-module input kind"));
    }
    let embedded_frontend_authority_commitment = validate_authenticated_compiler_module(
        request.compiler_module().bytes(),
        canonical.as_str().as_bytes(),
    )?;
    if !request.external_providers().is_empty() {
        return Err(profile_mismatch("external provider closure"));
    }
    if !request.import_symbols().is_empty() || !request.export_symbols().is_empty() {
        return Err(profile_mismatch("device FFI symbol closure"));
    }
    if request.final_symbols() != [SCALAR_GEMM_V1_KERNEL_ID, SCALAR_GEMM_V1_DESCRIPTOR] {
        return Err(profile_mismatch("final symbol closure"));
    }
    Ok(embedded_frontend_authority_commitment)
}

fn validate_authenticated_compiler_module(
    module: &[u8],
    canonical_lowering: &[u8],
) -> Result<[u8; 32], ScalarGemmV1WorkerValidationErrorV1> {
    let suffix = module
        .strip_prefix(canonical_lowering)
        .ok_or_else(|| profile_mismatch("canonical compiler-module prefix"))?;
    let descriptor_header = module_assembly_section_header(COMPILER_DESCRIPTOR_SECTION_NAME_V1);
    let authority_header =
        module_assembly_section_header(SCALAR_GEMM_V1_FRONTEND_AUTHORITY_SECTION);
    let suffix = suffix
        .strip_prefix(descriptor_header.as_bytes())
        .ok_or_else(|| profile_mismatch("compiler descriptor section header"))?;
    let (descriptor_text, authority_text) =
        split_once_exact(suffix, authority_header.as_bytes())
            .ok_or_else(|| profile_mismatch("frontend-authority section closure"))?;
    let descriptor_bytes = decode_module_assembly_bytes(descriptor_text)
        .ok_or_else(|| profile_mismatch("compiler descriptor section encoding"))?;
    let descriptor = CompilerDescriptorSourceV1::decode(&descriptor_bytes)
        .map_err(|_| profile_mismatch("canonical compiler descriptor source"))?;
    validate_scalar_gemm_v1_descriptor_source(&descriptor)?;

    let authority = decode_module_assembly_bytes(authority_text)
        .ok_or_else(|| profile_mismatch("frontend-authority section encoding"))?;
    let authority: [u8; SCALAR_GEMM_V1_FRONTEND_AUTHORITY_BYTES] = authority
        .try_into()
        .map_err(|_| profile_mismatch("frontend-authority commitment size"))?;
    if authority == [0; SCALAR_GEMM_V1_FRONTEND_AUTHORITY_BYTES] {
        return Err(profile_mismatch("frontend-authority commitment"));
    }
    Ok(authority)
}

fn module_assembly_section_header(section: &str) -> String {
    format!("\nmodule asm \".section {section},\\22\\22,@progbits\"\nmodule asm \".balign 8\"\n")
}

fn split_once_exact<'a>(bytes: &'a [u8], delimiter: &[u8]) -> Option<(&'a [u8], &'a [u8])> {
    let position = bytes
        .windows(delimiter.len())
        .position(|window| window == delimiter)?;
    Some((&bytes[..position], &bytes[position + delimiter.len()..]))
}

fn decode_module_assembly_bytes(encoded: &[u8]) -> Option<Vec<u8>> {
    const PREFIX: &[u8] = b"module asm \".byte ";
    const SUFFIX: &[u8] = b"\"\n";

    if encoded.is_empty() {
        return None;
    }
    let mut remaining = encoded;
    let mut result = Vec::new();
    while !remaining.is_empty() {
        let end = remaining
            .windows(SUFFIX.len())
            .position(|window| window == SUFFIX)?;
        let line_end = end.checked_add(SUFFIX.len())?;
        let line = &remaining[..line_end];
        let values = line.strip_prefix(PREFIX)?.strip_suffix(SUFFIX)?;
        let mut count = 0usize;
        for value in values.split(|byte| *byte == b',') {
            let value = if count == 0 {
                value
            } else {
                value.strip_prefix(b" ")?
            };
            let [b'0', b'x', high, low] = value else {
                return None;
            };
            result.push((decode_hex_nibble(*high)? << 4) | decode_hex_nibble(*low)?);
            count += 1;
        }
        if count == 0 || count > 16 || (line_end != remaining.len() && count != 16) {
            return None;
        }
        remaining = &remaining[line_end..];
    }
    Some(result)
}

const fn decode_hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

fn validate_scalar_gemm_v1_descriptor_source(
    source: &CompilerDescriptorSourceV1,
) -> Result<(), ScalarGemmV1WorkerValidationErrorV1> {
    let table = source.table();
    let [kernel] = table.kernels() else {
        return Err(profile_mismatch("compiler descriptor kernel count"));
    };
    if table.code_object_version() != CodeObjectVersion::V6
        || table.device_target() != exact_target()
        || table.canonical_code_object_digest().as_bytes() != &[0; 32]
        || table.compiler().name().as_str() != "rustc-codegen-fe2o3"
        || table.producer().name().as_str() != "rustc-codegen-fe2o3-worker-v2"
        || table.producer().version().as_str() != "typed-general-gfx942-cov6-v1"
    {
        return Err(profile_mismatch("compiler descriptor profile"));
    }
    validate_scalar_gemm_v1_kernel_descriptor_v1(kernel)
}

/// Validates the exact compiler-generated scalar GEMM V1 descriptor profile.
///
/// This is intentionally narrower than generic descriptor validation: it fixes the kernel
/// identity, exported names, capabilities, physical ABI, launch constraints, argument ownership,
/// access modes, aliasing, and every physical kernarg component used by the production scalar
/// Worker V3 verifier.
pub fn validate_scalar_gemm_v1_kernel_descriptor_v1(
    kernel: &KernelDescriptorV1,
) -> Result<(), ScalarGemmV1WorkerValidationErrorV1> {
    if kernel.kernel_id().as_bytes() != &SCALAR_GEMM_V1_KERNEL_BINDING
        || kernel.logical_name().as_str() != SCALAR_GEMM_V1_KERNEL_ID
        || kernel.entry_name().as_str() != SCALAR_GEMM_V1_KERNEL_ID
        || kernel.descriptor_symbol().as_str() != SCALAR_GEMM_V1_DESCRIPTOR
        || kernel.capabilities() != [CapabilityV1::AmdWave]
    {
        return Err(profile_mismatch("compiler descriptor kernel identity"));
    }
    let abi = kernel.abi_layout();
    if abi.explicit_argument_size() != SCALAR_GEMM_V1_EXPLICIT_KERNARG_BYTES as u32
        || abi.kernarg_segment_size() != SCALAR_GEMM_V1_TOTAL_KERNARG_BYTES as u32
        || abi.kernarg_segment_alignment() != 8
    {
        return Err(profile_mismatch("compiler descriptor kernarg layout"));
    }
    let launch = kernel.launch();
    let BlockSizeV1::Exact(block) = launch.block_size() else {
        return Err(profile_mismatch("compiler descriptor launch block"));
    };
    let max_grid = launch.max_grid();
    if launch.rank() != 1
        || [block.x(), block.y(), block.z()] != [256, 1, 1]
        || [max_grid.x(), max_grid.y(), max_grid.z()] != [u32::MAX, 1, 1]
        || launch.max_flat_workgroup_size() != 256
        || launch.static_shared_memory_bytes() != 0
        || launch.max_dynamic_shared_memory_bytes() != 0
    {
        return Err(profile_mismatch("compiler descriptor launch constraints"));
    }
    validate_scalar_gemm_v1_descriptor_arguments(kernel.arguments())
}

fn validate_scalar_gemm_v1_descriptor_arguments(
    arguments: &[fe2o3_kernel_descriptor::LogicalArgumentV1],
) -> Result<(), ScalarGemmV1WorkerValidationErrorV1> {
    const NAMES: [&str; 6] = ["a", "b", "c", "m", "n", "k"];
    const OWNERSHIP: [OwnershipSemantics; 6] = [
        OwnershipSemantics::SharedBorrow,
        OwnershipSemantics::SharedBorrow,
        OwnershipSemantics::UniqueBorrow,
        OwnershipSemantics::ByValue,
        OwnershipSemantics::ByValue,
        OwnershipSemantics::ByValue,
    ];
    const ACCESS: [AccessMode; 6] = [
        AccessMode::ReadOnly,
        AccessMode::ReadOnly,
        AccessMode::ReadWrite,
        AccessMode::ByValue,
        AccessMode::ByValue,
        AccessMode::ByValue,
    ];
    const ALIAS: [AliasSemantics; 6] = [
        AliasSemantics::SharedReadOnly,
        AliasSemantics::SharedReadOnly,
        AliasSemantics::Exclusive,
        AliasSemantics::Value,
        AliasSemantics::Value,
        AliasSemantics::Value,
    ];
    const COMPONENTS: [&[(PhysicalAbiComponentKind, u32, u16, u16)]; 6] = [
        &[
            (PhysicalAbiComponentKind::GlobalPointer, 0, 8, 8),
            (PhysicalAbiComponentKind::SliceLengthU64, 8, 8, 8),
        ],
        &[
            (PhysicalAbiComponentKind::GlobalPointer, 16, 8, 8),
            (PhysicalAbiComponentKind::SliceLengthU64, 24, 8, 8),
        ],
        &[
            (PhysicalAbiComponentKind::GlobalPointer, 32, 8, 8),
            (PhysicalAbiComponentKind::SliceLengthU64, 40, 8, 8),
        ],
        &[(
            PhysicalAbiComponentKind::ScalarByValue(ScalarTypeV1::U32),
            48,
            4,
            4,
        )],
        &[(
            PhysicalAbiComponentKind::ScalarByValue(ScalarTypeV1::U32),
            52,
            4,
            4,
        )],
        &[(
            PhysicalAbiComponentKind::ScalarByValue(ScalarTypeV1::U32),
            56,
            4,
            4,
        )],
    ];
    if arguments.len() != NAMES.len() {
        return Err(profile_mismatch("compiler descriptor argument count"));
    }
    for (index, argument) in arguments.iter().enumerate() {
        if usize::from(argument.source_index()) != index
            || argument.name().as_str() != NAMES[index]
            || argument.ownership() != OWNERSHIP[index]
            || argument.access() != ACCESS[index]
            || argument.alias() != ALIAS[index]
            || argument.physical_components().collect::<Vec<_>>() != COMPONENTS[index]
        {
            return Err(profile_mismatch("compiler descriptor argument ABI"));
        }
    }
    Ok(())
}

fn validate_response(
    request: &WorkerRequestV2,
    response: &WorkerResponseV2,
) -> Result<(), ScalarGemmV1WorkerValidationErrorV1> {
    if !response.binds_request(request) {
        return Err(profile_mismatch("response request binding"));
    }
    if response.stage() != WorkerStageV1::Complete {
        return Err(profile_mismatch("response completion stage"));
    }
    if response.diagnostics().len() != SCALAR_GEMM_V1_SUCCESS_DIAGNOSTICS.len()
        || response
            .diagnostics()
            .iter()
            .zip(SCALAR_GEMM_V1_SUCCESS_DIAGNOSTICS)
            .any(|(actual, expected)| actual != expected)
    {
        return Err(profile_mismatch("completed response post-link diagnostics"));
    }
    let output = response
        .output()
        .ok_or_else(|| profile_mismatch("completed response output"))?;
    if output.request_identity() != request.identity()
        || output.compiler_envelope_identity() != request.compiler_envelope_identity()
        || !output.identity().matches(output.bytes())
        || output.identity().byte_len() != request.output_constraints().max_bytes()
    {
        return Err(profile_mismatch("response output binding"));
    }
    Ok(())
}

fn exact_target() -> DeviceTargetV1 {
    DeviceTargetV1::parse(SCALAR_GEMM_V1_TARGET).expect("fixed scalar GEMM V1 target is valid")
}

fn exact_compiler_envelope() -> Result<CompilerFfiEnvelopeV1, ScalarGemmV1WorkerValidationErrorV1> {
    CompilerFfiEnvelopeV1::for_module_without_device_ffi(exact_target(), CodeObjectVersion::V6)
        .map_err(|_| profile_mismatch("internal empty compiler FFI envelope"))
}

fn exact_symbol_manifest()
-> Result<CompilerModuleSymbolManifestV1, ScalarGemmV1WorkerValidationErrorV1> {
    CompilerModuleSymbolManifestV1::new([
        (
            CompilerModuleSymbolRoleV1::KernelEntry,
            SCALAR_GEMM_V1_KERNEL_ID,
        ),
        (
            CompilerModuleSymbolRoleV1::KernelDescriptor,
            SCALAR_GEMM_V1_DESCRIPTOR,
        ),
    ])
    .map_err(|_| profile_mismatch("internal compiler symbol manifest"))
}

fn calculate_exchange_identity(
    request: &WorkerRequestV2,
    response: &WorkerResponseV2,
) -> ScalarGemmV1WorkerExchangeIdentityV1 {
    let mut digest = Sha256::new();
    digest.update(EXCHANGE_IDENTITY_DOMAIN_V1);
    digest.update((request.canonical_bytes().len() as u64).to_le_bytes());
    digest.update(request.canonical_bytes());
    digest.update((response.canonical_bytes().len() as u64).to_le_bytes());
    digest.update(response.canonical_bytes());
    ScalarGemmV1WorkerExchangeIdentityV1(digest.finalize().into())
}

fn profile_mismatch(field: &'static str) -> ScalarGemmV1WorkerValidationErrorV1 {
    ScalarGemmV1WorkerValidationErrorV1::ProfileMismatch(field)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        WORKER_RESPONSE_MAGIC_V2, WorkerInputV1, WorkerOutputConstraintsV1,
        worker_protocol_v2::SealedWorkerRequestV2Parts,
    };
    use fe2o3_kernel_descriptor::{
        BuildEvidenceV1, CanonicalCodeObjectDigest, CompilerIdentityV1, DeviceDescriptorTableV1,
        DeviceLayoutDescriptorV1, DeviceLayoutRecordV1, DimensionsV1, EvidenceDigest,
        EvidenceIdentity, KernelAbiLayoutV1, KernelDescriptorV1, KernelId, LaunchConstraintsV1,
        LogicalArgumentV1, ProducerIdentityV1, SourceTypeDescriptorV1, SourceTypeRecordV1, Text,
        ValidName,
    };

    fn request_with(
        module: Vec<u8>,
        target: DeviceTargetV1,
        code_object_version: CodeObjectVersion,
        options: WorkerOptionsV1,
        external_providers: Vec<WorkerInputV1>,
        final_symbols: Vec<String>,
        compiler_envelope: WorkerCompilerFfiEnvelopeIdentityV2,
    ) -> WorkerRequestV2 {
        WorkerRequestV2::from_sealed_parts(SealedWorkerRequestV2Parts {
            request_id: [0x11; 32],
            llvm_build_identity: "upstream-llvm-22".to_owned(),
            worker_build_identity: "fe2o3-direct-llvm-lld-worker-v2".to_owned(),
            worker_executable: ContentIdentityV1::from_parts([0x22; 32], 4096),
            target,
            code_object_version,
            options,
            compiler_envelope,
            compiler_module: WorkerInputV1::new(WorkerInputKindV1::LlvmTextIr, module).unwrap(),
            external_providers,
            import_symbols: Vec::new(),
            export_symbols: Vec::new(),
            final_symbols,
            output: WorkerOutputConstraintsV1::new(b"linked-cov6".len() as u64).unwrap(),
        })
        .unwrap()
    }

    fn exact_request() -> WorkerRequestV2 {
        exact_request_with(
            test_descriptor_source().canonical_bytes(),
            &[0xa5; SCALAR_GEMM_V1_FRONTEND_AUTHORITY_BYTES],
        )
    }

    fn exact_request_with(descriptor: &[u8], authority: &[u8]) -> WorkerRequestV2 {
        let canonical = lower_scalar_gemm_v1_to_gfx942_llvm_ir(
            &scalar_gemm_v1_module(),
            ScalarGemmTargetRequirementsV1::gfx942_xnack_minus_cov6(),
        )
        .unwrap();
        let mut module = canonical.as_str().as_bytes().to_vec();
        append_module_assembly_section(
            &mut module,
            COMPILER_DESCRIPTOR_SECTION_NAME_V1,
            descriptor,
        );
        append_module_assembly_section(
            &mut module,
            SCALAR_GEMM_V1_FRONTEND_AUTHORITY_SECTION,
            authority,
        );
        let envelope = exact_compiler_envelope().unwrap();
        request_with(
            module,
            exact_target(),
            CodeObjectVersion::V6,
            WorkerOptionsV1::new(WorkerOptimizationLevelV1::O0, true, true),
            Vec::new(),
            vec![
                SCALAR_GEMM_V1_KERNEL_ID.to_owned(),
                SCALAR_GEMM_V1_DESCRIPTOR.to_owned(),
            ],
            WorkerCompilerFfiEnvelopeIdentityV2::from_compiler_identity(envelope.identity()),
        )
    }

    fn test_descriptor_source() -> CompilerDescriptorSourceV1 {
        let shared_source =
            SourceTypeRecordV1::new(SourceTypeDescriptorV1::shared_slice(ScalarTypeV1::F32));
        let disjoint_source =
            SourceTypeRecordV1::new(SourceTypeDescriptorV1::disjoint_slice(ScalarTypeV1::F32));
        let scalar_source =
            SourceTypeRecordV1::new(SourceTypeDescriptorV1::scalar(ScalarTypeV1::U32));
        let shared_layout =
            DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::shared_slice(ScalarTypeV1::F32));
        let disjoint_layout =
            DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::disjoint_slice(ScalarTypeV1::F32));
        let scalar_layout =
            DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::scalar(ScalarTypeV1::U32));
        let arguments = vec![
            LogicalArgumentV1::shared_slice(0, name("a"), &shared_source, &shared_layout, 0)
                .unwrap(),
            LogicalArgumentV1::shared_slice(1, name("b"), &shared_source, &shared_layout, 16)
                .unwrap(),
            LogicalArgumentV1::disjoint_slice(
                2,
                name("c"),
                &disjoint_source,
                &disjoint_layout,
                AccessMode::ReadWrite,
                32,
            )
            .unwrap(),
            LogicalArgumentV1::scalar(3, name("m"), &scalar_source, &scalar_layout, 48).unwrap(),
            LogicalArgumentV1::scalar(4, name("n"), &scalar_source, &scalar_layout, 52).unwrap(),
            LogicalArgumentV1::scalar(5, name("k"), &scalar_source, &scalar_layout, 56).unwrap(),
        ];
        let kernel = KernelDescriptorV1::new(
            KernelId::from_bytes(SCALAR_GEMM_V1_KERNEL_BINDING),
            name(SCALAR_GEMM_V1_KERNEL_ID),
            name(SCALAR_GEMM_V1_KERNEL_ID),
            name(SCALAR_GEMM_V1_DESCRIPTOR),
            evidence(0x11, 0x12),
            evidence(0x13, 0x14),
            vec![CapabilityV1::AmdWave],
            KernelAbiLayoutV1::new(64, 320, 8).unwrap(),
            LaunchConstraintsV1::new(
                1,
                BlockSizeV1::Exact(DimensionsV1::new(256, 1, 1).unwrap()),
                DimensionsV1::new(u32::MAX, 1, 1).unwrap(),
                256,
                0,
                0,
            )
            .unwrap(),
            arguments,
        )
        .unwrap();
        CompilerDescriptorSourceV1::new(
            DeviceDescriptorTableV1::new(
                CanonicalCodeObjectDigest::from_bytes([0; 32]),
                CodeObjectVersion::V6,
                CompilerIdentityV1::new(text("rustc-codegen-fe2o3"), text("0.1.0"), [0; 20]),
                ProducerIdentityV1::new(
                    text("rustc-codegen-fe2o3-worker-v2"),
                    text("typed-general-gfx942-cov6-v1"),
                ),
                exact_target(),
                vec![shared_source, disjoint_source, scalar_source],
                vec![shared_layout, disjoint_layout, scalar_layout],
                vec![kernel],
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn append_module_assembly_section(module: &mut Vec<u8>, section: &str, bytes: &[u8]) {
        module.extend_from_slice(module_assembly_section_header(section).as_bytes());
        for chunk in bytes.chunks(16) {
            module.extend_from_slice(b"module asm \".byte ");
            for (index, byte) in chunk.iter().copied().enumerate() {
                if index != 0 {
                    module.extend_from_slice(b", ");
                }
                module.extend_from_slice(format!("0x{byte:02x}").as_bytes());
            }
            module.extend_from_slice(b"\"\n");
        }
    }

    fn evidence(identity: u8, digest: u8) -> BuildEvidenceV1 {
        BuildEvidenceV1::new(
            EvidenceIdentity::from_opaque_bytes([identity; 32]),
            EvidenceDigest::from_sha256_bytes([digest; 32]),
        )
    }

    fn name(value: &str) -> ValidName {
        ValidName::new(value).unwrap()
    }

    fn text(value: &str) -> Text {
        Text::new(value).unwrap()
    }

    fn response(request: &WorkerRequestV2, diagnostics: &[&str]) -> Vec<u8> {
        let mut encoded = WORKER_RESPONSE_MAGIC_V2.to_vec();
        push_field(&mut encoded, 1, request.request_id());
        push_field(&mut encoded, 2, request.identity());
        push_field(
            &mut encoded,
            3,
            &request.compiler_envelope_identity().as_bytes(),
        );
        push_field(&mut encoded, 4, request.worker_build_identity().as_bytes());
        push_field(&mut encoded, 5, &[WorkerStageV1::Complete as u8]);
        let mut diagnostic_bytes = Vec::new();
        diagnostic_bytes.extend_from_slice(&(diagnostics.len() as u32).to_le_bytes());
        for diagnostic in diagnostics {
            diagnostic_bytes.extend_from_slice(&(diagnostic.len() as u32).to_le_bytes());
            diagnostic_bytes.extend_from_slice(diagnostic.as_bytes());
        }
        push_field(&mut encoded, 6, &diagnostic_bytes);
        let output_bytes = b"linked-cov6";
        let output_identity = ContentIdentityV1::calculate(output_bytes);
        let mut output = vec![1];
        output.extend_from_slice(output_identity.sha256());
        output.extend_from_slice(&output_identity.byte_len().to_le_bytes());
        output.extend_from_slice(output_bytes);
        push_field(&mut encoded, 7, &output);
        encoded
    }

    fn exchange(request: &WorkerRequestV2, diagnostics: &[&str]) -> InertDecodedWorkerExchangeV2 {
        InertDecodedWorkerExchangeV2::decode(
            request.canonical_bytes(),
            &response(request, diagnostics),
        )
        .unwrap()
    }

    fn success_diagnostics() -> Vec<&'static str> {
        SCALAR_GEMM_V1_SUCCESS_DIAGNOSTICS.to_vec()
    }

    fn push_field(encoded: &mut Vec<u8>, tag: u16, bytes: &[u8]) {
        encoded.extend_from_slice(&tag.to_le_bytes());
        encoded.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        encoded.extend_from_slice(bytes);
    }

    #[test]
    fn exact_canonical_request_and_bound_response_are_admitted_inertly() {
        let request = exact_request();
        let exchange = exchange(&request, &success_diagnostics());
        let validated = validate_exchange_parts(
            &exchange,
            &exact_compiler_envelope().unwrap(),
            &exact_symbol_manifest().unwrap(),
        )
        .unwrap();
        assert_ne!(validated.identity().as_bytes(), &[0; 32]);
        assert_eq!(
            validated.compiler_module_identity(),
            request.compiler_module().identity()
        );
        assert_eq!(
            validated.embedded_frontend_authority_commitment(),
            &[0xa5; SCALAR_GEMM_V1_FRONTEND_AUTHORITY_BYTES]
        );
        assert!(!validated.code_object_version_was_inspected());
        assert!(!validated.authenticates_frontend_origin());
        assert!(!validated.grants_publication_authority());
        assert!(!validated.grants_load_authority());
        assert!(!validated.grants_launch_authority());
    }

    #[test]
    fn near_match_and_untrusted_module_identities_cannot_enter_the_profile() {
        let exact = exact_request();
        let mut near_match = exact.compiler_module().bytes().to_vec();
        near_match.extend_from_slice(b"; same symbol, untrusted extra text\n");
        let request = request_with(
            near_match,
            exact_target(),
            CodeObjectVersion::V6,
            exact.options(),
            Vec::new(),
            exact.final_symbols().to_vec(),
            exact.compiler_envelope_identity(),
        );
        let exchange = exchange(&request, &success_diagnostics());
        assert!(matches!(
            validate_exchange_parts(
                &exchange,
                &exact_compiler_envelope().unwrap(),
                &exact_symbol_manifest().unwrap(),
            ),
            Err(ScalarGemmV1WorkerValidationErrorV1::ProfileMismatch(
                "frontend-authority section encoding"
            ))
        ));
    }

    #[test]
    fn legacy_layout_cannot_enter_the_upstream_llvm_scalar_profile() {
        let exact = exact_request();
        let canonical = std::str::from_utf8(exact.compiler_module().bytes()).unwrap();
        let legacy = canonical.replacen(
            "target datalayout = \"e-m:e-p:",
            "target datalayout = \"e-p:",
            1,
        );
        assert_ne!(legacy.as_bytes(), exact.compiler_module().bytes());
        let request = request_with(
            legacy.into_bytes(),
            exact_target(),
            CodeObjectVersion::V6,
            exact.options(),
            Vec::new(),
            exact.final_symbols().to_vec(),
            exact.compiler_envelope_identity(),
        );
        let exchange = exchange(&request, &success_diagnostics());
        assert!(matches!(
            validate_exchange_parts(
                &exchange,
                &exact_compiler_envelope().unwrap(),
                &exact_symbol_manifest().unwrap(),
            ),
            Err(ScalarGemmV1WorkerValidationErrorV1::ProfileMismatch(
                "canonical compiler-module prefix"
            ))
        ));
    }

    #[test]
    fn descriptor_and_frontend_commitment_substitutions_fail_closed() {
        let mut descriptor = test_descriptor_source().canonical_bytes().to_vec();
        let binding = descriptor
            .windows(SCALAR_GEMM_V1_KERNEL_BINDING.len())
            .position(|window| window == SCALAR_GEMM_V1_KERNEL_BINDING)
            .expect("test descriptor contains scalar binding");
        descriptor[binding] ^= 1;
        let substituted_descriptor = exact_request_with(
            &descriptor,
            &[0xa5; SCALAR_GEMM_V1_FRONTEND_AUTHORITY_BYTES],
        );
        let substituted_exchange = exchange(&substituted_descriptor, &success_diagnostics());
        assert!(matches!(
            validate_exchange_parts(
                &substituted_exchange,
                &exact_compiler_envelope().unwrap(),
                &exact_symbol_manifest().unwrap(),
            ),
            Err(ScalarGemmV1WorkerValidationErrorV1::ProfileMismatch(
                "compiler descriptor kernel identity"
            ))
        ));

        let zero_authority = exact_request_with(
            test_descriptor_source().canonical_bytes(),
            &[0; SCALAR_GEMM_V1_FRONTEND_AUTHORITY_BYTES],
        );
        let exchange = exchange(&zero_authority, &success_diagnostics());
        assert!(matches!(
            validate_exchange_parts(
                &exchange,
                &exact_compiler_envelope().unwrap(),
                &exact_symbol_manifest().unwrap(),
            ),
            Err(ScalarGemmV1WorkerValidationErrorV1::ProfileMismatch(
                "frontend-authority commitment"
            ))
        ));
    }

    #[test]
    fn every_request_profile_substitution_fails_closed() {
        let exact = exact_request();
        let canonical = exact.compiler_module().bytes().to_vec();
        let expected_envelope = exact_compiler_envelope().unwrap();
        let expected_manifest = exact_symbol_manifest().unwrap();
        let provider = WorkerInputV1::new(
            WorkerInputKindV1::LlvmBitcode,
            b"untrusted-provider".to_vec(),
        )
        .unwrap();
        let cases = [
            request_with(
                canonical.clone(),
                DeviceTargetV1::parse("gfx942").unwrap(),
                CodeObjectVersion::V6,
                exact.options(),
                Vec::new(),
                exact.final_symbols().to_vec(),
                exact.compiler_envelope_identity(),
            ),
            request_with(
                canonical.clone(),
                exact_target(),
                CodeObjectVersion::V5,
                exact.options(),
                Vec::new(),
                exact.final_symbols().to_vec(),
                exact.compiler_envelope_identity(),
            ),
            request_with(
                canonical.clone(),
                exact_target(),
                CodeObjectVersion::V6,
                WorkerOptionsV1::new(WorkerOptimizationLevelV1::O1, true, true),
                Vec::new(),
                exact.final_symbols().to_vec(),
                exact.compiler_envelope_identity(),
            ),
            request_with(
                canonical.clone(),
                exact_target(),
                CodeObjectVersion::V6,
                exact.options(),
                vec![provider],
                exact.final_symbols().to_vec(),
                exact.compiler_envelope_identity(),
            ),
            request_with(
                canonical.clone(),
                exact_target(),
                CodeObjectVersion::V6,
                exact.options(),
                Vec::new(),
                vec!["scalar_gemm_v1_alias".to_owned()],
                exact.compiler_envelope_identity(),
            ),
            request_with(
                canonical,
                exact_target(),
                CodeObjectVersion::V6,
                exact.options(),
                Vec::new(),
                exact.final_symbols().to_vec(),
                WorkerCompilerFfiEnvelopeIdentityV2::from_compiler_identity(
                    CompilerFfiEnvelopeV1::for_module_without_device_ffi(
                        DeviceTargetV1::parse("gfx942").unwrap(),
                        CodeObjectVersion::V6,
                    )
                    .unwrap()
                    .identity(),
                ),
            ),
        ];
        for request in cases {
            let exchange = exchange(&request, &success_diagnostics());
            assert!(
                validate_exchange_parts(&exchange, &expected_envelope, &expected_manifest).is_err(),
                "accepted substituted request: {request:?}"
            );
        }
    }

    #[test]
    fn response_diagnostics_and_cross_request_replay_fail_closed() {
        let request = exact_request();
        let with_diagnostic = exchange(&request, &["unexpected warning"]);
        assert!(matches!(
            validate_exchange_parts(
                &with_diagnostic,
                &exact_compiler_envelope().unwrap(),
                &exact_symbol_manifest().unwrap(),
            ),
            Err(ScalarGemmV1WorkerValidationErrorV1::ProfileMismatch(
                "completed response post-link diagnostics"
            ))
        ));

        let mut replay = response(&request, &success_diagnostics());
        replay[14] ^= 1;
        assert!(InertDecodedWorkerExchangeV2::decode(request.canonical_bytes(), &replay).is_err());
    }
}
