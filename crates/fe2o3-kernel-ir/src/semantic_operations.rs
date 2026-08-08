//! Versioned contracts shared by target-neutral semantic operation families.
//!
//! This module is deliberately separate from the module wire format. Kernel IR
//! V1 through V3 remain frozen. A later module wire version can carry an
//! operation family only after that family's strongly typed payload, semantic
//! instance identity, verifier, and lowering are implemented.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::{
    Axis, IndexKind, IntrinsicKind, IntrinsicOperation, MemoryEffect, TargetCapability, Type,
    ValueDef, ValueId,
};

pub const SEMANTIC_OPERATION_SCHEMA_MAGIC_V1: [u8; 8] = *b"FE2O3SO\0";
pub const SEMANTIC_OPERATION_INSTANCE_MAGIC_V1: [u8; 8] = *b"FE2O3SI\0";
pub const SEMANTIC_OPERATION_VERSION_V1: u16 = 1;
pub const SEMANTIC_OPERATION_SCHEMA_BYTES_V1: usize = 16;
pub const SEMANTIC_OPERATION_INSTANCE_HEADER_BYTES_V1: usize = 20;
pub const MAX_SEMANTIC_OPERATION_INSTANCE_PAYLOAD_BYTES_V1: usize = 4096;

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

    const fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Self::MemoryIntrinsic),
            2 => Some(Self::Collective),
            3 => Some(Self::Debug),
            4 => Some(Self::Launch),
            5 => Some(Self::Matrix),
            _ => None,
        }
    }
}

/// Registered operation opcode. Numeric values are scoped to a family.
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

    const fn from_parts(family: SemanticOperationFamily, opcode: u16) -> Option<Self> {
        match (family, opcode) {
            (SemanticOperationFamily::Launch, 1) => Some(Self::LaunchInvocationIndex),
            (SemanticOperationFamily::Launch, 2) => Some(Self::LaunchExtent),
            _ => None,
        }
    }
}

/// Payload-blind schema key used only for operation dispatch and codec selection.
///
/// A schema does not distinguish axes, index levels, types, layouts, scopes, or
/// other operation payload. It must never be used as a proof, artifact, cache,
/// semantic-equivalence, or executable identity. Use
/// SemanticOperationInstanceId for those bindings.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SemanticOperationSchema {
    version: u16,
    kind: SemanticOperationKind,
}

impl SemanticOperationSchema {
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

/// Encodes a payload-blind schema key in its fixed-width canonical form.
pub fn encode_semantic_operation_schema(
    schema: SemanticOperationSchema,
) -> [u8; SEMANTIC_OPERATION_SCHEMA_BYTES_V1] {
    let mut bytes = [0_u8; SEMANTIC_OPERATION_SCHEMA_BYTES_V1];
    bytes[..8].copy_from_slice(&SEMANTIC_OPERATION_SCHEMA_MAGIC_V1);
    bytes[8..10].copy_from_slice(&schema.version.to_le_bytes());
    bytes[10] = schema.family().tag();
    bytes[12..14].copy_from_slice(&schema.kind.opcode().to_le_bytes());
    bytes
}

/// Decodes a schema key and rejects unknown dispatch authority.
pub fn decode_semantic_operation_schema(
    bytes: &[u8],
) -> Result<SemanticOperationSchema, SemanticOperationSchemaDecodeError> {
    if bytes.len() < SEMANTIC_OPERATION_SCHEMA_BYTES_V1 {
        return Err(SemanticOperationSchemaDecodeError::Truncated {
            actual: bytes.len(),
        });
    }
    if bytes.len() > SEMANTIC_OPERATION_SCHEMA_BYTES_V1 {
        return Err(SemanticOperationSchemaDecodeError::TrailingBytes {
            actual: bytes.len(),
        });
    }
    if bytes[..8] != SEMANTIC_OPERATION_SCHEMA_MAGIC_V1 {
        return Err(SemanticOperationSchemaDecodeError::InvalidMagic);
    }

    let version = u16::from_le_bytes([bytes[8], bytes[9]]);
    if version != SEMANTIC_OPERATION_VERSION_V1 {
        return Err(SemanticOperationSchemaDecodeError::UnknownVersion(version));
    }
    for offset in [11, 14, 15] {
        if bytes[offset] != 0 {
            return Err(SemanticOperationSchemaDecodeError::ReservedNonZero { offset });
        }
    }

    let family = SemanticOperationFamily::from_tag(bytes[10])
        .ok_or(SemanticOperationSchemaDecodeError::UnknownFamily(bytes[10]))?;
    let opcode = u16::from_le_bytes([bytes[12], bytes[13]]);
    let kind = SemanticOperationKind::from_parts(family, opcode)
        .ok_or(SemanticOperationSchemaDecodeError::UnknownOperation { family, opcode })?;
    Ok(SemanticOperationSchema { version, kind })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticOperationSchemaDecodeError {
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

impl fmt::Display for SemanticOperationSchemaDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated { actual } | Self::TrailingBytes { actual } => write!(
                formatter,
                "semantic-operation schema has {actual} bytes; expected {SEMANTIC_OPERATION_SCHEMA_BYTES_V1}"
            ),
            Self::InvalidMagic => formatter.write_str("invalid semantic-operation schema magic"),
            Self::UnknownVersion(version) => {
                write!(
                    formatter,
                    "unknown semantic-operation schema version {version}"
                )
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
                "semantic-operation schema reserved byte at offset {offset} is nonzero"
            ),
        }
    }
}

