//! Canonical, bounded protocol for the out-of-process direct LLVM linker.
//!
//! The values in this module are inert evidence and data. They grant no
//! compiler, filesystem, HSACO loading, or kernel launch authority.

use std::{fmt, str};

use fe2o3_kernel_descriptor::{CodeObjectVersion, DeviceTargetV1};
use sha2::{Digest, Sha256};

use crate::{ContentIdentityV1, MAX_LINK_INPUTS};

pub const WORKER_REQUEST_MAGIC_V1: &[u8; 8] = b"F3LREQ01";
pub const WORKER_RESPONSE_MAGIC_V1: &[u8; 8] = b"F3LRSP01";
pub const MAX_WORKER_REQUEST_BYTES: usize = 64 * 1024 * 1024 + 256 * 1024;
pub const MAX_WORKER_TOTAL_INPUT_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_WORKER_OUTPUT_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_WORKER_TOOLCHAIN_ID_BYTES: usize = 160;
pub const MAX_WORKER_TARGET_BYTES: usize = 128;
pub const MAX_WORKER_SYMBOLS: usize = 4096;
pub const MAX_WORKER_SYMBOL_BYTES: usize = 256;
pub const MAX_WORKER_DIAGNOSTICS: usize = 64;
pub const MAX_WORKER_DIAGNOSTIC_BYTES: usize = 1024;
pub const MAX_WORKER_TOTAL_DIAGNOSTIC_BYTES: usize = 16 * 1024;
pub const MAX_WORKER_RESPONSE_BYTES: usize =
    MAX_WORKER_OUTPUT_BYTES + MAX_WORKER_TOTAL_DIAGNOSTIC_BYTES + 4096;

/// Permanent evidence classification for the Worker V1 wire domain.
///
/// Worker V1 has no field for an opaque FFI closure identity. Every request,
/// response, and output in this protocol is therefore generic link evidence,
/// including values independently constructed with FFI-like symbol strings.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WorkerEvidenceClassV1 {
    GenericLink,
}

const REQUEST_DOMAIN_V1: &[u8] = b"FE2O3/DIRECT-LLVM-WORKER-REQUEST/V1\0";
const REQUEST_FIELD_COUNT: u16 = 10;
const RESPONSE_FIELD_COUNT: u16 = 6;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum WorkerInputKindV1 {
    LlvmBitcode = 1,
    AmdGpuRelocatable = 2,
    LlvmTextIr = 3,
}

