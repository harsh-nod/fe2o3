//! Canonical, authority-free custody for semantic `u32` induction reports.

use std::{error::Error, fmt};

use sha2::{Digest, Sha256};

use crate::{
    MAX_SEMANTIC_U32_INDUCTION_CERTIFICATES_V1, MAX_SEMANTIC_U32_INDUCTION_WORK_V1,
    SemanticU32InductionBlockSiteV1, SemanticU32InductionNoOverflowCertificateV1,
    SemanticU32InductionNoOverflowReportV1, SemanticU32InductionPlaceBindingV1,
    SemanticU32InductionStatementSiteV1,
};

/// Canonical wire version for semantic induction report custody.
pub const SEMANTIC_U32_INDUCTION_EVIDENCE_VERSION_V1: u16 = 1;
/// Closed validation policy for semantic induction report custody.
pub const SEMANTIC_U32_INDUCTION_EVIDENCE_POLICY_V1: u16 = 1;
/// Maximum exact bytes admitted by one report evidence record.
pub const MAX_SEMANTIC_U32_INDUCTION_EVIDENCE_BYTES_V1: usize = 64 * 1024 * 1024;

const MAGIC_V1: [u8; 8] = *b"F2U32I\0\0";
const IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/SEMANTIC-U32-INDUCTION-EVIDENCE/V1\0";
const HEADER_BYTES_V1: usize = 104;
const PLACE_BYTES_V1: usize = 72;
const BLOCK_SITE_BYTES_V1: usize = 36;
const STATEMENT_SITE_BYTES_V1: usize = 40;
const OPTIONAL_STATEMENT_SITE_BYTES_V1: usize = 44;
const CERTIFICATE_BYTES_V1: usize = PLACE_BYTES_V1 * 5
    + BLOCK_SITE_BYTES_V1 * 4
    + STATEMENT_SITE_BYTES_V1 * 4
    + OPTIONAL_STATEMENT_SITE_BYTES_V1;

/// Exact local and type identities used by one induction fact.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticU32InductionPlaceEvidenceV1 {
    local: u32,
    local_identity: [u8; 32],
    ty: u32,
    type_identity: [u8; 32],
}

impl SemanticU32InductionPlaceEvidenceV1 {
    /// Returns the semantic local index.
    pub const fn local(&self) -> u32 {
        self.local
    }

    /// Returns the exact semantic local identity.
    pub const fn local_identity(&self) -> &[u8; 32] {
        &self.local_identity
    }

    /// Returns the semantic type index.
    pub const fn ty(&self) -> u32 {
        self.ty
    }

    /// Returns the exact semantic type identity.
    pub const fn type_identity(&self) -> &[u8; 32] {
        &self.type_identity
    }
}

/// Exact identity-bound semantic block site used by one induction fact.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticU32InductionBlockSiteEvidenceV1 {
    block: u32,
    identity: [u8; 32],
}

impl SemanticU32InductionBlockSiteEvidenceV1 {
    /// Returns the semantic block index.
    pub const fn block(&self) -> u32 {
        self.block
    }

    /// Returns the exact semantic block identity.
    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }
}

/// Exact statement site used by one induction fact.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticU32InductionStatementSiteEvidenceV1 {
    block: SemanticU32InductionBlockSiteEvidenceV1,
    statement: u32,
}

impl SemanticU32InductionStatementSiteEvidenceV1 {
    /// Returns the identity-bound semantic block site.
    pub const fn block(&self) -> &SemanticU32InductionBlockSiteEvidenceV1 {
        &self.block
    }

    /// Returns the zero-based semantic statement ordinal.
    pub const fn statement(&self) -> u32 {
        self.statement
    }
}