impl Error for SemanticOperationSchemaDecodeError {}

/// Canonical payload represented by a V1 semantic instance identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SemanticOperationInstancePayloadV1 {
    LaunchInvocationIndex { kind: IndexKind, axis: Axis },
    LaunchExtent { axis: Axis },
}

/// Full identity of one target-neutral semantic operation instance.
///
/// Unlike SemanticOperationSchema, this value includes every semantic payload
/// field admitted by its V1 operation contract.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SemanticOperationInstanceId {
    schema: SemanticOperationSchema,
    payload: SemanticOperationInstancePayloadV1,
}

impl SemanticOperationInstanceId {
    pub const fn launch_invocation_index(kind: IndexKind, axis: Axis) -> Self {
        Self {
            schema: SemanticOperationSchema::v1(SemanticOperationKind::LaunchInvocationIndex),
            payload: SemanticOperationInstancePayloadV1::LaunchInvocationIndex { kind, axis },
        }
    }

    pub const fn launch_extent(axis: Axis) -> Self {
        Self {
            schema: SemanticOperationSchema::v1(SemanticOperationKind::LaunchExtent),
            payload: SemanticOperationInstancePayloadV1::LaunchExtent { axis },
        }
    }

    pub const fn schema(self) -> SemanticOperationSchema {
        self.schema
    }

    pub const fn payload(self) -> SemanticOperationInstancePayloadV1 {
        self.payload
    }
}

/// Encodes the full canonical semantic instance, including operation payload.
pub fn encode_semantic_operation_instance_id(id: SemanticOperationInstanceId) -> Vec<u8> {
    let payload = match id.payload {
        SemanticOperationInstancePayloadV1::LaunchInvocationIndex { kind, axis } => {
            vec![index_kind_tag(kind), axis_tag(axis)]
        }
        SemanticOperationInstancePayloadV1::LaunchExtent { axis } => vec![axis_tag(axis)],
    };
    debug_assert!(payload.len() <= MAX_SEMANTIC_OPERATION_INSTANCE_PAYLOAD_BYTES_V1);

    let mut bytes = Vec::with_capacity(SEMANTIC_OPERATION_INSTANCE_HEADER_BYTES_V1 + payload.len());
    bytes.extend_from_slice(&SEMANTIC_OPERATION_INSTANCE_MAGIC_V1);
    bytes.extend_from_slice(&id.schema.version.to_le_bytes());
    bytes.push(id.schema.family().tag());
    bytes.push(0);
    bytes.extend_from_slice(&id.schema.kind.opcode().to_le_bytes());
    bytes.extend_from_slice(&(payload.len() as u16).to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&payload);
    bytes
}

