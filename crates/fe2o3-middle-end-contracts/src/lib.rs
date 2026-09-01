//! Canonical authority-free middle-end evidence records.
//!
//! This crate owns strict decoding for evidence crossing from the compiler
//! middle end into an independent verifier. It contains no Pliron graph,
//! compiler pass, proof execution, target backend, or runtime implementation.

#![forbid(unsafe_code)]

use std::{error::Error, fmt, ops::Range};

use sha2::{Digest, Sha256};

const MAGIC_V5: [u8; 8] = *b"F2MEV5\0\0";
const VERSION_V5: u16 = 5;
const FLAGS_V5: u16 = 0;
const ASSURANCE_INTERNAL_CHECKS_ONLY_V5: u8 = 1;
const SEMANTIC_OWNER_REVALIDATED_V5: u8 = 1;
const CLEAN_STATUS_V5: u8 = 1;
const PASS_TAGS_V5: [u8; 8] = [1, 2, 3, 4, 8, 5, 6, 7];
const PASS_RECORD_BYTES_V5: usize = 10;
const SHA256_BYTES: usize = 32;
const COVERAGE_COUNTERS_V5: usize = 4;
const SEMANTIC_COUNTERS_V5: usize = 6;
const TYPED_SUMMARY_COUNTERS_V5: usize = 10;
const RECONCILIATION_COUNTERS_V5: usize = 2;
const IDENTITY_DOMAIN_V5: &[u8] = b"FE2O3/PRODUCTION-MIDDLE-END-EVIDENCE-IDENTITY/V5\0";

/// Stable V5 wire domain.
pub const PRODUCTION_MIDDLE_END_EVIDENCE_DOMAIN_V5: &[u8] =
    b"fe2o3.production-middle-end-evidence.v5";

/// Fixed authority-free evidence policy.
pub const PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V5: &[u8] =
    b"fe2o3.internal-analysis-and-mir-operator-congruence-obligations-only.v5";

/// Maximum bytes in one canonical V5 record.
pub const MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V5: usize = 4 * 1024 * 1024;

const FIXED_RECORD_BYTES_V5: usize = MAGIC_V5.len()
    + 2
    + 2
    + 8
    + 4
    + 2
    + PRODUCTION_MIDDLE_END_EVIDENCE_DOMAIN_V5.len()
    + 2
    + PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V5.len()
    + 1
    + 1
    + 2
    + SHA256_BYTES
    + SHA256_BYTES
    + 4
    + 1
    + PASS_TAGS_V5.len() * PASS_RECORD_BYTES_V5
    + COVERAGE_COUNTERS_V5 * 8
    + SEMANTIC_COUNTERS_V5 * 8
    + TYPED_SUMMARY_COUNTERS_V5 * 8
    + RECONCILIATION_COUNTERS_V5 * 8
    + SHA256_BYTES
    + SHA256_BYTES;

/// Maximum deterministic ranked-IR bytes in one complete record.
pub const MAX_PRODUCTION_MIDDLE_END_RANKED_IR_BYTES_V5: usize =
    MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V5 - FIXED_RECORD_BYTES_V5;

/// Domain-separated identity of one exact canonical V5 record.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProductionMiddleEndEvidenceIdentityV5 {
    sha256: [u8; SHA256_BYTES],
    byte_len: u64,
}

impl ProductionMiddleEndEvidenceIdentityV5 {
    /// Returns the terminal evidence digest.
    pub const fn sha256(&self) -> &[u8; SHA256_BYTES] {
        &self.sha256
    }

    /// Returns the exact canonical byte length.
    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }

    /// Recomputes this identity over exact canonical bytes.
    pub fn matches_canonical_bytes(self, bytes: &[u8]) -> bool {
        if self.byte_len != bytes.len() as u64 || bytes.len() < SHA256_BYTES {
            return false;
        }
        let terminal = bytes.len() - SHA256_BYTES;
        bytes[terminal..] == self.sha256
            && derive_evidence_identity_v5(&bytes[..terminal]) == Some(self.sha256)
    }
}