/// Lossless authority-free representation of one semantic no-overflow certificate.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticU32InductionNoOverflowCertificateEvidenceV1 {
    induction: SemanticU32InductionPlaceEvidenceV1,
    guard_induction: SemanticU32InductionPlaceEvidenceV1,
    bound: SemanticU32InductionPlaceEvidenceV1,
    predicate: SemanticU32InductionPlaceEvidenceV1,
    checked_result: SemanticU32InductionPlaceEvidenceV1,
    preheader: SemanticU32InductionBlockSiteEvidenceV1,
    header: SemanticU32InductionBlockSiteEvidenceV1,
    body_entry: SemanticU32InductionBlockSiteEvidenceV1,
    exit: SemanticU32InductionBlockSiteEvidenceV1,
    initialization: SemanticU32InductionStatementSiteEvidenceV1,
    guard_induction_snapshot: Option<SemanticU32InductionStatementSiteEvidenceV1>,
    guard: SemanticU32InductionStatementSiteEvidenceV1,
    checked_addition: SemanticU32InductionStatementSiteEvidenceV1,
    update: SemanticU32InductionStatementSiteEvidenceV1,
}

macro_rules! place_getter {
    ($name:ident) => {
        /// Returns the exact retained place binding.
        pub const fn $name(&self) -> &SemanticU32InductionPlaceEvidenceV1 {
            &self.$name
        }
    };
}

macro_rules! block_getter {
    ($name:ident) => {
        /// Returns the exact retained block site.
        pub const fn $name(&self) -> &SemanticU32InductionBlockSiteEvidenceV1 {
            &self.$name
        }
    };
}

macro_rules! statement_getter {
    ($name:ident) => {
        /// Returns the exact retained statement site.
        pub const fn $name(&self) -> &SemanticU32InductionStatementSiteEvidenceV1 {
            &self.$name
        }
    };
}

impl SemanticU32InductionNoOverflowCertificateEvidenceV1 {
    place_getter!(induction);
    place_getter!(guard_induction);
    place_getter!(bound);
    place_getter!(predicate);
    place_getter!(checked_result);
    block_getter!(preheader);
    block_getter!(header);
    block_getter!(body_entry);
    block_getter!(exit);
    statement_getter!(initialization);
    statement_getter!(guard);
    statement_getter!(checked_addition);
    statement_getter!(update);

    /// Returns the exact optional header-snapshot definition site.
    pub const fn guard_induction_snapshot(
        &self,
    ) -> Option<&SemanticU32InductionStatementSiteEvidenceV1> {
        self.guard_induction_snapshot.as_ref()
    }

    /// Canonical report evidence never grants compiler or runtime authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Strict canonical custody for one complete semantic induction report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InertCanonicalSemanticU32InductionEvidenceV1 {
    canonical_bytes: Box<[u8]>,
    identity: [u8; 32],
    semantic_mir_sha256: [u8; 32],
    function: u32,
    function_identity: [u8; 32],
    checked_additions_examined: u32,
    work_units: u64,
    certificates: Box<[SemanticU32InductionNoOverflowCertificateEvidenceV1]>,
}

impl InertCanonicalSemanticU32InductionEvidenceV1 {
    /// Constructs and independently decodes canonical evidence from one live report.
    pub fn from_report(
        report: &SemanticU32InductionNoOverflowReportV1,
    ) -> Result<Self, SemanticU32InductionEvidenceErrorV1> {
        if report.certificates().iter().any(|certificate| {
            certificate.semantic_mir_sha256() != report.semantic_mir_sha256()
                || certificate.function() != report.function()
                || certificate.function_identity() != report.function_identity()
        }) {
            return Err(SemanticU32InductionEvidenceErrorV1::InvalidReport);
        }
        let checked_additions_examined = u32::try_from(report.checked_additions_examined())
            .map_err(|_| SemanticU32InductionEvidenceErrorV1::Overflow)?;
        let work_units = u64::try_from(report.work_units())
            .map_err(|_| SemanticU32InductionEvidenceErrorV1::Overflow)?;
        let mut certificates = report
            .certificates()
            .iter()
            .copied()
            .map(certificate_from_live)
            .collect::<Vec<_>>();
        certificates.sort_unstable_by_key(|certificate| {
            (
                certificate.checked_addition.block.block,
                certificate.checked_addition.statement,
            )
        });
        let bytes = encode(
            *report.semantic_mir_sha256().as_bytes(),
            report.function().index(),
            *report.function_identity().as_bytes(),
            checked_additions_examined,
            work_units,
            &certificates,
        )?;
        Self::decode(&bytes)
    }