/// Decodes a full canonical instance and rejects malformed or unknown payload.
pub fn decode_semantic_operation_instance_id(
    bytes: &[u8],
) -> Result<SemanticOperationInstanceId, SemanticOperationInstanceDecodeError> {
    if bytes.len() < SEMANTIC_OPERATION_INSTANCE_HEADER_BYTES_V1 {
        return Err(SemanticOperationInstanceDecodeError::Truncated {
            actual: bytes.len(),
            expected: SEMANTIC_OPERATION_INSTANCE_HEADER_BYTES_V1,
        });
    }
    if bytes[..8] != SEMANTIC_OPERATION_INSTANCE_MAGIC_V1 {
        return Err(SemanticOperationInstanceDecodeError::InvalidMagic);
    }

    let version = u16::from_le_bytes([bytes[8], bytes[9]]);
    if version != SEMANTIC_OPERATION_VERSION_V1 {
        return Err(SemanticOperationInstanceDecodeError::UnknownVersion(
            version,
        ));
    }
    if bytes[11] != 0 {
        return Err(SemanticOperationInstanceDecodeError::UnsupportedFlags(
            bytes[11],
        ));
    }
    for (relative_offset, byte) in bytes[16..20].iter().enumerate() {
        if *byte != 0 {
            let offset = 16 + relative_offset;
            return Err(SemanticOperationInstanceDecodeError::ReservedNonZero { offset });
        }
    }

    let family = SemanticOperationFamily::from_tag(bytes[10]).ok_or(
        SemanticOperationInstanceDecodeError::UnknownFamily(bytes[10]),
    )?;
    let opcode = u16::from_le_bytes([bytes[12], bytes[13]]);
    let kind = SemanticOperationKind::from_parts(family, opcode)
        .ok_or(SemanticOperationInstanceDecodeError::UnknownOperation { family, opcode })?;
    let payload_length = u16::from_le_bytes([bytes[14], bytes[15]]) as usize;
    if payload_length > MAX_SEMANTIC_OPERATION_INSTANCE_PAYLOAD_BYTES_V1 {
        return Err(SemanticOperationInstanceDecodeError::PayloadLimitExceeded {
            actual: payload_length,
            max: MAX_SEMANTIC_OPERATION_INSTANCE_PAYLOAD_BYTES_V1,
        });
    }
    let expected_payload_length = match kind {
        SemanticOperationKind::LaunchInvocationIndex => 2,
        SemanticOperationKind::LaunchExtent => 1,
    };
    if payload_length != expected_payload_length {
        return Err(SemanticOperationInstanceDecodeError::InvalidPayloadLength {
            kind,
            actual: payload_length,
            expected: expected_payload_length,
        });
    }

    let expected_length = SEMANTIC_OPERATION_INSTANCE_HEADER_BYTES_V1 + payload_length;
    if bytes.len() < expected_length {
        return Err(SemanticOperationInstanceDecodeError::Truncated {
            actual: bytes.len(),
            expected: expected_length,
        });
    }
    if bytes.len() > expected_length {
        return Err(SemanticOperationInstanceDecodeError::TrailingBytes {
            actual: bytes.len(),
            expected: expected_length,
        });
    }
    let payload = &bytes[SEMANTIC_OPERATION_INSTANCE_HEADER_BYTES_V1..];
    match kind {
        SemanticOperationKind::LaunchInvocationIndex => {
            let index_kind = decode_index_kind(payload[0])?;
            let axis = decode_axis(payload[1])?;
            Ok(SemanticOperationInstanceId::launch_invocation_index(
                index_kind, axis,
            ))
        }
        SemanticOperationKind::LaunchExtent => {
            let axis = decode_axis(payload[0])?;
            Ok(SemanticOperationInstanceId::launch_extent(axis))
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticOperationInstanceDecodeError {
    Truncated {
        actual: usize,
        expected: usize,
    },
    TrailingBytes {
        actual: usize,
        expected: usize,
    },
    InvalidMagic,
    UnknownVersion(u16),
    UnsupportedFlags(u8),
    UnknownFamily(u8),
    UnknownOperation {
        family: SemanticOperationFamily,
        opcode: u16,
    },
    ReservedNonZero {
        offset: usize,
    },
    PayloadLimitExceeded {
        actual: usize,
        max: usize,
    },
    InvalidPayloadLength {
        kind: SemanticOperationKind,
        actual: usize,
        expected: usize,
    },
    UnknownPayloadTag {
        field: &'static str,
        tag: u8,
    },
}

impl fmt::Display for SemanticOperationInstanceDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated { actual, expected } | Self::TrailingBytes { actual, expected } => {
                write!(
                    formatter,
                    "semantic-operation instance has {actual} bytes; expected {expected}"
                )
            }
            Self::InvalidMagic => formatter.write_str("invalid semantic-operation instance magic"),
            Self::UnknownVersion(version) => {
                write!(
                    formatter,
                    "unknown semantic-operation instance version {version}"
                )
            }
            Self::UnsupportedFlags(flags) => {
                write!(formatter, "unsupported semantic-operation flags {flags:#x}")
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
                "semantic-operation instance reserved byte at offset {offset} is nonzero"
            ),
            Self::PayloadLimitExceeded { actual, max } => write!(
                formatter,
                "semantic-operation payload has {actual} bytes; maximum is {max}"
            ),
            Self::InvalidPayloadLength {
                kind,
                actual,
                expected,
            } => write!(
                formatter,
                "{kind:?} semantic payload has {actual} bytes; expected {expected}"
            ),
            Self::UnknownPayloadTag { field, tag } => {
                write!(formatter, "unknown semantic-operation {field} tag {tag}")
            }
        }
    }
}

