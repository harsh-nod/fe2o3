//! Exact direct Worker V2 validation for the row-softmax V1 profile.
//!
//! The profile starts from an out-of-band-pinned rustc handoff, then checks the
//! measured worker exchange and the existing structural HSACO boundary. It
//! does not assign mathematical meaning to OCML or authenticate the origin of
//! the handoff pin.

use std::{error::Error, fmt};

use fe2o3_compiler_ffi::{
    COMPILER_DESCRIPTOR_SECTION_NAME_V1, CompilerDescriptorSourceV1, CompilerFfiEnvelopeV1,
    CompilerModuleHandoffErrorV2, CompilerModuleHandoffV2, CompilerModuleKindV1,
    CompilerModuleSymbolManifestV1, CompilerModuleSymbolRoleV1,
};
use fe2o3_kernel_descriptor::{
    CanonicalCodeObjectDigest, CodeObjectVersion, ROW_SOFTMAX_V1_DESCRIPTOR_SYMBOL,
    ROW_SOFTMAX_V1_ENTRY_NAME, RowSoftmaxV1StructuralDescriptorErrorV1,
    RowSoftmaxV1StructuralDescriptorExpectationV1, admit_row_softmax_v1_structural_descriptor_v1,
};
use sha2::{Digest, Sha256};

use crate::{
    ContentIdentityV1, InertDecodedWorkerExchangeV2, InertFirstBuildWorkerV2EvidenceV1,
    InspectedRowSoftmaxV1StructuralWorkerV2HsacoV1, WorkerCompilerFfiEnvelopeIdentityV2,
    WorkerInputKindV1, WorkerOptimizationLevelV1, WorkerOptionsV1, WorkerProtocolError,
    WorkerRequestV2, WorkerResponseV2, WorkerStageV1,
    inspect_row_softmax_v1_structural_worker_v2_hsaco_v1,
};

const TARGET: &str = "gfx942:xnack-";
const OCML_EXP_F32: &str = "__ocml_exp_f32";
const FRONTEND_AUTHORITY_SECTION: &str = ".fe2o3.row-softmax-auth.v1";
const FRONTEND_AUTHORITY_BYTES: usize = 32;
const MEASURED_OCML_PROVIDER_FILE_COUNT: usize = 4;
const SUCCESS_DIAGNOSTICS: [&str; 6] = [
    "device_library.check=identity status=ok provider=gfx942-ocml-v1 roots=[__ocml_exp_f32] files=4",
    "post_link.check=exports status=ok symbols=[__ocml_exp_f32,row_softmax_v1,row_softmax_v1.kd]",
    "post_link.check=metadata status=ok kernels=1 target=amdgcn-amd-amdhsa--gfx942%3Axnack-",
    "post_link.check=target status=ok arch=gfx942 code_object_version=6 e_flags=0x64c",
    "post_link.check=unresolved status=ok symbols=[]",
    "post_link.kernel name=row_softmax_v1 symbol=row_softmax_v1.kd kernarg_size=288 group_size=0 private_size=0 kernarg_align=8 wavefront_size=64 max_workgroup_size=64 reqd_workgroup_size=[64,1,1]",
];
const EXCHANGE_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/ROW-SOFTMAX-V1/DIRECT-WORKER-EXCHANGE/V1\0";

/// Exact out-of-band pins needed to admit the row-softmax direct-worker path.
///
/// Construction validates the complete canonical handoff digest, exact row
/// compiler descriptor, and embedded frontend commitment. The pin source is
/// still external to this type and is not authenticated by construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RowSoftmaxV1DirectWorkerExpectationV1 {
    handoff_sha256: [u8; 32],
    frontend_authority_commitment: [u8; FRONTEND_AUTHORITY_BYTES],
    descriptor: RowSoftmaxV1StructuralDescriptorExpectationV1,
}

impl RowSoftmaxV1DirectWorkerExpectationV1 {
    /// Creates a row profile from one independently pinned rustc handoff.
    pub fn from_pinned_rustc_handoff(
        handoff: &CompilerModuleHandoffV2,
        expected_handoff_sha256: [u8; 32],
        expected_frontend_authority_commitment: [u8; FRONTEND_AUTHORITY_BYTES],
    ) -> Result<Self, RowSoftmaxV1DirectWorkerErrorV1> {
        if expected_handoff_sha256 == [0; 32]
            || handoff.identity().sha256() != &expected_handoff_sha256
        {
            return Err(profile_mismatch("pinned rustc handoff identity"));
        }
        if expected_frontend_authority_commitment == [0; FRONTEND_AUTHORITY_BYTES] {
            return Err(profile_mismatch("frontend-authority commitment"));
        }
        let descriptor =
            validate_handoff_profile(handoff, &expected_frontend_authority_commitment)?;
        Ok(Self {
            handoff_sha256: expected_handoff_sha256,
            frontend_authority_commitment: expected_frontend_authority_commitment,
            descriptor,
        })
    }

    pub const fn handoff_sha256(&self) -> &[u8; 32] {
        &self.handoff_sha256
    }

    pub const fn frontend_authority_commitment(&self) -> &[u8; 32] {
        &self.frontend_authority_commitment
    }

    pub const fn descriptor_expectation(self) -> RowSoftmaxV1StructuralDescriptorExpectationV1 {
        self.descriptor
    }

