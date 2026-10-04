use alloc::string::String;
use core::fmt;

/// A bounded resource enforced by the V1 handoff model.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HandoffLimitV1 {
    CanonicalBytes,
    DeviceLibraries,
    DeviceLibraryBytes,
    FunctionAttributes,
    Kernels,
    KernelParameters,
    ModuleFlags,
    NamedMetadata,
    Obligations,
    Origins,
    ParameterAttributes,
    SourcePathBytes,
    SymbolBytes,
}

/// A typed validation failure from checked V1 model construction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HandoffDiagnosticV1 {
    LimitExceeded {
        limit: HandoffLimitV1,
        observed: u64,
        maximum: u64,
    },
    EmptyCollection(&'static str),
    ZeroIdentity(&'static str),
    InvalidSymbol,
    InvalidSourcePath,
    InvalidSourceSpan,
    InvalidDeviceLibrarySize,
    UnsupportedTargetPolicy,
    InvalidWorkgroupSizeRange,
    InvalidWavesPerEu,
    DuplicateTargetFeature,
    ConflictingTargetFeature,
    MissingTargetFeature(&'static str),
    DuplicateKernel(String),
    DuplicateKernelParameter(String),
    DuplicateParameterAttribute(&'static str),
    ConflictingParameterAttributes,
    AttributeRequiresPointer(&'static str),
    InvalidParameterAttribute(&'static str),
    DuplicateFunctionAttribute(&'static str),
    MissingFunctionAttribute(&'static str),
    DuplicateModuleFlag(&'static str),
    MissingModuleFlag(&'static str),
    DuplicateNamedMetadata(&'static str),
    DuplicateDeviceLibrary(&'static str),
    DuplicateOrigin,
    DuplicateObligation,
    MissingOriginReference,
}

impl fmt::Display for HandoffDiagnosticV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LimitExceeded {
                limit,
                observed,
                maximum,
            } => write!(
                formatter,
                "LLVM handoff {limit:?} limit exceeded: observed {observed}, maximum {maximum}"
            ),
            Self::EmptyCollection(name) => {
                write!(formatter, "LLVM handoff {name} must not be empty")
            }
            Self::ZeroIdentity(name) => {
                write!(formatter, "LLVM handoff {name} identity must not be zero")
            }
            Self::InvalidSymbol => formatter.write_str("invalid canonical LLVM symbol"),
            Self::InvalidSourcePath => {
                formatter.write_str("origin path must be normalized relative ASCII")
            }
            Self::InvalidSourceSpan => formatter.write_str("invalid source span"),
            Self::InvalidDeviceLibrarySize => {
                formatter.write_str("device-library byte length must be nonzero and bounded")
            }
            Self::UnsupportedTargetPolicy => {
                formatter.write_str("unsupported gfx942 LLVM handoff target policy")
            }
            Self::InvalidWorkgroupSizeRange => {
                formatter.write_str("invalid gfx942 wave64 flat workgroup-size range")
            }
            Self::InvalidWavesPerEu => formatter.write_str("invalid gfx942 waves-per-EU range"),
            Self::DuplicateTargetFeature => formatter.write_str("duplicate target feature"),
            Self::ConflictingTargetFeature => {
                formatter.write_str("conflicting target feature states")
            }
            Self::MissingTargetFeature(name) => {
                write!(formatter, "missing required target feature {name}")
            }
            Self::DuplicateKernel(symbol) => write!(formatter, "duplicate kernel {symbol}"),
            Self::DuplicateKernelParameter(name) => {
                write!(formatter, "duplicate kernel parameter {name}")
            }
            Self::DuplicateParameterAttribute(name) => {
                write!(formatter, "duplicate parameter attribute {name}")
            }
            Self::ConflictingParameterAttributes => {
                formatter.write_str("conflicting readonly and writeonly parameter attributes")
            }
            Self::AttributeRequiresPointer(name) => {
                write!(formatter, "parameter attribute {name} requires a pointer")
            }
            Self::InvalidParameterAttribute(name) => {
                write!(formatter, "invalid parameter attribute {name}")
            }
            Self::DuplicateFunctionAttribute(name) => {
                write!(formatter, "duplicate function attribute {name}")
            }
            Self::MissingFunctionAttribute(name) => {
                write!(formatter, "missing required function attribute {name}")
            }
            Self::DuplicateModuleFlag(name) => write!(formatter, "duplicate module flag {name}"),
            Self::MissingModuleFlag(name) => write!(formatter, "missing module flag {name}"),
            Self::DuplicateNamedMetadata(name) => {
                write!(formatter, "duplicate named metadata {name}")
            }
            Self::DuplicateDeviceLibrary(name) => {
                write!(formatter, "duplicate device-library input {name}")
            }
            Self::DuplicateOrigin => formatter.write_str("duplicate origin identity"),
            Self::DuplicateObligation => formatter.write_str("duplicate obligation identity"),
            Self::MissingOriginReference => {
                formatter.write_str("kernel or obligation references an absent origin")
            }
        }
    }
}

impl core::error::Error for HandoffDiagnosticV1 {}