impl WorkerInputKindV1 {
    fn decode(value: u8) -> Result<Self, WorkerProtocolError> {
        match value {
            1 => Ok(Self::LlvmBitcode),
            2 => Ok(Self::AmdGpuRelocatable),
            3 => Ok(Self::LlvmTextIr),
            _ => Err(WorkerProtocolError::UnknownEnum("input kind")),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum WorkerOptimizationLevelV1 {
    O0 = 0,
    O1 = 1,
    O2 = 2,
    O3 = 3,
}

impl WorkerOptimizationLevelV1 {
    fn decode(value: u8) -> Result<Self, WorkerProtocolError> {
        match value {
            0 => Ok(Self::O0),
            1 => Ok(Self::O1),
            2 => Ok(Self::O2),
            3 => Ok(Self::O3),
            _ => Err(WorkerProtocolError::UnsupportedOption),
        }
    }
}

/// The complete V1 option whitelist. There is no generic flag escape hatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerOptionsV1 {
    optimization: WorkerOptimizationLevelV1,
    strip_debug: bool,
    verify_each: bool,
}

impl WorkerOptionsV1 {
    pub const fn new(
        optimization: WorkerOptimizationLevelV1,
        strip_debug: bool,
        verify_each: bool,
    ) -> Self {
        Self {
            optimization,
            strip_debug,
            verify_each,
        }
    }

    pub const fn optimization(self) -> WorkerOptimizationLevelV1 {
        self.optimization
    }

    pub const fn strip_debug(self) -> bool {
        self.strip_debug
    }

    pub const fn verify_each(self) -> bool {
        self.verify_each
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerInputV1 {
    kind: WorkerInputKindV1,
    identity: ContentIdentityV1,
    bytes: Vec<u8>,
}

impl WorkerInputV1 {
    pub fn new(kind: WorkerInputKindV1, bytes: Vec<u8>) -> Result<Self, WorkerProtocolError> {
        let identity = ContentIdentityV1::calculate(&bytes);
        Self::from_declared(kind, identity, bytes)
    }

    pub fn from_declared(
        kind: WorkerInputKindV1,
        identity: ContentIdentityV1,
        bytes: Vec<u8>,
    ) -> Result<Self, WorkerProtocolError> {
        if bytes.is_empty() {
            return Err(WorkerProtocolError::EmptyInput);
        }
        if bytes.len() > MAX_WORKER_TOTAL_INPUT_BYTES {
            return Err(WorkerProtocolError::InputBytesTooLarge);
        }
        if !identity.matches(&bytes) {
            return Err(WorkerProtocolError::ContentIdentityMismatch);
        }
        Ok(Self {
            kind,
            identity,
            bytes,
        })
    }

    pub const fn kind(&self) -> WorkerInputKindV1 {
        self.kind
    }

    pub const fn identity(&self) -> ContentIdentityV1 {
        self.identity
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerOutputConstraintsV1 {
    max_bytes: u64,
}

impl WorkerOutputConstraintsV1 {
    pub fn new(max_bytes: u64) -> Result<Self, WorkerProtocolError> {
        if max_bytes == 0 || max_bytes > MAX_WORKER_OUTPUT_BYTES as u64 {
            return Err(WorkerProtocolError::InvalidOutputBound);
        }
        Ok(Self { max_bytes })
    }

    pub const fn max_bytes(&self) -> u64 {
        self.max_bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerRequestV1 {
    request_id: [u8; 32],
    llvm_build_identity: String,
    target: DeviceTargetV1,
    code_object_version: CodeObjectVersion,
    options: WorkerOptionsV1,
    inputs: Vec<WorkerInputV1>,
    required_symbols: Vec<String>,
    expected_defined_symbols: Vec<String>,
    output: WorkerOutputConstraintsV1,
    canonical_bytes: Vec<u8>,
    identity: [u8; 32],
}

impl WorkerRequestV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        request_id: [u8; 32],
        llvm_build_identity: impl Into<String>,
        target: DeviceTargetV1,
        code_object_version: CodeObjectVersion,
        options: WorkerOptionsV1,
        inputs: Vec<WorkerInputV1>,
        required_symbols: Vec<String>,
        expected_defined_symbols: Vec<String>,
        output: WorkerOutputConstraintsV1,
    ) -> Result<Self, WorkerProtocolError> {
        let llvm_build_identity = llvm_build_identity.into();
        validate_request_parts(
            &request_id,
            &llvm_build_identity,
            &inputs,
            &required_symbols,
            &expected_defined_symbols,
        )?;
        let mut request = Self {
            request_id,
            llvm_build_identity,
            target,
            code_object_version,
            options,
            inputs,
            required_symbols,
            expected_defined_symbols,
            output,
            canonical_bytes: Vec::new(),
            identity: [0; 32],
        };
        request.canonical_bytes = encode_request(&request)?;
        if request.canonical_bytes.len() > MAX_WORKER_REQUEST_BYTES {
            return Err(WorkerProtocolError::RequestTooLarge);
        }
        request.identity = calculate_request_identity(&request.canonical_bytes);
        Ok(request)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, WorkerProtocolError> {
        if bytes.len() > MAX_WORKER_REQUEST_BYTES {
            return Err(WorkerProtocolError::RequestTooLarge);
        }
        let mut decoder = Decoder::new(bytes, WORKER_REQUEST_MAGIC_V1)?;
        let request_id = decode_fixed::<32>(decoder.field(1, 32)?)?;
        let llvm_build_identity = decode_text(
            decoder.field(2, MAX_WORKER_TOOLCHAIN_ID_BYTES)?,
            MAX_WORKER_TOOLCHAIN_ID_BYTES,
            "LLVM build identity",
        )?;
        let target_text = decode_text(
            decoder.field(3, MAX_WORKER_TARGET_BYTES)?,
            MAX_WORKER_TARGET_BYTES,
            "target",
        )?;
        let target =
            DeviceTargetV1::parse(&target_text).map_err(|_| WorkerProtocolError::InvalidTarget)?;
        let code_object_version = decode_code_object(decoder.field(4, 1)?)?;
        let options = decode_options(decoder.field(5, 3)?)?;
        let inputs = decode_inputs(decoder.field(6, MAX_WORKER_TOTAL_INPUT_BYTES + 8192)?)?;
        let required_symbols = decode_symbols(
            decoder.field(7, MAX_WORKER_SYMBOLS * (MAX_WORKER_SYMBOL_BYTES + 4) + 4)?,
        )?;
        let expected_defined_symbols = decode_symbols(
            decoder.field(8, MAX_WORKER_SYMBOLS * (MAX_WORKER_SYMBOL_BYTES + 4) + 4)?,
        )?;
        let output = decode_output_constraints(decoder.field(9, 8)?)?;
        let declared_identity = decode_fixed::<32>(decoder.field(10, 32)?)?;
        decoder.finish(REQUEST_FIELD_COUNT)?;

        let mut request = Self::new(
            request_id,
            llvm_build_identity,
            target,
            code_object_version,
            options,
            inputs,
            required_symbols,
            expected_defined_symbols,
            output,
        )?;
        let encoded_without_identity = encode_request_without_identity(&request)?;
        let calculated = calculate_request_identity_without_final_field(&encoded_without_identity);
        if declared_identity != calculated {
            return Err(WorkerProtocolError::RequestIdentityMismatch);
        }
        request.identity = declared_identity;
        request.canonical_bytes = bytes.to_vec();
        if request.canonical_bytes != encode_request(&request)? {
            return Err(WorkerProtocolError::NonCanonicalEncoding);
        }
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

    pub const fn target(&self) -> DeviceTargetV1 {
        self.target
    }

    pub const fn code_object_version(&self) -> CodeObjectVersion {
        self.code_object_version
    }

    pub const fn options(&self) -> WorkerOptionsV1 {
        self.options
    }

    pub fn inputs(&self) -> &[WorkerInputV1] {
        &self.inputs
    }

    pub fn required_symbols(&self) -> &[String] {
        &self.required_symbols
    }

    pub fn expected_defined_symbols(&self) -> &[String] {
        &self.expected_defined_symbols
    }

    pub const fn output_constraints(&self) -> &WorkerOutputConstraintsV1 {
        &self.output
    }

    /// Classifies this request without granting artifact or FFI provenance.
    pub const fn evidence_class(&self) -> WorkerEvidenceClassV1 {
        WorkerEvidenceClassV1::GenericLink
    }

    pub const fn grants_link_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum WorkerStageV1 {
    Decode = 1,
    Toolchain = 2,
    InputValidation = 3,
    BitcodeLink = 4,
    Optimization = 5,
    Codegen = 6,
    NativeLink = 7,
    OutputInspection = 8,
    Complete = 9,
}

impl WorkerStageV1 {
    fn decode(value: u8) -> Result<Self, WorkerProtocolError> {
        match value {
            1 => Ok(Self::Decode),
            2 => Ok(Self::Toolchain),
            3 => Ok(Self::InputValidation),
            4 => Ok(Self::BitcodeLink),
            5 => Ok(Self::Optimization),
            6 => Ok(Self::Codegen),
            7 => Ok(Self::NativeLink),
            8 => Ok(Self::OutputInspection),
            9 => Ok(Self::Complete),
            _ => Err(WorkerProtocolError::UnknownEnum("worker stage")),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerOutputV1 {
    identity: ContentIdentityV1,
    bytes: Vec<u8>,
}

impl WorkerOutputV1 {
    pub fn new(bytes: Vec<u8>) -> Result<Self, WorkerProtocolError> {
        if bytes.is_empty() || bytes.len() > MAX_WORKER_OUTPUT_BYTES {
            return Err(WorkerProtocolError::InvalidOutputBound);
        }
        Ok(Self {
            identity: ContentIdentityV1::calculate(&bytes),
            bytes,
        })
    }

    pub const fn identity(&self) -> ContentIdentityV1 {
        self.identity
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Classifies this output independently of its byte contents.
    pub const fn evidence_class(&self) -> WorkerEvidenceClassV1 {
        WorkerEvidenceClassV1::GenericLink
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerResponseV1 {
    request_id: [u8; 32],
    request_identity: [u8; 32],
    worker_build_identity: String,
    stage: WorkerStageV1,
    diagnostics: Vec<String>,
    output: Option<WorkerOutputV1>,
    canonical_bytes: Vec<u8>,
}

impl WorkerResponseV1 {
    pub fn success(
        request: &WorkerRequestV1,
        worker_build_identity: impl Into<String>,
        diagnostics: Vec<String>,
        output: WorkerOutputV1,
    ) -> Result<Self, WorkerProtocolError> {
        if output.bytes.len() as u64 > request.output.max_bytes {
            return Err(WorkerProtocolError::InvalidOutputBound);
        }
        Self::new(
            request.request_id,
            request.identity,
            worker_build_identity.into(),
            WorkerStageV1::Complete,
            diagnostics,
            Some(output),
        )
    }

    pub fn failure(
        request_id: [u8; 32],
        request_identity: [u8; 32],
        worker_build_identity: impl Into<String>,
        stage: WorkerStageV1,
        diagnostics: Vec<String>,
    ) -> Result<Self, WorkerProtocolError> {
        if stage == WorkerStageV1::Complete {
            return Err(WorkerProtocolError::InvalidResponseState);
        }
        Self::new(
            request_id,
            request_identity,
            worker_build_identity.into(),
            stage,
            diagnostics,
            None,
        )
    }

    fn new(
        request_id: [u8; 32],
        request_identity: [u8; 32],
        worker_build_identity: String,
        stage: WorkerStageV1,
        diagnostics: Vec<String>,
        output: Option<WorkerOutputV1>,
    ) -> Result<Self, WorkerProtocolError> {
        validate_identity_text(&worker_build_identity)?;
        validate_diagnostics(&diagnostics)?;
        if (stage == WorkerStageV1::Complete) != output.is_some() {
            return Err(WorkerProtocolError::InvalidResponseState);
        }
        let mut response = Self {
            request_id,
            request_identity,
            worker_build_identity,
            stage,
            diagnostics,
            output,
            canonical_bytes: Vec::new(),
        };
        response.canonical_bytes = encode_response(&response)?;
        if response.canonical_bytes.len() > MAX_WORKER_RESPONSE_BYTES {
            return Err(WorkerProtocolError::ResponseTooLarge);
        }
        Ok(response)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, WorkerProtocolError> {
        if bytes.len() > MAX_WORKER_RESPONSE_BYTES {
            return Err(WorkerProtocolError::ResponseTooLarge);
        }
        let mut decoder = Decoder::new(bytes, WORKER_RESPONSE_MAGIC_V1)?;
        let request_id = decode_fixed::<32>(decoder.field(1, 32)?)?;
        let request_identity = decode_fixed::<32>(decoder.field(2, 32)?)?;
        let worker_build_identity = decode_text(
            decoder.field(3, MAX_WORKER_TOOLCHAIN_ID_BYTES)?,
            MAX_WORKER_TOOLCHAIN_ID_BYTES,
            "worker build identity",
        )?;
        let stage = WorkerStageV1::decode(decode_byte(decoder.field(4, 1)?)?)?;
        let diagnostics = decode_diagnostics(decoder.field(
            5,
            MAX_WORKER_TOTAL_DIAGNOSTIC_BYTES + MAX_WORKER_DIAGNOSTICS * 4 + 4,
        )?)?;
        let output = decode_output(decoder.field(6, MAX_WORKER_OUTPUT_BYTES + 45)?)?;
        decoder.finish(RESPONSE_FIELD_COUNT)?;
        let response = Self::new(
            request_id,
            request_identity,
            worker_build_identity,
            stage,
            diagnostics,
            output,
        )?;
        if response.canonical_bytes != bytes {
            return Err(WorkerProtocolError::NonCanonicalEncoding);
        }
        Ok(response)
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

    pub fn worker_build_identity(&self) -> &str {
        &self.worker_build_identity
    }

    pub const fn stage(&self) -> WorkerStageV1 {
        self.stage
    }

    pub fn diagnostics(&self) -> &[String] {
        &self.diagnostics
    }

    pub const fn output(&self) -> Option<&WorkerOutputV1> {
        self.output.as_ref()
    }

    pub fn binds_request(&self, request: &WorkerRequestV1) -> bool {
        self.request_id == request.request_id && self.request_identity == request.identity
    }

    /// Classifies this response independently of success or output contents.
    pub const fn evidence_class(&self) -> WorkerEvidenceClassV1 {
        WorkerEvidenceClassV1::GenericLink
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WorkerProtocolError {
    BadMagic,
    RequestTooLarge,
    ResponseTooLarge,
    Truncated,
    TrailingBytes,
    UnknownTag(u16),
    DuplicateTag(u16),
    NonCanonicalTag { expected: u16, actual: u16 },
    FieldTooLarge(u16),
    InvalidFieldLength(u16),
    InvalidUtf8,
    InvalidText(&'static str),
    InvalidTarget,
    UnknownEnum(&'static str),
    UnsupportedOption,
    EmptyBuildIdentity,
    EmptyRequestId,
    EmptyInput,
    TooManyInputs,
    InputBytesTooLarge,
    ContentIdentityMismatch,
    NonCanonicalInputs,
    DuplicateInput,
    TooManySymbols,
    InvalidSymbol,
    NonCanonicalSymbols,
    DuplicateSymbol,
    RequiredSymbolNotExpected,
    InvalidOutputBound,
    RequestIdentityMismatch,
    ProviderEvidenceMismatch,
    ProviderManifestIdentityMismatch,
    ResponseIdentityMismatch,
    TooManyDiagnostics,
    DiagnosticsTooLarge,
    InvalidDiagnostic,
    NonCanonicalDiagnostics,
    DuplicateDiagnostic,
    InvalidResponseState,
    NonCanonicalEncoding,
    IntegerOverflow,
}

impl fmt::Display for WorkerProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadMagic => formatter.write_str("invalid worker protocol magic/version"),
            Self::RequestTooLarge => formatter.write_str("worker request exceeds its bound"),
            Self::ResponseTooLarge => formatter.write_str("worker response exceeds its bound"),
            Self::Truncated => formatter.write_str("worker message is truncated"),
            Self::TrailingBytes => formatter.write_str("worker message has trailing bytes"),
            Self::UnknownTag(tag) => write!(formatter, "unknown worker field tag {tag}"),
            Self::DuplicateTag(tag) => write!(formatter, "duplicate worker field tag {tag}"),
            Self::NonCanonicalTag { expected, actual } => write!(
                formatter,
                "worker field tag {actual} is not canonical; expected {expected}"
            ),
            Self::FieldTooLarge(tag) => write!(formatter, "worker field {tag} exceeds its bound"),
            Self::InvalidFieldLength(tag) => {
                write!(formatter, "worker field {tag} has an invalid length")
            }
            Self::InvalidUtf8 => formatter.write_str("worker text is not UTF-8"),
            Self::InvalidText(field) => write!(formatter, "invalid canonical {field}"),
            Self::InvalidTarget => formatter.write_str("invalid canonical AMD target"),
            Self::UnknownEnum(field) => write!(formatter, "unknown {field} discriminant"),
            Self::UnsupportedOption => formatter.write_str("unsupported worker option"),
            Self::EmptyBuildIdentity => formatter.write_str("LLVM build identity is empty"),
            Self::EmptyRequestId => formatter.write_str("request ID is the reserved zero value"),
            Self::EmptyInput => formatter.write_str("worker inputs must not be empty"),
            Self::TooManyInputs => formatter.write_str("too many worker inputs"),
            Self::InputBytesTooLarge => formatter.write_str("worker input bytes exceed the bound"),
            Self::ContentIdentityMismatch => {
                formatter.write_str("declared content identity does not match bytes")
            }
            Self::NonCanonicalInputs => formatter.write_str("worker inputs are not canonical"),
            Self::DuplicateInput => formatter.write_str("duplicate worker input"),
            Self::TooManySymbols => formatter.write_str("too many worker symbols"),
            Self::InvalidSymbol => formatter.write_str("invalid worker symbol"),
            Self::NonCanonicalSymbols => formatter.write_str("worker symbols are not canonical"),
            Self::DuplicateSymbol => formatter.write_str("duplicate worker symbol"),
            Self::RequiredSymbolNotExpected => {
                formatter.write_str("a required symbol is absent from the exact defined-symbol set")
            }
            Self::InvalidOutputBound => formatter.write_str("invalid worker output bound"),
            Self::RequestIdentityMismatch => formatter.write_str("request identity mismatch"),
            Self::ProviderEvidenceMismatch => {
                formatter.write_str("device-library provider evidence does not match request")
            }
            Self::ProviderManifestIdentityMismatch => {
                formatter.write_str("device-library provider manifest identity mismatch")
            }
            Self::ResponseIdentityMismatch => {
                formatter.write_str("worker response identity mismatch")
            }
            Self::TooManyDiagnostics => formatter.write_str("too many worker diagnostics"),
            Self::DiagnosticsTooLarge => formatter.write_str("worker diagnostics exceed the bound"),
            Self::InvalidDiagnostic => formatter.write_str("invalid worker diagnostic"),
            Self::NonCanonicalDiagnostics => {
                formatter.write_str("worker diagnostics are not canonical")
            }
            Self::DuplicateDiagnostic => formatter.write_str("duplicate worker diagnostic"),
            Self::InvalidResponseState => formatter.write_str("invalid worker response state"),
            Self::NonCanonicalEncoding => formatter.write_str("noncanonical worker encoding"),
            Self::IntegerOverflow => formatter.write_str("worker message integer overflow"),
        }
    }
}

impl std::error::Error for WorkerProtocolError {}

fn validate_request_parts(
    request_id: &[u8; 32],
    llvm_build_identity: &str,
    inputs: &[WorkerInputV1],
    required_symbols: &[String],
    expected_defined_symbols: &[String],
) -> Result<(), WorkerProtocolError> {
    if request_id.iter().all(|byte| *byte == 0) {
        return Err(WorkerProtocolError::EmptyRequestId);
    }
    validate_identity_text(llvm_build_identity)?;
    if inputs.is_empty() {
        return Err(WorkerProtocolError::EmptyInput);
    }
    if inputs.len() > MAX_LINK_INPUTS {
        return Err(WorkerProtocolError::TooManyInputs);
    }
    let total = inputs.iter().try_fold(0usize, |total, input| {
        total
            .checked_add(input.bytes.len())
            .ok_or(WorkerProtocolError::IntegerOverflow)
    })?;
    if total > MAX_WORKER_TOTAL_INPUT_BYTES {
        return Err(WorkerProtocolError::InputBytesTooLarge);
    }
    validate_input_order(inputs)?;
    validate_symbols(required_symbols)?;
    validate_symbols(expected_defined_symbols)?;
    if !required_symbols
        .iter()
        .all(|symbol| expected_defined_symbols.binary_search(symbol).is_ok())
    {
        return Err(WorkerProtocolError::RequiredSymbolNotExpected);
    }
    Ok(())
}

fn validate_identity_text(text: &str) -> Result<(), WorkerProtocolError> {
    if text.is_empty() {
        return Err(WorkerProtocolError::EmptyBuildIdentity);
    }
    if text.len() > MAX_WORKER_TOOLCHAIN_ID_BYTES
        || !text.is_ascii()
        || text.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(WorkerProtocolError::InvalidText("LLVM build identity"));
    }
    Ok(())
}

fn validate_input_order(inputs: &[WorkerInputV1]) -> Result<(), WorkerProtocolError> {
    for pair in inputs.windows(2) {
        let before = (pair[0].identity, pair[0].kind);
        let after = (pair[1].identity, pair[1].kind);
        if before == after {
            return Err(WorkerProtocolError::DuplicateInput);
        }
        if before > after {
            return Err(WorkerProtocolError::NonCanonicalInputs);
        }
    }
    Ok(())
}

pub(crate) fn validate_symbols(symbols: &[String]) -> Result<(), WorkerProtocolError> {
    if symbols.len() > MAX_WORKER_SYMBOLS {
        return Err(WorkerProtocolError::TooManySymbols);
    }
    for symbol in symbols {
        if symbol.is_empty()
            || symbol.len() > MAX_WORKER_SYMBOL_BYTES
            || !symbol.is_ascii()
            || symbol.bytes().any(|byte| {
                byte.is_ascii_control()
                    || byte.is_ascii_whitespace()
                    || matches!(byte, b'/' | b'\\' | b'\'' | b'\"')
            })
        {
            return Err(WorkerProtocolError::InvalidSymbol);
        }
    }
    for pair in symbols.windows(2) {
        if pair[0] == pair[1] {
            return Err(WorkerProtocolError::DuplicateSymbol);
        }
        if pair[0] > pair[1] {
            return Err(WorkerProtocolError::NonCanonicalSymbols);
        }
    }
    Ok(())
}

fn validate_diagnostics(diagnostics: &[String]) -> Result<(), WorkerProtocolError> {
    if diagnostics.len() > MAX_WORKER_DIAGNOSTICS {
        return Err(WorkerProtocolError::TooManyDiagnostics);
    }
    let mut total = 0usize;
    for diagnostic in diagnostics {
        if diagnostic.is_empty()
            || diagnostic.len() > MAX_WORKER_DIAGNOSTIC_BYTES
            || !diagnostic.is_ascii()
            || diagnostic
                .bytes()
                .any(|byte| byte == 0 || (!byte.is_ascii_graphic() && byte != b' '))
        {
            return Err(WorkerProtocolError::InvalidDiagnostic);
        }
        total = total
            .checked_add(diagnostic.len())
            .ok_or(WorkerProtocolError::IntegerOverflow)?;
    }
    if total > MAX_WORKER_TOTAL_DIAGNOSTIC_BYTES {
        return Err(WorkerProtocolError::DiagnosticsTooLarge);
    }
    for pair in diagnostics.windows(2) {
        if pair[0] == pair[1] {
            return Err(WorkerProtocolError::DuplicateDiagnostic);
        }
        if pair[0] > pair[1] {
            return Err(WorkerProtocolError::NonCanonicalDiagnostics);
        }
    }
    Ok(())
}

fn encode_request(request: &WorkerRequestV1) -> Result<Vec<u8>, WorkerProtocolError> {
    let mut encoded = encode_request_without_identity(request)?;
    let identity = calculate_request_identity_without_final_field(&encoded);
    push_field(&mut encoded, 10, &identity)?;
    Ok(encoded)
}

fn encode_request_without_identity(
    request: &WorkerRequestV1,
) -> Result<Vec<u8>, WorkerProtocolError> {
    let mut encoded = WORKER_REQUEST_MAGIC_V1.to_vec();
    push_field(&mut encoded, 1, &request.request_id)?;
    push_field(&mut encoded, 2, request.llvm_build_identity.as_bytes())?;
    push_field(&mut encoded, 3, request.target.to_string().as_bytes())?;
    push_field(
        &mut encoded,
        4,
        &[encode_code_object(request.code_object_version)],
    )?;
    push_field(
        &mut encoded,
        5,
        &[
            request.options.optimization as u8,
            u8::from(request.options.strip_debug),
            u8::from(request.options.verify_each),
        ],
    )?;
    let inputs = encode_inputs(&request.inputs)?;
    push_field(&mut encoded, 6, &inputs)?;
    let required = encode_strings(&request.required_symbols)?;
    push_field(&mut encoded, 7, &required)?;
    let defined = encode_strings(&request.expected_defined_symbols)?;
    push_field(&mut encoded, 8, &defined)?;
    push_field(&mut encoded, 9, &request.output.max_bytes.to_le_bytes())?;
    Ok(encoded)
}

fn calculate_request_identity(encoded: &[u8]) -> [u8; 32] {
    // The final field is fixed-width: tag + length + digest.
    calculate_request_identity_without_final_field(&encoded[..encoded.len() - 38])
}

fn calculate_request_identity_without_final_field(encoded: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(REQUEST_DOMAIN_V1);
    hasher.update((encoded.len() as u64).to_le_bytes());
    hasher.update(encoded);
    hasher.finalize().into()
}

fn encode_inputs(inputs: &[WorkerInputV1]) -> Result<Vec<u8>, WorkerProtocolError> {
    let mut encoded = Vec::new();
    push_u32(&mut encoded, inputs.len())?;
    for input in inputs {
        encoded.push(input.kind as u8);
        encoded.extend_from_slice(input.identity.sha256());
        encoded.extend_from_slice(&input.identity.byte_len().to_le_bytes());
        encoded.extend_from_slice(&input.bytes);
    }
    Ok(encoded)
}

fn decode_inputs(bytes: &[u8]) -> Result<Vec<WorkerInputV1>, WorkerProtocolError> {
    let mut cursor = Cursor::new(bytes);
    let count = cursor.u32()? as usize;
    if count == 0 {
        return Err(WorkerProtocolError::EmptyInput);
    }
    if count > MAX_LINK_INPUTS {
        return Err(WorkerProtocolError::TooManyInputs);
    }
    let mut inputs = Vec::with_capacity(count);
    let mut total = 0usize;
    for _ in 0..count {
        let kind = WorkerInputKindV1::decode(cursor.byte()?)?;
        let digest = cursor.fixed::<32>()?;
        let byte_len_u64 = cursor.u64()?;
        let byte_len =
            usize::try_from(byte_len_u64).map_err(|_| WorkerProtocolError::InputBytesTooLarge)?;
        if byte_len == 0 || byte_len > MAX_WORKER_TOTAL_INPUT_BYTES {
            return Err(if byte_len == 0 {
                WorkerProtocolError::EmptyInput
            } else {
                WorkerProtocolError::InputBytesTooLarge
            });
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
    validate_input_order(&inputs)?;
    Ok(inputs)
}

fn encode_response(response: &WorkerResponseV1) -> Result<Vec<u8>, WorkerProtocolError> {
    let mut encoded = WORKER_RESPONSE_MAGIC_V1.to_vec();
    push_field(&mut encoded, 1, &response.request_id)?;
    push_field(&mut encoded, 2, &response.request_identity)?;
    push_field(&mut encoded, 3, response.worker_build_identity.as_bytes())?;
    push_field(&mut encoded, 4, &[response.stage as u8])?;
    let diagnostics = encode_strings(&response.diagnostics)?;
    push_field(&mut encoded, 5, &diagnostics)?;
    let output = encode_output(response.output.as_ref())?;
    push_field(&mut encoded, 6, &output)?;
    Ok(encoded)
}

fn encode_output(output: Option<&WorkerOutputV1>) -> Result<Vec<u8>, WorkerProtocolError> {
    let Some(output) = output else {
        return Ok(vec![0]);
    };
    let mut encoded = Vec::with_capacity(output.bytes.len() + 41);
    encoded.push(1);
    encoded.extend_from_slice(output.identity.sha256());
    encoded.extend_from_slice(&output.identity.byte_len().to_le_bytes());
    encoded.extend_from_slice(&output.bytes);
    Ok(encoded)
}

fn decode_output(bytes: &[u8]) -> Result<Option<WorkerOutputV1>, WorkerProtocolError> {
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
            let output = WorkerOutputV1::new(payload)?;
            if output.identity != ContentIdentityV1::from_parts(digest, byte_len_u64) {
                return Err(WorkerProtocolError::ContentIdentityMismatch);
            }
            Ok(Some(output))
        }
        _ => Err(WorkerProtocolError::UnknownEnum("output presence")),
    }
}

fn encode_strings(values: &[String]) -> Result<Vec<u8>, WorkerProtocolError> {
    let mut encoded = Vec::new();
    push_u32(&mut encoded, values.len())?;
    for value in values {
        push_u32(&mut encoded, value.len())?;
        encoded.extend_from_slice(value.as_bytes());
    }
    Ok(encoded)
}

fn decode_symbols(bytes: &[u8]) -> Result<Vec<String>, WorkerProtocolError> {
    decode_strings(
        bytes,
        MAX_WORKER_SYMBOLS,
        MAX_WORKER_SYMBOL_BYTES,
        MAX_WORKER_SYMBOLS * MAX_WORKER_SYMBOL_BYTES,
        "symbol",
    )
    .and_then(|symbols| {
        validate_symbols(&symbols)?;
        Ok(symbols)
    })
}

fn decode_diagnostics(bytes: &[u8]) -> Result<Vec<String>, WorkerProtocolError> {
    decode_strings(
        bytes,
        MAX_WORKER_DIAGNOSTICS,
        MAX_WORKER_DIAGNOSTIC_BYTES,
        MAX_WORKER_TOTAL_DIAGNOSTIC_BYTES,
        "diagnostic",
    )
    .and_then(|diagnostics| {
        validate_diagnostics(&diagnostics)?;
        Ok(diagnostics)
    })
}

fn decode_strings(
    bytes: &[u8],
    max_count: usize,
    max_each: usize,
    max_total: usize,
    field: &'static str,
) -> Result<Vec<String>, WorkerProtocolError> {
    let mut cursor = Cursor::new(bytes);
    let count = cursor.u32()? as usize;
    if count > max_count {
        return Err(if field == "diagnostic" {
            WorkerProtocolError::TooManyDiagnostics
        } else {
            WorkerProtocolError::TooManySymbols
        });
    }
    let mut values = Vec::with_capacity(count);
    let mut total = 0usize;
    for _ in 0..count {
        let len = cursor.u32()? as usize;
        if len > max_each {
            return Err(if field == "diagnostic" {
                WorkerProtocolError::DiagnosticsTooLarge
            } else {
                WorkerProtocolError::InvalidSymbol
            });
        }
        total = total
            .checked_add(len)
            .ok_or(WorkerProtocolError::IntegerOverflow)?;
        if total > max_total {
            return Err(if field == "diagnostic" {
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
    Ok(values)
}

fn decode_options(bytes: &[u8]) -> Result<WorkerOptionsV1, WorkerProtocolError> {
    if bytes.len() != 3 {
        return Err(WorkerProtocolError::InvalidFieldLength(5));
    }
    Ok(WorkerOptionsV1::new(
        WorkerOptimizationLevelV1::decode(bytes[0])?,
        decode_bool(bytes[1])?,
        decode_bool(bytes[2])?,
    ))
}

fn decode_output_constraints(
    bytes: &[u8],
) -> Result<WorkerOutputConstraintsV1, WorkerProtocolError> {
    WorkerOutputConstraintsV1::new(u64::from_le_bytes(decode_fixed::<8>(bytes)?))
}

fn decode_code_object(bytes: &[u8]) -> Result<CodeObjectVersion, WorkerProtocolError> {
    match decode_byte(bytes)? {
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

fn decode_byte(bytes: &[u8]) -> Result<u8, WorkerProtocolError> {
    if let [byte] = bytes {
        Ok(*byte)
    } else {
        Err(WorkerProtocolError::Truncated)
    }
}

fn decode_fixed<const N: usize>(bytes: &[u8]) -> Result<[u8; N], WorkerProtocolError> {
    bytes.try_into().map_err(|_| WorkerProtocolError::Truncated)
}

fn decode_text(
    bytes: &[u8],
    max: usize,
    field: &'static str,
) -> Result<String, WorkerProtocolError> {
    if bytes.is_empty() || bytes.len() > max {
        return Err(WorkerProtocolError::InvalidText(field));
    }
    let text = str::from_utf8(bytes).map_err(|_| WorkerProtocolError::InvalidUtf8)?;
    if !text.is_ascii() || text.bytes().any(|byte| byte.is_ascii_control()) {
        return Err(WorkerProtocolError::InvalidText(field));
    }
    Ok(text.to_owned())
}

fn push_field(encoded: &mut Vec<u8>, tag: u16, bytes: &[u8]) -> Result<(), WorkerProtocolError> {
    let len = u32::try_from(bytes.len()).map_err(|_| WorkerProtocolError::IntegerOverflow)?;
    encoded.extend_from_slice(&tag.to_le_bytes());
    encoded.extend_from_slice(&len.to_le_bytes());
    encoded.extend_from_slice(bytes);
    Ok(())
}

fn push_u32(encoded: &mut Vec<u8>, value: usize) -> Result<(), WorkerProtocolError> {
    let value = u32::try_from(value).map_err(|_| WorkerProtocolError::IntegerOverflow)?;
    encoded.extend_from_slice(&value.to_le_bytes());
    Ok(())
}

struct Decoder<'a> {
    cursor: Cursor<'a>,
    last_tag: u16,
}

impl<'a> Decoder<'a> {
    fn new(bytes: &'a [u8], magic: &[u8; 8]) -> Result<Self, WorkerProtocolError> {
        let mut cursor = Cursor::new(bytes);
        if cursor.take(magic.len())? != magic {
            return Err(WorkerProtocolError::BadMagic);
        }
        Ok(Self {
            cursor,
            last_tag: 0,
        })
    }

    fn field(&mut self, expected: u16, max_len: usize) -> Result<&'a [u8], WorkerProtocolError> {
        let tag = self.cursor.u16()?;
        if tag > REQUEST_FIELD_COUNT.max(RESPONSE_FIELD_COUNT) {
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
        let result = self
            .bytes
            .get(self.position..end)
            .ok_or(WorkerProtocolError::Truncated)?;
        self.position = end;
        Ok(result)
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
        self.take(N)?
            .try_into()
            .map_err(|_| WorkerProtocolError::Truncated)
    }

    fn finish(self) -> Result<(), WorkerProtocolError> {
        if self.position == self.bytes.len() {
            Ok(())
        } else {
            Err(WorkerProtocolError::TrailingBytes)
        }
    }
}