    pub const fn authenticates_pin_origin(&self) -> bool {
        false
    }

    pub const fn proves_exp_math_accuracy(&self) -> bool {
        false
    }

    pub const fn proves_functional_softmax(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Identity of one exact row-softmax request/response exchange.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RowSoftmaxV1DirectWorkerExchangeIdentityV1([u8; 32]);

impl RowSoftmaxV1DirectWorkerExchangeIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Inert evidence for the exact row handoff and measured OCML worker exchange.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidatedRowSoftmaxV1DirectWorkerExchangeV1 {
    identity: RowSoftmaxV1DirectWorkerExchangeIdentityV1,
    compiler_module: ContentIdentityV1,
    linked_output: ContentIdentityV1,
    frontend_authority_commitment: [u8; FRONTEND_AUTHORITY_BYTES],
}

impl ValidatedRowSoftmaxV1DirectWorkerExchangeV1 {
    pub const fn identity(&self) -> RowSoftmaxV1DirectWorkerExchangeIdentityV1 {
        self.identity
    }

    pub const fn compiler_module_identity(&self) -> ContentIdentityV1 {
        self.compiler_module
    }

    pub const fn linked_output_identity(&self) -> ContentIdentityV1 {
        self.linked_output
    }

    pub const fn embedded_frontend_authority_commitment(&self) -> &[u8; 32] {
        &self.frontend_authority_commitment
    }

    pub const fn measured_gfx942_ocml_provider_closure_was_checked(&self) -> bool {
        true
    }

    pub const fn measured_ocml_provider_file_count(&self) -> usize {
        MEASURED_OCML_PROVIDER_FILE_COUNT
    }

    pub const fn requested_ocml_import(&self) -> &'static str {
        OCML_EXP_F32
    }

    pub const fn authenticates_frontend_origin(&self) -> bool {
        false
    }

    pub const fn proves_exp_math_accuracy(&self) -> bool {
        false
    }

    pub const fn proves_functional_softmax(&self) -> bool {
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

/// Exact row Worker V2 exchange joined to the existing structural HSACO check.
#[derive(Debug)]
pub struct InspectedRowSoftmaxV1DirectWorkerHsacoV1 {
    exchange: ValidatedRowSoftmaxV1DirectWorkerExchangeV1,
    structural: InspectedRowSoftmaxV1StructuralWorkerV2HsacoV1,
}

impl InspectedRowSoftmaxV1DirectWorkerHsacoV1 {
    pub const fn exchange(&self) -> ValidatedRowSoftmaxV1DirectWorkerExchangeV1 {
        self.exchange
    }

    pub const fn structural(&self) -> &InspectedRowSoftmaxV1StructuralWorkerV2HsacoV1 {
        &self.structural
    }

    /// Transfers the already admitted structural capability into canonical finalization.
    pub fn into_structural(self) -> InspectedRowSoftmaxV1StructuralWorkerV2HsacoV1 {
        self.structural
    }

    pub const fn proves_exp_math_accuracy(&self) -> bool {
        false
    }

    pub const fn proves_functional_softmax(&self) -> bool {
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
pub enum RowSoftmaxV1DirectWorkerErrorV1 {
    Handoff(CompilerModuleHandoffErrorV2),
    WorkerProtocol(WorkerProtocolError),
    Descriptor(RowSoftmaxV1StructuralDescriptorErrorV1),
    ProfileMismatch(&'static str),
    Structural(crate::RowSoftmaxV1StructuralArtifactErrorV1),
}

impl fmt::Display for RowSoftmaxV1DirectWorkerErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Handoff(error) => write!(formatter, "invalid row-softmax handoff: {error}"),
            Self::WorkerProtocol(error) => {
                write!(formatter, "invalid row-softmax Worker V2 exchange: {error}")
            }
            Self::Descriptor(error) => {
                write!(
                    formatter,
                    "invalid row-softmax compiler descriptor: {error}"
                )
            }
            Self::ProfileMismatch(field) => {
                write!(
                    formatter,
                    "row-softmax direct-worker profile mismatch: {field}"
                )
            }
            Self::Structural(error) => {
                write!(
                    formatter,
                    "row-softmax structural HSACO admission failed: {error}"
                )
            }
        }
    }
}

impl Error for RowSoftmaxV1DirectWorkerErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Handoff(error) => Some(error),
            Self::WorkerProtocol(error) => Some(error),
            Self::Descriptor(error) => Some(error),
            Self::Structural(error) => Some(error),
            Self::ProfileMismatch(_) => None,
        }
    }
}