/// Strictly decoded, authority-free V5 middle-end evidence.
#[derive(Eq, PartialEq)]
pub struct InertProductionMiddleEndEvidenceV5 {
    source_semantic_identity: [u8; SHA256_BYTES],
    ranked_kernel_identity: [u8; SHA256_BYTES],
    ranked_ir_range: Range<usize>,
    identity: ProductionMiddleEndEvidenceIdentityV5,
    canonical_bytes: Box<[u8]>,
}

impl InertProductionMiddleEndEvidenceV5 {
    /// Strictly decodes one complete canonical V5 record.
    pub fn decode(bytes: &[u8]) -> Result<Self, ProductionMiddleEndEvidenceCodecErrorV5> {
        if bytes.len() > MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V5 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV5::TooLarge {
                actual: bytes.len(),
                limit: MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V5,
            });
        }
        let mut reader = ReaderV5::new(bytes);
        if reader.fixed::<8>()? != MAGIC_V5 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV5::InvalidMagic);
        }
        let version = reader.u16()?;
        if version != VERSION_V5 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV5::UnsupportedVersion(
                version,
            ));
        }
        let flags = reader.u16()?;
        if flags != FLAGS_V5 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV5::UnsupportedFlags(
                flags,
            ));
        }
        let declared_len = reader.u64()?;
        let declared_len_usize = usize::try_from(declared_len)
            .map_err(|_| ProductionMiddleEndEvidenceCodecErrorV5::InvalidLength(declared_len))?;
        if declared_len_usize > MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V5 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV5::TooLarge {
                actual: declared_len_usize,
                limit: MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V5,
            });
        }
        match declared_len_usize.cmp(&bytes.len()) {
            std::cmp::Ordering::Greater => {
                return Err(ProductionMiddleEndEvidenceCodecErrorV5::Truncated);
            }
            std::cmp::Ordering::Less => {
                return Err(ProductionMiddleEndEvidenceCodecErrorV5::TrailingBytes);
            }
            std::cmp::Ordering::Equal => {}
        }
        if declared_len_usize < FIXED_RECORD_BYTES_V5 + 1 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV5::InvalidLength(
                declared_len,
            ));
        }
        require_zero(reader.u32()?)?;
        read_exact_bytes(
            &mut reader,
            PRODUCTION_MIDDLE_END_EVIDENCE_DOMAIN_V5,
            ProductionMiddleEndEvidenceCodecErrorV5::InvalidDomain,
        )?;
        read_exact_bytes(
            &mut reader,
            PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V5,
            ProductionMiddleEndEvidenceCodecErrorV5::InvalidPolicy,
        )?;
        if reader.u8()? != ASSURANCE_INTERNAL_CHECKS_ONLY_V5 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV5::InvalidAssurance);
        }
        if reader.u8()? != SEMANTIC_OWNER_REVALIDATED_V5 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV5::SemanticOwnerNotRevalidated);
        }
        require_zero(u32::from(reader.u16()?))?;

        let source_semantic_identity = reader.fixed::<SHA256_BYTES>()?;
        if source_semantic_identity == [0; SHA256_BYTES] {
            return Err(ProductionMiddleEndEvidenceCodecErrorV5::ZeroSemanticIdentity);
        }
        let ranked_kernel_identity = reader.fixed::<SHA256_BYTES>()?;
        if ranked_kernel_identity == [0; SHA256_BYTES] {
            return Err(ProductionMiddleEndEvidenceCodecErrorV5::ZeroRankedKernelIdentity);
        }
        let ranked_ir_len = usize::try_from(reader.u32()?).map_err(|_| {
            ProductionMiddleEndEvidenceCodecErrorV5::RankedIrTooLarge {
                actual: usize::MAX,
                limit: MAX_PRODUCTION_MIDDLE_END_RANKED_IR_BYTES_V5,
            }
        })?;
        if ranked_ir_len > MAX_PRODUCTION_MIDDLE_END_RANKED_IR_BYTES_V5 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV5::RankedIrTooLarge {
                actual: ranked_ir_len,
                limit: MAX_PRODUCTION_MIDDLE_END_RANKED_IR_BYTES_V5,
            });
        }
        let ranked_ir_start = reader.offset();
        validate_ranked_ir_v5(reader.take(ranked_ir_len)?)?;
        let ranked_ir_end = reader.offset();

        let pass_count = reader.u8()?;
        if usize::from(pass_count) != PASS_TAGS_V5.len() {
            return Err(ProductionMiddleEndEvidenceCodecErrorV5::InvalidPassCount(
                pass_count,
            ));
        }
        for (index, expected) in PASS_TAGS_V5.iter().copied().enumerate() {
            let actual = reader.u8()?;
            if actual != expected {
                return Err(ProductionMiddleEndEvidenceCodecErrorV5::InvalidPassOrder {
                    index,
                    expected,
                    actual,
                });
            }
            if reader.u8()? != CLEAN_STATUS_V5 || reader.u32()? != 0 {
                return Err(ProductionMiddleEndEvidenceCodecErrorV5::PassNotClean(
                    expected,
                ));
            }
            if reader.u8()? != 0 || reader.u8()? != 0 {
                return Err(ProductionMiddleEndEvidenceCodecErrorV5::AuthorityClaim(
                    expected,
                ));
            }
            require_zero(u32::from(reader.u16()?))?;
        }

        let coverage = read_counters::<COVERAGE_COUNTERS_V5>(&mut reader)?;
        if coverage[0] != coverage[1] || coverage[2] != coverage[3] {
            return Err(ProductionMiddleEndEvidenceCodecErrorV5::InvalidCoverageSummary);
        }
        let semantics = read_counters::<SEMANTIC_COUNTERS_V5>(&mut reader)?;
        if semantics[0] != semantics[1]
            || semantics[2] != semantics[3]
            || semantics[4] != semantics[5]
        {
            return Err(ProductionMiddleEndEvidenceCodecErrorV5::InvalidSemanticSummary);
        }
        let typed = read_counters::<TYPED_SUMMARY_COUNTERS_V5>(&mut reader)?;
        validate_typed_summary_v5(typed)?;
        let reconciliation_roots = reader.u64()?;
        let reconciliation_commitments = reader.u64()?;
        let reconciliation_digest = reader.fixed::<SHA256_BYTES>()?;
        if reconciliation_roots != typed[0]
            || reconciliation_commitments != typed[0]
            || reconciliation_digest == [0; SHA256_BYTES]
        {
            return Err(
                ProductionMiddleEndEvidenceCodecErrorV5::InvalidTypedSemanticReconciliation,
            );
        }

        let terminal_offset = reader.offset();
        let declared_identity = reader.fixed::<SHA256_BYTES>()?;
        if declared_identity == [0; SHA256_BYTES] {
            return Err(ProductionMiddleEndEvidenceCodecErrorV5::ZeroIdentity);
        }
        if !reader.is_empty() {
            return Err(ProductionMiddleEndEvidenceCodecErrorV5::TrailingBytes);
        }
        if derive_evidence_identity_v5(&bytes[..terminal_offset]) != Some(declared_identity) {
            return Err(ProductionMiddleEndEvidenceCodecErrorV5::IdentityMismatch);
        }

        let mut canonical = Vec::new();
        canonical
            .try_reserve_exact(bytes.len())
            .map_err(|_| ProductionMiddleEndEvidenceCodecErrorV5::AllocationFailed)?;
        canonical.extend_from_slice(bytes);
        Ok(Self {
            source_semantic_identity,
            ranked_kernel_identity,
            ranked_ir_range: ranked_ir_start..ranked_ir_end,
            identity: ProductionMiddleEndEvidenceIdentityV5 {
                sha256: declared_identity,
                byte_len: declared_len,
            },
            canonical_bytes: canonical.into_boxed_slice(),
        })
    }

    /// Returns the exact admitted semantic-MIR identity.
    pub const fn source_semantic_identity(&self) -> &[u8; SHA256_BYTES] {
        &self.source_semantic_identity
    }

    /// Returns the exact ranked-kernel identity.
    pub const fn ranked_kernel_identity(&self) -> &[u8; SHA256_BYTES] {
        &self.ranked_kernel_identity
    }

    /// Returns the validated deterministic ranked IR.
    pub fn ranked_ir(&self) -> &str {
        std::str::from_utf8(&self.canonical_bytes[self.ranked_ir_range.clone()])
            .expect("validated V5 ranked IR remains UTF-8")
    }

    /// Returns the canonical evidence identity.
    pub const fn identity(&self) -> ProductionMiddleEndEvidenceIdentityV5 {
        self.identity
    }

    /// Returns the exact canonical evidence bytes.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Returns false because inert evidence never authenticates its producer.
    pub const fn authenticates_producer(&self) -> bool {
        false
    }

    /// Returns false because this record does not claim Verus execution.
    pub const fn claims_verus_verification(&self) -> bool {
        false
    }

    /// Returns false because this record does not claim complete arithmetic correctness.
    pub const fn claims_full_arithmetic_correctness(&self) -> bool {
        false
    }

    /// Returns false because inert evidence grants no compiler authority.
    pub const fn grants_compiler_refinement_authority(&self) -> bool {
        false
    }

    /// Returns false because inert evidence grants no artifact or launch authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }

    /// Returns false because inert evidence grants no target-value authority.
    pub const fn grants_target_value_authority(&self) -> bool {
        false
    }

    /// Returns false because inert evidence grants no publication authority.
    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    /// Returns false because inert evidence grants no load authority.
    pub const fn grants_load_authority(&self) -> bool {
        false
    }
}