    /// Strictly decodes one complete canonical report-evidence value.
    pub fn decode(bytes: &[u8]) -> Result<Self, SemanticU32InductionEvidenceErrorV1> {
        if bytes.len() > MAX_SEMANTIC_U32_INDUCTION_EVIDENCE_BYTES_V1 {
            return Err(SemanticU32InductionEvidenceErrorV1::TooLarge);
        }
        let mut reader = ReaderV1::new(bytes);
        if reader.fixed::<8>()? != MAGIC_V1 {
            return Err(SemanticU32InductionEvidenceErrorV1::InvalidHeader);
        }
        if reader.u16()? != SEMANTIC_U32_INDUCTION_EVIDENCE_VERSION_V1
            || reader.u16()? != SEMANTIC_U32_INDUCTION_EVIDENCE_POLICY_V1
            || reader.u32()? != 0
        {
            return Err(SemanticU32InductionEvidenceErrorV1::InvalidHeader);
        }
        let declared = usize::try_from(reader.u32()?)
            .map_err(|_| SemanticU32InductionEvidenceErrorV1::Overflow)?;
        if declared != bytes.len() {
            return Err(SemanticU32InductionEvidenceErrorV1::InvalidLength);
        }
        let semantic_mir_sha256 = reader.fixed::<32>()?;
        let function = reader.u32()?;
        let function_identity = reader.fixed::<32>()?;
        let checked_additions_examined = reader.u32()?;
        let work_units = reader.u64()?;
        let certificate_count = usize::try_from(reader.u32()?)
            .map_err(|_| SemanticU32InductionEvidenceErrorV1::Overflow)?;
        if certificate_count > MAX_SEMANTIC_U32_INDUCTION_CERTIFICATES_V1
            || certificate_count > checked_additions_examined as usize
            || work_units > MAX_SEMANTIC_U32_INDUCTION_WORK_V1 as u64
        {
            return Err(SemanticU32InductionEvidenceErrorV1::InvalidReport);
        }
        let exact_remaining = certificate_count
            .checked_mul(CERTIFICATE_BYTES_V1)
            .ok_or(SemanticU32InductionEvidenceErrorV1::Overflow)?;
        if reader.remaining() != exact_remaining {
            return Err(SemanticU32InductionEvidenceErrorV1::InvalidLength);
        }
        let mut certificates = Vec::with_capacity(certificate_count);
        for _ in 0..certificate_count {
            certificates.push(decode_certificate(&mut reader)?);
        }
        reader.finish()?;
        validate_report(
            &semantic_mir_sha256,
            &function_identity,
            checked_additions_examined,
            work_units,
            &certificates,
        )?;
        let reencoded = encode(
            semantic_mir_sha256,
            function,
            function_identity,
            checked_additions_examined,
            work_units,
            &certificates,
        )?;
        if reencoded != bytes {
            return Err(SemanticU32InductionEvidenceErrorV1::NonCanonical);
        }
        let identity = evidence_identity(&reencoded);
        require_nonzero(&identity)?;
        Ok(Self {
            canonical_bytes: reencoded.into_boxed_slice(),
            identity,
            semantic_mir_sha256,
            function,
            function_identity,
            checked_additions_examined,
            work_units,
            certificates: certificates.into_boxed_slice(),
        })
    }

    /// Re-decodes the exact retained bytes and identity.
    pub fn revalidate(&self) -> Result<(), SemanticU32InductionEvidenceErrorV1> {
        let decoded = Self::decode(&self.canonical_bytes)?;
        if decoded != *self {
            return Err(SemanticU32InductionEvidenceErrorV1::IdentityMismatch);
        }
        Ok(())
    }