/// Validates exact request, response, handoff, OCML import, and provider-check evidence.
pub fn validate_row_softmax_v1_direct_worker_exchange_v1(
    source: &InertFirstBuildWorkerV2EvidenceV1,
    expected: RowSoftmaxV1DirectWorkerExpectationV1,
) -> Result<ValidatedRowSoftmaxV1DirectWorkerExchangeV1, RowSoftmaxV1DirectWorkerErrorV1> {
    if source.handoff_identity().as_bytes() != expected.handoff_sha256() {
        return Err(profile_mismatch("consumed rustc handoff identity"));
    }
    let exchange = InertDecodedWorkerExchangeV2::decode(
        source.authorized_request_bytes(),
        source.authorized().response().canonical_bytes(),
    )
    .map_err(RowSoftmaxV1DirectWorkerErrorV1::WorkerProtocol)?;
    let validated = validate_exchange_parts(
        &exchange,
        source.compiler_envelope(),
        source.symbol_manifest(),
        expected,
    )?;

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

/// Validates the exact exchange, then consumes it through row structural admission.
pub fn inspect_row_softmax_v1_direct_worker_hsaco_v1(
    source: InertFirstBuildWorkerV2EvidenceV1,
    expected: RowSoftmaxV1DirectWorkerExpectationV1,
) -> Result<InspectedRowSoftmaxV1DirectWorkerHsacoV1, RowSoftmaxV1DirectWorkerErrorV1> {
    let exchange = validate_row_softmax_v1_direct_worker_exchange_v1(&source, expected)?;
    let structural = inspect_row_softmax_v1_structural_worker_v2_hsaco_v1(
        source,
        expected.descriptor_expectation(),
    )
    .map_err(RowSoftmaxV1DirectWorkerErrorV1::Structural)?;
    Ok(InspectedRowSoftmaxV1DirectWorkerHsacoV1 {
        exchange,
        structural,
    })
}

fn validate_exchange_parts(
    exchange: &InertDecodedWorkerExchangeV2,
    envelope: &CompilerFfiEnvelopeV1,
    manifest: &CompilerModuleSymbolManifestV1,
    expected: RowSoftmaxV1DirectWorkerExpectationV1,
) -> Result<ValidatedRowSoftmaxV1DirectWorkerExchangeV1, RowSoftmaxV1DirectWorkerErrorV1> {
    let request = exchange.request();
    validate_request(request, envelope, manifest, expected)?;
    validate_response(request, exchange.response())?;
    let output = exchange
        .response()
        .output()
        .ok_or_else(|| profile_mismatch("completed response output"))?;
    Ok(ValidatedRowSoftmaxV1DirectWorkerExchangeV1 {
        identity: calculate_exchange_identity(request, exchange.response()),
        compiler_module: request.compiler_module().identity(),
        linked_output: output.identity(),
        frontend_authority_commitment: expected.frontend_authority_commitment,
    })
}

fn validate_request(
    request: &WorkerRequestV2,
    envelope: &CompilerFfiEnvelopeV1,
    manifest: &CompilerModuleSymbolManifestV1,
    expected: RowSoftmaxV1DirectWorkerExpectationV1,
) -> Result<(), RowSoftmaxV1DirectWorkerErrorV1> {
    if request.target().to_string() != TARGET {
        return Err(profile_mismatch("request target"));
    }
    if request.code_object_version() != CodeObjectVersion::V6 {
        return Err(profile_mismatch("requested code-object version"));
    }
    if request.options() != WorkerOptionsV1::new(WorkerOptimizationLevelV1::O0, true, true) {
        return Err(profile_mismatch("worker options"));
    }
    if request.compiler_envelope_identity()
        != WorkerCompilerFfiEnvelopeIdentityV2::from_compiler_identity(envelope.identity())
    {
        return Err(profile_mismatch("request compiler-envelope identity"));
    }
    if request.compiler_module().kind() != WorkerInputKindV1::LlvmTextIr {
        return Err(profile_mismatch("compiler-module input kind"));
    }
    if !request.external_providers().is_empty() {
        return Err(profile_mismatch("request-side external provider closure"));
    }
    if request.import_symbols() != [OCML_EXP_F32] || !request.export_symbols().is_empty() {
        return Err(profile_mismatch("device FFI symbol closure"));
    }
    if request.final_symbols()
        != [
            OCML_EXP_F32,
            ROW_SOFTMAX_V1_ENTRY_NAME,
            ROW_SOFTMAX_V1_DESCRIPTOR_SYMBOL,
        ]
    {
        return Err(profile_mismatch("final symbol closure"));
    }

    validate_manifest(manifest)?;
    validate_envelope(envelope)?;
    let handoff = CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        envelope.target(),
        envelope.code_object_version(),
        envelope.clone(),
        manifest.clone(),
        request.compiler_module().bytes(),
    )
    .map_err(RowSoftmaxV1DirectWorkerErrorV1::Handoff)?;
    if handoff.identity().sha256() != expected.handoff_sha256() {
        return Err(profile_mismatch("pinned rustc handoff identity"));
    }
    let descriptor = validate_handoff_profile(&handoff, expected.frontend_authority_commitment())?;
    if descriptor != expected.descriptor_expectation() {
        return Err(profile_mismatch("compiler descriptor expectation"));
    }
    Ok(())
}