impl fmt::Debug for InertProductionMiddleEndEvidenceV5 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InertProductionMiddleEndEvidenceV5")
            .field("identity", &self.identity)
            .field("source_semantic_identity", &self.source_semantic_identity)
            .field("ranked_kernel_identity", &self.ranked_kernel_identity)
            .finish_non_exhaustive()
    }
}

/// Strict V5 middle-end evidence decoding failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductionMiddleEndEvidenceCodecErrorV5 {
    /// A counter cannot be represented on this host.
    CounterOverflow,
    /// Coverage counts are inconsistent.
    InvalidCoverageSummary,
    /// Semantic proof counts are inconsistent.
    InvalidSemanticSummary,
    /// Typed semantic counts are inconsistent.
    InvalidTypedSemanticSummary,
    /// Typed recipe and ranked commitments disagree.
    InvalidTypedSemanticReconciliation,
    /// Ranked IR is empty.
    EmptyRankedIr,
    /// Ranked IR exceeds its deterministic limit.
    RankedIrTooLarge { actual: usize, limit: usize },
    /// Ranked IR is not UTF-8.
    InvalidRankedIrUtf8,
    /// Ranked IR contains a noncanonical byte.
    NonCanonicalRankedIrByte { offset: usize },
    /// Ranked IR lacks its canonical final newline.
    RankedIrMissingFinalNewline,
    /// The aggregate record exceeds its limit.
    TooLarge { actual: usize, limit: usize },
    /// The record magic is invalid.
    InvalidMagic,
    /// The wire version is unsupported.
    UnsupportedVersion(u16),
    /// Nonzero wire flags are unsupported.
    UnsupportedFlags(u16),
    /// The declared record length is invalid.
    InvalidLength(u64),
    /// The record is truncated.
    Truncated,
    /// The record contains trailing bytes.
    TrailingBytes,
    /// A reserved field is nonzero.
    NonzeroReserved,
    /// The wire domain is invalid.
    InvalidDomain,
    /// The evidence policy is invalid.
    InvalidPolicy,
    /// The assurance tag is invalid.
    InvalidAssurance,
    /// The semantic-owner revalidation marker is absent.
    SemanticOwnerNotRevalidated,
    /// The semantic identity is zero.
    ZeroSemanticIdentity,
    /// The ranked-kernel identity is zero.
    ZeroRankedKernelIdentity,
    /// The pass count is not the frozen count.
    InvalidPassCount(u8),
    /// A pass tag differs from the frozen order.
    InvalidPassOrder {
        index: usize,
        expected: u8,
        actual: u8,
    },
    /// A pass is not exactly clean.
    PassNotClean(u8),
    /// A pass contains a forbidden authority claim.
    AuthorityClaim(u8),
    /// The terminal identity is zero.
    ZeroIdentity,
    /// The terminal identity does not match the canonical preimage.
    IdentityMismatch,
    /// Allocation of retained canonical bytes failed.
    AllocationFailed,
}

