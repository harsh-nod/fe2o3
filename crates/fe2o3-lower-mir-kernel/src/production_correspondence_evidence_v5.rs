//! Exact function-owner custody layered over the frozen V4 MIR-to-KIR evidence.

use std::{collections::BTreeSet, error::Error, fmt};

use sha2::{Digest, Sha256};

use crate::{
    InertCanonicalMirToKirCorrespondenceEvidenceV4, MAX_MIR_TO_KIR_CORRESPONDENCE_FUNCTIONS_V3,
    ProductionCorrespondenceEvidenceErrorV4, ProductionSemanticKirOwnerV1,
    SemanticKirFunctionRoleV1,
};

/// Current additive wire version retaining exact KIR function ownership.
pub const MIR_TO_KIR_CORRESPONDENCE_EVIDENCE_VERSION_V5: u16 = 5;
/// Closed validation policy for exact function-owner custody.
pub const MIR_TO_KIR_CORRESPONDENCE_EVIDENCE_POLICY_V5: u16 = 1;
/// Maximum aggregate bytes, matching the outer non-MIR lineage receipt budget.
pub const MAX_MIR_TO_KIR_CORRESPONDENCE_EVIDENCE_BYTES_V5: usize = 4 * 1024 * 1024;
/// Maximum UTF-8 bytes in one retained KIR function identity.
pub const MAX_MIR_TO_KIR_FUNCTION_NAME_BYTES_V5: usize = 4096;

const MAGIC_V5: [u8; 8] = *b"F2M2K5\0\0";
const IDENTITY_DOMAIN_V5: &[u8] = b"FE2O3/EXACT-FUNCTION-MIR-TO-KIR-CORRESPONDENCE-EVIDENCE/V5\0";
const HEADER_BYTES_V5: usize = 28;
const FUNCTION_PREFIX_BYTES_V5: usize = 20;

/// Closed KIR role retained for one exact semantic function instance.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum MirToKirFunctionRoleEvidenceV5 {
    /// The selected semantic root lowered as a kernel entry.
    KernelEntry = 1,
    /// A reachable ordinary Rust helper lowered as an internal definition.
    InternalHelper = 2,
}

/// Exact semantic owner/function to canonical KIR function correspondence.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MirToKirFunctionCorrespondenceEvidenceV5 {
    correspondence_owner: u32,
    semantic_function: u32,
    kernel_ir_function_ordinal: u32,
    role: MirToKirFunctionRoleEvidenceV5,
    kernel_ir_function: Box<str>,
}

impl MirToKirFunctionCorrespondenceEvidenceV5 {
    /// Returns the semantic root that owns this lowered function instance.
    pub const fn correspondence_owner(&self) -> u32 {
        self.correspondence_owner
    }

    /// Returns the exact semantic function index.
    pub const fn semantic_function(&self) -> u32 {
        self.semantic_function
    }

    /// Returns the exact ordinal in the bound canonical KIR function roster.
    pub const fn kernel_ir_function_ordinal(&self) -> u32 {
        self.kernel_ir_function_ordinal
    }

    /// Returns the exact closed KIR function role.
    pub const fn role(&self) -> MirToKirFunctionRoleEvidenceV5 {
        self.role
    }

    /// Returns the exact KIR function identity at the retained ordinal.
    pub fn kernel_ir_function(&self) -> &str {
        &self.kernel_ir_function
    }
}

/// Complete V4 correspondence plus an exact function-owner roster.
#[derive(Debug, Eq, PartialEq)]
pub struct InertCanonicalMirToKirCorrespondenceEvidenceV5 {
    canonical_bytes: Box<[u8]>,
    identity: [u8; 32],
    nested_v4: InertCanonicalMirToKirCorrespondenceEvidenceV4,
    functions: Box<[MirToKirFunctionCorrespondenceEvidenceV5]>,
}