fn validate_handoff_profile(
    handoff: &CompilerModuleHandoffV2,
    expected_frontend_authority: &[u8; FRONTEND_AUTHORITY_BYTES],
) -> Result<RowSoftmaxV1StructuralDescriptorExpectationV1, RowSoftmaxV1DirectWorkerErrorV1> {
    if handoff.kind() != CompilerModuleKindV1::LlvmTextIr {
        return Err(profile_mismatch("rustc handoff module kind"));
    }
    if handoff.target().to_string() != TARGET {
        return Err(profile_mismatch("rustc handoff target"));
    }
    if handoff.code_object_version() != fe2o3_compiler_ffi::CodeObjectVersion::V6 {
        return Err(profile_mismatch("rustc handoff code-object version"));
    }
    validate_envelope(handoff.envelope())?;
    validate_manifest(handoff.symbol_manifest())?;

    let (descriptor_bytes, authority) = decode_bound_sections(handoff.module_bytes())?;
    if authority.as_slice() != expected_frontend_authority {
        return Err(profile_mismatch("frontend-authority commitment"));
    }
    let source = CompilerDescriptorSourceV1::decode(&descriptor_bytes)
        .map_err(|_| profile_mismatch("compiler descriptor source"))?;
    let table = source.table();
    if table.canonical_code_object_digest() != CanonicalCodeObjectDigest::from_bytes([0; 32])
        || table.compiler().name().as_str() != "rustc-codegen-fe2o3"
        || table.producer().name().as_str() != "rustc-codegen-fe2o3-worker-v2"
        || table.producer().version().as_str() != "typed-general-gfx942-cov6-v1"
    {
        return Err(profile_mismatch("compiler descriptor producer profile"));
    }
    let [kernel] = table.kernels() else {
        return Err(profile_mismatch("compiler descriptor kernel closure"));
    };
    let expected = RowSoftmaxV1StructuralDescriptorExpectationV1::new(
        kernel.kernel_id(),
        kernel.source_evidence(),
        kernel.executable_ir_evidence(),
    )
    .map_err(RowSoftmaxV1DirectWorkerErrorV1::Descriptor)?;
    admit_row_softmax_v1_structural_descriptor_v1(table, expected)
        .map_err(RowSoftmaxV1DirectWorkerErrorV1::Descriptor)?;
    Ok(expected)
}

fn validate_envelope(
    envelope: &CompilerFfiEnvelopeV1,
) -> Result<(), RowSoftmaxV1DirectWorkerErrorV1> {
    if envelope.target().to_string() != TARGET
        || envelope.code_object_version() != fe2o3_compiler_ffi::CodeObjectVersion::V6
    {
        return Err(profile_mismatch("compiler FFI envelope target"));
    }
    let directional = envelope.directional_symbols();
    if directional.imports().collect::<Vec<_>>() != [OCML_EXP_F32]
        || directional.exports().next().is_some()
    {
        return Err(profile_mismatch("compiler FFI envelope symbol closure"));
    }
    Ok(())
}

fn validate_manifest(
    manifest: &CompilerModuleSymbolManifestV1,
) -> Result<(), RowSoftmaxV1DirectWorkerErrorV1> {
    let expected = [
        (
            CompilerModuleSymbolRoleV1::KernelEntry,
            ROW_SOFTMAX_V1_ENTRY_NAME,
        ),
        (
            CompilerModuleSymbolRoleV1::KernelDescriptor,
            ROW_SOFTMAX_V1_DESCRIPTOR_SYMBOL,
        ),
        (
            CompilerModuleSymbolRoleV1::UnresolvedExternalImport,
            OCML_EXP_F32,
        ),
    ];
    if manifest.entries().collect::<Vec<_>>() != expected {
        return Err(profile_mismatch("compiler symbol manifest"));
    }
    Ok(())
}