impl fmt::Display for ProductionMiddleEndEvidenceCodecErrorV5 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CounterOverflow => formatter.write_str("V5 evidence counter overflow"),
            Self::InvalidCoverageSummary => {
                formatter.write_str("V5 coverage summary is inconsistent")
            }
            Self::InvalidSemanticSummary => {
                formatter.write_str("V5 semantic obligation summary is inconsistent")
            }
            Self::InvalidTypedSemanticSummary => {
                formatter.write_str("V5 typed semantic summary is inconsistent")
            }
            Self::InvalidTypedSemanticReconciliation => {
                formatter.write_str("V5 typed semantic reconciliation is inconsistent")
            }
            Self::EmptyRankedIr => formatter.write_str("deterministic ranked IR is empty"),
            Self::RankedIrTooLarge { actual, limit } => {
                write!(formatter, "ranked IR byte length {actual} exceeds {limit}")
            }
            Self::InvalidRankedIrUtf8 => formatter.write_str("ranked IR is not valid UTF-8"),
            Self::NonCanonicalRankedIrByte { offset } => write!(
                formatter,
                "ranked IR has a noncanonical byte at offset {offset}"
            ),
            Self::RankedIrMissingFinalNewline => {
                formatter.write_str("ranked IR must end in a newline")
            }
            Self::TooLarge { actual, limit } => {
                write!(formatter, "V5 record length {actual} exceeds {limit}")
            }
            Self::InvalidMagic => formatter.write_str("invalid V5 evidence magic"),
            Self::UnsupportedVersion(version) => {
                write!(formatter, "unsupported V5 evidence version {version}")
            }
            Self::UnsupportedFlags(flags) => write!(formatter, "unsupported V5 flags {flags}"),
            Self::InvalidLength(length) => write!(formatter, "invalid V5 length {length}"),
            Self::Truncated => formatter.write_str("truncated V5 evidence"),
            Self::TrailingBytes => formatter.write_str("trailing V5 evidence bytes"),
            Self::NonzeroReserved => formatter.write_str("V5 reserved field is nonzero"),
            Self::InvalidDomain => formatter.write_str("invalid V5 evidence domain"),
            Self::InvalidPolicy => formatter.write_str("invalid V5 evidence policy"),
            Self::InvalidAssurance => formatter.write_str("invalid V5 assurance"),
            Self::SemanticOwnerNotRevalidated => {
                formatter.write_str("V5 semantic owner revalidation fact is absent")
            }
            Self::ZeroSemanticIdentity => formatter.write_str("V5 semantic identity is zero"),
            Self::ZeroRankedKernelIdentity => formatter.write_str("V5 ranked identity is zero"),
            Self::InvalidPassCount(count) => write!(formatter, "invalid V5 pass count {count}"),
            Self::InvalidPassOrder { index, .. } => {
                write!(formatter, "noncanonical V5 pass at position {index}")
            }
            Self::PassNotClean(pass) => {
                write!(formatter, "V5 pass tag {pass} is not exactly clean")
            }
            Self::AuthorityClaim(pass) => write!(
                formatter,
                "V5 pass tag {pass} contains a forbidden authority claim"
            ),
            Self::ZeroIdentity => formatter.write_str("V5 evidence identity is zero"),
            Self::IdentityMismatch => formatter.write_str("V5 evidence identity mismatch"),
            Self::AllocationFailed => formatter.write_str("V5 evidence allocation failed"),
        }
    }
}

