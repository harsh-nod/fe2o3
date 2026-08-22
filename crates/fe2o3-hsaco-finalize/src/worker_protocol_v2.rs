//! Sealed Worker V2 protocol for compiler-FFI-aware direct links.
//!
//! Unlike Worker V1, a V2 request can only be created by the finalizer's
//! sealed construction path. The wire format is still inert: it grants no
//! publication, loading, or launch authority.

#![allow(dead_code)] // Framing remains dormant until compiler-owned provenance exists.

use std::{fmt, str};

use fe2o3_compiler_ffi::CompilerFfiEnvelopeIdentityV1;
use fe2o3_kernel_descriptor::{CodeObjectVersion, DeviceTargetV1};
use sha2::{Digest, Sha256};

use crate::{
    ContentIdentityV1, MAX_LINK_INPUTS, MAX_WORKER_DIAGNOSTIC_BYTES, MAX_WORKER_DIAGNOSTICS,
    MAX_WORKER_OUTPUT_BYTES, MAX_WORKER_REQUEST_BYTES, MAX_WORKER_RESPONSE_BYTES,
    MAX_WORKER_TOOLCHAIN_ID_BYTES, MAX_WORKER_TOTAL_DIAGNOSTIC_BYTES, MAX_WORKER_TOTAL_INPUT_BYTES,
    WorkerInputV1, WorkerOptionsV1, WorkerOutputConstraintsV1, WorkerProtocolError, WorkerStageV1,
    worker_protocol::validate_symbols,
};

use crate::{
    MAX_WORKER_SYMBOL_BYTES, MAX_WORKER_SYMBOLS, MAX_WORKER_TARGET_BYTES, WorkerInputKindV1,
    WorkerOptimizationLevelV1,
};

pub const WORKER_REQUEST_MAGIC_V2: &[u8; 8] = b"F3LREQ02";
pub const WORKER_RESPONSE_MAGIC_V2: &[u8; 8] = b"F3LRSP02";
pub const WORKER_RESPONSE_MAGIC_V3: &[u8; 8] = b"F3LRSP03";

const REQUEST_DOMAIN_V2: &[u8] = b"FE2O3/DIRECT-LLVM-WORKER-REQUEST/V2\0";
const PROVIDER_MANIFEST_DOMAIN_V1: &[u8] = b"FE2O3/DEVICE-LIBRARY-PROVIDER-MANIFEST/V1\0";
const RESPONSE_DOMAIN_V3: &[u8] = b"FE2O3/DIRECT-LLVM-WORKER-RESPONSE/V3\0";
const REQUEST_FIELD_COUNT_V2: u16 = 15;
const RESPONSE_FIELD_COUNT_V2: u16 = 7;
const RESPONSE_FIELD_COUNT_V3: u16 = 9;
const INPUT_OVERHEAD_BYTES: usize = 1 + 32 + 8;
const CONTENT_IDENTITY_BYTES: usize = 32 + 8;
const MAX_PROVIDER_IDENTITY_BYTES: usize = 128;
const MAX_PROVIDER_FILES: usize = 16;
const MAX_PROVIDER_BASENAME_BYTES: usize = 128;
const MAX_RESPONSE_DIAGNOSTICS_BODY_BYTES: usize = checked_bound_add(
    checked_bound_add(
        MAX_WORKER_TOTAL_DIAGNOSTIC_BYTES,
        checked_bound_mul(MAX_WORKER_DIAGNOSTICS, 4),
    ),
    4,
);
const MAX_RESPONSE_OUTPUT_BODY_BYTES: usize = checked_bound_add(MAX_WORKER_OUTPUT_BYTES, 45);
const MAX_PROVIDER_EVIDENCE_BYTES: usize = checked_bound_add(
    checked_bound_add(
        checked_bound_add(MAX_PROVIDER_IDENTITY_BYTES, MAX_WORKER_TARGET_BYTES),
        checked_bound_mul(
            MAX_WORKER_SYMBOLS,
            checked_bound_add(MAX_WORKER_SYMBOL_BYTES, 4),
        ),
    ),
    checked_bound_add(
        checked_bound_mul(
            MAX_PROVIDER_FILES,
            checked_bound_add(MAX_PROVIDER_BASENAME_BYTES, 36),
        ),
        49,
    ),
);

/// Maximum exact response-body bytes retained by one replay metadata shell.
pub(crate) const MAX_WORKER_RESPONSE_REPLAY_METADATA_SHELL_BYTES_V1: usize = checked_bound_add(
    MAX_RESPONSE_DIAGNOSTICS_BODY_BYTES,
    MAX_PROVIDER_EVIDENCE_BYTES,
);

/// Maximum exact response-body bytes retained by two independent metadata shells.
pub(crate) const MAX_WORKER_RESPONSE_REPLAY_METADATA_TWO_SHELL_BYTES_V1: usize = checked_bound_add(
    MAX_WORKER_RESPONSE_REPLAY_METADATA_SHELL_BYTES_V1,
    MAX_WORKER_RESPONSE_REPLAY_METADATA_SHELL_BYTES_V1,
);

const fn checked_bound_add(left: usize, right: usize) -> usize {
    match left.checked_add(right) {
        Some(value) => value,
        None => panic!("worker response metadata bound overflow"),
    }
}

const fn checked_bound_mul(left: usize, right: usize) -> usize {
    match left.checked_mul(right) {
        Some(value) => value,
        None => panic!("worker response metadata bound overflow"),
    }
}

/// Evidence available only from the sealed compiler-envelope V2 path.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WorkerEvidenceClassV2 {
    CompilerFfiLink,
}

/// Opaque copy of the complete compiler-envelope identity bound into V2.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorkerCompilerFfiEnvelopeIdentityV2([u8; 32]);

impl WorkerCompilerFfiEnvelopeIdentityV2 {
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }

    pub fn to_hex(self) -> String {
        lower_hex(&self.0)
    }

    pub(crate) fn from_compiler_identity(identity: CompilerFfiEnvelopeIdentityV1) -> Self {
        Self(identity.as_bytes())
    }

    #[cfg(test)]
    pub(crate) const fn from_test_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

/// Private validated parts accepted by the only V2 request constructor.
pub(crate) struct SealedWorkerRequestV2Parts {
    pub request_id: [u8; 32],
    pub llvm_build_identity: String,
    pub worker_build_identity: String,
    pub worker_executable: ContentIdentityV1,
    pub target: DeviceTargetV1,
    pub code_object_version: CodeObjectVersion,
    pub options: WorkerOptionsV1,
    pub compiler_envelope: WorkerCompilerFfiEnvelopeIdentityV2,
    pub compiler_module: WorkerInputV1,
    pub external_providers: Vec<WorkerInputV1>,
    pub import_symbols: Vec<String>,
    pub export_symbols: Vec<String>,
    pub final_symbols: Vec<String>,
    pub output: WorkerOutputConstraintsV1,
}

/// Canonical V2 request created only from sealed compiler observations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerRequestV2 {
    request_id: [u8; 32],
    llvm_build_identity: String,
    worker_build_identity: String,
    worker_executable: ContentIdentityV1,
    target: DeviceTargetV1,
    code_object_version: CodeObjectVersion,
    options: WorkerOptionsV1,
    compiler_envelope: WorkerCompilerFfiEnvelopeIdentityV2,
    compiler_module: WorkerInputV1,
    external_providers: Vec<WorkerInputV1>,
    import_symbols: Vec<String>,
    export_symbols: Vec<String>,
    final_symbols: Vec<String>,
    output: WorkerOutputConstraintsV1,
    canonical_bytes: Vec<u8>,
    identity: [u8; 32],
}

impl WorkerRequestV2 {
    pub(crate) fn from_sealed_parts(
        parts: SealedWorkerRequestV2Parts,
    ) -> Result<Self, WorkerProtocolError> {
        validate_request_parts(&parts)?;
        let mut request = Self {
            request_id: parts.request_id,
            llvm_build_identity: parts.llvm_build_identity,
            worker_build_identity: parts.worker_build_identity,
            worker_executable: parts.worker_executable,
            target: parts.target,
            code_object_version: parts.code_object_version,
            options: parts.options,
            compiler_envelope: parts.compiler_envelope,
            compiler_module: parts.compiler_module,
            external_providers: parts.external_providers,
            import_symbols: parts.import_symbols,
            export_symbols: parts.export_symbols,
            final_symbols: parts.final_symbols,
            output: parts.output,
            canonical_bytes: Vec::new(),
            identity: [0; 32],
        };
        let (canonical_bytes, identity) = encode_request(&request)?;
        request.canonical_bytes = canonical_bytes;
        request.identity = identity;
        Ok(request)
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn request_id(&self) -> &[u8; 32] {
        &self.request_id
    }

    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }

    pub fn llvm_build_identity(&self) -> &str {
        &self.llvm_build_identity
    }

    pub fn worker_build_identity(&self) -> &str {
        &self.worker_build_identity
    }

    pub const fn worker_executable(&self) -> ContentIdentityV1 {
        self.worker_executable
    }

    pub const fn target(&self) -> DeviceTargetV1 {
        self.target
    }

    pub const fn code_object_version(&self) -> CodeObjectVersion {
        self.code_object_version
    }

    pub const fn options(&self) -> WorkerOptionsV1 {
        self.options
    }

    pub const fn compiler_envelope_identity(&self) -> WorkerCompilerFfiEnvelopeIdentityV2 {
        self.compiler_envelope
    }

    pub const fn compiler_module(&self) -> &WorkerInputV1 {
        &self.compiler_module
    }

    pub fn external_providers(&self) -> &[WorkerInputV1] {
        &self.external_providers
    }

    pub fn import_symbols(&self) -> &[String] {
        &self.import_symbols
    }

    pub fn export_symbols(&self) -> &[String] {
        &self.export_symbols
    }

    pub fn final_symbols(&self) -> &[String] {
        &self.final_symbols
    }

    pub const fn output_constraints(&self) -> &WorkerOutputConstraintsV1 {
        &self.output
    }

    pub const fn evidence_class(&self) -> WorkerEvidenceClassV2 {
        WorkerEvidenceClassV2::CompilerFfiLink
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

    #[cfg(test)]
    fn decode_for_test(bytes: &[u8]) -> Result<Self, WorkerProtocolError> {
        decode_request(bytes)
    }
}

/// V2 output whose provenance is inseparable from its sealed request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerOutputV2 {
    request_identity: [u8; 32],
    compiler_envelope: WorkerCompilerFfiEnvelopeIdentityV2,
    identity: ContentIdentityV1,
    bytes: Vec<u8>,
}

