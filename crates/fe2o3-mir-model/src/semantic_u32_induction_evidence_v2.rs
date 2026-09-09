//! Inert induction custody in checked execution coordinates, retaining original MIR.
//!
//! Expansion origins live in the separately retained expansion evidence. Decoding
//! this record establishes neither those origins nor any induction fact: consumers
//! must replay both the expansion and the complete report against the original.

use std::{error::Error, fmt};

use sha2::{Digest, Sha256};

use crate::{
    semantic_direct_call_expansion_v1::{
        InertCanonicalSemanticCallExpansionEvidenceV1, SemanticCallExpansionEvidenceErrorV1,
        SemanticCallExpansionV1,
    },
    semantic_mir_v1::{AdmittedInertSemanticMirV1, SemanticFunctionIdV1},
    semantic_u32_induction::{
        SemanticU32InductionAnalysisErrorV1, SemanticU32InductionAnalysisLimitsV1,
        SemanticU32InductionNoOverflowReportV1,
        analyze_expanded_semantic_u32_induction_no_overflow_with_limits_v1,
        validate_analysis_limits_v1,
    },
    semantic_u32_induction_evidence_v1::{
        CERTIFICATE_BYTES_V1, MAX_SEMANTIC_U32_INDUCTION_EVIDENCE_BYTES_V1, ReaderV1,
        SemanticU32InductionEvidenceErrorV1, SemanticU32InductionNoOverflowCertificateEvidenceV1,
        certificate_from_live, decode_certificate, encode_certificate, require_nonzero,
        validate_report,
    },
};

pub const SEMANTIC_U32_INDUCTION_EVIDENCE_VERSION_V2: u16 = 2;
pub const SEMANTIC_U32_INDUCTION_EVIDENCE_POLICY_V2: u16 = 1;
pub const MAX_SEMANTIC_U32_INDUCTION_EVIDENCE_BYTES_V2: usize =
    MAX_SEMANTIC_U32_INDUCTION_EVIDENCE_BYTES_V1;
/// Cumulative byte/record traversal across encoding, decoding, hashing and comparison.
pub const MAX_SEMANTIC_U32_INDUCTION_EVIDENCE_CODEC_WORK_V2: usize =
    12 * MAX_SEMANTIC_U32_INDUCTION_EVIDENCE_BYTES_V2;

const MAGIC: [u8; 8] = *b"F2U32I\0\0";
const DOMAIN: &[u8] = b"FE2O3/SEMANTIC-U32-INDUCTION-EVIDENCE/V2\0";
const HEADER_BYTES: usize = 204;
type Result<T> = std::result::Result<T, SemanticU32InductionEvidenceErrorV2>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticU32InductionEvidenceErrorV2 {
    Codec(SemanticU32InductionEvidenceErrorV1),
    Expansion(SemanticCallExpansionEvidenceErrorV1),
    Analysis(SemanticU32InductionAnalysisErrorV1),
    CallFreeRequiresV1,
    BindingMismatch,
    ReportMismatch,
    CodecWorkLimit { actual: usize, limit: usize },
    Storage,
}