impl InertCanonicalMirToKirCorrespondenceEvidenceV5 {
    /// Replays the live owner and retains each exact KIR function owner and ordinal.
    pub fn from_live_owner(
        owner: &ProductionSemanticKirOwnerV1,
        induction_report: &fe2o3_mir_model::SemanticU32InductionNoOverflowReportV1,
    ) -> Result<Self, ProductionCorrespondenceEvidenceErrorV5> {
        let nested_v4 = InertCanonicalMirToKirCorrespondenceEvidenceV4::from_live_owner(
            owner,
            induction_report,
        )
        .map_err(ProductionCorrespondenceEvidenceErrorV5::NestedV4)?;
        let correspondence = owner.correspondence();
        if correspondence.lowered_functions().len()
            != usize::try_from(nested_v4.function_count())
                .map_err(|_| ProductionCorrespondenceEvidenceErrorV5::Overflow)?
        {
            return Err(ProductionCorrespondenceEvidenceErrorV5::InvalidFunctionRoster);
        }

        let mut functions = Vec::new();
        functions
            .try_reserve_exact(correspondence.lowered_functions().len())
            .map_err(|_| ProductionCorrespondenceEvidenceErrorV5::AllocationFailure)?;
        for record in correspondence.lowered_functions() {
            let mut matches = owner
                .module()
                .functions
                .iter()
                .enumerate()
                .filter(|(_, function)| &function.id == record.kernel_ir_function());
            let Some((ordinal, function)) = matches.next() else {
                return Err(ProductionCorrespondenceEvidenceErrorV5::InvalidFunctionRoster);
            };
            if matches.next().is_some() || function.body.is_none() {
                return Err(ProductionCorrespondenceEvidenceErrorV5::InvalidFunctionRoster);
            }
            let role = match record.role() {
                SemanticKirFunctionRoleV1::KernelEntry
                    if function.role == fe2o3_kernel_ir::FunctionRole::KernelEntry =>
                {
                    MirToKirFunctionRoleEvidenceV5::KernelEntry
                }
                SemanticKirFunctionRoleV1::InternalHelper
                    if function.role == fe2o3_kernel_ir::FunctionRole::InternalHelper =>
                {
                    MirToKirFunctionRoleEvidenceV5::InternalHelper
                }
                _ => {
                    return Err(ProductionCorrespondenceEvidenceErrorV5::InvalidFunctionRoster);
                }
            };
            let function_name = function.id.as_str();
            if function_name.is_empty()
                || function_name.len() > MAX_MIR_TO_KIR_FUNCTION_NAME_BYTES_V5
            {
                return Err(ProductionCorrespondenceEvidenceErrorV5::InvalidFunctionRoster);
            }
            functions.push(MirToKirFunctionCorrespondenceEvidenceV5 {
                correspondence_owner: record.correspondence_owner().index(),
                semantic_function: record.semantic_function().index(),
                kernel_ir_function_ordinal: u32::try_from(ordinal)
                    .map_err(|_| ProductionCorrespondenceEvidenceErrorV5::Overflow)?,
                role,
                kernel_ir_function: function_name.to_owned().into_boxed_str(),
            });
        }
        functions
            .sort_unstable_by_key(|record| (record.correspondence_owner, record.semantic_function));
        validate_function_roster(&nested_v4, &functions)?;
        let bytes = encode(&nested_v4, &functions)?;
        let evidence = Self::decode(&bytes)?;
        evidence.validate_against_module(owner.module())?;
        Ok(evidence)
    }