    /// Returns the complete canonical bytes.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Returns the exact evidence identity.
    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }

    /// Returns the bound semantic MIR SHA-256.
    pub const fn semantic_mir_sha256(&self) -> &[u8; 32] {
        &self.semantic_mir_sha256
    }

    /// Returns the semantic function index.
    pub const fn function(&self) -> u32 {
        self.function
    }

    /// Returns the exact semantic function identity.
    pub const fn function_identity(&self) -> &[u8; 32] {
        &self.function_identity
    }

    /// Returns the number of checked additions examined.
    pub const fn checked_additions_examined(&self) -> u32 {
        self.checked_additions_examined
    }

    /// Returns the bounded analysis work consumed.
    pub const fn work_units(&self) -> u64 {
        self.work_units
    }

    /// Returns every canonical certificate in checked-addition site order.
    pub fn certificates(&self) -> &[SemanticU32InductionNoOverflowCertificateEvidenceV1] {
        &self.certificates
    }

    /// Canonical report custody never grants compiler or runtime authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Fail-closed canonical report custody error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticU32InductionEvidenceErrorV1 {
    /// Input exceeds the fixed wire budget.
    TooLarge,
    /// Header magic, version, policy, flags, or reserved bytes are invalid.
    InvalidHeader,
    /// Declared, computed, or available lengths differ.
    InvalidLength,
    /// A count or byte calculation overflowed.
    Overflow,
    /// An identity was the reserved all-zero value.
    ZeroIdentity,
    /// Report counts or certificate structure are inconsistent.
    InvalidReport,
    /// Input ended before a complete field was available.
    Truncated,
    /// Decoded content does not use the unique canonical representation.
    NonCanonical,
    /// Retained bytes and identity do not revalidate.
    IdentityMismatch,
}

impl fmt::Display for SemanticU32InductionEvidenceErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::TooLarge => "semantic induction evidence exceeds its byte limit",
            Self::InvalidHeader => "semantic induction evidence header is invalid",
            Self::InvalidLength => "semantic induction evidence length is invalid",
            Self::Overflow => "semantic induction evidence arithmetic overflowed",
            Self::ZeroIdentity => "semantic induction evidence contains a zero identity",
            Self::InvalidReport => "semantic induction evidence report is inconsistent",
            Self::Truncated => "semantic induction evidence is truncated",
            Self::NonCanonical => "semantic induction evidence is not canonical",
            Self::IdentityMismatch => "semantic induction evidence identity changed",
        })
    }
}

impl Error for SemanticU32InductionEvidenceErrorV1 {}

fn certificate_from_live(
    certificate: SemanticU32InductionNoOverflowCertificateV1,
) -> SemanticU32InductionNoOverflowCertificateEvidenceV1 {
    SemanticU32InductionNoOverflowCertificateEvidenceV1 {
        induction: place_from_live(certificate.induction()),
        guard_induction: place_from_live(certificate.guard_induction()),
        bound: place_from_live(certificate.bound()),
        predicate: place_from_live(certificate.predicate()),
        checked_result: place_from_live(certificate.checked_result()),
        preheader: block_from_live(certificate.preheader()),
        header: block_from_live(certificate.header()),
        body_entry: block_from_live(certificate.body_entry()),
        exit: block_from_live(certificate.exit()),
        initialization: statement_from_live(certificate.initialization()),
        guard_induction_snapshot: certificate
            .guard_induction_snapshot()
            .map(statement_from_live),
        guard: statement_from_live(certificate.guard()),
        checked_addition: statement_from_live(certificate.checked_addition()),
        update: statement_from_live(certificate.update()),
    }
}

fn place_from_live(
    binding: SemanticU32InductionPlaceBindingV1,
) -> SemanticU32InductionPlaceEvidenceV1 {
    SemanticU32InductionPlaceEvidenceV1 {
        local: binding.local().index(),
        local_identity: *binding.local_identity().as_bytes(),
        ty: binding.ty().index(),
        type_identity: *binding.type_identity().as_bytes(),
    }
}

fn block_from_live(
    site: SemanticU32InductionBlockSiteV1,
) -> SemanticU32InductionBlockSiteEvidenceV1 {
    SemanticU32InductionBlockSiteEvidenceV1 {
        block: site.block().index(),
        identity: *site.identity().as_bytes(),
    }
}