fn validate_response(
    request: &WorkerRequestV2,
    response: &WorkerResponseV2,
) -> Result<(), RowSoftmaxV1DirectWorkerErrorV1> {
    if !response.binds_request(request) {
        return Err(profile_mismatch("response request binding"));
    }
    if response.worker_build_identity() != request.worker_build_identity() {
        return Err(profile_mismatch("response worker identity"));
    }
    if response.stage() != WorkerStageV1::Complete {
        return Err(profile_mismatch("response completion stage"));
    }
    if response.diagnostics().len() != SUCCESS_DIAGNOSTICS.len()
        || response
            .diagnostics()
            .iter()
            .zip(SUCCESS_DIAGNOSTICS)
            .any(|(actual, expected)| actual != expected)
    {
        return Err(profile_mismatch("measured OCML and post-link diagnostics"));
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

fn decode_bound_sections(
    module: &[u8],
) -> Result<(Vec<u8>, Vec<u8>), RowSoftmaxV1DirectWorkerErrorV1> {
    let descriptor_header = module_assembly_section_header(COMPILER_DESCRIPTOR_SECTION_NAME_V1);
    let authority_header = module_assembly_section_header(FRONTEND_AUTHORITY_SECTION);
    let descriptor_positions = positions(module, descriptor_header.as_bytes());
    let authority_positions = positions(module, authority_header.as_bytes());
    let ([descriptor_position], [authority_position]) = (
        descriptor_positions.as_slice(),
        authority_positions.as_slice(),
    ) else {
        return Err(profile_mismatch("bound compiler section closure"));
    };
    if *descriptor_position == 0
        || *authority_position <= descriptor_position + descriptor_header.len()
    {
        return Err(profile_mismatch("bound compiler section order"));
    }
    let descriptor_start = descriptor_position + descriptor_header.len();
    let authority_start = authority_position + authority_header.len();
    let descriptor = decode_module_assembly_bytes(&module[descriptor_start..*authority_position])
        .ok_or_else(|| profile_mismatch("compiler descriptor section encoding"))?;
    let authority = decode_module_assembly_bytes(&module[authority_start..])
        .ok_or_else(|| profile_mismatch("frontend-authority section encoding"))?;
    if authority.len() != FRONTEND_AUTHORITY_BYTES {
        return Err(profile_mismatch("frontend-authority commitment size"));
    }
    Ok((descriptor, authority))
}

fn positions(bytes: &[u8], needle: &[u8]) -> Vec<usize> {
    bytes
        .windows(needle.len())
        .enumerate()
        .filter_map(|(index, window)| (window == needle).then_some(index))
        .collect()
}

fn module_assembly_section_header(section: &str) -> String {
    format!("\nmodule asm \".section {section},\\22\\22,@progbits\"\nmodule asm \".balign 8\"\n")
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
        let values = remaining[..line_end]
            .strip_prefix(PREFIX)?
            .strip_suffix(SUFFIX)?;
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

fn calculate_exchange_identity(
    request: &WorkerRequestV2,
    response: &WorkerResponseV2,
) -> RowSoftmaxV1DirectWorkerExchangeIdentityV1 {
    let mut digest = Sha256::new();
    digest.update(EXCHANGE_IDENTITY_DOMAIN_V1);
    digest.update((request.canonical_bytes().len() as u64).to_le_bytes());
    digest.update(request.canonical_bytes());
    digest.update((response.canonical_bytes().len() as u64).to_le_bytes());
    digest.update(response.canonical_bytes());
    RowSoftmaxV1DirectWorkerExchangeIdentityV1(digest.finalize().into())
}

const fn profile_mismatch(field: &'static str) -> RowSoftmaxV1DirectWorkerErrorV1 {
    RowSoftmaxV1DirectWorkerErrorV1::ProfileMismatch(field)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        WORKER_RESPONSE_MAGIC_V2, WorkerInputV1, WorkerOutputConstraintsV1,
        worker_protocol_v2::SealedWorkerRequestV2Parts,
    };
    use fe2o3_compiler_ffi::{
        CodeObjectVersion as CompilerCodeObjectVersion, CompilerFfiContractV1,
        CompilerFfiEnvelopeBuilderV1, CompilerFfiLinkRoleV1, CompilerFfiSourceOwnerV1,
        CompilerModuleSymbolManifestV1, DeviceTargetV1 as CompilerDeviceTargetV1,
    };
    use fe2o3_kernel_descriptor::{
        AccessMode, BlockSizeV1, BuildEvidenceV1, CapabilityV1, CompilerIdentityV1,
        DeviceDescriptorTableV1, DeviceLayoutDescriptorV1, DeviceLayoutRecordV1, DimensionsV1,
        EvidenceDigest, EvidenceIdentity, KernelAbiLayoutV1, KernelDescriptorV1, KernelId,
        LaunchConstraintsV1, LogicalArgumentV1, ProducerIdentityV1, SourceTypeDescriptorV1,
        SourceTypeRecordV1, Text, ValidName, encode_device_descriptor_table_v1,
    };
    use reserved_fe2o3_symbols::{
        DEVICE_FFI_DIRECTION_IMPORT_V1, DeviceFfiContractFieldsV1, DeviceFfiDirectionV1,
        derive_device_ffi_contract_id_v1,
    };

    const OUTPUT: &[u8] = b"linked-row";
    const AUTHORITY: [u8; FRONTEND_AUTHORITY_BYTES] = [0xa5; FRONTEND_AUTHORITY_BYTES];
    const OCML_ABI: &str = "C(f32[size=4,align=4])->f32[size=4,align=4]";

    fn exact_handoff() -> CompilerModuleHandoffV2 {
        handoff_with(exact_descriptor_source().canonical_bytes(), &AUTHORITY, b"")
    }

    fn handoff_with(
        descriptor: &[u8],
        authority: &[u8],
        extra_text: &[u8],
    ) -> CompilerModuleHandoffV2 {
        let mut module = br#"; ModuleID = 'row-softmax-v1-test'
target triple = "amdgcn-amd-amdhsa"

declare float @__ocml_exp_f32(float)
define amdgpu_kernel void @row_softmax_v1(ptr %input, i64 %input_len, ptr %output, i64 %output_len) {
entry:
  ret void
}
@row_softmax_v1.kd = external addrspace(1) global i8
"#
        .to_vec();
        module.extend_from_slice(extra_text);
        append_module_assembly_section(
            &mut module,
            COMPILER_DESCRIPTOR_SECTION_NAME_V1,
            descriptor,
        );
        append_module_assembly_section(&mut module, FRONTEND_AUTHORITY_SECTION, authority);
        let envelope = exact_envelope();
        CompilerModuleHandoffV2::new(
            CompilerModuleKindV1::LlvmTextIr,
            compiler_target(),
            CompilerCodeObjectVersion::V6,
            envelope,
            exact_manifest(),
            &module,
        )
        .unwrap()
    }

    fn exact_expectation(
        handoff: &CompilerModuleHandoffV2,
    ) -> RowSoftmaxV1DirectWorkerExpectationV1 {
        RowSoftmaxV1DirectWorkerExpectationV1::from_pinned_rustc_handoff(
            handoff,
            *handoff.identity().sha256(),
            AUTHORITY,
        )
        .unwrap()
    }

    fn exact_envelope() -> CompilerFfiEnvelopeV1 {
        let semantic_identity = [0x91; 32];
        let semantic_text = lower_hex(&semantic_identity);
        let fields = DeviceFfiContractFieldsV1 {
            direction: DEVICE_FFI_DIRECTION_IMPORT_V1,
            symbol: OCML_EXP_F32,
            calling_convention: "C",
            code_object_version: 6,
            target: TARGET,
            physical_abi: OCML_ABI,
            effects: "none",
            semantic_identity: &semantic_text,
        };
        let contract = CompilerFfiContractV1::new(
            derive_device_ffi_contract_id_v1(fields),
            DeviceFfiDirectionV1::Import,
            CompilerFfiLinkRoleV1::RequiresExternalDefinition,
            compiler_target(),
            CompilerCodeObjectVersion::V6,
            CompilerFfiSourceOwnerV1::new(
                "rustc-codegen-fe2o3",
                "rustc-codegen-fe2o3::row_softmax_v1::__ocml_exp_f32",
                [0x92; 16],
                "_RNvNtCs1234_21rustc_codegen_fe2o321row_softmax_v1_14__ocml_exp_f32",
            )
            .unwrap(),
            OCML_EXP_F32,
            OCML_ABI,
            "none",
            semantic_identity,
        )
        .unwrap();
        let mut builder =
            CompilerFfiEnvelopeBuilderV1::new(compiler_target(), CompilerCodeObjectVersion::V6, 1)
                .unwrap();
        builder.push(contract).unwrap();
        builder.finish().unwrap()
    }

    fn exact_manifest() -> CompilerModuleSymbolManifestV1 {
        CompilerModuleSymbolManifestV1::new([
            (
                CompilerModuleSymbolRoleV1::KernelEntry,
                ROW_SOFTMAX_V1_ENTRY_NAME,
            ),
            (
                CompilerModuleSymbolRoleV1::KernelDescriptor,
                ROW_SOFTMAX_V1_DESCRIPTOR_SYMBOL,
            ),
            (
                CompilerModuleSymbolRoleV1::UnresolvedExternalImport,
                OCML_EXP_F32,
            ),
        ])
        .unwrap()
    }

    fn request(
        handoff: &CompilerModuleHandoffV2,
        request_id: u8,
        target: fe2o3_kernel_descriptor::DeviceTargetV1,
        code_object_version: CodeObjectVersion,
        imports: Vec<String>,
        providers: Vec<WorkerInputV1>,
        final_symbols: Vec<String>,
    ) -> WorkerRequestV2 {
        WorkerRequestV2::from_sealed_parts(SealedWorkerRequestV2Parts {
            request_id: [request_id; 32],
            llvm_build_identity: "upstream-llvm-22-row".to_owned(),
            worker_build_identity: "fe2o3-direct-llvm-lld-worker-v2-row".to_owned(),
            worker_executable: ContentIdentityV1::from_parts([0x22; 32], 4096),
            target,
            code_object_version,
            options: WorkerOptionsV1::new(WorkerOptimizationLevelV1::O0, true, true),
            compiler_envelope: WorkerCompilerFfiEnvelopeIdentityV2::from_compiler_identity(
                handoff.envelope().identity(),
            ),
            compiler_module: WorkerInputV1::new(
                WorkerInputKindV1::LlvmTextIr,
                handoff.module_bytes().to_vec(),
            )
            .unwrap(),
            external_providers: providers,
            import_symbols: imports,
            export_symbols: Vec::new(),
            final_symbols,
            output: WorkerOutputConstraintsV1::new(OUTPUT.len() as u64).unwrap(),
        })
        .unwrap()
    }

    fn exact_request(handoff: &CompilerModuleHandoffV2, request_id: u8) -> WorkerRequestV2 {
        request(
            handoff,
            request_id,
            descriptor_target(),
            CodeObjectVersion::V6,
            vec![OCML_EXP_F32.to_owned()],
            Vec::new(),
            exact_final_symbols(),
        )
    }

    fn exact_final_symbols() -> Vec<String> {
        [
            OCML_EXP_F32,
            ROW_SOFTMAX_V1_ENTRY_NAME,
            ROW_SOFTMAX_V1_DESCRIPTOR_SYMBOL,
        ]
        .map(str::to_owned)
        .to_vec()
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
        let output_identity = ContentIdentityV1::calculate(OUTPUT);
        let mut output = vec![1];
        output.extend_from_slice(output_identity.sha256());
        output.extend_from_slice(&output_identity.byte_len().to_le_bytes());
        output.extend_from_slice(OUTPUT);
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

    fn validate(
        handoff: &CompilerModuleHandoffV2,
        request: &WorkerRequestV2,
        expected: RowSoftmaxV1DirectWorkerExpectationV1,
        diagnostics: &[&str],
    ) -> Result<ValidatedRowSoftmaxV1DirectWorkerExchangeV1, RowSoftmaxV1DirectWorkerErrorV1> {
        validate_exchange_parts(
            &exchange(request, diagnostics),
            handoff.envelope(),
            handoff.symbol_manifest(),
            expected,
        )
    }

    fn success_diagnostics() -> Vec<&'static str> {
        SUCCESS_DIAGNOSTICS.to_vec()
    }

    fn exact_descriptor_source() -> CompilerDescriptorSourceV1 {
        let input_source = SourceTypeRecordV1::new(SourceTypeDescriptorV1::shared_slice(
            fe2o3_kernel_descriptor::ScalarTypeV1::F32,
        ));
        let input_layout = DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::shared_slice(
            fe2o3_kernel_descriptor::ScalarTypeV1::F32,
        ));
        let output_source = SourceTypeRecordV1::new(SourceTypeDescriptorV1::disjoint_slice(
            fe2o3_kernel_descriptor::ScalarTypeV1::F32,
        ));
        let output_layout = DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::disjoint_slice(
            fe2o3_kernel_descriptor::ScalarTypeV1::F32,
        ));
        let kernel = KernelDescriptorV1::new(
            KernelId::from_bytes([0x81; 32]),
            name(ROW_SOFTMAX_V1_ENTRY_NAME),
            name(ROW_SOFTMAX_V1_ENTRY_NAME),
            name(ROW_SOFTMAX_V1_DESCRIPTOR_SYMBOL),
            evidence(0x82, 0x83),
            evidence(0x84, 0x85),
            vec![CapabilityV1::Subgroup, CapabilityV1::AmdWave],
            KernelAbiLayoutV1::new(32, 288, 8).unwrap(),
            LaunchConstraintsV1::new(
                1,
                BlockSizeV1::Exact(DimensionsV1::new(64, 1, 1).unwrap()),
                DimensionsV1::new(1, 1, 1).unwrap(),
                64,
                0,
                0,
            )
            .unwrap(),
            vec![
                LogicalArgumentV1::shared_slice(0, name("input"), &input_source, &input_layout, 0)
                    .unwrap(),
                LogicalArgumentV1::disjoint_slice(
                    1,
                    name("output"),
                    &output_source,
                    &output_layout,
                    AccessMode::ReadWrite,
                    16,
                )
                .unwrap(),
            ],
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
                descriptor_target(),
                vec![input_source, output_source],
                vec![input_layout, output_layout],
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

    fn descriptor_target() -> fe2o3_kernel_descriptor::DeviceTargetV1 {
        fe2o3_kernel_descriptor::DeviceTargetV1::parse(TARGET).unwrap()
    }

    fn compiler_target() -> CompilerDeviceTargetV1 {
        CompilerDeviceTargetV1::parse(TARGET).unwrap()
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

    fn lower_hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    fn push_field(encoded: &mut Vec<u8>, tag: u16, bytes: &[u8]) {
        encoded.extend_from_slice(&tag.to_le_bytes());
        encoded.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        encoded.extend_from_slice(bytes);
    }

    #[test]
    fn exact_pinned_row_exchange_is_admitted_without_math_or_execution_authority() {
        let handoff = exact_handoff();
        let expected = exact_expectation(&handoff);
        let request = exact_request(&handoff, 0x11);
        let validated = validate(&handoff, &request, expected, &success_diagnostics()).unwrap();

        assert_ne!(validated.identity().as_bytes(), &[0; 32]);
        assert_eq!(
            validated.compiler_module_identity(),
            request.compiler_module().identity()
        );
        assert_eq!(
            validated.linked_output_identity(),
            ContentIdentityV1::calculate(OUTPUT)
        );
        assert_eq!(
            validated.embedded_frontend_authority_commitment(),
            &AUTHORITY
        );
        assert!(validated.measured_gfx942_ocml_provider_closure_was_checked());
        assert_eq!(validated.measured_ocml_provider_file_count(), 4);
        assert_eq!(validated.requested_ocml_import(), OCML_EXP_F32);
        assert!(!validated.authenticates_frontend_origin());
        assert!(!validated.proves_exp_math_accuracy());
        assert!(!validated.proves_functional_softmax());
        assert!(!validated.grants_publication_authority());
        assert!(!validated.grants_load_authority());
        assert!(!validated.grants_launch_authority());
    }

    #[test]
    fn missing_extra_ocml_imports_and_request_providers_fail_closed() {
        let handoff = exact_handoff();
        let expected = exact_expectation(&handoff);
        let provider = WorkerInputV1::new(
            WorkerInputKindV1::LlvmBitcode,
            b"request-side-ocml-provider".to_vec(),
        )
        .unwrap();
        let cases = [
            request(
                &handoff,
                0x21,
                descriptor_target(),
                CodeObjectVersion::V6,
                Vec::new(),
                Vec::new(),
                exact_final_symbols(),
            ),
            request(
                &handoff,
                0x22,
                descriptor_target(),
                CodeObjectVersion::V6,
                vec!["__ocml_cos_f32".to_owned(), OCML_EXP_F32.to_owned()],
                Vec::new(),
                vec![
                    "__ocml_cos_f32".to_owned(),
                    OCML_EXP_F32.to_owned(),
                    ROW_SOFTMAX_V1_ENTRY_NAME.to_owned(),
                    ROW_SOFTMAX_V1_DESCRIPTOR_SYMBOL.to_owned(),
                ],
            ),
            request(
                &handoff,
                0x23,
                descriptor_target(),
                CodeObjectVersion::V6,
                vec![OCML_EXP_F32.to_owned()],
                vec![provider],
                exact_final_symbols(),
            ),
        ];
        for request in cases {
            assert!(
                validate(&handoff, &request, expected, &success_diagnostics()).is_err(),
                "accepted substituted OCML closure: {request:?}"
            );
        }
    }

    #[test]
    fn target_cov_symbol_and_provider_diagnostic_substitutions_fail_closed() {
        let handoff = exact_handoff();
        let expected = exact_expectation(&handoff);
        let requests = [
            request(
                &handoff,
                0x31,
                fe2o3_kernel_descriptor::DeviceTargetV1::parse("gfx942").unwrap(),
                CodeObjectVersion::V6,
                vec![OCML_EXP_F32.to_owned()],
                Vec::new(),
                exact_final_symbols(),
            ),
            request(
                &handoff,
                0x32,
                descriptor_target(),
                CodeObjectVersion::V5,
                vec![OCML_EXP_F32.to_owned()],
                Vec::new(),
                exact_final_symbols(),
            ),
            request(
                &handoff,
                0x33,
                descriptor_target(),
                CodeObjectVersion::V6,
                vec![OCML_EXP_F32.to_owned()],
                Vec::new(),
                vec![OCML_EXP_F32.to_owned(), "row_softmax_alias".to_owned()],
            ),
        ];
        for request in requests {
            assert!(
                validate(&handoff, &request, expected, &success_diagnostics()).is_err(),
                "accepted substituted row request: {request:?}"
            );
        }

        let request = exact_request(&handoff, 0x34);
        for diagnostic in [
            "device_library.check=identity status=ok provider=gfx942-ocml-v1 roots=[] files=4",
            "device_library.check=identity status=ok provider=gfx942-ocml-v1 roots=[__ocml_exp_f32] files=5",
        ] {
            let mut diagnostics = success_diagnostics();
            diagnostics[0] = diagnostic;
            assert!(validate(&handoff, &request, expected, &diagnostics).is_err());
        }
    }

    #[test]
    fn descriptor_authority_and_arbitrary_semantic_text_cannot_replace_the_pin() {
        let exact = exact_handoff();
        let expected = exact_expectation(&exact);

        let mut descriptor = exact_descriptor_source().canonical_bytes().to_vec();
        let symbol = descriptor
            .windows(ROW_SOFTMAX_V1_DESCRIPTOR_SYMBOL.len())
            .position(|window| window == ROW_SOFTMAX_V1_DESCRIPTOR_SYMBOL.as_bytes())
            .unwrap();
        descriptor[symbol] = b's';
        let wrong_descriptor = handoff_with(&descriptor, &AUTHORITY, b"");
        assert!(
            RowSoftmaxV1DirectWorkerExpectationV1::from_pinned_rustc_handoff(
                &wrong_descriptor,
                *wrong_descriptor.identity().sha256(),
                AUTHORITY,
            )
            .is_err()
        );

        let wrong_authority = handoff_with(
            exact_descriptor_source().canonical_bytes(),
            &[0xb6; FRONTEND_AUTHORITY_BYTES],
            b"",
        );
        assert!(
            RowSoftmaxV1DirectWorkerExpectationV1::from_pinned_rustc_handoff(
                &wrong_authority,
                *wrong_authority.identity().sha256(),
                AUTHORITY,
            )
            .is_err()
        );

        let overclaim = handoff_with(
            exact_descriptor_source().canonical_bytes(),
            &AUTHORITY,
            b"; arbitrary text claims exact exp and softmax semantics\n",
        );
        let request = exact_request(&overclaim, 0x41);
        assert!(validate(&overclaim, &request, expected, &success_diagnostics()).is_err());
        assert!(!expected.proves_exp_math_accuracy());
        assert!(!expected.proves_functional_softmax());
    }

    #[test]
    fn cross_request_output_replay_is_rejected_before_profile_admission() {
        let handoff = exact_handoff();
        let first = exact_request(&handoff, 0x51);
        let second = exact_request(&handoff, 0x52);
        let replay = response(&first, &success_diagnostics());
        assert!(InertDecodedWorkerExchangeV2::decode(second.canonical_bytes(), &replay).is_err());
    }

    #[test]
    fn handoff_pin_and_authority_must_be_nonzero_and_exact() {
        let handoff = exact_handoff();
        assert!(
            RowSoftmaxV1DirectWorkerExpectationV1::from_pinned_rustc_handoff(
                &handoff, [0; 32], AUTHORITY,
            )
            .is_err()
        );
        assert!(
            RowSoftmaxV1DirectWorkerExpectationV1::from_pinned_rustc_handoff(
                &handoff,
                *handoff.identity().sha256(),
                [0; FRONTEND_AUTHORITY_BYTES],
            )
            .is_err()
        );
        let mut wrong = *handoff.identity().sha256();
        wrong[0] ^= 1;
        assert!(
            RowSoftmaxV1DirectWorkerExpectationV1::from_pinned_rustc_handoff(
                &handoff, wrong, AUTHORITY,
            )
            .is_err()
        );
    }

    #[test]
    fn descriptor_encoding_is_canonical() {
        let source = exact_descriptor_source();
        let encoded = encode_device_descriptor_table_v1(source.table()).unwrap();
        assert!(!encoded.is_empty());
    }
}