impl fmt::Display for SemanticU32InductionEvidenceErrorV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Codec(error) => write!(f, "induction V2 codec: {error}"),
            Self::Expansion(error) => write!(f, "induction V2 expansion: {error}"),
            Self::Analysis(error) => write!(f, "induction V2 analysis: {error}"),
            Self::CallFreeRequiresV1 => f.write_str("call-free induction reports require V1"),
            Self::BindingMismatch => {
                f.write_str("induction V2 source or execution binding differs")
            }
            Self::ReportMismatch => f.write_str("induction V2 complete report does not replay"),
            Self::CodecWorkLimit { actual, limit } => {
                write!(f, "induction V2 codec work {actual} exceeds {limit}")
            }
            Self::Storage => f.write_str("induction V2 storage allocation failed"),
        }
    }
}
impl Error for SemanticU32InductionEvidenceErrorV2 {}
impl From<SemanticU32InductionEvidenceErrorV1> for SemanticU32InductionEvidenceErrorV2 {
    fn from(error: SemanticU32InductionEvidenceErrorV1) -> Self {
        Self::Codec(error)
    }
}
impl From<SemanticCallExpansionEvidenceErrorV1> for SemanticU32InductionEvidenceErrorV2 {
    fn from(error: SemanticCallExpansionEvidenceErrorV1) -> Self {
        Self::Expansion(error)
    }
}
impl From<SemanticU32InductionAnalysisErrorV1> for SemanticU32InductionEvidenceErrorV2 {
    fn from(error: SemanticU32InductionAnalysisErrorV1) -> Self {
        Self::Analysis(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Binding {
    semantic_mir_sha256: [u8; 32],
    expansion_evidence_identity: [u8; 32],
    expansion_identity: [u8; 32],
    root: u32,
    execution_view_identity: [u8; 32],
    function: u32,
    function_identity: [u8; 32],
}

/// Canonical bytes are inert, including after successful source replay. No public
/// constructor accepts arbitrary execution functions, origins or claimed hashes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InertCanonicalSemanticU32InductionEvidenceV2 {
    canonical_bytes: Box<[u8]>,
    identity: [u8; 32],
    binding: Binding,
    checked_additions_examined: u32,
    work_units: u64,
    certificates: Box<[SemanticU32InductionNoOverflowCertificateEvidenceV1]>,
}

impl InertCanonicalSemanticU32InductionEvidenceV2 {
    pub fn from_expanded_report(
        source: &AdmittedInertSemanticMirV1,
        expansion: &SemanticCallExpansionV1,
        expansion_evidence: &InertCanonicalSemanticCallExpansionEvidenceV1,
        root: SemanticFunctionIdV1,
        report: &SemanticU32InductionNoOverflowReportV1,
        limits: SemanticU32InductionAnalysisLimitsV1,
    ) -> Result<Self> {
        let mut budget = CodecBudget::default();
        let (binding, replayed) =
            replay_report(source, expansion, expansion_evidence, root, limits)?;
        budget.charge(exact_size(report.certificates().len())?)?;
        if report != &replayed {
            return Err(SemanticU32InductionEvidenceErrorV2::ReportMismatch);
        }
        let bytes = encode_live(binding, &replayed, &mut budget)?;
        Self::decode_with_budget(&bytes, &mut budget)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        Self::decode_with_budget(bytes, &mut CodecBudget::default())
    }

    fn decode_with_budget(bytes: &[u8], budget: &mut CodecBudget) -> Result<Self> {
        if bytes.len() > MAX_SEMANTIC_U32_INDUCTION_EVIDENCE_BYTES_V2 {
            return Err(SemanticU32InductionEvidenceErrorV1::TooLarge.into());
        }
        budget.charge(bytes.len())?;
        let mut reader = ReaderV1::new(bytes);
        if reader.fixed::<8>()? != MAGIC
            || reader.u16()? != SEMANTIC_U32_INDUCTION_EVIDENCE_VERSION_V2
            || reader.u16()? != SEMANTIC_U32_INDUCTION_EVIDENCE_POLICY_V2
            || reader.u32()? != 0
        {
            return Err(SemanticU32InductionEvidenceErrorV1::InvalidHeader.into());
        }
        if reader.u32()? as usize != bytes.len() {
            return Err(SemanticU32InductionEvidenceErrorV1::InvalidLength.into());
        }
        let binding = Binding {
            semantic_mir_sha256: reader.fixed()?,
            expansion_evidence_identity: reader.fixed()?,
            expansion_identity: reader.fixed()?,
            root: reader.u32()?,
            execution_view_identity: reader.fixed()?,
            function: reader.u32()?,
            function_identity: reader.fixed()?,
        };
        let checked_additions_examined = reader.u32()?;
        let work_units = reader.u64()?;
        let count = reader.u32()? as usize;
        validate_report(
            &binding.semantic_mir_sha256,
            &binding.function_identity,
            checked_additions_examined,
            work_units,
            &[],
        )?;
        if count > checked_additions_examined as usize {
            return Err(SemanticU32InductionEvidenceErrorV1::InvalidReport.into());
        }
        if exact_size(count)? != bytes.len() || reader.remaining() != bytes.len() - HEADER_BYTES {
            return Err(SemanticU32InductionEvidenceErrorV1::InvalidLength.into());
        }
        budget.charge(count)?;
        let mut certificates = reserve(count)?;
        for _ in 0..count {
            certificates.push(decode_certificate(&mut reader)?);
        }
        reader.finish()?;
        let reencoded = encode(
            binding,
            checked_additions_examined,
            work_units,
            &certificates,
            budget,
        )?;
        budget.charge(bytes.len())?;
        if reencoded != bytes {
            return Err(SemanticU32InductionEvidenceErrorV1::NonCanonical.into());
        }
        budget.charge(DOMAIN.len() + 8 + bytes.len())?;
        let mut digest = Sha256::new();
        digest.update(DOMAIN);
        digest.update((bytes.len() as u64).to_le_bytes());
        digest.update(bytes);
        let identity = digest.finalize().into();
        require_nonzero(&identity)?;
        Ok(Self {
            canonical_bytes: reencoded.into_boxed_slice(),
            identity,
            binding,
            checked_additions_examined,
            work_units,
            certificates: certificates.into_boxed_slice(),
        })
    }

    /// Byte revalidation alone does not establish source correspondence or facts.
    pub fn revalidate(&self) -> Result<()> {
        self.revalidate_with_budget(&mut CodecBudget::default())
    }

    fn revalidate_with_budget(&self, budget: &mut CodecBudget) -> Result<()> {
        let decoded = Self::decode_with_budget(&self.canonical_bytes, budget)?;
        budget.charge(self.canonical_bytes.len())?;
        budget.charge(exact_size(self.certificates.len())?)?;
        if decoded != *self {
            return Err(SemanticU32InductionEvidenceErrorV1::IdentityMismatch.into());
        }
        Ok(())
    }

    /// Returns only the independently recomputed complete report, never decoded facts.
    pub fn verify_replay(
        &self,
        source: &AdmittedInertSemanticMirV1,
        expansion: &SemanticCallExpansionV1,
        expansion_evidence: &InertCanonicalSemanticCallExpansionEvidenceV1,
        limits: SemanticU32InductionAnalysisLimitsV1,
    ) -> Result<SemanticU32InductionNoOverflowReportV1> {
        validate_analysis_limits_v1(limits)?;
        let mut budget = CodecBudget::default();
        self.revalidate_with_budget(&mut budget)?;
        let (binding, report) = replay_report(
            source,
            expansion,
            expansion_evidence,
            SemanticFunctionIdV1::from_index(self.binding.root),
            limits,
        )?;
        if binding != self.binding {
            return Err(SemanticU32InductionEvidenceErrorV2::BindingMismatch);
        }
        let expected = encode_live(binding, &report, &mut budget)?;
        budget.charge(expected.len())?;
        if expected.as_slice() != self.canonical_bytes.as_ref() {
            return Err(SemanticU32InductionEvidenceErrorV2::ReportMismatch);
        }
        Ok(report)
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }
    pub const fn semantic_mir_sha256(&self) -> &[u8; 32] {
        &self.binding.semantic_mir_sha256
    }
    pub const fn expansion_evidence_identity(&self) -> &[u8; 32] {
        &self.binding.expansion_evidence_identity
    }
    pub const fn expansion_identity(&self) -> &[u8; 32] {
        &self.binding.expansion_identity
    }
    pub const fn root(&self) -> SemanticFunctionIdV1 {
        SemanticFunctionIdV1::from_index(self.binding.root)
    }
    pub const fn execution_view_identity(&self) -> &[u8; 32] {
        &self.binding.execution_view_identity
    }
    /// Original source-body index; certificate coordinates belong to the execution view.
    pub const fn function(&self) -> u32 {
        self.binding.function
    }
    /// Expanded function identity, not the original function-table entry's identity.
    pub const fn function_identity(&self) -> &[u8; 32] {
        &self.binding.function_identity
    }
    pub const fn checked_additions_examined(&self) -> u32 {
        self.checked_additions_examined
    }
    pub const fn work_units(&self) -> u64 {
        self.work_units
    }
    pub fn certificates(&self) -> &[SemanticU32InductionNoOverflowCertificateEvidenceV1] {
        &self.certificates
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
    pub const fn authorizes_compiler_transform(&self) -> bool {
        false
    }
}

fn replay_report(
    source: &AdmittedInertSemanticMirV1,
    expansion: &SemanticCallExpansionV1,
    evidence: &InertCanonicalSemanticCallExpansionEvidenceV1,
    root: SemanticFunctionIdV1,
    limits: SemanticU32InductionAnalysisLimitsV1,
) -> Result<(Binding, SemanticU32InductionNoOverflowReportV1)> {
    validate_analysis_limits_v1(limits)?;
    let view = expansion
        .root(root)
        .ok_or(SemanticU32InductionEvidenceErrorV2::BindingMismatch)?;
    if !view.has_expanded_calls() {
        return Err(SemanticU32InductionEvidenceErrorV2::CallFreeRequiresV1);
    }
    // Exactly two expansion replays (each independently hard-bounded), one codec
    // verification and one induction analysis per operation, never per certificate.
    evidence.verify_against_checked_expansion(source, expansion)?;
    let report = analyze_expanded_semantic_u32_induction_no_overflow_with_limits_v1(
        source, expansion, root, limits,
    )?;
    if report.semantic_mir_sha256() != source.semantic_sha256()
        || report.execution_view_identity() != Some(view.identity())
        || report.function() != view.source_body()
        || report.function_identity() != view.body().identity()
        || evidence.expansion_identity() != expansion.identity()
    {
        return Err(SemanticU32InductionEvidenceErrorV2::BindingMismatch);
    }
    Ok((
        Binding {
            semantic_mir_sha256: *source.semantic_sha256().as_bytes(),
            expansion_evidence_identity: *evidence.identity(),
            expansion_identity: *expansion.identity(),
            root: root.index(),
            execution_view_identity: *view.identity(),
            function: report.function().index(),
            function_identity: *report.function_identity().as_bytes(),
        },
        report,
    ))
}

fn exact_size(count: usize) -> Result<usize> {
    if count > crate::MAX_SEMANTIC_U32_INDUCTION_CERTIFICATES_V1 {
        return Err(SemanticU32InductionEvidenceErrorV1::InvalidReport.into());
    }
    let size = count
        .checked_mul(CERTIFICATE_BYTES_V1)
        .and_then(|size| size.checked_add(HEADER_BYTES))
        .ok_or(SemanticU32InductionEvidenceErrorV1::Overflow)?;
    if size > MAX_SEMANTIC_U32_INDUCTION_EVIDENCE_BYTES_V2 {
        return Err(SemanticU32InductionEvidenceErrorV1::TooLarge.into());
    }
    Ok(size)
}

fn encode_live(
    binding: Binding,
    report: &SemanticU32InductionNoOverflowReportV1,
    budget: &mut CodecBudget,
) -> Result<Vec<u8>> {
    budget.charge(exact_size(report.certificates().len())?)?;
    let mut certificates = reserve(report.certificates().len())?;
    for certificate in report.certificates() {
        if certificate.semantic_mir_sha256() != report.semantic_mir_sha256()
            || certificate.function() != report.function()
            || certificate.function_identity() != report.function_identity()
        {
            return Err(SemanticU32InductionEvidenceErrorV2::ReportMismatch);
        }
        certificates.push(certificate_from_live(*certificate));
    }
    encode(
        binding,
        u32::try_from(report.checked_additions_examined())
            .map_err(|_| SemanticU32InductionEvidenceErrorV1::Overflow)?,
        u64::try_from(report.work_units())
            .map_err(|_| SemanticU32InductionEvidenceErrorV1::Overflow)?,
        &certificates,
        budget,
    )
}

fn encode(
    binding: Binding,
    examined: u32,
    work: u64,
    certificates: &[SemanticU32InductionNoOverflowCertificateEvidenceV1],
    budget: &mut CodecBudget,
) -> Result<Vec<u8>> {
    let size = exact_size(certificates.len())?;
    budget.charge(size)?;
    budget.charge(certificates.len())?;
    for identity in [
        &binding.expansion_evidence_identity,
        &binding.expansion_identity,
        &binding.execution_view_identity,
    ] {
        require_nonzero(identity)?;
    }
    validate_report(
        &binding.semantic_mir_sha256,
        &binding.function_identity,
        examined,
        work,
        certificates,
    )?;
    let mut bytes = reserve(size)?;
    bytes.extend_from_slice(&MAGIC);
    bytes.extend_from_slice(&SEMANTIC_U32_INDUCTION_EVIDENCE_VERSION_V2.to_le_bytes());
    bytes.extend_from_slice(&SEMANTIC_U32_INDUCTION_EVIDENCE_POLICY_V2.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&(size as u32).to_le_bytes());
    bytes.extend_from_slice(&binding.semantic_mir_sha256);
    bytes.extend_from_slice(&binding.expansion_evidence_identity);
    bytes.extend_from_slice(&binding.expansion_identity);
    bytes.extend_from_slice(&binding.root.to_le_bytes());
    bytes.extend_from_slice(&binding.execution_view_identity);
    bytes.extend_from_slice(&binding.function.to_le_bytes());
    bytes.extend_from_slice(&binding.function_identity);
    bytes.extend_from_slice(&examined.to_le_bytes());
    bytes.extend_from_slice(&work.to_le_bytes());
    bytes.extend_from_slice(&(certificates.len() as u32).to_le_bytes());
    for certificate in certificates {
        encode_certificate(&mut bytes, certificate);
    }
    if bytes.len() != size {
        return Err(SemanticU32InductionEvidenceErrorV1::InvalidLength.into());
    }
    Ok(bytes)
}

fn reserve<T>(count: usize) -> Result<Vec<T>> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| SemanticU32InductionEvidenceErrorV2::Storage)?;
    Ok(values)
}

struct CodecBudget {
    used: usize,
    limit: usize,
}
impl Default for CodecBudget {
    fn default() -> Self {
        Self {
            used: 0,
            limit: MAX_SEMANTIC_U32_INDUCTION_EVIDENCE_CODEC_WORK_V2,
        }
    }
}
impl CodecBudget {
    fn charge(&mut self, amount: usize) -> Result<()> {
        let actual = self
            .used
            .checked_add(amount)
            .ok_or(SemanticU32InductionEvidenceErrorV1::Overflow)?;
        if actual > self.limit {
            return Err(SemanticU32InductionEvidenceErrorV2::CodecWorkLimit {
                actual,
                limit: self.limit,
            });
        }
        self.used = actual;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