fn statement_from_live(
    site: SemanticU32InductionStatementSiteV1,
) -> SemanticU32InductionStatementSiteEvidenceV1 {
    SemanticU32InductionStatementSiteEvidenceV1 {
        block: block_from_live(site.block()),
        statement: site.statement(),
    }
}

fn validate_report(
    semantic_mir_sha256: &[u8; 32],
    function_identity: &[u8; 32],
    checked_additions_examined: u32,
    work_units: u64,
    certificates: &[SemanticU32InductionNoOverflowCertificateEvidenceV1],
) -> Result<(), SemanticU32InductionEvidenceErrorV1> {
    require_nonzero(semantic_mir_sha256)?;
    require_nonzero(function_identity)?;
    if certificates.len() > checked_additions_examined as usize
        || certificates.len() > MAX_SEMANTIC_U32_INDUCTION_CERTIFICATES_V1
        || work_units > MAX_SEMANTIC_U32_INDUCTION_WORK_V1 as u64
    {
        return Err(SemanticU32InductionEvidenceErrorV1::InvalidReport);
    }
    let mut previous = None;
    for certificate in certificates {
        validate_certificate(certificate)?;
        let site = (
            certificate.checked_addition.block.block,
            certificate.checked_addition.statement,
        );
        if previous.is_some_and(|previous| previous >= site) {
            return Err(SemanticU32InductionEvidenceErrorV1::NonCanonical);
        }
        previous = Some(site);
    }
    Ok(())
}

fn validate_certificate(
    certificate: &SemanticU32InductionNoOverflowCertificateEvidenceV1,
) -> Result<(), SemanticU32InductionEvidenceErrorV1> {
    for place in [
        certificate.induction,
        certificate.guard_induction,
        certificate.bound,
        certificate.predicate,
        certificate.checked_result,
    ] {
        require_nonzero(&place.local_identity)?;
        require_nonzero(&place.type_identity)?;
    }
    for block in [
        certificate.preheader,
        certificate.header,
        certificate.body_entry,
        certificate.exit,
        certificate.initialization.block,
        certificate.guard.block,
        certificate.checked_addition.block,
        certificate.update.block,
    ] {
        require_nonzero(&block.identity)?;
    }
    if certificate.induction.ty != certificate.guard_induction.ty
        || certificate.induction.type_identity != certificate.guard_induction.type_identity
        || certificate.induction.ty != certificate.bound.ty
        || certificate.induction.type_identity != certificate.bound.type_identity
        || certificate.initialization.block != certificate.preheader
        || certificate.guard.block != certificate.header
    {
        return Err(SemanticU32InductionEvidenceErrorV1::InvalidReport);
    }
    match certificate.guard_induction_snapshot {
        Some(snapshot) => {
            require_nonzero(&snapshot.block.identity)?;
            if certificate.guard_induction == certificate.induction
                || snapshot.block != certificate.header
                || snapshot.statement >= certificate.guard.statement
            {
                return Err(SemanticU32InductionEvidenceErrorV1::InvalidReport);
            }
        }
        None if certificate.guard_induction != certificate.induction => {
            return Err(SemanticU32InductionEvidenceErrorV1::InvalidReport);
        }
        None => {}
    }
    Ok(())
}