impl WorkerOutputV2 {
    pub const fn request_identity(&self) -> &[u8; 32] {
        &self.request_identity
    }

    pub const fn compiler_envelope_identity(&self) -> WorkerCompilerFfiEnvelopeIdentityV2 {
        self.compiler_envelope
    }

    pub const fn identity(&self) -> ContentIdentityV1 {
        self.identity
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn evidence_class(&self) -> WorkerEvidenceClassV2 {
        WorkerEvidenceClassV2::CompilerFfiLink
    }

    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }
}

/// One ordered, content-addressed file in a worker-owned device-library closure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerDeviceLibraryProviderFileEvidenceV1 {
    basename: String,
    sha256: [u8; 32],
}

impl WorkerDeviceLibraryProviderFileEvidenceV1 {
    pub fn basename(&self) -> &str {
        &self.basename
    }

    pub const fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }
}

/// Structured provider closure emitted by the measured worker.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerDeviceLibraryProviderEvidenceV1 {
    provider_identity: String,
    target: DeviceTargetV1,
    code_object_version: CodeObjectVersion,
    import_symbols: Vec<String>,
    files: Vec<WorkerDeviceLibraryProviderFileEvidenceV1>,
    manifest_identity: [u8; 32],
}

impl WorkerDeviceLibraryProviderEvidenceV1 {
    pub fn provider_identity(&self) -> &str {
        &self.provider_identity
    }

    pub const fn target(&self) -> DeviceTargetV1 {
        self.target
    }

    pub const fn code_object_version(&self) -> CodeObjectVersion {
        self.code_object_version
    }

    pub fn import_symbols(&self) -> &[String] {
        &self.import_symbols
    }

    pub fn files(&self) -> &[WorkerDeviceLibraryProviderFileEvidenceV1] {
        &self.files
    }

    pub const fn manifest_identity(&self) -> &[u8; 32] {
        &self.manifest_identity
    }
}

/// Borrowed canonical response bodies needed to reconstruct replay metadata.
///
/// This view deliberately carries neither the raw worker output nor any
/// publication, load, launch, or execution authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WorkerResponseReplayMetadataV1<'response> {
    diagnostics_body: &'response [u8],
    provider_evidence_body: Option<&'response [u8]>,
}

impl<'response> WorkerResponseReplayMetadataV1<'response> {
    #[cfg(test)]
    pub(crate) const fn from_test_bodies(
        diagnostics_body: &'response [u8],
        provider_evidence_body: Option<&'response [u8]>,
    ) -> Self {
        Self {
            diagnostics_body,
            provider_evidence_body,
        }
    }

    pub(crate) const fn diagnostics_body(&self) -> &'response [u8] {
        self.diagnostics_body
    }

    pub(crate) const fn provider_evidence_body(&self) -> Option<&'response [u8]> {
        self.provider_evidence_body
    }
}

/// Canonical worker response decoded only in the context of one sealed V2 request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerResponseV2 {
    request_id: [u8; 32],
    request_identity: [u8; 32],
    compiler_envelope: WorkerCompilerFfiEnvelopeIdentityV2,
    worker_build_identity: String,
    stage: WorkerStageV1,
    diagnostics: Vec<String>,
    output: Option<WorkerOutputV2>,
    device_library_provider: Option<WorkerDeviceLibraryProviderEvidenceV1>,
    response_identity: Option<[u8; 32]>,
    canonical_bytes: Vec<u8>,
}

impl WorkerResponseV2 {
    pub(crate) fn decode_for_request(
        bytes: &[u8],
        request: &WorkerRequestV2,
    ) -> Result<Self, WorkerProtocolError> {
        if bytes.len() > MAX_WORKER_RESPONSE_BYTES {
            return Err(WorkerProtocolError::ResponseTooLarge);
        }
        let has_provider_extension = bytes.starts_with(WORKER_RESPONSE_MAGIC_V3);
        let (magic, field_count) = if has_provider_extension {
            (WORKER_RESPONSE_MAGIC_V3, RESPONSE_FIELD_COUNT_V3)
        } else {
            (WORKER_RESPONSE_MAGIC_V2, RESPONSE_FIELD_COUNT_V2)
        };
        let mut decoder = Decoder::new(bytes, magic, field_count)?;
        let request_id = fixed::<32>(decoder.field(1, 32)?)?;
        let request_identity = fixed::<32>(decoder.field(2, 32)?)?;
        let compiler_envelope =
            WorkerCompilerFfiEnvelopeIdentityV2(fixed::<32>(decoder.field(3, 32)?)?);
        let worker_build_identity = text(
            decoder.field(4, MAX_WORKER_TOOLCHAIN_ID_BYTES)?,
            MAX_WORKER_TOOLCHAIN_ID_BYTES,
            "worker build identity",
        )?;
        let stage = decode_stage(one_byte(decoder.field(5, 1)?)?)?;
        let diagnostics = decode_strings(
            decoder.field(6, MAX_RESPONSE_DIAGNOSTICS_BODY_BYTES)?,
            MAX_WORKER_DIAGNOSTICS,
            MAX_WORKER_DIAGNOSTIC_BYTES,
            MAX_WORKER_TOTAL_DIAGNOSTIC_BYTES,
            true,
        )?;
        let raw_output = decode_output(decoder.field(7, MAX_RESPONSE_OUTPUT_BODY_BYTES)?)?;
        let (device_library_provider, response_identity) = if has_provider_extension {
            let provider =
                decode_provider_evidence(decoder.field(8, MAX_PROVIDER_EVIDENCE_BYTES)?)?;
            let identity_field_offset = decoder.position();
            let declared_identity = fixed::<32>(decoder.field(9, 32)?)?;
            decoder.finish(RESPONSE_FIELD_COUNT_V3)?;
            if calculate_response_identity(&bytes[..identity_field_offset]) != declared_identity {
                return Err(WorkerProtocolError::ResponseIdentityMismatch);
            }
            (Some(provider), Some(declared_identity))
        } else {
            decoder.finish(RESPONSE_FIELD_COUNT_V2)?;
            (None, None)
        };

        if request_id != request.request_id
            || request_identity != request.identity
            || compiler_envelope != request.compiler_envelope
        {
            return Err(WorkerProtocolError::RequestIdentityMismatch);
        }
        if device_library_provider.as_ref().is_some_and(|provider| {
            provider.target != request.target
                || provider.code_object_version != request.code_object_version
                || provider
                    .import_symbols
                    .iter()
                    .any(|symbol| request.import_symbols.binary_search(symbol).is_err())
        }) {
            return Err(WorkerProtocolError::ProviderEvidenceMismatch);
        }
        if (stage == WorkerStageV1::Complete) != raw_output.is_some() {
            return Err(WorkerProtocolError::InvalidResponseState);
        }
        let output = raw_output.map(|(identity, bytes)| WorkerOutputV2 {
            request_identity,
            compiler_envelope,
            identity,
            bytes,
        });
        if output
            .as_ref()
            .is_some_and(|value| value.bytes.len() as u64 > request.output.max_bytes())
        {
            return Err(WorkerProtocolError::InvalidOutputBound);
        }
        Ok(Self {
            request_id,
            request_identity,
            compiler_envelope,
            worker_build_identity,
            stage,
            diagnostics,
            output,
            device_library_provider,
            response_identity,
            canonical_bytes: copy_bytes(bytes, "decoded response canonical bytes")?,
        })
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub(crate) fn replay_metadata(
        &self,
    ) -> Result<WorkerResponseReplayMetadataV1<'_>, WorkerProtocolError> {
        if self.stage != WorkerStageV1::Complete || self.output.is_none() {
            return Err(WorkerProtocolError::InvalidResponseState);
        }
        let metadata = response_replay_metadata_from_bytes(self.canonical_bytes())?;
        if metadata.provider_evidence_body.is_some() != self.device_library_provider.is_some() {
            return Err(WorkerProtocolError::NonCanonicalEncoding);
        }
        Ok(metadata)
    }

    pub const fn request_id(&self) -> &[u8; 32] {
        &self.request_id
    }

    pub const fn request_identity(&self) -> &[u8; 32] {
        &self.request_identity
    }

    pub const fn compiler_envelope_identity(&self) -> WorkerCompilerFfiEnvelopeIdentityV2 {
        self.compiler_envelope
    }

    pub fn worker_build_identity(&self) -> &str {
        &self.worker_build_identity
    }

    pub const fn stage(&self) -> WorkerStageV1 {
        self.stage
    }

    pub fn diagnostics(&self) -> &[String] {
        &self.diagnostics
    }

    pub const fn output(&self) -> Option<&WorkerOutputV2> {
        self.output.as_ref()
    }

    pub const fn device_library_provider(&self) -> Option<&WorkerDeviceLibraryProviderEvidenceV1> {
        self.device_library_provider.as_ref()
    }

    pub const fn response_identity(&self) -> Option<&[u8; 32]> {
        self.response_identity.as_ref()
    }

    pub fn binds_request(&self, request: &WorkerRequestV2) -> bool {
        self.request_id == request.request_id
            && self.request_identity == request.identity
            && self.compiler_envelope == request.compiler_envelope
    }