    /// Strictly decodes one complete canonical V5 aggregate.
    pub fn decode(bytes: &[u8]) -> Result<Self, ProductionCorrespondenceEvidenceErrorV5> {
        if bytes.len() > MAX_MIR_TO_KIR_CORRESPONDENCE_EVIDENCE_BYTES_V5 {
            return Err(ProductionCorrespondenceEvidenceErrorV5::TooLarge);
        }
        let mut reader = ReaderV5::new(bytes);
        if reader.fixed::<8>()? != MAGIC_V5
            || reader.u16()? != MIR_TO_KIR_CORRESPONDENCE_EVIDENCE_VERSION_V5
            || reader.u16()? != MIR_TO_KIR_CORRESPONDENCE_EVIDENCE_POLICY_V5
            || reader.u32()? != 0
        {
            return Err(ProductionCorrespondenceEvidenceErrorV5::InvalidHeader);
        }
        if reader.usize_u32()? != bytes.len() {
            return Err(ProductionCorrespondenceEvidenceErrorV5::InvalidLength);
        }
        let nested_length = reader.usize_u32()?;
        let function_count = reader.usize_u32()?;
        if function_count == 0 || function_count > MAX_MIR_TO_KIR_CORRESPONDENCE_FUNCTIONS_V3 {
            return Err(ProductionCorrespondenceEvidenceErrorV5::LimitExceeded);
        }
        let minimum_function_bytes = function_count
            .checked_mul(FUNCTION_PREFIX_BYTES_V5)
            .ok_or(ProductionCorrespondenceEvidenceErrorV5::Overflow)?;
        if nested_length == 0
            || nested_length > MAX_MIR_TO_KIR_CORRESPONDENCE_EVIDENCE_BYTES_V5
            || reader.remaining() < nested_length.saturating_add(minimum_function_bytes)
        {
            return Err(ProductionCorrespondenceEvidenceErrorV5::InvalidLength);
        }
        let nested_v4 =
            InertCanonicalMirToKirCorrespondenceEvidenceV4::decode(reader.take(nested_length)?)
                .map_err(ProductionCorrespondenceEvidenceErrorV5::NestedV4)?;
        let mut functions = Vec::new();
        functions
            .try_reserve_exact(function_count)
            .map_err(|_| ProductionCorrespondenceEvidenceErrorV5::AllocationFailure)?;
        for _ in 0..function_count {
            let correspondence_owner = reader.u32()?;
            let semantic_function = reader.u32()?;
            let kernel_ir_function_ordinal = reader.u32()?;
            let role = match reader.u8()? {
                1 => MirToKirFunctionRoleEvidenceV5::KernelEntry,
                2 => MirToKirFunctionRoleEvidenceV5::InternalHelper,
                _ => return Err(ProductionCorrespondenceEvidenceErrorV5::InvalidFunctionRoster),
            };
            if reader.fixed::<3>()? != [0; 3] {
                return Err(ProductionCorrespondenceEvidenceErrorV5::InvalidFunctionRoster);
            }
            let name_length = reader.usize_u32()?;
            if name_length == 0 || name_length > MAX_MIR_TO_KIR_FUNCTION_NAME_BYTES_V5 {
                return Err(ProductionCorrespondenceEvidenceErrorV5::InvalidFunctionRoster);
            }
            let name = std::str::from_utf8(reader.take(name_length)?)
                .map_err(|_| ProductionCorrespondenceEvidenceErrorV5::InvalidFunctionRoster)?;
            functions.push(MirToKirFunctionCorrespondenceEvidenceV5 {
                correspondence_owner,
                semantic_function,
                kernel_ir_function_ordinal,
                role,
                kernel_ir_function: name.to_owned().into_boxed_str(),
            });
        }
        reader.finish()?;
        validate_function_roster(&nested_v4, &functions)?;
        let reencoded = encode(&nested_v4, &functions)?;
        if reencoded != bytes {
            return Err(ProductionCorrespondenceEvidenceErrorV5::NonCanonical);
        }
        let identity = evidence_identity(&reencoded)?;
        Ok(Self {
            canonical_bytes: reencoded.into_boxed_slice(),
            identity,
            nested_v4,
            functions: functions.into_boxed_slice(),
        })
    }

    /// Re-decodes the exact retained aggregate and identity.
    pub fn revalidate(&self) -> Result<(), ProductionCorrespondenceEvidenceErrorV5> {
        if Self::decode(&self.canonical_bytes)? != *self {
            return Err(ProductionCorrespondenceEvidenceErrorV5::IdentityMismatch);
        }
        Ok(())
    }