impl Error for ProductionMiddleEndEvidenceCodecErrorV5 {}

fn read_exact_bytes(
    reader: &mut ReaderV5<'_>,
    expected: &[u8],
    mismatch: ProductionMiddleEndEvidenceCodecErrorV5,
) -> Result<(), ProductionMiddleEndEvidenceCodecErrorV5> {
    let length = usize::from(reader.u16()?);
    if length != expected.len() || reader.take(length)? != expected {
        return Err(mismatch);
    }
    Ok(())
}

fn require_zero(value: u32) -> Result<(), ProductionMiddleEndEvidenceCodecErrorV5> {
    if value != 0 {
        return Err(ProductionMiddleEndEvidenceCodecErrorV5::NonzeroReserved);
    }
    Ok(())
}

fn read_counters<const N: usize>(
    reader: &mut ReaderV5<'_>,
) -> Result<[u64; N], ProductionMiddleEndEvidenceCodecErrorV5> {
    let mut counters = [0; N];
    for counter in &mut counters {
        *counter = reader.u64()?;
    }
    Ok(counters)
}

fn validate_typed_summary_v5(
    summary: [u64; TYPED_SUMMARY_COUNTERS_V5],
) -> Result<(), ProductionMiddleEndEvidenceCodecErrorV5> {
    if summary.iter().any(|value| usize::try_from(*value).is_err()) {
        return Err(ProductionMiddleEndEvidenceCodecErrorV5::CounterOverflow);
    }
    let contract_roots = summary[8]
        .checked_add(summary[9])
        .ok_or(ProductionMiddleEndEvidenceCodecErrorV5::InvalidTypedSemanticSummary)?;
    if contract_roots != summary[0]
        || summary[7] > summary[0]
        || summary[2] > summary[1]
        || summary[3] > summary[1]
        || summary[4] > summary[1]
        || summary[5] > summary[1]
        || summary[6] > summary[2]
        || (summary[0] == 0 && summary[1] != 0)
        || (summary[0] != 0 && summary[1] < summary[0])
    {
        return Err(ProductionMiddleEndEvidenceCodecErrorV5::InvalidTypedSemanticSummary);
    }
    Ok(())
}