    pub const fn evidence_class(&self) -> WorkerEvidenceClassV2 {
        WorkerEvidenceClassV2::CompilerFfiLink
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

fn response_replay_metadata_from_bytes(
    bytes: &[u8],
) -> Result<WorkerResponseReplayMetadataV1<'_>, WorkerProtocolError> {
    if bytes.len() > MAX_WORKER_RESPONSE_BYTES {
        return Err(WorkerProtocolError::ResponseTooLarge);
    }
    let magic = bytes
        .get(..WORKER_RESPONSE_MAGIC_V2.len())
        .ok_or(WorkerProtocolError::Truncated)?;
    let has_provider_extension = if magic == WORKER_RESPONSE_MAGIC_V2 {
        false
    } else if magic == WORKER_RESPONSE_MAGIC_V3 {
        true
    } else {
        return Err(WorkerProtocolError::BadMagic);
    };
    let (expected_magic, field_count) = if has_provider_extension {
        (WORKER_RESPONSE_MAGIC_V3, RESPONSE_FIELD_COUNT_V3)
    } else {
        (WORKER_RESPONSE_MAGIC_V2, RESPONSE_FIELD_COUNT_V2)
    };
    let mut decoder = Decoder::new(bytes, expected_magic, field_count)?;

    fixed::<32>(decoder.field(1, 32)?)?;
    fixed::<32>(decoder.field(2, 32)?)?;
    fixed::<32>(decoder.field(3, 32)?)?;
    validate_text_body(
        decoder.field(4, MAX_WORKER_TOOLCHAIN_ID_BYTES)?,
        MAX_WORKER_TOOLCHAIN_ID_BYTES,
        "worker build identity",
    )?;
    if decode_stage(one_byte(decoder.field(5, 1)?)?)? != WorkerStageV1::Complete {
        return Err(WorkerProtocolError::InvalidResponseState);
    }
    let diagnostics_body = decoder.field(6, MAX_RESPONSE_DIAGNOSTICS_BODY_BYTES)?;
    validate_diagnostics_body(diagnostics_body)?;
    validate_complete_output_body(decoder.field(7, MAX_RESPONSE_OUTPUT_BODY_BYTES)?)?;

    let provider_evidence_body = if has_provider_extension {
        let body = decoder.field(8, MAX_PROVIDER_EVIDENCE_BYTES)?;
        if body.is_empty() {
            return Err(WorkerProtocolError::InvalidFieldLength(8));
        }
        fixed::<32>(decoder.field(9, 32)?)?;
        Some(body)
    } else {
        None
    };
    decoder.finish(field_count)?;

    Ok(WorkerResponseReplayMetadataV1 {
        diagnostics_body,
        provider_evidence_body,
    })
}

pub(crate) fn validate_worker_response_replay_metadata_bodies_v1(
    diagnostics_body: &[u8],
    provider_evidence_body: Option<&[u8]>,
) -> Result<(), WorkerProtocolError> {
    if diagnostics_body.len() > MAX_RESPONSE_DIAGNOSTICS_BODY_BYTES {
        return Err(WorkerProtocolError::DiagnosticsTooLarge);
    }
    validate_diagnostics_body(diagnostics_body)?;
    if let Some(provider) = provider_evidence_body {
        if provider.is_empty() || provider.len() > MAX_PROVIDER_EVIDENCE_BYTES {
            return Err(WorkerProtocolError::InvalidFieldLength(8));
        }
        decode_provider_evidence(provider)?;
    }
    Ok(())
}

fn validate_diagnostics_body(bytes: &[u8]) -> Result<(), WorkerProtocolError> {
    let mut cursor = Cursor::new(bytes);
    let count = cursor.u32()? as usize;
    if count > MAX_WORKER_DIAGNOSTICS {
        return Err(WorkerProtocolError::TooManyDiagnostics);
    }
    let mut total = 0_usize;
    let mut previous = None;
    for _ in 0..count {
        let len = cursor.u32()? as usize;
        total = total
            .checked_add(len)
            .ok_or(WorkerProtocolError::IntegerOverflow)?;
        if len == 0 || len > MAX_WORKER_DIAGNOSTIC_BYTES {
            return Err(WorkerProtocolError::InvalidDiagnostic);
        }
        if total > MAX_WORKER_TOTAL_DIAGNOSTIC_BYTES {
            return Err(WorkerProtocolError::DiagnosticsTooLarge);
        }
        let value =
            str::from_utf8(cursor.take(len)?).map_err(|_| WorkerProtocolError::InvalidUtf8)?;
        if !value.is_ascii() || value.bytes().any(|byte| byte.is_ascii_control()) {
            return Err(WorkerProtocolError::InvalidDiagnostic);
        }
        if let Some(prior) = previous {
            if prior == value {
                return Err(WorkerProtocolError::DuplicateDiagnostic);
            }
            if prior > value {
                return Err(WorkerProtocolError::NonCanonicalDiagnostics);
            }
        }
        previous = Some(value);
    }
    cursor.finish()
}

fn validate_complete_output_body(bytes: &[u8]) -> Result<(), WorkerProtocolError> {
    let mut cursor = Cursor::new(bytes);
    if cursor.byte()? != 1 {
        return Err(WorkerProtocolError::InvalidResponseState);
    }
    cursor.fixed::<32>()?;
    let byte_len_u64 = cursor.u64()?;
    let byte_len =
        usize::try_from(byte_len_u64).map_err(|_| WorkerProtocolError::InvalidOutputBound)?;
    if byte_len == 0 || byte_len > MAX_WORKER_OUTPUT_BYTES {
        return Err(WorkerProtocolError::InvalidOutputBound);
    }
    cursor.take(byte_len)?;
    cursor.finish()
}

fn validate_text_body(
    bytes: &[u8],
    max: usize,
    field: &'static str,
) -> Result<(), WorkerProtocolError> {
    if bytes.is_empty() || bytes.len() > max {
        return Err(WorkerProtocolError::InvalidText(field));
    }
    let value = str::from_utf8(bytes).map_err(|_| WorkerProtocolError::InvalidUtf8)?;
    if !value.is_ascii() || value.bytes().any(|byte| byte.is_ascii_control()) {
        return Err(WorkerProtocolError::InvalidText(field));
    }
    Ok(())
}

/// Canonically decoded V2 request/response bytes with no execution provenance.
///
/// This wrapper is intentionally distinct from the compiler-handoff request
/// and execution receipt types. Decoding bytes cannot establish that the
/// compiler emitted the request or that the measured worker executed it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InertDecodedWorkerExchangeV2 {
    request: WorkerRequestV2,
    response: WorkerResponseV2,
}

impl InertDecodedWorkerExchangeV2 {
    /// Strictly decodes one canonical response in the context of one canonical request.
    pub fn decode(
        request_bytes: &[u8],
        response_bytes: &[u8],
    ) -> Result<Self, WorkerProtocolError> {
        let request = decode_request(request_bytes)?;
        let response = WorkerResponseV2::decode_for_request(response_bytes, &request)?;
        Ok(Self { request, response })
    }

    pub const fn request(&self) -> &WorkerRequestV2 {
        &self.request
    }

    pub const fn response(&self) -> &WorkerResponseV2 {
        &self.response
    }