fn encode(
    semantic_mir_sha256: [u8; 32],
    function: u32,
    function_identity: [u8; 32],
    checked_additions_examined: u32,
    work_units: u64,
    certificates: &[SemanticU32InductionNoOverflowCertificateEvidenceV1],
) -> Result<Vec<u8>, SemanticU32InductionEvidenceErrorV1> {
    validate_report(
        &semantic_mir_sha256,
        &function_identity,
        checked_additions_examined,
        work_units,
        certificates,
    )?;
    let exact_size = HEADER_BYTES_V1
        .checked_add(
            certificates
                .len()
                .checked_mul(CERTIFICATE_BYTES_V1)
                .ok_or(SemanticU32InductionEvidenceErrorV1::Overflow)?,
        )
        .ok_or(SemanticU32InductionEvidenceErrorV1::Overflow)?;
    if exact_size > MAX_SEMANTIC_U32_INDUCTION_EVIDENCE_BYTES_V1 {
        return Err(SemanticU32InductionEvidenceErrorV1::TooLarge);
    }
    let declared =
        u32::try_from(exact_size).map_err(|_| SemanticU32InductionEvidenceErrorV1::Overflow)?;
    let count = u32::try_from(certificates.len())
        .map_err(|_| SemanticU32InductionEvidenceErrorV1::Overflow)?;
    let mut bytes = Vec::with_capacity(exact_size);
    bytes.extend_from_slice(&MAGIC_V1);
    bytes.extend_from_slice(&SEMANTIC_U32_INDUCTION_EVIDENCE_VERSION_V1.to_le_bytes());
    bytes.extend_from_slice(&SEMANTIC_U32_INDUCTION_EVIDENCE_POLICY_V1.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&declared.to_le_bytes());
    bytes.extend_from_slice(&semantic_mir_sha256);
    bytes.extend_from_slice(&function.to_le_bytes());
    bytes.extend_from_slice(&function_identity);
    bytes.extend_from_slice(&checked_additions_examined.to_le_bytes());
    bytes.extend_from_slice(&work_units.to_le_bytes());
    bytes.extend_from_slice(&count.to_le_bytes());
    for certificate in certificates {
        encode_certificate(&mut bytes, certificate);
    }
    if bytes.len() != exact_size {
        return Err(SemanticU32InductionEvidenceErrorV1::InvalidLength);
    }
    Ok(bytes)
}

fn encode_certificate(
    bytes: &mut Vec<u8>,
    certificate: &SemanticU32InductionNoOverflowCertificateEvidenceV1,
) {
    for place in [
        certificate.induction,
        certificate.guard_induction,
        certificate.bound,
        certificate.predicate,
        certificate.checked_result,
    ] {
        encode_place(bytes, place);
    }
    for block in [
        certificate.preheader,
        certificate.header,
        certificate.body_entry,
        certificate.exit,
    ] {
        encode_block(bytes, block);
    }
    encode_statement(bytes, certificate.initialization);
    match certificate.guard_induction_snapshot {
        Some(site) => {
            bytes.extend_from_slice(&[1, 0, 0, 0]);
            encode_statement(bytes, site);
        }
        None => bytes.extend_from_slice(&[0; OPTIONAL_STATEMENT_SITE_BYTES_V1]),
    }
    encode_statement(bytes, certificate.guard);
    encode_statement(bytes, certificate.checked_addition);
    encode_statement(bytes, certificate.update);
}

fn encode_place(bytes: &mut Vec<u8>, place: SemanticU32InductionPlaceEvidenceV1) {
    bytes.extend_from_slice(&place.local.to_le_bytes());
    bytes.extend_from_slice(&place.local_identity);
    bytes.extend_from_slice(&place.ty.to_le_bytes());
    bytes.extend_from_slice(&place.type_identity);
}

fn encode_block(bytes: &mut Vec<u8>, block: SemanticU32InductionBlockSiteEvidenceV1) {
    bytes.extend_from_slice(&block.block.to_le_bytes());
    bytes.extend_from_slice(&block.identity);
}

fn encode_statement(bytes: &mut Vec<u8>, site: SemanticU32InductionStatementSiteEvidenceV1) {
    encode_block(bytes, site.block);
    bytes.extend_from_slice(&site.statement.to_le_bytes());
}