fn validate_ranked_ir_v5(bytes: &[u8]) -> Result<(), ProductionMiddleEndEvidenceCodecErrorV5> {
    if bytes.is_empty() {
        return Err(ProductionMiddleEndEvidenceCodecErrorV5::EmptyRankedIr);
    }
    if bytes.len() > MAX_PRODUCTION_MIDDLE_END_RANKED_IR_BYTES_V5 {
        return Err(ProductionMiddleEndEvidenceCodecErrorV5::RankedIrTooLarge {
            actual: bytes.len(),
            limit: MAX_PRODUCTION_MIDDLE_END_RANKED_IR_BYTES_V5,
        });
    }
    std::str::from_utf8(bytes)
        .map_err(|_| ProductionMiddleEndEvidenceCodecErrorV5::InvalidRankedIrUtf8)?;
    for (offset, byte) in bytes.iter().copied().enumerate() {
        if byte != b'\n' && !(b' '..=b'~').contains(&byte) {
            return Err(
                ProductionMiddleEndEvidenceCodecErrorV5::NonCanonicalRankedIrByte { offset },
            );
        }
    }
    if bytes.last() != Some(&b'\n') {
        return Err(ProductionMiddleEndEvidenceCodecErrorV5::RankedIrMissingFinalNewline);
    }
    Ok(())
}

fn derive_evidence_identity_v5(preimage: &[u8]) -> Option<[u8; SHA256_BYTES]> {
    let mut digest = Sha256::new();
    digest.update(IDENTITY_DOMAIN_V5);
    digest.update((preimage.len() as u64).to_le_bytes());
    digest.update(preimage);
    let identity: [u8; SHA256_BYTES] = digest.finalize().into();
    (identity != [0; SHA256_BYTES]).then_some(identity)
}

