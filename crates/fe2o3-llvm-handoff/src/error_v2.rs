use core::fmt;

use crate::{BlockIdV2, FunctionIdV2, GlobalIdV2, ValueIdV2};

/// A bounded resource enforced by the V2 executable-module schema.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HandoffLimitV2 {
    CanonicalBytes,
    EmbeddedV1Bytes,
    EvidenceObligations,
    FunctionAttributes,
    FunctionBlocks,
    FunctionInstructions,
    FunctionParameters,
    Functions,
    GetElementPtrIndices,
    Globals,
    Intrinsics,
    ModuleFlags,
    NamedMetadata,
    ParameterAttributes,
    SymbolBytes,
    Values,
    VectorLanes,
    ArrayElements,
    LocalArrayBytes,
}

/// The typed definition family associated with a V2 validation failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefinitionKindV2 {
    Block,
    Function,
    Global,
    Intrinsic,
    ModuleFlag,
    NamedMetadata,
    Obligation,
    Parameter,
    Symbol,
    Value,
}

/// A typed validation failure from checked V2 model construction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HandoffDiagnosticV2 {
    LimitExceeded {
        limit: HandoffLimitV2,
        observed: u64,
        maximum: u64,
    },
    EmptyCollection(&'static str),
    InvalidSymbol,
    InvalidIdentity,
    InvalidScalarConstant,
    InvalidAlignment,
    InvalidFunctionAttribute,
    UnsupportedValueType,
    UnsupportedCallingConvention,
    UnsupportedInstruction,
    DuplicateDefinition(DefinitionKindV2),
    ConflictingFunctionAttributes,
    ConflictingParameterAttributes,
    AttributeRequiresPointer,
    InvalidParameterAttribute,
    MissingOriginReference,
    MissingObligationReference,
    MissingKernelSignature,
    KernelSignatureMismatch,
    MetadataMismatch,
    MissingEntryBlock(BlockIdV2),
    MissingBlockReference(BlockIdV2),
    MissingFunctionReference(FunctionIdV2),
    MissingGlobalReference(GlobalIdV2),
    MissingIntrinsicReference,
    MissingValueReference(ValueIdV2),
    ValueTypeMismatch(ValueIdV2),
    InvalidInstructionResult,
    InvalidTerminator,
}

impl fmt::Display for HandoffDiagnosticV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LimitExceeded {
                limit,
                observed,
                maximum,
            } => write!(
                formatter,
                "LLVM V2 handoff {limit:?} limit exceeded: observed {observed}, maximum {maximum}"
            ),
            Self::EmptyCollection(name) => write!(formatter, "LLVM V2 {name} must not be empty"),
            Self::InvalidSymbol => formatter.write_str("invalid canonical LLVM V2 symbol"),
            Self::InvalidIdentity => formatter.write_str("invalid LLVM V2 identity"),
            Self::InvalidScalarConstant => {
                formatter.write_str("invalid typed LLVM V2 scalar constant")
            }
            Self::InvalidAlignment => formatter.write_str("invalid LLVM V2 memory alignment"),
            Self::InvalidFunctionAttribute => {
                formatter.write_str("invalid LLVM V2 function attribute")
            }
            Self::UnsupportedValueType => formatter.write_str("unsupported LLVM V2 value type"),
            Self::UnsupportedCallingConvention => {
                formatter.write_str("unsupported LLVM V2 calling convention")
            }
            Self::UnsupportedInstruction => {
                formatter.write_str("unsupported LLVM V2 instruction semantics")
            }
            Self::DuplicateDefinition(kind) => {
                write!(formatter, "duplicate LLVM V2 {kind:?} definition")
            }
            Self::ConflictingFunctionAttributes => {
                formatter.write_str("conflicting LLVM V2 function attributes")
            }
            Self::ConflictingParameterAttributes => {
                formatter.write_str("conflicting LLVM V2 parameter attributes")
            }
            Self::AttributeRequiresPointer => {
                formatter.write_str("LLVM V2 parameter attribute requires a pointer")
            }
            Self::InvalidParameterAttribute => {
                formatter.write_str("invalid LLVM V2 parameter attribute")
            }
            Self::MissingOriginReference => {
                formatter.write_str("LLVM V2 module references an absent V1 origin")
            }
            Self::MissingObligationReference => {
                formatter.write_str("LLVM V2 module references an absent V1 obligation")
            }
            Self::MissingKernelSignature => {
                formatter.write_str("LLVM V2 kernel is absent from the V1 ABI handoff")
            }
            Self::KernelSignatureMismatch => {
                formatter.write_str("LLVM V2 kernel disagrees with the V1 ABI handoff")
            }
            Self::MetadataMismatch => {
                formatter.write_str("LLVM V2 module metadata disagrees with V1")
            }
            Self::MissingEntryBlock(block) => {
                write!(
                    formatter,
                    "LLVM V2 function entry block {} is absent",
                    block.get()
                )
            }
            Self::MissingBlockReference(block) => {
                write!(formatter, "LLVM V2 block {} is absent", block.get())
            }
            Self::MissingFunctionReference(function) => {
                write!(formatter, "LLVM V2 function {} is absent", function.get())
            }
            Self::MissingGlobalReference(global) => {
                write!(formatter, "LLVM V2 global {} is absent", global.get())
            }
            Self::MissingIntrinsicReference => {
                formatter.write_str("LLVM V2 intrinsic declaration is absent")
            }
            Self::MissingValueReference(value) => {
                write!(formatter, "LLVM V2 value {} is absent", value.get())
            }
            Self::ValueTypeMismatch(value) => {
                write!(
                    formatter,
                    "LLVM V2 value {} has the wrong type",
                    value.get()
                )
            }
            Self::InvalidInstructionResult => {
                formatter.write_str("LLVM V2 instruction has an invalid result")
            }
            Self::InvalidTerminator => formatter.write_str("invalid LLVM V2 terminator"),
        }
    }
}

impl core::error::Error for HandoffDiagnosticV2 {}