    pub const fn grants_worker_execution_authority(&self) -> bool {
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

fn validate_request_parts(parts: &SealedWorkerRequestV2Parts) -> Result<(), WorkerProtocolError> {
    if parts.request_id == [0; 32] {
        return Err(WorkerProtocolError::EmptyRequestId);
    }
    validate_identity_text(&parts.llvm_build_identity, "LLVM build identity")?;
    validate_identity_text(&parts.worker_build_identity, "worker build identity")?;
    if parts.worker_executable.byte_len() == 0 {
        return Err(WorkerProtocolError::ContentIdentityMismatch);
    }
    if parts.compiler_envelope.0 == [0; 32] {
        return Err(WorkerProtocolError::RequestIdentityMismatch);
    }
    validate_input_order(&parts.external_providers, true)?;
    let mut total = usize::try_from(parts.compiler_module.identity().byte_len())
        .map_err(|_| WorkerProtocolError::InputBytesTooLarge)?;
    for provider in &parts.external_providers {
        total = total
            .checked_add(
                usize::try_from(provider.identity().byte_len())
                    .map_err(|_| WorkerProtocolError::InputBytesTooLarge)?,
            )
            .ok_or(WorkerProtocolError::IntegerOverflow)?;
        if provider.identity() == parts.compiler_module.identity()
            && provider.kind() == parts.compiler_module.kind()
        {
            return Err(WorkerProtocolError::DuplicateInput);
        }
    }
    if total > MAX_WORKER_TOTAL_INPUT_BYTES || parts.external_providers.len() + 1 > MAX_LINK_INPUTS
    {
        return Err(WorkerProtocolError::InputBytesTooLarge);
    }
    validate_symbols(&parts.import_symbols)?;
    validate_symbols(&parts.export_symbols)?;
    validate_symbols(&parts.final_symbols)?;
    if parts.final_symbols.is_empty() {
        return Err(WorkerProtocolError::RequiredSymbolNotExpected);
    }
    validate_symbol_roles(
        &parts.import_symbols,
        &parts.export_symbols,
        &parts.final_symbols,
    )
}

fn validate_symbol_roles(
    imports: &[String],
    exports: &[String],
    final_symbols: &[String],
) -> Result<(), WorkerProtocolError> {
    for symbol in imports {
        if final_symbols.binary_search(symbol).is_err() || exports.binary_search(symbol).is_ok() {
            return Err(WorkerProtocolError::RequiredSymbolNotExpected);
        }
    }
    for symbol in exports {
        if final_symbols.binary_search(symbol).is_err() {
            return Err(WorkerProtocolError::RequiredSymbolNotExpected);
        }
    }
    Ok(())
}

fn validate_input_order(
    inputs: &[WorkerInputV1],
    allow_empty: bool,
) -> Result<(), WorkerProtocolError> {
    if inputs.is_empty() && !allow_empty {
        return Err(WorkerProtocolError::EmptyInput);
    }
    if inputs.len() > MAX_LINK_INPUTS {
        return Err(WorkerProtocolError::TooManyInputs);
    }
    for pair in inputs.windows(2) {
        let left = (pair[0].identity(), pair[0].kind());
        let right = (pair[1].identity(), pair[1].kind());
        if left == right {
            return Err(WorkerProtocolError::DuplicateInput);
        }
        if left > right {
            return Err(WorkerProtocolError::NonCanonicalInputs);
        }
    }
    Ok(())
}

fn encode_request(request: &WorkerRequestV2) -> Result<(Vec<u8>, [u8; 32]), WorkerProtocolError> {
    let module = encode_input(&request.compiler_module)?;
    let providers = encode_inputs(&request.external_providers)?;
    let imports = encode_strings(&request.import_symbols)?;
    let exports = encode_strings(&request.export_symbols)?;
    let final_symbols = encode_strings(&request.final_symbols)?;
    let executable = encode_content_identity(request.worker_executable);
    let target = request.target.to_string();
    let fields = [
        request.request_id.len(),
        request.llvm_build_identity.len(),
        request.worker_build_identity.len(),
        executable.len(),
        target.len(),
        1,
        3,
        32,
        module.len(),
        providers.len(),
        imports.len(),
        exports.len(),
        final_symbols.len(),
        8,
    ];
    let body_len = fields
        .into_iter()
        .try_fold(WORKER_REQUEST_MAGIC_V2.len(), |sum, len| {
            sum.checked_add(6 + len)
                .ok_or(WorkerProtocolError::IntegerOverflow)
        })?;
    let total_len = body_len
        .checked_add(6 + 32)
        .ok_or(WorkerProtocolError::IntegerOverflow)?;
    if total_len > MAX_WORKER_REQUEST_BYTES {
        return Err(WorkerProtocolError::RequestTooLarge);
    }

    let mut encoded = fallible_vec(total_len, "encoded worker request")?;
    encoded.extend_from_slice(WORKER_REQUEST_MAGIC_V2);
    push_field(&mut encoded, 1, &request.request_id)?;
    push_field(&mut encoded, 2, request.llvm_build_identity.as_bytes())?;
    push_field(&mut encoded, 3, request.worker_build_identity.as_bytes())?;
    push_field(&mut encoded, 4, &executable)?;
    push_field(&mut encoded, 5, target.as_bytes())?;
    push_field(
        &mut encoded,
        6,
        &[encode_code_object(request.code_object_version)],
    )?;
    push_field(
        &mut encoded,
        7,
        &[
            request.options.optimization() as u8,
            u8::from(request.options.strip_debug()),
            u8::from(request.options.verify_each()),
        ],
    )?;
    push_field(&mut encoded, 8, &request.compiler_envelope.0)?;
    push_field(&mut encoded, 9, &module)?;
    push_field(&mut encoded, 10, &providers)?;
    push_field(&mut encoded, 11, &imports)?;
    push_field(&mut encoded, 12, &exports)?;
    push_field(&mut encoded, 13, &final_symbols)?;
    push_field(&mut encoded, 14, &request.output.max_bytes().to_le_bytes())?;
    let identity = calculate_request_identity(&encoded);
    push_field(&mut encoded, 15, &identity)?;
    debug_assert_eq!(encoded.len(), total_len);
    Ok((encoded, identity))
}

fn decode_request(bytes: &[u8]) -> Result<WorkerRequestV2, WorkerProtocolError> {
    if bytes.len() > MAX_WORKER_REQUEST_BYTES {
        return Err(WorkerProtocolError::RequestTooLarge);
    }
    let mut decoder = Decoder::new(bytes, WORKER_REQUEST_MAGIC_V2, REQUEST_FIELD_COUNT_V2)?;
    let request_id = fixed::<32>(decoder.field(1, 32)?)?;
    let llvm_build_identity = text(
        decoder.field(2, MAX_WORKER_TOOLCHAIN_ID_BYTES)?,
        MAX_WORKER_TOOLCHAIN_ID_BYTES,
        "LLVM build identity",
    )?;
    let worker_build_identity = text(
        decoder.field(3, MAX_WORKER_TOOLCHAIN_ID_BYTES)?,
        MAX_WORKER_TOOLCHAIN_ID_BYTES,
        "worker build identity",
    )?;
    let worker_executable = decode_content_identity(decoder.field(4, CONTENT_IDENTITY_BYTES)?)?;
    let target_text = text(
        decoder.field(5, MAX_WORKER_TARGET_BYTES)?,
        MAX_WORKER_TARGET_BYTES,
        "target",
    )?;
    let target =
        DeviceTargetV1::parse(&target_text).map_err(|_| WorkerProtocolError::InvalidTarget)?;
    let code_object_version = decode_code_object(one_byte(decoder.field(6, 1)?)?)?;
    let options = decode_options(decoder.field(7, 3)?)?;
    let compiler_envelope =
        WorkerCompilerFfiEnvelopeIdentityV2(fixed::<32>(decoder.field(8, 32)?)?);
    let compiler_module =
        decode_input(decoder.field(9, MAX_WORKER_TOTAL_INPUT_BYTES + INPUT_OVERHEAD_BYTES)?)?;
    let external_providers = decode_inputs(
        decoder.field(
            10,
            MAX_WORKER_TOTAL_INPUT_BYTES + 4 + MAX_LINK_INPUTS * INPUT_OVERHEAD_BYTES,
        )?,
        true,
    )?;
    let max_symbol_field = MAX_WORKER_SYMBOLS * (MAX_WORKER_SYMBOL_BYTES + 4) + 4;
    let import_symbols = decode_symbols(decoder.field(11, max_symbol_field)?)?;
    let export_symbols = decode_symbols(decoder.field(12, max_symbol_field)?)?;
    let final_symbols = decode_symbols(decoder.field(13, max_symbol_field)?)?;
    let output =
        WorkerOutputConstraintsV1::new(u64::from_le_bytes(fixed::<8>(decoder.field(14, 8)?)?))?;
    let identity_field_offset = decoder.position();
    let declared_identity = fixed::<32>(decoder.field(15, 32)?)?;
    decoder.finish(REQUEST_FIELD_COUNT_V2)?;
    if calculate_request_identity(&bytes[..identity_field_offset]) != declared_identity {
        return Err(WorkerProtocolError::RequestIdentityMismatch);
    }
    let parts = SealedWorkerRequestV2Parts {
        request_id,
        llvm_build_identity,
        worker_build_identity,
        worker_executable,
        target,
        code_object_version,
        options,
        compiler_envelope,
        compiler_module,
        external_providers,
        import_symbols,
        export_symbols,
        final_symbols,
        output,
    };
    validate_request_parts(&parts)?;
    let request = WorkerRequestV2 {
        request_id: parts.request_id,
        llvm_build_identity: parts.llvm_build_identity,
        worker_build_identity: parts.worker_build_identity,
        worker_executable: parts.worker_executable,
        target: parts.target,
        code_object_version: parts.code_object_version,
        options: parts.options,
        compiler_envelope: parts.compiler_envelope,
        compiler_module: parts.compiler_module,
        external_providers: parts.external_providers,
        import_symbols: parts.import_symbols,
        export_symbols: parts.export_symbols,
        final_symbols: parts.final_symbols,
        output: parts.output,
        canonical_bytes: copy_bytes(bytes, "decoded request canonical bytes")?,
        identity: declared_identity,
    };
    if encode_request(&request)?.0 != bytes {
        return Err(WorkerProtocolError::NonCanonicalEncoding);
    }
    Ok(request)
}

fn calculate_request_identity(encoded_without_identity: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(REQUEST_DOMAIN_V2);
    hasher.update((encoded_without_identity.len() as u64).to_le_bytes());
    hasher.update(encoded_without_identity);
    hasher.finalize().into()
}

fn encode_content_identity(identity: ContentIdentityV1) -> [u8; CONTENT_IDENTITY_BYTES] {
    let mut encoded = [0; CONTENT_IDENTITY_BYTES];
    encoded[..32].copy_from_slice(identity.sha256());
    encoded[32..].copy_from_slice(&identity.byte_len().to_le_bytes());
    encoded
}

fn decode_content_identity(bytes: &[u8]) -> Result<ContentIdentityV1, WorkerProtocolError> {
    if bytes.len() != CONTENT_IDENTITY_BYTES {
        return Err(WorkerProtocolError::InvalidFieldLength(4));
    }
    let digest = fixed::<32>(&bytes[..32])?;
    let byte_len = u64::from_le_bytes(fixed::<8>(&bytes[32..])?);
    if byte_len == 0 {
        return Err(WorkerProtocolError::ContentIdentityMismatch);
    }
    Ok(ContentIdentityV1::from_parts(digest, byte_len))
}

fn encode_input(input: &WorkerInputV1) -> Result<Vec<u8>, WorkerProtocolError> {
    let capacity = INPUT_OVERHEAD_BYTES
        .checked_add(input.bytes().len())
        .ok_or(WorkerProtocolError::IntegerOverflow)?;
    let mut encoded = fallible_vec(capacity, "encoded worker input")?;
    encoded.push(input.kind() as u8);
    encoded.extend_from_slice(input.identity().sha256());
    encoded.extend_from_slice(&input.identity().byte_len().to_le_bytes());
    encoded.extend_from_slice(input.bytes());
    Ok(encoded)
}

fn decode_input(bytes: &[u8]) -> Result<WorkerInputV1, WorkerProtocolError> {
    let mut cursor = Cursor::new(bytes);
    let kind = decode_input_kind(cursor.byte()?)?;
    let digest = cursor.fixed::<32>()?;
    let byte_len_u64 = cursor.u64()?;
    let byte_len =
        usize::try_from(byte_len_u64).map_err(|_| WorkerProtocolError::InputBytesTooLarge)?;
    if byte_len == 0 || byte_len > MAX_WORKER_TOTAL_INPUT_BYTES {
        return Err(WorkerProtocolError::InputBytesTooLarge);
    }
    let payload = copy_bytes(cursor.take(byte_len)?, "decoded compiler module")?;
    cursor.finish()?;
    WorkerInputV1::from_declared(
        kind,
        ContentIdentityV1::from_parts(digest, byte_len_u64),
        payload,
    )
}

fn encode_inputs(inputs: &[WorkerInputV1]) -> Result<Vec<u8>, WorkerProtocolError> {
    let exact = inputs.iter().try_fold(4_usize, |sum, input| {
        sum.checked_add(INPUT_OVERHEAD_BYTES + input.bytes().len())
            .ok_or(WorkerProtocolError::IntegerOverflow)
    })?;
    let mut encoded = fallible_vec(exact, "encoded worker inputs")?;
    push_u32(&mut encoded, inputs.len())?;
    for input in inputs {
        encoded.extend_from_slice(&encode_input(input)?);
    }
    Ok(encoded)
}

fn decode_inputs(
    bytes: &[u8],
    allow_empty: bool,
) -> Result<Vec<WorkerInputV1>, WorkerProtocolError> {
    let mut cursor = Cursor::new(bytes);
    let count = cursor.u32()? as usize;
    if count > MAX_LINK_INPUTS {
        return Err(WorkerProtocolError::TooManyInputs);
    }
    if count == 0 && !allow_empty {
        return Err(WorkerProtocolError::EmptyInput);
    }
    let mut inputs = fallible_vec(count, "decoded worker input records")?;
    let mut total = 0_usize;
    for _ in 0..count {
        let kind = decode_input_kind(cursor.byte()?)?;
        let digest = cursor.fixed::<32>()?;
        let byte_len_u64 = cursor.u64()?;
        let byte_len =
            usize::try_from(byte_len_u64).map_err(|_| WorkerProtocolError::InputBytesTooLarge)?;
        if byte_len == 0 || byte_len > MAX_WORKER_TOTAL_INPUT_BYTES {
            return Err(WorkerProtocolError::InputBytesTooLarge);
        }
        total = total
            .checked_add(byte_len)
            .ok_or(WorkerProtocolError::IntegerOverflow)?;
        if total > MAX_WORKER_TOTAL_INPUT_BYTES {
            return Err(WorkerProtocolError::InputBytesTooLarge);
        }
        let payload = copy_bytes(cursor.take(byte_len)?, "decoded provider input")?;
        inputs.push(WorkerInputV1::from_declared(
            kind,
            ContentIdentityV1::from_parts(digest, byte_len_u64),
            payload,
        )?);
    }
    cursor.finish()?;
    validate_input_order(&inputs, allow_empty)?;
    Ok(inputs)
}

fn decode_input_kind(value: u8) -> Result<WorkerInputKindV1, WorkerProtocolError> {
    match value {
        1 => Ok(WorkerInputKindV1::LlvmBitcode),
        2 => Ok(WorkerInputKindV1::AmdGpuRelocatable),
        3 => Ok(WorkerInputKindV1::LlvmTextIr),
        _ => Err(WorkerProtocolError::UnknownEnum("input kind")),
    }
}

fn encode_strings(values: &[String]) -> Result<Vec<u8>, WorkerProtocolError> {
    let exact = values.iter().try_fold(4_usize, |sum, value| {
        sum.checked_add(4 + value.len())
            .ok_or(WorkerProtocolError::IntegerOverflow)
    })?;
    let mut encoded = fallible_vec(exact, "encoded worker strings")?;
    push_u32(&mut encoded, values.len())?;
    for value in values {
        push_u32(&mut encoded, value.len())?;
        encoded.extend_from_slice(value.as_bytes());
    }
    Ok(encoded)
}

fn decode_symbols(bytes: &[u8]) -> Result<Vec<String>, WorkerProtocolError> {
    let values = decode_strings(
        bytes,
        MAX_WORKER_SYMBOLS,
        MAX_WORKER_SYMBOL_BYTES,
        MAX_WORKER_SYMBOLS * MAX_WORKER_SYMBOL_BYTES,
        false,
    )?;
    validate_symbols(&values)?;
    Ok(values)
}

fn decode_strings(
    bytes: &[u8],
    max_count: usize,
    max_each: usize,
    max_total: usize,
    diagnostics: bool,
) -> Result<Vec<String>, WorkerProtocolError> {
    let mut cursor = Cursor::new(bytes);
    let count = cursor.u32()? as usize;
    if count > max_count {
        return Err(if diagnostics {
            WorkerProtocolError::TooManyDiagnostics
        } else {
            WorkerProtocolError::TooManySymbols
        });
    }
    let mut values = fallible_vec(count, "decoded worker strings")?;
    let mut total = 0_usize;
    for _ in 0..count {
        let len = cursor.u32()? as usize;
        if len > max_each {
            return Err(if diagnostics {
                WorkerProtocolError::DiagnosticsTooLarge
            } else {
                WorkerProtocolError::InvalidSymbol
            });
        }
        total = total
            .checked_add(len)
            .ok_or(WorkerProtocolError::IntegerOverflow)?;
        if total > max_total {
            return Err(if diagnostics {
                WorkerProtocolError::DiagnosticsTooLarge
            } else {
                WorkerProtocolError::TooManySymbols
            });
        }
        let value =
            str::from_utf8(cursor.take(len)?).map_err(|_| WorkerProtocolError::InvalidUtf8)?;
        values.push(copy_text(value, "decoded worker string")?);
    }
    cursor.finish()?;
    if diagnostics {
        validate_diagnostics(&values)?;
    }
    Ok(values)
}

fn validate_diagnostics(values: &[String]) -> Result<(), WorkerProtocolError> {
    for value in values {
        if value.is_empty()
            || value.len() > MAX_WORKER_DIAGNOSTIC_BYTES
            || !value.is_ascii()
            || value.bytes().any(|byte| byte.is_ascii_control())
        {
            return Err(WorkerProtocolError::InvalidDiagnostic);
        }
    }
    for pair in values.windows(2) {
        if pair[0] == pair[1] {
            return Err(WorkerProtocolError::DuplicateDiagnostic);
        }
        if pair[0] > pair[1] {
            return Err(WorkerProtocolError::NonCanonicalDiagnostics);
        }
    }
    Ok(())
}

fn decode_provider_evidence(
    bytes: &[u8],
) -> Result<WorkerDeviceLibraryProviderEvidenceV1, WorkerProtocolError> {
    let preimage_len = bytes
        .len()
        .checked_sub(32)
        .ok_or(WorkerProtocolError::Truncated)?;
    let mut cursor = Cursor::new(bytes);
    let provider_len = cursor.u32()? as usize;
    let provider_identity = text(
        cursor.take(provider_len)?,
        MAX_PROVIDER_IDENTITY_BYTES,
        "device-library provider identity",
    )?;
    let target_len = cursor.u32()? as usize;
    let target_text = text(
        cursor.take(target_len)?,
        MAX_WORKER_TARGET_BYTES,
        "device-library provider target",
    )?;
    let target =
        DeviceTargetV1::parse(&target_text).map_err(|_| WorkerProtocolError::InvalidTarget)?;
    let code_object_version = decode_code_object(cursor.byte()?)?;

    let import_count = cursor.u32()? as usize;
    if import_count == 0 || import_count > MAX_WORKER_SYMBOLS {
        return Err(WorkerProtocolError::TooManySymbols);
    }
    let mut import_symbols = fallible_vec(import_count, "decoded provider imports")?;
    let mut total_import_bytes = 0_usize;
    for _ in 0..import_count {
        let len = cursor.u32()? as usize;
        if len > MAX_WORKER_SYMBOL_BYTES {
            return Err(WorkerProtocolError::InvalidSymbol);
        }
        total_import_bytes = total_import_bytes
            .checked_add(len)
            .ok_or(WorkerProtocolError::IntegerOverflow)?;
        if total_import_bytes > MAX_WORKER_SYMBOLS * MAX_WORKER_SYMBOL_BYTES {
            return Err(WorkerProtocolError::TooManySymbols);
        }
        let value =
            str::from_utf8(cursor.take(len)?).map_err(|_| WorkerProtocolError::InvalidUtf8)?;
        import_symbols.push(copy_text(value, "decoded provider import")?);
    }
    validate_symbols(&import_symbols)?;

    let file_count = cursor.u32()? as usize;
    if file_count == 0 || file_count > MAX_PROVIDER_FILES {
        return Err(WorkerProtocolError::InvalidFieldLength(8));
    }
    let mut files = fallible_vec(file_count, "decoded provider files")?;
    for _ in 0..file_count {
        let len = cursor.u32()? as usize;
        let basename = text(
            cursor.take(len)?,
            MAX_PROVIDER_BASENAME_BYTES,
            "device-library provider basename",
        )?;
        if basename.contains('/')
            || basename.contains('\\')
            || files
                .iter()
                .any(|file: &WorkerDeviceLibraryProviderFileEvidenceV1| file.basename == basename)
        {
            return Err(WorkerProtocolError::InvalidText(
                "device-library provider basename",
            ));
        }
        files.push(WorkerDeviceLibraryProviderFileEvidenceV1 {
            basename,
            sha256: cursor.fixed::<32>()?,
        });
    }
    if cursor.position != preimage_len {
        return Err(WorkerProtocolError::NonCanonicalEncoding);
    }
    let manifest_identity = cursor.fixed::<32>()?;
    cursor.finish()?;
    if calculate_provider_manifest_identity(&bytes[..preimage_len]) != manifest_identity {
        return Err(WorkerProtocolError::ProviderManifestIdentityMismatch);
    }
    Ok(WorkerDeviceLibraryProviderEvidenceV1 {
        provider_identity,
        target,
        code_object_version,
        import_symbols,
        files,
        manifest_identity,
    })
}

fn calculate_provider_manifest_identity(preimage: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(PROVIDER_MANIFEST_DOMAIN_V1);
    hasher.update((preimage.len() as u64).to_le_bytes());
    hasher.update(preimage);
    hasher.finalize().into()
}

fn calculate_response_identity(encoded_without_identity: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(RESPONSE_DOMAIN_V3);
    hasher.update((encoded_without_identity.len() as u64).to_le_bytes());
    hasher.update(encoded_without_identity);
    hasher.finalize().into()
}

fn decode_output(
    bytes: &[u8],
) -> Result<Option<(ContentIdentityV1, Vec<u8>)>, WorkerProtocolError> {
    let mut cursor = Cursor::new(bytes);
    match cursor.byte()? {
        0 => {
            cursor.finish()?;
            Ok(None)
        }
        1 => {
            let digest = cursor.fixed::<32>()?;
            let byte_len_u64 = cursor.u64()?;
            let byte_len = usize::try_from(byte_len_u64)
                .map_err(|_| WorkerProtocolError::InvalidOutputBound)?;
            if byte_len == 0 || byte_len > MAX_WORKER_OUTPUT_BYTES {
                return Err(WorkerProtocolError::InvalidOutputBound);
            }
            let payload = copy_bytes(cursor.take(byte_len)?, "decoded worker output")?;
            cursor.finish()?;
            let identity = ContentIdentityV1::from_parts(digest, byte_len_u64);
            if !identity.matches(&payload) {
                return Err(WorkerProtocolError::ContentIdentityMismatch);
            }
            Ok(Some((identity, payload)))
        }
        _ => Err(WorkerProtocolError::InvalidResponseState),
    }
}

fn decode_options(bytes: &[u8]) -> Result<WorkerOptionsV1, WorkerProtocolError> {
    if bytes.len() != 3 {
        return Err(WorkerProtocolError::InvalidFieldLength(7));
    }
    let optimization = match bytes[0] {
        0 => WorkerOptimizationLevelV1::O0,
        1 => WorkerOptimizationLevelV1::O1,
        2 => WorkerOptimizationLevelV1::O2,
        3 => WorkerOptimizationLevelV1::O3,
        _ => return Err(WorkerProtocolError::UnsupportedOption),
    };
    Ok(WorkerOptionsV1::new(
        optimization,
        decode_bool(bytes[1])?,
        decode_bool(bytes[2])?,
    ))
}

fn decode_stage(value: u8) -> Result<WorkerStageV1, WorkerProtocolError> {
    match value {
        1 => Ok(WorkerStageV1::Decode),
        2 => Ok(WorkerStageV1::Toolchain),
        3 => Ok(WorkerStageV1::InputValidation),
        4 => Ok(WorkerStageV1::BitcodeLink),
        5 => Ok(WorkerStageV1::Optimization),
        6 => Ok(WorkerStageV1::Codegen),
        7 => Ok(WorkerStageV1::NativeLink),
        8 => Ok(WorkerStageV1::OutputInspection),
        9 => Ok(WorkerStageV1::Complete),
        _ => Err(WorkerProtocolError::UnknownEnum("worker stage")),
    }
}

fn decode_code_object(value: u8) -> Result<CodeObjectVersion, WorkerProtocolError> {
    match value {
        4 => Ok(CodeObjectVersion::V4),
        5 => Ok(CodeObjectVersion::V5),
        6 => Ok(CodeObjectVersion::V6),
        _ => Err(WorkerProtocolError::UnknownEnum("code-object version")),
    }
}

const fn encode_code_object(version: CodeObjectVersion) -> u8 {
    match version {
        CodeObjectVersion::V4 => 4,
        CodeObjectVersion::V5 => 5,
        CodeObjectVersion::V6 => 6,
    }
}

fn decode_bool(value: u8) -> Result<bool, WorkerProtocolError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(WorkerProtocolError::UnsupportedOption),
    }
}