    /// Returns the complete canonical V5 bytes.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Returns the exact aggregate content identity.
    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }

    /// Returns the frozen lossless V4 correspondence nested by exact bytes.
    pub const fn nested_v4(&self) -> &InertCanonicalMirToKirCorrespondenceEvidenceV4 {
        &self.nested_v4
    }

    /// Consumes the V5 envelope and returns its exact frozen V4 proof payload.
    pub fn into_nested_v4(self) -> InertCanonicalMirToKirCorrespondenceEvidenceV4 {
        self.nested_v4
    }

    /// Returns the exact, canonical function-owner roster.
    pub fn functions(&self) -> &[MirToKirFunctionCorrespondenceEvidenceV5] {
        &self.functions
    }

    /// Looks up one unambiguous owner/function record.
    pub fn function(
        &self,
        correspondence_owner: u32,
        semantic_function: u32,
    ) -> Option<&MirToKirFunctionCorrespondenceEvidenceV5> {
        self.functions
            .binary_search_by_key(&(correspondence_owner, semantic_function), |record| {
                (record.correspondence_owner, record.semantic_function)
            })
            .ok()
            .map(|index| &self.functions[index])
    }

    /// Replays every ordinal, name, role, and definition against the exact bound KIR module.
    pub fn validate_against_module(
        &self,
        module: &fe2o3_kernel_ir::Module,
    ) -> Result<(), ProductionCorrespondenceEvidenceErrorV5> {
        let defined_count = module
            .functions
            .iter()
            .filter(|function| function.body.is_some())
            .count();
        if defined_count != self.functions.len() {
            return Err(ProductionCorrespondenceEvidenceErrorV5::InvalidFunctionRoster);
        }
        let mut ordinals = BTreeSet::new();
        for record in &self.functions {
            let ordinal = usize::try_from(record.kernel_ir_function_ordinal)
                .map_err(|_| ProductionCorrespondenceEvidenceErrorV5::Overflow)?;
            let function = module
                .functions
                .get(ordinal)
                .ok_or(ProductionCorrespondenceEvidenceErrorV5::InvalidFunctionRoster)?;
            let expected_role = match record.role {
                MirToKirFunctionRoleEvidenceV5::KernelEntry => {
                    fe2o3_kernel_ir::FunctionRole::KernelEntry
                }
                MirToKirFunctionRoleEvidenceV5::InternalHelper => {
                    fe2o3_kernel_ir::FunctionRole::InternalHelper
                }
            };
            if !ordinals.insert(ordinal)
                || function.body.is_none()
                || function.role != expected_role
                || function.id.as_str() != record.kernel_ir_function.as_ref()
            {
                return Err(ProductionCorrespondenceEvidenceErrorV5::InvalidFunctionRoster);
            }
        }
        if ordinals.len() != defined_count {
            return Err(ProductionCorrespondenceEvidenceErrorV5::InvalidFunctionRoster);
        }
        Ok(())
    }

    /// Exact correspondence custody grants no compiler or runtime authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Fail-closed exact function-owner correspondence error.
#[derive(Debug)]
pub enum ProductionCorrespondenceEvidenceErrorV5 {
    /// The nested frozen V4 record failed.
    NestedV4(ProductionCorrespondenceEvidenceErrorV4),
    /// Aggregate exceeds the outer receipt budget.
    TooLarge,
    /// Header magic, version, policy, flags, or reserved fields are invalid.
    InvalidHeader,
    /// Declared, computed, and available lengths differ.
    InvalidLength,
    /// Count or byte arithmetic overflowed.
    Overflow,
    /// A record count exceeds its fixed bound.
    LimitExceeded,
    /// A function is absent, ambiguous, reordered, duplicated, or role-mismatched.
    InvalidFunctionRoster,
    /// Input ended before a complete field was available.
    Truncated,
    /// Decoded fields are not in their unique canonical representation.
    NonCanonical,
    /// Retained bytes and content identity changed.
    IdentityMismatch,
    /// A bounded allocation failed.
    AllocationFailure,
    /// Derived aggregate identity was the reserved all-zero value.
    ZeroIdentity,
}