struct ReaderV5<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> ReaderV5<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    const fn offset(&self) -> usize {
        self.offset
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], ProductionMiddleEndEvidenceCodecErrorV5> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(ProductionMiddleEndEvidenceCodecErrorV5::Truncated)?;
        let result = self
            .bytes
            .get(self.offset..end)
            .ok_or(ProductionMiddleEndEvidenceCodecErrorV5::Truncated)?;
        self.offset = end;
        Ok(result)
    }

    fn fixed<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], ProductionMiddleEndEvidenceCodecErrorV5> {
        self.take(N)?
            .try_into()
            .map_err(|_| ProductionMiddleEndEvidenceCodecErrorV5::Truncated)
    }

    fn u8(&mut self) -> Result<u8, ProductionMiddleEndEvidenceCodecErrorV5> {
        Ok(self.fixed::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, ProductionMiddleEndEvidenceCodecErrorV5> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }

    fn u32(&mut self) -> Result<u32, ProductionMiddleEndEvidenceCodecErrorV5> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }

    fn u64(&mut self) -> Result<u64, ProductionMiddleEndEvidenceCodecErrorV5> {
        Ok(u64::from_le_bytes(self.fixed()?))
    }

    fn is_empty(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canonical_record() -> Vec<u8> {
        let ranked = b"func @contract_test {\n  kernel.return\n}\n";
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&MAGIC_V5);
        bytes.extend_from_slice(&VERSION_V5.to_le_bytes());
        bytes.extend_from_slice(&FLAGS_V5.to_le_bytes());
        bytes.extend_from_slice(&0_u64.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(
            &(PRODUCTION_MIDDLE_END_EVIDENCE_DOMAIN_V5.len() as u16).to_le_bytes(),
        );
        bytes.extend_from_slice(PRODUCTION_MIDDLE_END_EVIDENCE_DOMAIN_V5);
        bytes.extend_from_slice(
            &(PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V5.len() as u16).to_le_bytes(),
        );
        bytes.extend_from_slice(PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V5);
        bytes.push(ASSURANCE_INTERNAL_CHECKS_ONLY_V5);
        bytes.push(SEMANTIC_OWNER_REVALIDATED_V5);
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&[1; 32]);
        bytes.extend_from_slice(&[2; 32]);
        bytes.extend_from_slice(&(ranked.len() as u32).to_le_bytes());
        bytes.extend_from_slice(ranked);
        bytes.push(PASS_TAGS_V5.len() as u8);
        for tag in PASS_TAGS_V5 {
            bytes.push(tag);
            bytes.push(CLEAN_STATUS_V5);
            bytes.extend_from_slice(&0_u32.to_le_bytes());
            bytes.push(0);
            bytes.push(0);
            bytes.extend_from_slice(&0_u16.to_le_bytes());
        }
        for _ in 0..COVERAGE_COUNTERS_V5
            + SEMANTIC_COUNTERS_V5
            + TYPED_SUMMARY_COUNTERS_V5
            + RECONCILIATION_COUNTERS_V5
        {
            bytes.extend_from_slice(&0_u64.to_le_bytes());
        }
        bytes.extend_from_slice(&[3; 32]);
        let total = bytes.len() + 32;
        bytes[12..20].copy_from_slice(&(total as u64).to_le_bytes());
        let identity = derive_evidence_identity_v5(&bytes).unwrap();
        bytes.extend_from_slice(&identity);
        bytes
    }

    #[test]
    fn canonical_record_roundtrips_with_exact_identity() {
        let bytes = canonical_record();
        let decoded = InertProductionMiddleEndEvidenceV5::decode(&bytes).unwrap();
        assert_eq!(decoded.canonical_bytes(), bytes);
        assert!(decoded.identity().matches_canonical_bytes(&bytes));
        assert_eq!(decoded.source_semantic_identity(), &[1; 32]);
        assert_eq!(decoded.ranked_kernel_identity(), &[2; 32]);
    }

    #[test]
    fn truncation_trailing_and_rehashed_noncanonical_pass_fail_closed() {
        let bytes = canonical_record();
        for end in 0..bytes.len() {
            assert!(InertProductionMiddleEndEvidenceV5::decode(&bytes[..end]).is_err());
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert_eq!(
            InertProductionMiddleEndEvidenceV5::decode(&trailing),
            Err(ProductionMiddleEndEvidenceCodecErrorV5::TrailingBytes)
        );
        let pass_offset = 20
            + 4
            + 2
            + PRODUCTION_MIDDLE_END_EVIDENCE_DOMAIN_V5.len()
            + 2
            + PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V5.len()
            + 4
            + 32
            + 32
            + 4
            + b"func @contract_test {\n  kernel.return\n}\n".len()
            + 1;
        let mut changed = bytes;
        changed[pass_offset] = 7;
        let terminal = changed.len() - 32;
        let identity = derive_evidence_identity_v5(&changed[..terminal]).unwrap();
        changed[terminal..].copy_from_slice(&identity);
        assert!(matches!(
            InertProductionMiddleEndEvidenceV5::decode(&changed),
            Err(ProductionMiddleEndEvidenceCodecErrorV5::InvalidPassOrder { .. })
        ));
    }
}