fn validate_identity_text(value: &str, field: &'static str) -> Result<(), WorkerProtocolError> {
    if value.is_empty()
        || value.len() > MAX_WORKER_TOOLCHAIN_ID_BYTES
        || !value.is_ascii()
        || value.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(WorkerProtocolError::InvalidText(field));
    }
    Ok(())
}

fn text(bytes: &[u8], max: usize, field: &'static str) -> Result<String, WorkerProtocolError> {
    if bytes.is_empty() || bytes.len() > max {
        return Err(WorkerProtocolError::InvalidText(field));
    }
    let value = str::from_utf8(bytes).map_err(|_| WorkerProtocolError::InvalidUtf8)?;
    if !value.is_ascii() || value.bytes().any(|byte| byte.is_ascii_control()) {
        return Err(WorkerProtocolError::InvalidText(field));
    }
    copy_text(value, field)
}

fn fallible_vec<T>(
    capacity: usize,
    component: &'static str,
) -> Result<Vec<T>, WorkerProtocolError> {
    let mut value = Vec::new();
    value
        .try_reserve_exact(capacity)
        .map_err(|_| WorkerProtocolError::AllocationFailed(component))?;
    Ok(value)
}

fn copy_bytes(bytes: &[u8], component: &'static str) -> Result<Vec<u8>, WorkerProtocolError> {
    let mut value = fallible_vec(bytes.len(), component)?;
    value.extend_from_slice(bytes);
    Ok(value)
}