impl fmt::Display for ProductionCorrespondenceEvidenceErrorV5 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NestedV4(error) => write!(formatter, "nested V4 correspondence failed: {error}"),
            Self::TooLarge => formatter.write_str("V5 correspondence exceeds its byte limit"),
            Self::InvalidHeader => formatter.write_str("V5 correspondence header is invalid"),
            Self::InvalidLength => formatter.write_str("V5 correspondence length is invalid"),
            Self::Overflow => formatter.write_str("V5 correspondence arithmetic overflowed"),
            Self::LimitExceeded => formatter.write_str("V5 correspondence count exceeds its limit"),
            Self::InvalidFunctionRoster => {
                formatter.write_str("V5 correspondence function-owner roster is invalid")
            }
            Self::Truncated => formatter.write_str("V5 correspondence is truncated"),
            Self::NonCanonical => formatter.write_str("V5 correspondence is not canonical"),
            Self::IdentityMismatch => formatter.write_str("V5 correspondence identity changed"),
            Self::AllocationFailure => formatter.write_str("V5 correspondence allocation failed"),
            Self::ZeroIdentity => formatter.write_str("V5 correspondence identity is zero"),
        }
    }
}

impl Error for ProductionCorrespondenceEvidenceErrorV5 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::NestedV4(error) => Some(error),
            _ => None,
        }
    }
}

fn validate_function_roster(
    nested_v4: &InertCanonicalMirToKirCorrespondenceEvidenceV4,
    functions: &[MirToKirFunctionCorrespondenceEvidenceV5],
) -> Result<(), ProductionCorrespondenceEvidenceErrorV5> {
    if functions.len()
        != usize::try_from(nested_v4.function_count())
            .map_err(|_| ProductionCorrespondenceEvidenceErrorV5::Overflow)?
        || functions.is_empty()
        || functions.len() > MAX_MIR_TO_KIR_CORRESPONDENCE_FUNCTIONS_V3
        || functions.windows(2).any(|pair| {
            (pair[0].correspondence_owner, pair[0].semantic_function)
                >= (pair[1].correspondence_owner, pair[1].semantic_function)
        })
        || functions
            .iter()
            .any(|record| record.kernel_ir_function.is_empty())
    {
        return Err(ProductionCorrespondenceEvidenceErrorV5::InvalidFunctionRoster);
    }
    let ordinals = functions
        .iter()
        .map(|record| record.kernel_ir_function_ordinal)
        .collect::<BTreeSet<_>>();
    let covered_semantic = nested_v4
        .blocks()
        .iter()
        .map(|record| record.semantic_function())
        .collect::<BTreeSet<_>>();
    let roster_semantic = functions
        .iter()
        .map(|record| record.semantic_function)
        .collect::<BTreeSet<_>>();
    if ordinals.len() != functions.len() || covered_semantic != roster_semantic {
        return Err(ProductionCorrespondenceEvidenceErrorV5::InvalidFunctionRoster);
    }
    Ok(())
}