impl Error for SemanticOperationInstanceDecodeError {}

fn axis_tag(axis: Axis) -> u8 {
    match axis {
        Axis::X => 1,
        Axis::Y => 2,
        Axis::Z => 3,
    }
}

fn decode_axis(tag: u8) -> Result<Axis, SemanticOperationInstanceDecodeError> {
    match tag {
        1 => Ok(Axis::X),
        2 => Ok(Axis::Y),
        3 => Ok(Axis::Z),
        tag => Err(SemanticOperationInstanceDecodeError::UnknownPayloadTag { field: "axis", tag }),
    }
}

fn index_kind_tag(kind: IndexKind) -> u8 {
    match kind {
        IndexKind::Global => 1,
        IndexKind::Workgroup => 2,
        IndexKind::Local => 3,
        IndexKind::WorkgroupSize => 4,
        IndexKind::WorkgroupCount => 5,
    }
}

fn decode_index_kind(tag: u8) -> Result<IndexKind, SemanticOperationInstanceDecodeError> {
    match tag {
        1 => Ok(IndexKind::Global),
        2 => Ok(IndexKind::Workgroup),
        3 => Ok(IndexKind::Local),
        4 => Ok(IndexKind::WorkgroupSize),
        5 => Ok(IndexKind::WorkgroupCount),
        tag => Err(SemanticOperationInstanceDecodeError::UnknownPayloadTag {
            field: "index kind",
            tag,
        }),
    }
}

/// Local shape and effects declared by a strongly typed semantic operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticOperationContract {
    instance_id: SemanticOperationInstanceId,
    pub operand_count: usize,
    pub result_types: Vec<Type>,
    pub memory_effects: Vec<MemoryEffect>,
    pub required_capabilities: BTreeSet<TargetCapability>,
}

impl SemanticOperationContract {
    pub fn new(
        instance_id: SemanticOperationInstanceId,
        operand_count: usize,
        result_types: Vec<Type>,
        memory_effects: Vec<MemoryEffect>,
        required_capabilities: BTreeSet<TargetCapability>,
    ) -> Self {
        Self {
            instance_id,
            operand_count,
            result_types,
            memory_effects,
            required_capabilities,
        }
    }

    pub const fn schema(&self) -> SemanticOperationSchema {
        self.instance_id.schema()
    }

    pub const fn instance_id(&self) -> SemanticOperationInstanceId {
        self.instance_id
    }
}

/// Independently extracted operation data supplied by the module verifier.
#[derive(Clone, Copy, Debug)]
pub struct SemanticOperationVerificationContext<'a> {
    pub operands: &'a [ValueId],
    pub results: &'a [ValueDef],
    /// None means the normal SSA verifier has diagnosed an unknown value.
    pub operand_types: &'a [Option<Type>],
}

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
/// OperationKind, the module wire decoder, and each backend remain closed
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
        if context.operand_types.len() != context.operands.len() {
            issues.push(SemanticOperationIssue::new(
                SemanticOperationIssueKind::InvalidStructure,
                format!(
                    "semantic verifier received {} operands but {} operand types",
                    context.operands.len(),
                    context.operand_types.len()
                ),
            ));
        }
        if context.operands.len() != contract.operand_count {
            issues.push(SemanticOperationIssue::new(
                SemanticOperationIssueKind::InvalidStructure,
                format!(
                    "operation contains {} operands but schema requires {}",
                    context.operands.len(),
                    contract.operand_count
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
        let instance_id = match self.kind {
            IntrinsicKind::InvocationIndex { kind, axis } => {
                SemanticOperationInstanceId::launch_invocation_index(kind, axis)
            }
            IntrinsicKind::LaunchExtent { axis } => {
                SemanticOperationInstanceId::launch_extent(axis)
            }
        };
        SemanticOperationContract::new(
            instance_id,
            0,
            vec![metadata.result_type],
            metadata.memory_effects.effects().iter().cloned().collect(),
            metadata.required_capabilities,
        )
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