fn copy_text(value: &str, component: &'static str) -> Result<String, WorkerProtocolError> {
    let mut output = String::new();
    output
        .try_reserve_exact(value.len())
        .map_err(|_| WorkerProtocolError::AllocationFailed(component))?;
    output.push_str(value);
    Ok(output)
}

fn one_byte(bytes: &[u8]) -> Result<u8, WorkerProtocolError> {
    match bytes {
        [value] => Ok(*value),
        _ => Err(WorkerProtocolError::Truncated),
    }
}

fn fixed<const N: usize>(bytes: &[u8]) -> Result<[u8; N], WorkerProtocolError> {
    bytes.try_into().map_err(|_| WorkerProtocolError::Truncated)
}

fn push_field(encoded: &mut Vec<u8>, tag: u16, bytes: &[u8]) -> Result<(), WorkerProtocolError> {
    let len = u32::try_from(bytes.len()).map_err(|_| WorkerProtocolError::IntegerOverflow)?;
    encoded.extend_from_slice(&tag.to_le_bytes());
    encoded.extend_from_slice(&len.to_le_bytes());
    encoded.extend_from_slice(bytes);
    Ok(())
}

fn push_u32(encoded: &mut Vec<u8>, value: usize) -> Result<(), WorkerProtocolError> {
    encoded.extend_from_slice(
        &u32::try_from(value)
            .map_err(|_| WorkerProtocolError::IntegerOverflow)?
            .to_le_bytes(),
    );
    Ok(())
}

struct Decoder<'a> {
    cursor: Cursor<'a>,
    last_tag: u16,
    max_tag: u16,
}

impl<'a> Decoder<'a> {
    fn new(bytes: &'a [u8], magic: &[u8; 8], max_tag: u16) -> Result<Self, WorkerProtocolError> {
        let mut cursor = Cursor::new(bytes);
        if cursor.take(magic.len())? != magic {
            return Err(WorkerProtocolError::BadMagic);
        }
        Ok(Self {
            cursor,
            last_tag: 0,
            max_tag,
        })
    }

    fn field(&mut self, expected: u16, max_len: usize) -> Result<&'a [u8], WorkerProtocolError> {
        let tag = self.cursor.u16()?;
        if tag > self.max_tag {
            return Err(WorkerProtocolError::UnknownTag(tag));
        }
        if tag == self.last_tag {
            return Err(WorkerProtocolError::DuplicateTag(tag));
        }
        if tag != expected {
            return Err(WorkerProtocolError::NonCanonicalTag {
                expected,
                actual: tag,
            });
        }
        self.last_tag = tag;
        let len = self.cursor.u32()? as usize;
        if len > max_len {
            return Err(WorkerProtocolError::FieldTooLarge(tag));
        }
        self.cursor.take(len)
    }

    const fn position(&self) -> usize {
        self.cursor.position
    }

    fn finish(self, final_tag: u16) -> Result<(), WorkerProtocolError> {
        if self.last_tag != final_tag {
            return Err(WorkerProtocolError::Truncated);
        }
        self.cursor.finish()
    }
}

struct Cursor<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Cursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], WorkerProtocolError> {
        let end = self
            .position
            .checked_add(len)
            .ok_or(WorkerProtocolError::IntegerOverflow)?;
        let value = self
            .bytes
            .get(self.position..end)
            .ok_or(WorkerProtocolError::Truncated)?;
        self.position = end;
        Ok(value)
    }

    fn byte(&mut self) -> Result<u8, WorkerProtocolError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, WorkerProtocolError> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }

    fn u32(&mut self) -> Result<u32, WorkerProtocolError> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }

    fn u64(&mut self) -> Result<u64, WorkerProtocolError> {
        Ok(u64::from_le_bytes(self.fixed()?))
    }

    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], WorkerProtocolError> {
        fixed(self.take(N)?)
    }

    fn finish(self) -> Result<(), WorkerProtocolError> {
        if self.position == self.bytes.len() {
            Ok(())
        } else {
            Err(WorkerProtocolError::TrailingBytes)
        }
    }
}

fn lower_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

impl fmt::Display for WorkerCompilerFfiEnvelopeIdentityV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_hex())
    }
}

#[cfg(test)]
#[allow(unsafe_code)] // Test-only forwarding allocator for deterministic allocation qualification.
mod tests {
    use std::{
        alloc::{GlobalAlloc, Layout, System},
        cell::Cell,
        ptr,
    };

    use super::*;

    const MAX_RECORDED_ALLOCATIONS: usize = 256;

    #[derive(Clone, Copy)]
    struct AllocationProbe {
        enabled: bool,
        fail_at_or_above: usize,
        event_count: usize,
        total_bytes: usize,
        sizes: [usize; MAX_RECORDED_ALLOCATIONS],
        overflowed: bool,
    }

    impl AllocationProbe {
        const fn disabled() -> Self {
            Self {
                enabled: false,
                fail_at_or_above: usize::MAX,
                event_count: 0,
                total_bytes: 0,
                sizes: [0; MAX_RECORDED_ALLOCATIONS],
                overflowed: false,
            }
        }
    }

    thread_local! {
        static ALLOCATION_PROBE: Cell<AllocationProbe> = const {
            Cell::new(AllocationProbe::disabled())
        };
    }

    struct ForwardingProbeAllocator;

    #[global_allocator]
    static TEST_ALLOCATOR: ForwardingProbeAllocator = ForwardingProbeAllocator;

