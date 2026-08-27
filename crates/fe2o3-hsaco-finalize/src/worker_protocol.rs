//! Canonical, bounded protocol for the out-of-process direct LLVM linker.
//!
//! The values in this module are inert evidence and data. They grant no
//! compiler, filesystem, HSACO loading, or kernel launch authority.

use std::fmt;

use crate::ContentIdentityV1;

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

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum WorkerInputKindV1 {
    LlvmBitcode = 1,
    AmdGpuRelocatable = 2,
    LlvmTextIr = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum WorkerOptimizationLevelV1 {
    O0 = 0,
    O1 = 1,
    O2 = 2,
    O3 = 3,
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
    AllocationFailed(&'static str),
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
            Self::AllocationFailed(component) => {
                write!(
                    formatter,
                    "worker protocol allocation failed at {component}"
                )
            }
        }
    }
}

impl std::error::Error for WorkerProtocolError {}

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
