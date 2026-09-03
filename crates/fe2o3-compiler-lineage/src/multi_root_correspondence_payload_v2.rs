//! Canonical function-qualified MIR-to-KIR correspondence carried by one multi-root roster root.

use std::{error::Error, fmt, str};

use crate::MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3;

/// Magic prefix for the existing multi-root correspondence payload.
pub const MULTI_ROOT_CORRESPONDENCE_PAYLOAD_MAGIC_V2: [u8; 8] = *b"F2MRCOP2";
/// Wire version for the existing multi-root correspondence payload.
pub const MULTI_ROOT_CORRESPONDENCE_PAYLOAD_VERSION_V2: u16 = 2;
/// Association-only policy for the existing multi-root correspondence payload.
pub const MULTI_ROOT_CORRESPONDENCE_PAYLOAD_POLICY_V2: u16 = 1;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
/// Exact role of one semantic function in a root-owned closure.
pub enum MultiRootCorrespondenceFunctionRoleV2 {
    /// Exported kernel entry for this root.
    KernelEntry,
    /// Internal helper owned exclusively by this root.
    InternalHelper,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
/// Producer rule for an operation that has no semantic statement owner.
pub enum MultiRootCorrespondenceSyntheticRuleV2 {
    /// Storage introduced while lowering an enum payload.
    EnumPayloadStorage,
    /// Trap introduced for a failed runtime assertion.
    RuntimeAssertFailureTrap,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
/// Function-qualified KIR function record.
pub struct MultiRootCorrespondenceFunctionV2 {
    semantic_function: u32,
    role: MultiRootCorrespondenceFunctionRoleV2,
    kernel_ir_function: Box<str>,
}

impl MultiRootCorrespondenceFunctionV2 {
    /// Returns the absolute semantic-function ordinal.
    pub const fn semantic_function(&self) -> u32 {
        self.semantic_function
    }
    /// Returns the exact function role.
    pub const fn role(&self) -> MultiRootCorrespondenceFunctionRoleV2 {
        self.role
    }
    /// Returns the exact KIR function symbol.
    pub fn kernel_ir_function(&self) -> &str {
        &self.kernel_ir_function
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
/// Function-qualified semantic-block to KIR-block record.
pub struct MultiRootCorrespondenceBlockV2 {
    semantic_function: u32,
    semantic_block: u32,
    kernel_ir_block: u32,
    source_statement_count: u32,
}

impl MultiRootCorrespondenceBlockV2 {
    /// Returns the absolute semantic-function ordinal.
    pub const fn semantic_function(self) -> u32 {
        self.semantic_function
    }
    /// Returns the semantic-block ordinal.
    pub const fn semantic_block(self) -> u32 {
        self.semantic_block
    }
    /// Returns the KIR block identifier.
    pub const fn kernel_ir_block(self) -> u32 {
        self.kernel_ir_block
    }
    /// Returns the number of semantic statements in this block.
    pub const fn source_statement_count(self) -> u32 {
        self.source_statement_count
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
/// Function-qualified semantic-statement to KIR-operation span.
pub struct MultiRootCorrespondenceStatementV2 {
    semantic_function: u32,
    semantic_block: u32,
    statement: u32,
    kernel_ir_block: u32,
    first_operation: u32,
    operation_count: u32,
}

impl MultiRootCorrespondenceStatementV2 {
    /// Returns the absolute semantic-function ordinal.
    pub const fn semantic_function(self) -> u32 {
        self.semantic_function
    }
    /// Returns the semantic-block ordinal.
    pub const fn semantic_block(self) -> u32 {
        self.semantic_block
    }
    /// Returns the statement ordinal within its semantic block.
    pub const fn statement(self) -> u32 {
        self.statement
    }
    /// Returns the KIR block identifier.
    pub const fn kernel_ir_block(self) -> u32 {
        self.kernel_ir_block
    }
    /// Returns the first KIR operation ordinal.
    pub const fn first_operation(self) -> u32 {
        self.first_operation
    }
    /// Returns the number of KIR operations, including zero for elimination.
    pub const fn operation_count(self) -> u32 {
        self.operation_count
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
/// Function-qualified semantic-terminator to KIR-operation span.
pub struct MultiRootCorrespondenceTerminatorV2 {
    semantic_function: u32,
    semantic_block: u32,
    kernel_ir_block: u32,
    first_operation: u32,
    operation_count: u32,
}

impl MultiRootCorrespondenceTerminatorV2 {
    /// Returns the absolute semantic-function ordinal.
    pub const fn semantic_function(self) -> u32 {
        self.semantic_function
    }
    /// Returns the semantic-block ordinal.
    pub const fn semantic_block(self) -> u32 {
        self.semantic_block
    }
    /// Returns the KIR block identifier.
    pub const fn kernel_ir_block(self) -> u32 {
        self.kernel_ir_block
    }
    /// Returns the first KIR operation ordinal.
    pub const fn first_operation(self) -> u32 {
        self.first_operation
    }
    /// Returns the number of KIR operations.
    pub const fn operation_count(self) -> u32 {
        self.operation_count
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
/// Function-qualified synthetic KIR-operation span.
pub struct MultiRootCorrespondenceSyntheticV2 {
    semantic_function: u32,
    rule: MultiRootCorrespondenceSyntheticRuleV2,
    kernel_ir_block: u32,
    first_operation: u32,
    operation_count: u32,
}

impl MultiRootCorrespondenceSyntheticV2 {
    /// Returns the absolute semantic-function ordinal.
    pub const fn semantic_function(self) -> u32 {
        self.semantic_function
    }
    /// Returns the exact synthetic producer rule.
    pub const fn rule(self) -> MultiRootCorrespondenceSyntheticRuleV2 {
        self.rule
    }
    /// Returns the KIR block identifier.
    pub const fn kernel_ir_block(self) -> u32 {
        self.kernel_ir_block
    }
    /// Returns the first KIR operation ordinal.
    pub const fn first_operation(self) -> u32 {
        self.first_operation
    }
    /// Returns the number of KIR operations.
    pub const fn operation_count(self) -> u32 {
        self.operation_count
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
/// Function-qualified semantic-parameter to KIR-value binding.
pub struct MultiRootCorrespondenceParameterV2 {
    semantic_function: u32,
    semantic_local: u32,
    kernel_ir_value: u32,
}

impl MultiRootCorrespondenceParameterV2 {
    /// Returns the absolute semantic-function ordinal.
    pub const fn semantic_function(self) -> u32 {
        self.semantic_function
    }
    /// Returns the semantic local ordinal.
    pub const fn semantic_local(self) -> u32 {
        self.semantic_local
    }
    /// Returns the KIR value identifier.
    pub const fn kernel_ir_value(self) -> u32 {
        self.kernel_ir_value
    }
}

#[derive(Debug, Eq, PartialEq)]
/// Strict, bounded decoded form of one canonical root payload.
pub struct MultiRootCorrespondencePayloadV2 {
    root_ordinal: u32,
    correspondence_owner: u32,
    induction: Box<[u8]>,
    functions: Box<[MultiRootCorrespondenceFunctionV2]>,
    blocks: Box<[MultiRootCorrespondenceBlockV2]>,
    statements: Box<[MultiRootCorrespondenceStatementV2]>,
    terminators: Box<[MultiRootCorrespondenceTerminatorV2]>,
    synthetics: Box<[MultiRootCorrespondenceSyntheticV2]>,
    parameters: Box<[MultiRootCorrespondenceParameterV2]>,
}

impl MultiRootCorrespondencePayloadV2 {
    /// Decodes one canonical payload without granting execution or publication authority.
    pub fn decode(bytes: &[u8]) -> Result<Self, MultiRootCorrespondencePayloadErrorV2> {
        if bytes.len() > MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3 {
            return Err(MultiRootCorrespondencePayloadErrorV2::ResourceLimit);
        }
        let mut reader = Reader::new(bytes);
        if reader.fixed::<8>()? != MULTI_ROOT_CORRESPONDENCE_PAYLOAD_MAGIC_V2 {
            return Err(MultiRootCorrespondencePayloadErrorV2::InvalidMagic);
        }
        if reader.u16()? != MULTI_ROOT_CORRESPONDENCE_PAYLOAD_VERSION_V2 {
            return Err(MultiRootCorrespondencePayloadErrorV2::InvalidVersion);
        }
        if reader.u16()? != MULTI_ROOT_CORRESPONDENCE_PAYLOAD_POLICY_V2 {
            return Err(MultiRootCorrespondencePayloadErrorV2::InvalidPolicy);
        }
        let root_ordinal = reader.u32()?;
        let correspondence_owner = reader.u32()?;
        let induction_bytes = reader.bytes()?;
        let mut induction = Vec::new();
        induction
            .try_reserve_exact(induction_bytes.len())
            .map_err(|_| MultiRootCorrespondencePayloadErrorV2::AllocationFailure)?;
        induction.extend_from_slice(induction_bytes);
        let induction = induction.into_boxed_slice();
        let functions = decode_functions(&mut reader)?;
        let blocks = decode_fixed(&mut reader, 16, |reader| {
            Ok(MultiRootCorrespondenceBlockV2 {
                semantic_function: reader.u32()?,
                semantic_block: reader.u32()?,
                kernel_ir_block: reader.u32()?,
                source_statement_count: reader.u32()?,
            })
        })?;
        let statements = decode_fixed(&mut reader, 24, |reader| {
            Ok(MultiRootCorrespondenceStatementV2 {
                semantic_function: reader.u32()?,
                semantic_block: reader.u32()?,
                statement: reader.u32()?,
                kernel_ir_block: reader.u32()?,
                first_operation: reader.u32()?,
                operation_count: reader.u32()?,
            })
        })?;
        let terminators = decode_fixed(&mut reader, 20, |reader| {
            Ok(MultiRootCorrespondenceTerminatorV2 {
                semantic_function: reader.u32()?,
                semantic_block: reader.u32()?,
                kernel_ir_block: reader.u32()?,
                first_operation: reader.u32()?,
                operation_count: reader.u32()?,
            })
        })?;
        let synthetics = decode_fixed(&mut reader, 17, |reader| {
            let semantic_function = reader.u32()?;
            let rule = match reader.u8()? {
                1 => MultiRootCorrespondenceSyntheticRuleV2::EnumPayloadStorage,
                2 => MultiRootCorrespondenceSyntheticRuleV2::RuntimeAssertFailureTrap,
                _ => return Err(MultiRootCorrespondencePayloadErrorV2::InvalidSyntheticRule),
            };
            Ok(MultiRootCorrespondenceSyntheticV2 {
                semantic_function,
                rule,
                kernel_ir_block: reader.u32()?,
                first_operation: reader.u32()?,
                operation_count: reader.u32()?,
            })
        })?;
        let parameters = decode_fixed(&mut reader, 12, |reader| {
            Ok(MultiRootCorrespondenceParameterV2 {
                semantic_function: reader.u32()?,
                semantic_local: reader.u32()?,
                kernel_ir_value: reader.u32()?,
            })
        })?;
        if !reader.finished() {
            return Err(MultiRootCorrespondencePayloadErrorV2::TrailingBytes);
        }
        if functions.is_empty() || blocks.is_empty() {
            return Err(MultiRootCorrespondencePayloadErrorV2::EmptyRequiredRoster);
        }
        Ok(Self {
            root_ordinal,
            correspondence_owner,
            induction,
            functions,
            blocks,
            statements,
            terminators,
            synthetics,
            parameters,
        })
    }

    /// Returns the ordinal of this root in the outer roster.
    pub const fn root_ordinal(&self) -> u32 {
        self.root_ordinal
    }
    /// Returns the semantic root that owns every record.
    pub const fn correspondence_owner(&self) -> u32 {
        self.correspondence_owner
    }
    /// Returns the exact per-root induction evidence.
    pub fn induction(&self) -> &[u8] {
        &self.induction
    }
    /// Returns the canonical function roster.
    pub fn functions(&self) -> &[MultiRootCorrespondenceFunctionV2] {
        &self.functions
    }
    /// Returns the canonical block roster.
    pub fn blocks(&self) -> &[MultiRootCorrespondenceBlockV2] {
        &self.blocks
    }
    /// Returns the canonical statement spans.
    pub fn statements(&self) -> &[MultiRootCorrespondenceStatementV2] {
        &self.statements
    }
    /// Returns the canonical terminator spans.
    pub fn terminators(&self) -> &[MultiRootCorrespondenceTerminatorV2] {
        &self.terminators
    }
    /// Returns the canonical synthetic spans.
    pub fn synthetics(&self) -> &[MultiRootCorrespondenceSyntheticV2] {
        &self.synthetics
    }
    /// Returns the canonical parameter bindings.
    pub fn parameters(&self) -> &[MultiRootCorrespondenceParameterV2] {
        &self.parameters
    }
}

fn decode_functions(
    reader: &mut Reader<'_>,
) -> Result<Box<[MultiRootCorrespondenceFunctionV2]>, MultiRootCorrespondencePayloadErrorV2> {
    let count = reader.count()?;
    if count > reader.remaining() / 10 {
        return Err(MultiRootCorrespondencePayloadErrorV2::Truncated);
    }
    let mut records = Vec::new();
    records
        .try_reserve_exact(count)
        .map_err(|_| MultiRootCorrespondencePayloadErrorV2::AllocationFailure)?;
    for _ in 0..count {
        let semantic_function = reader.u32()?;
        let role = match reader.u8()? {
            1 => MultiRootCorrespondenceFunctionRoleV2::KernelEntry,
            2 => MultiRootCorrespondenceFunctionRoleV2::InternalHelper,
            _ => return Err(MultiRootCorrespondencePayloadErrorV2::InvalidFunctionRole),
        };
        let symbol = str::from_utf8(reader.bytes()?)
            .map_err(|_| MultiRootCorrespondencePayloadErrorV2::InvalidUtf8)?;
        let mut owned_symbol = String::new();
        owned_symbol
            .try_reserve_exact(symbol.len())
            .map_err(|_| MultiRootCorrespondencePayloadErrorV2::AllocationFailure)?;
        owned_symbol.push_str(symbol);
        records.push(MultiRootCorrespondenceFunctionV2 {
            semantic_function,
            role,
            kernel_ir_function: owned_symbol.into_boxed_str(),
        });
    }
    require_strict_order(&records)?;
    Ok(records.into_boxed_slice())
}

fn decode_fixed<T: Ord>(
    reader: &mut Reader<'_>,
    minimum_record_bytes: usize,
    mut decode: impl FnMut(&mut Reader<'_>) -> Result<T, MultiRootCorrespondencePayloadErrorV2>,
) -> Result<Box<[T]>, MultiRootCorrespondencePayloadErrorV2> {
    let count = reader.count()?;
    if count > reader.remaining() / minimum_record_bytes {
        return Err(MultiRootCorrespondencePayloadErrorV2::Truncated);
    }
    let mut records = Vec::new();
    records
        .try_reserve_exact(count)
        .map_err(|_| MultiRootCorrespondencePayloadErrorV2::AllocationFailure)?;
    for _ in 0..count {
        records.push(decode(reader)?);
    }
    require_strict_order(&records)?;
    Ok(records.into_boxed_slice())
}

fn require_strict_order<T: Ord>(
    records: &[T],
) -> Result<(), MultiRootCorrespondencePayloadErrorV2> {
    if records.windows(2).any(|pair| pair[0] >= pair[1]) {
        Err(MultiRootCorrespondencePayloadErrorV2::NonCanonicalOrder)
    } else {
        Ok(())
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    fn take(&mut self, length: usize) -> Result<&'a [u8], MultiRootCorrespondencePayloadErrorV2> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(MultiRootCorrespondencePayloadErrorV2::LengthOverflow)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(MultiRootCorrespondencePayloadErrorV2::Truncated)?;
        self.offset = end;
        Ok(value)
    }
    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], MultiRootCorrespondencePayloadErrorV2> {
        self.take(N)?
            .try_into()
            .map_err(|_| MultiRootCorrespondencePayloadErrorV2::Truncated)
    }
    fn u8(&mut self) -> Result<u8, MultiRootCorrespondencePayloadErrorV2> {
        Ok(self.fixed::<1>()?[0])
    }
    fn u16(&mut self) -> Result<u16, MultiRootCorrespondencePayloadErrorV2> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }
    fn u32(&mut self) -> Result<u32, MultiRootCorrespondencePayloadErrorV2> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }
    fn count(&mut self) -> Result<usize, MultiRootCorrespondencePayloadErrorV2> {
        let count = self.u32()? as usize;
        if count > self.bytes.len() {
            Err(MultiRootCorrespondencePayloadErrorV2::ResourceLimit)
        } else {
            Ok(count)
        }
    }
    fn bytes(&mut self) -> Result<&'a [u8], MultiRootCorrespondencePayloadErrorV2> {
        let length = self.count()?;
        if length == 0 {
            return Err(MultiRootCorrespondencePayloadErrorV2::EmptyField);
        }
        self.take(length)
    }
    const fn finished(&self) -> bool {
        self.offset == self.bytes.len()
    }

    const fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Typed rejection reasons for a root correspondence payload.
pub enum MultiRootCorrespondencePayloadErrorV2 {
    /// Payload exceeds the lineage receipt bound.
    ResourceLimit,
    /// Allocation failed while retaining bounded records.
    AllocationFailure,
    /// A length computation overflowed.
    LengthOverflow,
    /// The payload ended before a declared field.
    Truncated,
    /// Magic does not identify this contract.
    InvalidMagic,
    /// Version is not V2.
    InvalidVersion,
    /// Policy is not association-only V1.
    InvalidPolicy,
    /// A required byte string is empty.
    EmptyField,
    /// A required roster is empty.
    EmptyRequiredRoster,
    /// A symbol is not UTF-8.
    InvalidUtf8,
    /// A function role is unknown.
    InvalidFunctionRole,
    /// A synthetic rule is unknown.
    InvalidSyntheticRule,
    /// Records are duplicated or reordered.
    NonCanonicalOrder,
    /// Canonical fields are followed by unknown data.
    TrailingBytes,
}

impl fmt::Display for MultiRootCorrespondencePayloadErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid multi-root correspondence payload: {self:?}"
        )
    }
}

impl Error for MultiRootCorrespondencePayloadErrorV2 {}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes_field(bytes: &mut Vec<u8>, value: &[u8]) {
        bytes.extend_from_slice(&(value.len() as u32).to_le_bytes());
        bytes.extend_from_slice(value);
    }

    fn payload(functions: &[(u32, u8, &str)], synthetic_rule: Option<u8>) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&MULTI_ROOT_CORRESPONDENCE_PAYLOAD_MAGIC_V2);
        bytes.extend_from_slice(&MULTI_ROOT_CORRESPONDENCE_PAYLOAD_VERSION_V2.to_le_bytes());
        bytes.extend_from_slice(&MULTI_ROOT_CORRESPONDENCE_PAYLOAD_POLICY_V2.to_le_bytes());
        bytes.extend_from_slice(&3_u32.to_le_bytes());
        bytes.extend_from_slice(&7_u32.to_le_bytes());
        bytes_field(&mut bytes, b"induction");
        bytes.extend_from_slice(&(functions.len() as u32).to_le_bytes());
        for (semantic_function, role, symbol) in functions {
            bytes.extend_from_slice(&semantic_function.to_le_bytes());
            bytes.push(*role);
            bytes_field(&mut bytes, symbol.as_bytes());
        }
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        for value in [7_u32, 0, 11, 1] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        for value in [7_u32, 0, 0, 11, 0, 1] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        for value in [7_u32, 0, 11, 1, 1] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(&u32::from(synthetic_rule.is_some()).to_le_bytes());
        if let Some(rule) = synthetic_rule {
            bytes.extend_from_slice(&7_u32.to_le_bytes());
            bytes.push(rule);
            for value in [12_u32, 0, 1] {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        for value in [7_u32, 1, 4] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn exact_payload_retains_every_function_qualified_record() {
        let bytes = payload(&[(7, 1, "alpha"), (8, 2, "helper")], Some(2));
        let decoded = MultiRootCorrespondencePayloadV2::decode(&bytes).unwrap();
        assert_eq!(decoded.root_ordinal(), 3);
        assert_eq!(decoded.correspondence_owner(), 7);
        assert_eq!(decoded.induction(), b"induction");
        assert_eq!(decoded.functions().len(), 2);
        assert_eq!(decoded.blocks()[0].semantic_function(), 7);
        assert_eq!(decoded.statements()[0].statement(), 0);
        assert_eq!(decoded.terminators()[0].first_operation(), 1);
        assert_eq!(decoded.synthetics()[0].semantic_function(), 7);
        assert_eq!(decoded.parameters()[0].kernel_ir_value(), 4);
    }

    #[test]
    fn duplicate_reordered_and_unknown_records_fail_closed() {
        assert_eq!(
            MultiRootCorrespondencePayloadV2::decode(&payload(
                &[(8, 2, "helper"), (7, 1, "alpha")],
                None,
            )),
            Err(MultiRootCorrespondencePayloadErrorV2::NonCanonicalOrder),
        );
        assert_eq!(
            MultiRootCorrespondencePayloadV2::decode(&payload(
                &[(7, 1, "alpha"), (7, 1, "alpha")],
                None,
            )),
            Err(MultiRootCorrespondencePayloadErrorV2::NonCanonicalOrder),
        );
        assert_eq!(
            MultiRootCorrespondencePayloadV2::decode(&payload(&[(7, 9, "alpha")], None)),
            Err(MultiRootCorrespondencePayloadErrorV2::InvalidFunctionRole),
        );
        assert_eq!(
            MultiRootCorrespondencePayloadV2::decode(&payload(&[(7, 1, "alpha")], Some(9))),
            Err(MultiRootCorrespondencePayloadErrorV2::InvalidSyntheticRule),
        );
    }

    #[test]
    fn truncation_trailing_and_oversize_fail_closed() {
        let bytes = payload(&[(7, 1, "alpha")], None);
        assert_eq!(
            MultiRootCorrespondencePayloadV2::decode(&bytes[..bytes.len() - 1]),
            Err(MultiRootCorrespondencePayloadErrorV2::Truncated),
        );
        let mut trailing = bytes;
        trailing.push(0);
        assert_eq!(
            MultiRootCorrespondencePayloadV2::decode(&trailing),
            Err(MultiRootCorrespondencePayloadErrorV2::TrailingBytes),
        );
        let oversize = vec![0; MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3 + 1];
        assert_eq!(
            MultiRootCorrespondencePayloadV2::decode(&oversize),
            Err(MultiRootCorrespondencePayloadErrorV2::ResourceLimit),
        );
    }
}
