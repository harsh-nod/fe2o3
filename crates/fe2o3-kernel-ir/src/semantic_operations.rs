//! Versioned contracts shared by target-neutral semantic operation families.
//!
//! This module is deliberately separate from the module wire format. Kernel IR
//! V1 through V3 remain frozen. A later module wire version can carry an
//! operation family only after that family's strongly typed payload, semantic
//! identity, verifier, and lowering are implemented.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::{
    IntrinsicKind, IntrinsicOperation, MemoryEffect, TargetCapability, Type, ValueDef, ValueId,
};

/// Fixed magic for a canonical semantic-operation identity.
pub const SEMANTIC_OPERATION_ID_MAGIC_V1: [u8; 8] = *b"FE2O3SO\0";
/// First semantic-operation contract version.
pub const SEMANTIC_OPERATION_VERSION_V1: u16 = 1;
/// Exact encoded size of a V1 semantic-operation identity.
pub const SEMANTIC_OPERATION_ID_BYTES_V1: usize = 16;

/// Target-neutral semantic family. The family does not select a target dialect.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SemanticOperationFamily {
    MemoryIntrinsic,
    Collective,
    Debug,
    /// Launch queries and declarative launch constraints.
    Launch,
    Matrix,
}

impl SemanticOperationFamily {
    const fn tag(self) -> u8 {
        match self {
            Self::MemoryIntrinsic => 1,
            Self::Collective => 2,
            Self::Debug => 3,
            Self::Launch => 4,
            Self::Matrix => 5,
        }
    }

    fn from_tag(tag: u8) -> Result<Self, SemanticOperationIdDecodeError> {
        match tag {
            1 => Ok(Self::MemoryIntrinsic),
            2 => Ok(Self::Collective),
            3 => Ok(Self::Debug),
            4 => Ok(Self::Launch),
            5 => Ok(Self::Matrix),
            _ => Err(SemanticOperationIdDecodeError::UnknownFamily(tag)),
        }
    }
}

/// Registered operation semantics. Numeric opcodes are scoped to a family.
///
/// Adding a variant is an explicit compatibility decision. Decoders reject
/// every family/opcode pair absent from this registry.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SemanticOperationKind {
    LaunchInvocationIndex,
    LaunchExtent,
}

impl SemanticOperationKind {
    pub const fn family(self) -> SemanticOperationFamily {
        match self {
            Self::LaunchInvocationIndex | Self::LaunchExtent => SemanticOperationFamily::Launch,
        }
    }

    const fn opcode(self) -> u16 {
        match self {
            Self::LaunchInvocationIndex => 1,
            Self::LaunchExtent => 2,
        }
    }

    fn from_parts(
        family: SemanticOperationFamily,
        opcode: u16,
    ) -> Result<Self, SemanticOperationIdDecodeError> {
        match (family, opcode) {
            (SemanticOperationFamily::Launch, 1) => Ok(Self::LaunchInvocationIndex),
            (SemanticOperationFamily::Launch, 2) => Ok(Self::LaunchExtent),
            _ => Err(SemanticOperationIdDecodeError::UnknownOperation { family, opcode }),
        }
    }
}

/// Canonical identity of one registered target-neutral semantic operation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SemanticOperationId {
    version: u16,
    kind: SemanticOperationKind,
}

impl SemanticOperationId {
    pub const fn v1(kind: SemanticOperationKind) -> Self {
        Self {
            version: SEMANTIC_OPERATION_VERSION_V1,
            kind,
        }
    }

    pub const fn version(self) -> u16 {
        self.version
    }

    pub const fn family(self) -> SemanticOperationFamily {
        self.kind.family()
    }

    pub const fn kind(self) -> SemanticOperationKind {
        self.kind
    }
}

/// Encodes a semantic identity in a fixed-width canonical representation.
pub fn encode_semantic_operation_id(
    id: SemanticOperationId,
) -> [u8; SEMANTIC_OPERATION_ID_BYTES_V1] {
    let mut bytes = [0_u8; SEMANTIC_OPERATION_ID_BYTES_V1];
    bytes[..8].copy_from_slice(&SEMANTIC_OPERATION_ID_MAGIC_V1);
    bytes[8..10].copy_from_slice(&id.version.to_le_bytes());
    bytes[10] = id.family().tag();
    bytes[12..14].copy_from_slice(&id.kind.opcode().to_le_bytes());
    bytes
}

/// Decodes a canonical semantic identity and rejects all unknown authority.
pub fn decode_semantic_operation_id(
    bytes: &[u8],
) -> Result<SemanticOperationId, SemanticOperationIdDecodeError> {
    if bytes.len() < SEMANTIC_OPERATION_ID_BYTES_V1 {
        return Err(SemanticOperationIdDecodeError::Truncated {
            actual: bytes.len(),
        });
    }
    if bytes.len() > SEMANTIC_OPERATION_ID_BYTES_V1 {
        return Err(SemanticOperationIdDecodeError::TrailingBytes {
            actual: bytes.len(),
        });
    }
    if bytes[..8] != SEMANTIC_OPERATION_ID_MAGIC_V1 {
        return Err(SemanticOperationIdDecodeError::InvalidMagic);
    }

    let version = u16::from_le_bytes([bytes[8], bytes[9]]);
    if version != SEMANTIC_OPERATION_VERSION_V1 {
        return Err(SemanticOperationIdDecodeError::UnknownVersion(version));
    }
    for offset in [11, 14, 15] {
        if bytes[offset] != 0 {
            return Err(SemanticOperationIdDecodeError::ReservedNonZero { offset });
        }
    }

    let family = SemanticOperationFamily::from_tag(bytes[10])?;
    let opcode = u16::from_le_bytes([bytes[12], bytes[13]]);
    let kind = SemanticOperationKind::from_parts(family, opcode)?;
    Ok(SemanticOperationId { version, kind })
}