    // The probe is thread-local and forwards every unselected operation to the
    // process allocator. Tests enable it only around one synchronous decode.
    unsafe impl GlobalAlloc for ForwardingProbeAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            if allocation_must_fail(layout.size()) {
                return ptr::null_mut();
            }
            // SAFETY: this forwards the allocator contract and the exact layout.
            let pointer = unsafe { System.alloc(layout) };
            record_allocation(pointer, layout.size());
            pointer
        }

        unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
            // SAFETY: this forwards the pointer and layout supplied by the caller.
            unsafe { System.dealloc(pointer, layout) };
        }

        unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            if allocation_must_fail(new_size) {
                return ptr::null_mut();
            }
            // SAFETY: this forwards the allocator contract and exact arguments.
            let new_pointer = unsafe { System.realloc(pointer, layout, new_size) };
            record_allocation(new_pointer, new_size);
            new_pointer
        }
    }

    fn allocation_must_fail(size: usize) -> bool {
        ALLOCATION_PROBE
            .try_with(|probe| {
                let probe = probe.get();
                probe.enabled && size >= probe.fail_at_or_above
            })
            .unwrap_or(false)
    }

    fn record_allocation(pointer: *mut u8, size: usize) {
        if pointer.is_null() {
            return;
        }
        let _ = ALLOCATION_PROBE.try_with(|probe| {
            let mut state = probe.get();
            if !state.enabled {
                return;
            }
            if let Some(total_bytes) = state.total_bytes.checked_add(size) {
                state.total_bytes = total_bytes;
            } else {
                state.overflowed = true;
            }
            if state.event_count < state.sizes.len() {
                state.sizes[state.event_count] = size;
                state.event_count += 1;
            } else {
                state.overflowed = true;
            }
            probe.set(state);
        });
    }

    fn probe_allocations<T>(
        fail_at_or_above: usize,
        operation: impl FnOnce() -> T,
    ) -> (T, AllocationProbe) {
        ALLOCATION_PROBE.with(|probe| {
            assert!(!probe.get().enabled, "allocation probes must not nest");
            let mut state = AllocationProbe::disabled();
            state.enabled = true;
            state.fail_at_or_above = fail_at_or_above;
            probe.set(state);
        });
        let guard = AllocationProbeGuard;
        let result = operation();
        let snapshot = ALLOCATION_PROBE.with(Cell::get);
        drop(guard);
        (result, snapshot)
    }

    struct AllocationProbeGuard;

    impl Drop for AllocationProbeGuard {
        fn drop(&mut self) {
            let _ = ALLOCATION_PROBE.try_with(|probe| {
                probe.set(AllocationProbe::disabled());
            });
        }
    }

    fn success_response_with_diagnostics(
        request: &WorkerRequestV2,
        output: &[u8],
        diagnostics: &[String],
    ) -> Vec<u8> {
        let mut encoded = Vec::new();
        encoded.extend_from_slice(WORKER_RESPONSE_MAGIC_V2);
        push_field(&mut encoded, 1, request.request_id()).unwrap();
        push_field(&mut encoded, 2, request.identity()).unwrap();
        push_field(
            &mut encoded,
            3,
            &request.compiler_envelope_identity().as_bytes(),
        )
        .unwrap();
        push_field(&mut encoded, 4, request.worker_build_identity().as_bytes()).unwrap();
        push_field(&mut encoded, 5, &[WorkerStageV1::Complete as u8]).unwrap();
        push_field(&mut encoded, 6, &encode_strings(diagnostics).unwrap()).unwrap();
        let identity = ContentIdentityV1::calculate(output);
        let mut output_field = vec![1];
        output_field.extend_from_slice(identity.sha256());
        output_field.extend_from_slice(&identity.byte_len().to_le_bytes());
        output_field.extend_from_slice(output);
        push_field(&mut encoded, 7, &output_field).unwrap();
        encoded
    }

    fn success_response(request: &WorkerRequestV2, output: &[u8]) -> Vec<u8> {
        success_response_with_diagnostics(request, output, &[])
    }

    fn provider_evidence_body(provider_identity: &str, file_digests: [[u8; 32]; 2]) -> Vec<u8> {
        let mut provider = Vec::new();
        for value in [provider_identity, "gfx942:xnack-"] {
            push_u32(&mut provider, value.len()).unwrap();
            provider.extend_from_slice(value.as_bytes());
        }
        provider.push(5);
        push_u32(&mut provider, 1).unwrap();
        push_u32(&mut provider, "external_helper".len()).unwrap();
        provider.extend_from_slice(b"external_helper");
        push_u32(&mut provider, 2).unwrap();
        for (basename, digest) in [("ocml.bc", file_digests[0]), ("isa.bc", file_digests[1])] {
            push_u32(&mut provider, basename.len()).unwrap();
            provider.extend_from_slice(basename.as_bytes());
            provider.extend_from_slice(&digest);
        }
        let manifest_identity = calculate_provider_manifest_identity(&provider);
        provider.extend_from_slice(&manifest_identity);
        provider
    }

    fn provider_response_with_metadata(
        request: &WorkerRequestV2,
        output: &[u8],
        diagnostics: &[String],
        provider_identity: &str,
        file_digests: [[u8; 32]; 2],
    ) -> Vec<u8> {
        let mut encoded = success_response_with_diagnostics(request, output, diagnostics);
        encoded[..8].copy_from_slice(WORKER_RESPONSE_MAGIC_V3);
        let provider = provider_evidence_body(provider_identity, file_digests);
        push_field(&mut encoded, 8, &provider).unwrap();
        let response_identity = calculate_response_identity(&encoded);
        push_field(&mut encoded, 9, &response_identity).unwrap();
        encoded
    }

    fn provider_response(request: &WorkerRequestV2, output: &[u8]) -> Vec<u8> {
        provider_response_with_metadata(
            request,
            output,
            &[],
            "gfx942-ocml-v1",
            [[0x41; 32], [0x42; 32]],
        )
    }

    fn incomplete_response(request: &WorkerRequestV2) -> Vec<u8> {
        let mut encoded = Vec::new();
        encoded.extend_from_slice(WORKER_RESPONSE_MAGIC_V2);
        push_field(&mut encoded, 1, request.request_id()).unwrap();
        push_field(&mut encoded, 2, request.identity()).unwrap();
        push_field(
            &mut encoded,
            3,
            &request.compiler_envelope_identity().as_bytes(),
        )
        .unwrap();
        push_field(&mut encoded, 4, request.worker_build_identity().as_bytes()).unwrap();
        push_field(&mut encoded, 5, &[WorkerStageV1::Codegen as u8]).unwrap();
        push_field(&mut encoded, 6, &0_u32.to_le_bytes()).unwrap();
        push_field(&mut encoded, 7, &[0]).unwrap();
        encoded
    }

    fn response_field_body_range(bytes: &[u8], expected_tag: u16) -> std::ops::Range<usize> {
        let mut cursor = Cursor::new(bytes);
        cursor.take(WORKER_RESPONSE_MAGIC_V2.len()).unwrap();
        loop {
            let tag = cursor.u16().unwrap();
            let len = cursor.u32().unwrap() as usize;
            let start = cursor.position;
            cursor.take(len).unwrap();
            if tag == expected_tag {
                return start..cursor.position;
            }
        }
    }

    fn request() -> WorkerRequestV2 {
        WorkerRequestV2::from_sealed_parts(SealedWorkerRequestV2Parts {
            request_id: [0x11; 32],
            llvm_build_identity: "llvm-v2".to_owned(),
            worker_build_identity: "worker-v2".to_owned(),
            worker_executable: ContentIdentityV1::from_parts([0x22; 32], 1234),
            target: DeviceTargetV1::parse("gfx942:xnack-").unwrap(),
            code_object_version: CodeObjectVersion::V5,
            options: WorkerOptionsV1::new(WorkerOptimizationLevelV1::O2, true, true),
            compiler_envelope: WorkerCompilerFfiEnvelopeIdentityV2([0x33; 32]),
            compiler_module: WorkerInputV1::new(
                WorkerInputKindV1::LlvmBitcode,
                b"compiler-module".to_vec(),
            )
            .unwrap(),
            external_providers: vec![
                WorkerInputV1::new(
                    WorkerInputKindV1::AmdGpuRelocatable,
                    b"provider-object".to_vec(),
                )
                .unwrap(),
            ],
            import_symbols: vec!["external_helper".to_owned()],
            export_symbols: vec!["kernel_entry".to_owned()],
            final_symbols: vec!["external_helper".to_owned(), "kernel_entry".to_owned()],
            output: WorkerOutputConstraintsV1::new(4096).unwrap(),
        })
        .unwrap()
    }

    fn maximum_payload_request() -> WorkerRequestV2 {
        WorkerRequestV2::from_sealed_parts(SealedWorkerRequestV2Parts {
            request_id: [0x51; 32],
            llvm_build_identity: "llvm-v2-allocation-qualification".to_owned(),
            worker_build_identity: "worker-v2-allocation-qualification".to_owned(),
            worker_executable: ContentIdentityV1::from_parts([0x52; 32], 4096),
            target: DeviceTargetV1::parse("gfx942:xnack-").unwrap(),
            code_object_version: CodeObjectVersion::V6,
            options: WorkerOptionsV1::new(WorkerOptimizationLevelV1::O2, true, true),
            compiler_envelope: WorkerCompilerFfiEnvelopeIdentityV2([0x53; 32]),
            compiler_module: WorkerInputV1::new(
                WorkerInputKindV1::LlvmBitcode,
                vec![0x54; MAX_WORKER_TOTAL_INPUT_BYTES],
            )
            .unwrap(),
            external_providers: Vec::new(),
            import_symbols: Vec::new(),
            export_symbols: vec!["kernel".to_owned()],
            final_symbols: vec!["kernel".to_owned()],
            output: WorkerOutputConstraintsV1::new(MAX_WORKER_OUTPUT_BYTES as u64).unwrap(),
        })
        .unwrap()
    }

    #[test]
    fn maximum_request_response_and_output_allocations_are_bounded_and_fallible() {
        let request = maximum_payload_request();
        assert_eq!(
            request.compiler_module().bytes().len(),
            MAX_WORKER_TOTAL_INPUT_BYTES
        );
        assert!(request.canonical_bytes().len() <= MAX_WORKER_REQUEST_BYTES);

        let (decoded_request, request_allocations) = probe_allocations(usize::MAX, || {
            WorkerRequestV2::decode_for_test(request.canonical_bytes())
        });
        let decoded_request = decoded_request.unwrap();
        assert_eq!(decoded_request, request);
        assert!(!request_allocations.overflowed);
        assert!(request_allocations.event_count > 0);
        assert!(
            request_allocations.sizes[..request_allocations.event_count]
                .iter()
                .all(|size| *size <= MAX_WORKER_REQUEST_BYTES)
        );
        assert!(
            request_allocations.total_bytes <= 4 * MAX_WORKER_REQUEST_BYTES + 1024 * 1024,
            "request decode allocated {} bytes",
            request_allocations.total_bytes
        );
        assert!(
            request_allocations.sizes[..request_allocations.event_count]
                .iter()
                .filter(|size| **size >= MAX_WORKER_TOTAL_INPUT_BYTES)
                .count()
                <= 4
        );
        drop(decoded_request);

        let (failed_request, request_failure_allocations) =
            probe_allocations(MAX_WORKER_TOTAL_INPUT_BYTES, || {
                WorkerRequestV2::decode_for_test(request.canonical_bytes())
            });
        assert_eq!(
            failed_request,
            Err(WorkerProtocolError::AllocationFailed(
                "decoded compiler module"
            ))
        );
        assert!(!request_failure_allocations.overflowed);
        assert!(
            request_failure_allocations.sizes[..request_failure_allocations.event_count]
                .iter()
                .all(|size| *size < MAX_WORKER_TOTAL_INPUT_BYTES)
        );

        let output = vec![0x55; MAX_WORKER_OUTPUT_BYTES];
        let response = success_response(&request, &output);
        assert!(response.len() <= MAX_WORKER_RESPONSE_BYTES);
        let (decoded_response, response_allocations) = probe_allocations(usize::MAX, || {
            WorkerResponseV2::decode_for_request(&response, &request)
        });
        let decoded_response = decoded_response.unwrap();
        assert_eq!(
            decoded_response.output().unwrap().bytes().len(),
            MAX_WORKER_OUTPUT_BYTES
        );
        assert!(!response_allocations.overflowed);
        assert!(response_allocations.event_count > 0);
        assert!(
            response_allocations.sizes[..response_allocations.event_count]
                .iter()
                .all(|size| *size <= MAX_WORKER_RESPONSE_BYTES)
        );
        assert!(
            response_allocations.total_bytes <= 2 * MAX_WORKER_RESPONSE_BYTES + 1024 * 1024,
            "response decode allocated {} bytes",
            response_allocations.total_bytes
        );
        assert!(
            response_allocations.sizes[..response_allocations.event_count]
                .iter()
                .filter(|size| **size >= MAX_WORKER_OUTPUT_BYTES)
                .count()
                <= 2
        );
        drop(decoded_response);

        let (failed_response, response_failure_allocations) =
            probe_allocations(MAX_WORKER_OUTPUT_BYTES, || {
                WorkerResponseV2::decode_for_request(&response, &request)
            });
        assert_eq!(
            failed_response,
            Err(WorkerProtocolError::AllocationFailed(
                "decoded worker output"
            ))
        );
        assert!(!response_failure_allocations.overflowed);
        assert!(
            response_failure_allocations.sizes[..response_failure_allocations.event_count]
                .iter()
                .all(|size| *size < MAX_WORKER_OUTPUT_BYTES)
        );
    }

    #[test]
    fn v2_round_trip_and_mutations_fail_closed() {
        let request = request();
        let decoded = WorkerRequestV2::decode_for_test(request.canonical_bytes()).unwrap();
        assert_eq!(decoded, request);
        assert_eq!(
            decoded.evidence_class(),
            WorkerEvidenceClassV2::CompilerFfiLink
        );
        assert!(
            WorkerRequestV2::decode_for_test(
                &request.canonical_bytes()[..request.canonical_bytes().len() - 1]
            )
            .is_err()
        );
        let mut mixed = request.canonical_bytes().to_vec();
        mixed[..8].copy_from_slice(crate::WORKER_REQUEST_MAGIC_V1);
        assert_eq!(
            WorkerRequestV2::decode_for_test(&mixed),
            Err(WorkerProtocolError::BadMagic)
        );
        for index in 0..request.canonical_bytes().len() {
            let mut mutated = request.canonical_bytes().to_vec();
            mutated[index] ^= 0x80;
            assert!(
                WorkerRequestV2::decode_for_test(&mutated).is_err(),
                "accepted V2 mutation at byte {index}"
            );
        }
    }

    #[test]
    fn inert_exchange_decode_binds_response_without_execution_authority() {
        let request_value = request();
        let response = success_response(&request_value, b"linked-cov6");
        let exchange =
            InertDecodedWorkerExchangeV2::decode(request_value.canonical_bytes(), &response)
                .unwrap();

        assert_eq!(exchange.request(), &request_value);
        assert_eq!(
            exchange.response().output().unwrap().bytes(),
            b"linked-cov6"
        );
        assert!(!exchange.grants_worker_execution_authority());
        assert!(!exchange.grants_publication_authority());
        assert!(!exchange.grants_load_authority());
        assert!(!exchange.grants_launch_authority());

        let mut mixed = success_response(&request_value, b"linked-cov6");
        mixed[14] ^= 1;
        assert!(
            InertDecodedWorkerExchangeV2::decode(request_value.canonical_bytes(), &mixed).is_err()
        );
    }

    #[test]
    fn v2_response_replay_metadata_borrows_exact_diagnostics_body() {
        let request_value = request();
        let diagnostics = vec!["codegen complete".to_owned(), "output inspected".to_owned()];
        let encoded =
            success_response_with_diagnostics(&request_value, b"linked-cov6", &diagnostics);
        let response = WorkerResponseV2::decode_for_request(&encoded, &request_value).unwrap();
        let diagnostics_range = response_field_body_range(response.canonical_bytes(), 6);
        let metadata = response.replay_metadata().unwrap();

        assert_eq!(
            metadata.diagnostics_body(),
            &response.canonical_bytes()[diagnostics_range.clone()]
        );
        assert_eq!(
            metadata.diagnostics_body().as_ptr(),
            response.canonical_bytes()[diagnostics_range].as_ptr()
        );
        assert_eq!(
            metadata.diagnostics_body(),
            encode_strings(&diagnostics).unwrap()
        );
        assert_eq!(metadata.provider_evidence_body(), None);

        let incomplete = incomplete_response(&request_value);
        let incomplete = WorkerResponseV2::decode_for_request(&incomplete, &request_value).unwrap();
        assert_eq!(
            incomplete.replay_metadata(),
            Err(WorkerProtocolError::InvalidResponseState)
        );
    }

    #[test]
    fn v3_response_replay_metadata_keeps_two_shells_independent() {
        let request_value = request();
        let bootstrap_diagnostics = vec!["bootstrap complete".to_owned()];
        let replay_diagnostics = vec!["replay complete".to_owned()];
        let bootstrap_bytes = provider_response_with_metadata(
            &request_value,
            b"bootstrap-output",
            &bootstrap_diagnostics,
            "gfx942-ocml-bootstrap-v1",
            [[0x61; 32], [0x62; 32]],
        );
        let replay_bytes = provider_response_with_metadata(
            &request_value,
            b"replay-output",
            &replay_diagnostics,
            "gfx942-ocml-replay-v1",
            [[0x71; 32], [0x72; 32]],
        );
        let bootstrap =
            WorkerResponseV2::decode_for_request(&bootstrap_bytes, &request_value).unwrap();
        let replay = WorkerResponseV2::decode_for_request(&replay_bytes, &request_value).unwrap();
        let bootstrap_metadata = bootstrap.replay_metadata().unwrap();
        let replay_metadata = replay.replay_metadata().unwrap();

        for (response, metadata) in [(&bootstrap, bootstrap_metadata), (&replay, replay_metadata)] {
            let diagnostics_range = response_field_body_range(response.canonical_bytes(), 6);
            let provider_range = response_field_body_range(response.canonical_bytes(), 8);
            assert_eq!(
                metadata.diagnostics_body().as_ptr(),
                response.canonical_bytes()[diagnostics_range].as_ptr()
            );
            assert_eq!(
                metadata.provider_evidence_body().unwrap().as_ptr(),
                response.canonical_bytes()[provider_range].as_ptr()
            );
        }
        assert_ne!(
            bootstrap_metadata.diagnostics_body(),
            replay_metadata.diagnostics_body()
        );
        assert_ne!(
            bootstrap_metadata.provider_evidence_body(),
            replay_metadata.provider_evidence_body()
        );
    }

    #[test]
    fn response_replay_metadata_bytes_seam_rejects_malformed_framing() {
        let request_value = request();
        let valid_v2 = success_response(&request_value, b"linked-cov6");
        assert!(response_replay_metadata_from_bytes(&valid_v2).is_ok());

        let mut bad_magic = valid_v2.clone();
        bad_magic[..8].copy_from_slice(b"F3LRSP04");
        assert_eq!(
            response_replay_metadata_from_bytes(&bad_magic),
            Err(WorkerProtocolError::BadMagic)
        );

        let mut wrong_field_count = valid_v2.clone();
        wrong_field_count[..8].copy_from_slice(WORKER_RESPONSE_MAGIC_V3);
        assert!(response_replay_metadata_from_bytes(&wrong_field_count).is_err());

        let mut wrong_tag = valid_v2.clone();
        let diagnostics_range = response_field_body_range(&wrong_tag, 6);
        wrong_tag[diagnostics_range.start - 6..diagnostics_range.start - 4]
            .copy_from_slice(&5_u16.to_le_bytes());
        assert!(response_replay_metadata_from_bytes(&wrong_tag).is_err());

        let mut wrong_length = valid_v2.clone();
        let request_id_range = response_field_body_range(&wrong_length, 1);
        wrong_length[request_id_range.start - 4..request_id_range.start]
            .copy_from_slice(&31_u32.to_le_bytes());
        assert!(response_replay_metadata_from_bytes(&wrong_length).is_err());

        let mut wrong_stage = valid_v2.clone();
        let stage_range = response_field_body_range(&wrong_stage, 5);
        wrong_stage[stage_range.start] = WorkerStageV1::Codegen as u8;
        assert_eq!(
            response_replay_metadata_from_bytes(&wrong_stage),
            Err(WorkerProtocolError::InvalidResponseState)
        );

        let mut missing_output = valid_v2.clone();
        let output_range = response_field_body_range(&missing_output, 7);
        missing_output[output_range.start] = 0;
        assert_eq!(
            response_replay_metadata_from_bytes(&missing_output),
            Err(WorkerProtocolError::InvalidResponseState)
        );

        let mut trailing = valid_v2;
        trailing.push(0);
        assert_eq!(
            response_replay_metadata_from_bytes(&trailing),
            Err(WorkerProtocolError::TrailingBytes)
        );
    }

    #[test]
    fn response_replay_metadata_shell_bounds_are_exact_and_independent() {
        assert_eq!(MAX_RESPONSE_DIAGNOSTICS_BODY_BYTES, 16_644);
        assert_eq!(MAX_PROVIDER_EVIDENCE_BYTES, 1_067_889);

        let bootstrap_shell = MAX_RESPONSE_DIAGNOSTICS_BODY_BYTES
            .checked_add(MAX_PROVIDER_EVIDENCE_BYTES)
            .unwrap();
        let replay_shell = MAX_RESPONSE_DIAGNOSTICS_BODY_BYTES
            .checked_add(MAX_PROVIDER_EVIDENCE_BYTES)
            .unwrap();
        assert_eq!(
            MAX_WORKER_RESPONSE_REPLAY_METADATA_SHELL_BYTES_V1,
            bootstrap_shell
        );
        assert_eq!(
            MAX_WORKER_RESPONSE_REPLAY_METADATA_SHELL_BYTES_V1,
            1_084_533
        );
        assert_eq!(
            MAX_WORKER_RESPONSE_REPLAY_METADATA_TWO_SHELL_BYTES_V1,
            bootstrap_shell.checked_add(replay_shell).unwrap()
        );
        assert_eq!(
            MAX_WORKER_RESPONSE_REPLAY_METADATA_TWO_SHELL_BYTES_V1,
            2_169_066
        );
    }

    #[test]
    fn provider_extension_binds_manifest_and_complete_response() {
        let request_value = request();
        let response = provider_response(&request_value, b"linked-with-provider");
        let exchange =
            InertDecodedWorkerExchangeV2::decode(request_value.canonical_bytes(), &response)
                .unwrap();
        let provider = exchange.response().device_library_provider().unwrap();
        assert_eq!(provider.provider_identity(), "gfx942-ocml-v1");
        assert_eq!(provider.target(), request_value.target());
        assert_eq!(
            provider.code_object_version(),
            request_value.code_object_version()
        );
        assert_eq!(provider.import_symbols(), ["external_helper"]);
        assert_eq!(provider.files().len(), 2);
        assert_eq!(provider.files()[0].basename(), "ocml.bc");
        assert_eq!(provider.files()[0].sha256(), &[0x41; 32]);
        assert!(exchange.response().response_identity().is_some());

        let mut false_manifest = response.clone();
        let manifest = false_manifest.len() - (6 + 32) - 32;
        false_manifest[manifest] ^= 1;
        assert_eq!(
            InertDecodedWorkerExchangeV2::decode(request_value.canonical_bytes(), &false_manifest),
            Err(WorkerProtocolError::ProviderManifestIdentityMismatch)
        );

        let mut false_response_identity = response;
        *false_response_identity.last_mut().unwrap() ^= 1;
        assert_eq!(
            InertDecodedWorkerExchangeV2::decode(
                request_value.canonical_bytes(),
                &false_response_identity
            ),
            Err(WorkerProtocolError::ResponseIdentityMismatch)
        );
    }
}
