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

#[cfg(test)]
use crate::{
    MAX_WORKER_SYMBOL_BYTES, MAX_WORKER_SYMBOLS, MAX_WORKER_TARGET_BYTES, WorkerInputKindV1,
    WorkerOptimizationLevelV1,
};

pub const WORKER_REQUEST_MAGIC_V2: &[u8; 8] = b"F3LREQ02";
pub const WORKER_RESPONSE_MAGIC_V2: &[u8; 8] = b"F3LRSP02";

const REQUEST_DOMAIN_V2: &[u8] = b"FE2O3/DIRECT-LLVM-WORKER-REQUEST/V2\0";
#[cfg(test)]
const REQUEST_FIELD_COUNT_V2: u16 = 15;
const RESPONSE_FIELD_COUNT_V2: u16 = 7;
const INPUT_OVERHEAD_BYTES: usize = 1 + 32 + 8;
const CONTENT_IDENTITY_BYTES: usize = 32 + 8;

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
        let mut decoder = Decoder::new(bytes, WORKER_RESPONSE_MAGIC_V2, RESPONSE_FIELD_COUNT_V2)?;
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
            decoder.field(
                6,
                MAX_WORKER_TOTAL_DIAGNOSTIC_BYTES + MAX_WORKER_DIAGNOSTICS * 4 + 4,
            )?,
            MAX_WORKER_DIAGNOSTICS,
            MAX_WORKER_DIAGNOSTIC_BYTES,
            MAX_WORKER_TOTAL_DIAGNOSTIC_BYTES,
            true,
        )?;
        let raw_output = decode_output(decoder.field(7, MAX_WORKER_OUTPUT_BYTES + 45)?)?;
        decoder.finish(RESPONSE_FIELD_COUNT_V2)?;

        if request_id != request.request_id
            || request_identity != request.identity
            || compiler_envelope != request.compiler_envelope
        {
            return Err(WorkerProtocolError::RequestIdentityMismatch);
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
            canonical_bytes: bytes.to_vec(),
        })
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
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

    let mut encoded = Vec::with_capacity(total_len);
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

#[cfg(test)]
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
        canonical_bytes: bytes.to_vec(),
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

#[cfg(test)]
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
    let mut encoded = Vec::with_capacity(capacity);
    encoded.push(input.kind() as u8);
    encoded.extend_from_slice(input.identity().sha256());
    encoded.extend_from_slice(&input.identity().byte_len().to_le_bytes());
    encoded.extend_from_slice(input.bytes());
    Ok(encoded)
}

#[cfg(test)]
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
    let payload = cursor.take(byte_len)?.to_vec();
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
    let mut encoded = Vec::with_capacity(exact);
    push_u32(&mut encoded, inputs.len())?;
    for input in inputs {
        encoded.extend_from_slice(&encode_input(input)?);
    }
    Ok(encoded)
}

#[cfg(test)]
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
    let mut inputs = Vec::with_capacity(count);
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
        let payload = cursor.take(byte_len)?.to_vec();
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

#[cfg(test)]
fn decode_input_kind(value: u8) -> Result<WorkerInputKindV1, WorkerProtocolError> {
    match value {
        1 => Ok(WorkerInputKindV1::LlvmBitcode),
        2 => Ok(WorkerInputKindV1::AmdGpuRelocatable),
        _ => Err(WorkerProtocolError::UnknownEnum("input kind")),
    }
}

fn encode_strings(values: &[String]) -> Result<Vec<u8>, WorkerProtocolError> {
    let exact = values.iter().try_fold(4_usize, |sum, value| {
        sum.checked_add(4 + value.len())
            .ok_or(WorkerProtocolError::IntegerOverflow)
    })?;
    let mut encoded = Vec::with_capacity(exact);
    push_u32(&mut encoded, values.len())?;
    for value in values {
        push_u32(&mut encoded, value.len())?;
        encoded.extend_from_slice(value.as_bytes());
    }
    Ok(encoded)
}

#[cfg(test)]
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
    let mut values = Vec::with_capacity(count);
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
        values.push(
            str::from_utf8(cursor.take(len)?)
                .map_err(|_| WorkerProtocolError::InvalidUtf8)?
                .to_owned(),
        );
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
            let payload = cursor.take(byte_len)?.to_vec();
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

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
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
    Ok(value.to_owned())
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

    #[cfg(test)]
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
mod tests {
    use super::*;

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
}