/// Failures at the semantic-operation identity trust boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticOperationIdDecodeError {
    Truncated {
        actual: usize,
    },
    TrailingBytes {
        actual: usize,
    },
    InvalidMagic,
    UnknownVersion(u16),
    UnknownFamily(u8),
    UnknownOperation {
        family: SemanticOperationFamily,
        opcode: u16,
    },
    ReservedNonZero {
        offset: usize,
    },
}

impl fmt::Display for SemanticOperationIdDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated { actual } | Self::TrailingBytes { actual } => write!(
                formatter,
                "semantic-operation identity has {actual} bytes; expected {SEMANTIC_OPERATION_ID_BYTES_V1}"
            ),
            Self::InvalidMagic => formatter.write_str("invalid semantic-operation identity magic"),
            Self::UnknownVersion(version) => {
                write!(formatter, "unknown semantic-operation version {version}")
            }
            Self::UnknownFamily(family) => {
                write!(formatter, "unknown semantic-operation family {family}")
            }
            Self::UnknownOperation { family, opcode } => write!(
                formatter,
                "unknown {family:?} semantic-operation opcode {opcode}"
            ),
            Self::ReservedNonZero { offset } => write!(
                formatter,
                "semantic-operation reserved byte at offset {offset} is nonzero"
            ),
        }
    }
}

impl Error for SemanticOperationIdDecodeError {}

/// Local shape and effects declared by a strongly typed semantic operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticOperationContract {
    pub id: SemanticOperationId,
    pub operands: Vec<ValueId>,
    pub result_types: Vec<Type>,
    pub memory_effects: Vec<MemoryEffect>,
    pub required_capabilities: BTreeSet<TargetCapability>,
}

/// Type information supplied by the module verifier to an operation hook.
#[derive(Clone, Copy, Debug)]
pub struct SemanticOperationVerificationContext<'a> {
    pub results: &'a [ValueDef],
    /// `None` means the normal SSA verifier has diagnosed an unknown value.
    pub operand_types: &'a [Option<Type>],
}

/// Portable diagnostic classes emitted by semantic operation hooks.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SemanticOperationIssueKind {
    InvalidStructure,
    InvalidOperandType,
    ResultArity,
    TypeMismatch,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticOperationIssue {
    pub kind: SemanticOperationIssueKind,
    pub message: String,
}

impl SemanticOperationIssue {
    pub fn new(kind: SemanticOperationIssueKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

/// Contract implemented by a strongly typed target-neutral operation payload.
///
/// Implementing this trait does not make an operation serializable or lowerable.
/// `OperationKind`, the module wire decoder, and each backend remain closed
/// admission boundaries.
pub trait SemanticOperation {
    fn contract(&self) -> SemanticOperationContract;

    /// Adds payload-specific structural and type checks after generic shape checks.
    fn verify_additional(
        &self,
        _context: SemanticOperationVerificationContext<'_>,
        _issues: &mut Vec<SemanticOperationIssue>,
    ) {
    }

    fn verify(
        &self,
        context: SemanticOperationVerificationContext<'_>,
    ) -> Vec<SemanticOperationIssue> {
        let contract = self.contract();
        let mut issues = Vec::new();
        if context.operand_types.len() != contract.operands.len() {
            issues.push(SemanticOperationIssue::new(
                SemanticOperationIssueKind::InvalidStructure,
                format!(
                    "semantic operation provides {} operand types but declares {} operands",
                    context.operand_types.len(),
                    contract.operands.len()
                ),
            ));
        }
        if context.results.len() != contract.result_types.len() {
            issues.push(SemanticOperationIssue::new(
                SemanticOperationIssueKind::ResultArity,
                format!(
                    "operation defines {} results but {} are required",
                    context.results.len(),
                    contract.result_types.len()
                ),
            ));
        }
        for (result, expected_ty) in context.results.iter().zip(&contract.result_types) {
            if &result.ty != expected_ty {
                issues.push(SemanticOperationIssue::new(
                    SemanticOperationIssueKind::TypeMismatch,
                    format!(
                        "result {} has type {:?}, expected {expected_ty:?}",
                        result.id, result.ty
                    ),
                ));
            }
        }
        self.verify_additional(context, &mut issues);
        issues
    }
}

impl SemanticOperation for IntrinsicOperation {
    fn contract(&self) -> SemanticOperationContract {
        let metadata = self.metadata();
        let kind = match self.kind {
            IntrinsicKind::InvocationIndex { .. } => SemanticOperationKind::LaunchInvocationIndex,
            IntrinsicKind::LaunchExtent { .. } => SemanticOperationKind::LaunchExtent,
        };
        SemanticOperationContract {
            id: SemanticOperationId::v1(kind),
            operands: Vec::new(),
            result_types: vec![self.result_type.clone()],
            memory_effects: metadata.memory_effects.effects().iter().cloned().collect(),
            required_capabilities: metadata.required_capabilities,
        }
    }

    fn verify_additional(
        &self,
        _context: SemanticOperationVerificationContext<'_>,
        issues: &mut Vec<SemanticOperationIssue>,
    ) {
        let expected = self.metadata().result_type;
        if self.result_type != expected {
            issues.push(SemanticOperationIssue::new(
                SemanticOperationIssueKind::TypeMismatch,
                format!(
                    "intrinsic declares result type {:?}, expected {:?}",
                    self.result_type, expected
                ),
            ));
        }
    }
}