fn encode(
    nested_v4: &InertCanonicalMirToKirCorrespondenceEvidenceV4,
    functions: &[MirToKirFunctionCorrespondenceEvidenceV5],
) -> Result<Vec<u8>, ProductionCorrespondenceEvidenceErrorV5> {
    let names_length = functions.iter().try_fold(0_usize, |total, record| {
        total.checked_add(record.kernel_ir_function.len())
    });
    let length = HEADER_BYTES_V5
        .checked_add(nested_v4.canonical_bytes().len())
        .and_then(|length| {
            functions
                .len()
                .checked_mul(FUNCTION_PREFIX_BYTES_V5)
                .and_then(|records| length.checked_add(records))
        })
        .and_then(|length| names_length.and_then(|names| length.checked_add(names)))
        .ok_or(ProductionCorrespondenceEvidenceErrorV5::Overflow)?;
    if length > MAX_MIR_TO_KIR_CORRESPONDENCE_EVIDENCE_BYTES_V5 {
        return Err(ProductionCorrespondenceEvidenceErrorV5::TooLarge);
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| ProductionCorrespondenceEvidenceErrorV5::AllocationFailure)?;
    bytes.extend_from_slice(&MAGIC_V5);
    push_u16(&mut bytes, MIR_TO_KIR_CORRESPONDENCE_EVIDENCE_VERSION_V5);
    push_u16(&mut bytes, MIR_TO_KIR_CORRESPONDENCE_EVIDENCE_POLICY_V5);
    push_u32(&mut bytes, 0);
    push_u32(
        &mut bytes,
        u32::try_from(length).map_err(|_| ProductionCorrespondenceEvidenceErrorV5::Overflow)?,
    );
    push_u32(
        &mut bytes,
        u32::try_from(nested_v4.canonical_bytes().len())
            .map_err(|_| ProductionCorrespondenceEvidenceErrorV5::Overflow)?,
    );
    push_u32(
        &mut bytes,
        u32::try_from(functions.len())
            .map_err(|_| ProductionCorrespondenceEvidenceErrorV5::Overflow)?,
    );
    bytes.extend_from_slice(nested_v4.canonical_bytes());
    for record in functions {
        push_u32(&mut bytes, record.correspondence_owner);
        push_u32(&mut bytes, record.semantic_function);
        push_u32(&mut bytes, record.kernel_ir_function_ordinal);
        bytes.push(record.role as u8);
        bytes.extend_from_slice(&[0; 3]);
        push_u32(
            &mut bytes,
            u32::try_from(record.kernel_ir_function.len())
                .map_err(|_| ProductionCorrespondenceEvidenceErrorV5::Overflow)?,
        );
        bytes.extend_from_slice(record.kernel_ir_function.as_bytes());
    }
    Ok(bytes)
}

fn evidence_identity(bytes: &[u8]) -> Result<[u8; 32], ProductionCorrespondenceEvidenceErrorV5> {
    let mut hasher = Sha256::new();
    hasher.update(IDENTITY_DOMAIN_V5);
    hasher.update(
        u64::try_from(bytes.len())
            .map_err(|_| ProductionCorrespondenceEvidenceErrorV5::Overflow)?
            .to_le_bytes(),
    );
    hasher.update(bytes);
    let identity: [u8; 32] = hasher.finalize().into();
    if identity == [0; 32] {
        Err(ProductionCorrespondenceEvidenceErrorV5::ZeroIdentity)
    } else {
        Ok(identity)
    }
}

fn push_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

struct ReaderV5<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> ReaderV5<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    const fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.cursor)
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], ProductionCorrespondenceEvidenceErrorV5> {
        let end = self
            .cursor
            .checked_add(length)
            .ok_or(ProductionCorrespondenceEvidenceErrorV5::Overflow)?;
        let value = self
            .bytes
            .get(self.cursor..end)
            .ok_or(ProductionCorrespondenceEvidenceErrorV5::Truncated)?;
        self.cursor = end;
        Ok(value)
    }

    fn fixed<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], ProductionCorrespondenceEvidenceErrorV5> {
        self.take(N)?
            .try_into()
            .map_err(|_| ProductionCorrespondenceEvidenceErrorV5::Truncated)
    }

    fn u8(&mut self) -> Result<u8, ProductionCorrespondenceEvidenceErrorV5> {
        Ok(self.fixed::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, ProductionCorrespondenceEvidenceErrorV5> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }

    fn u32(&mut self) -> Result<u32, ProductionCorrespondenceEvidenceErrorV5> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }

    fn usize_u32(&mut self) -> Result<usize, ProductionCorrespondenceEvidenceErrorV5> {
        usize::try_from(self.u32()?).map_err(|_| ProductionCorrespondenceEvidenceErrorV5::Overflow)
    }

    fn finish(self) -> Result<(), ProductionCorrespondenceEvidenceErrorV5> {
        if self.cursor == self.bytes.len() {
            Ok(())
        } else {
            Err(ProductionCorrespondenceEvidenceErrorV5::InvalidLength)
        }
    }
}