fn decode_certificate(
    reader: &mut ReaderV1<'_>,
) -> Result<SemanticU32InductionNoOverflowCertificateEvidenceV1, SemanticU32InductionEvidenceErrorV1>
{
    let induction = decode_place(reader)?;
    let guard_induction = decode_place(reader)?;
    let bound = decode_place(reader)?;
    let predicate = decode_place(reader)?;
    let checked_result = decode_place(reader)?;
    let preheader = decode_block(reader)?;
    let header = decode_block(reader)?;
    let body_entry = decode_block(reader)?;
    let exit = decode_block(reader)?;
    let initialization = decode_statement(reader)?;
    let guard_induction_snapshot = match reader.u8()? {
        0 => {
            if reader.fixed::<3>()? != [0; 3]
                || reader.fixed::<STATEMENT_SITE_BYTES_V1>()? != [0; STATEMENT_SITE_BYTES_V1]
            {
                return Err(SemanticU32InductionEvidenceErrorV1::NonCanonical);
            }
            None
        }
        1 => {
            if reader.fixed::<3>()? != [0; 3] {
                return Err(SemanticU32InductionEvidenceErrorV1::NonCanonical);
            }
            Some(decode_statement(reader)?)
        }
        _ => return Err(SemanticU32InductionEvidenceErrorV1::InvalidReport),
    };
    Ok(SemanticU32InductionNoOverflowCertificateEvidenceV1 {
        induction,
        guard_induction,
        bound,
        predicate,
        checked_result,
        preheader,
        header,
        body_entry,
        exit,
        initialization,
        guard_induction_snapshot,
        guard: decode_statement(reader)?,
        checked_addition: decode_statement(reader)?,
        update: decode_statement(reader)?,
    })
}

fn decode_place(
    reader: &mut ReaderV1<'_>,
) -> Result<SemanticU32InductionPlaceEvidenceV1, SemanticU32InductionEvidenceErrorV1> {
    Ok(SemanticU32InductionPlaceEvidenceV1 {
        local: reader.u32()?,
        local_identity: reader.fixed::<32>()?,
        ty: reader.u32()?,
        type_identity: reader.fixed::<32>()?,
    })
}

fn decode_block(
    reader: &mut ReaderV1<'_>,
) -> Result<SemanticU32InductionBlockSiteEvidenceV1, SemanticU32InductionEvidenceErrorV1> {
    Ok(SemanticU32InductionBlockSiteEvidenceV1 {
        block: reader.u32()?,
        identity: reader.fixed::<32>()?,
    })
}

fn decode_statement(
    reader: &mut ReaderV1<'_>,
) -> Result<SemanticU32InductionStatementSiteEvidenceV1, SemanticU32InductionEvidenceErrorV1> {
    Ok(SemanticU32InductionStatementSiteEvidenceV1 {
        block: decode_block(reader)?,
        statement: reader.u32()?,
    })
}

fn require_nonzero(identity: &[u8; 32]) -> Result<(), SemanticU32InductionEvidenceErrorV1> {
    if identity.iter().all(|byte| *byte == 0) {
        Err(SemanticU32InductionEvidenceErrorV1::ZeroIdentity)
    } else {
        Ok(())
    }
}

fn evidence_identity(bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(IDENTITY_DOMAIN_V1);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    digest.finalize().into()
}

struct ReaderV1<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> ReaderV1<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.offset)
    }

    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], SemanticU32InductionEvidenceErrorV1> {
        let end = self
            .offset
            .checked_add(N)
            .ok_or(SemanticU32InductionEvidenceErrorV1::Overflow)?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or(SemanticU32InductionEvidenceErrorV1::Truncated)?;
        self.offset = end;
        bytes
            .try_into()
            .map_err(|_| SemanticU32InductionEvidenceErrorV1::Truncated)
    }

    fn u8(&mut self) -> Result<u8, SemanticU32InductionEvidenceErrorV1> {
        Ok(self.fixed::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, SemanticU32InductionEvidenceErrorV1> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }

    fn u32(&mut self) -> Result<u32, SemanticU32InductionEvidenceErrorV1> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }

    fn u64(&mut self) -> Result<u64, SemanticU32InductionEvidenceErrorV1> {
        Ok(u64::from_le_bytes(self.fixed()?))
    }

    fn finish(self) -> Result<(), SemanticU32InductionEvidenceErrorV1> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(SemanticU32InductionEvidenceErrorV1::InvalidLength)
        }
    }
}
