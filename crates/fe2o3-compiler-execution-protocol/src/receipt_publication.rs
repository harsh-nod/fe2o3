#![forbid(unsafe_code)]

use std::{error::Error, fmt};

use crate::receipt_publication_codec as codec;
use sha2::{Digest, Sha256};

use crate::{
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_BYTES_V1,
    COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V1, COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V1,
    CompilerExecutionAttestationErrorV1, CompilerExecutionAttestationReceiptIdentityV1,
    CompilerExecutionAttestationReceiptV1, CompilerExecutionAttestationRequestV1,
    CompilerExecutionIssuerPolicyIdentityV1, CompilerExecutionIssuerPolicyV1,
};

const SHA256_BYTES: usize = 32;
const HEADER_BYTES: usize = 24;
const PUBLICATION_IDENTITY_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-RECEIPT-PUBLICATION/V1\0";
const ACK_IDENTITY_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-RECEIPT-PUBLICATION-ACK/V1\0";
const CARRIAGE_IDENTITY_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-RECEIPT-CARRIAGE/V1\0";

const PUBLICATION_PREIMAGE_BYTES: usize =
    HEADER_BYTES + (4 * SHA256_BYTES) + COMPILER_EXECUTION_ATTESTATION_RECEIPT_BYTES_V1;
/// Exact canonical byte length of one compiler-execution receipt publication sidecar V1.
pub const COMPILER_EXECUTION_RECEIPT_PUBLICATION_BYTES_V1: usize =
    PUBLICATION_PREIMAGE_BYTES + SHA256_BYTES;

const ACK_PREIMAGE_BYTES: usize = HEADER_BYTES + (6 * SHA256_BYTES) + 8 + SHA256_BYTES;
/// Exact canonical byte length of one compiler-execution receipt publication acknowledgment V1.
pub const COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_BYTES_V1: usize =
    ACK_PREIMAGE_BYTES + SHA256_BYTES;

const CARRIAGE_PREIMAGE_BYTES: usize = HEADER_BYTES
    + COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V1
    + COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V1
    + COMPILER_EXECUTION_RECEIPT_PUBLICATION_BYTES_V1
    + COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_BYTES_V1;
/// Exact canonical byte length of one complete compiler-execution receipt carriage V1.
pub const COMPILER_EXECUTION_RECEIPT_CARRIAGE_BYTES_V1: usize =
    CARRIAGE_PREIMAGE_BYTES + SHA256_BYTES;

macro_rules! identity_type {
    ($name:ident, $domain:ident, $size:ident, $preimage:ident) => {
        #[doc = concat!("Domain-separated identity of one canonical `", stringify!($name), "` record.")]
        #[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name([u8; SHA256_BYTES]);

        impl $name {
            /// Returns the exact domain-separated SHA-256 identity bytes.
            pub const fn as_bytes(&self) -> &[u8; SHA256_BYTES] {
                &self.0
            }

            /// Independently rederives this identity from exact canonical bytes.
            pub fn matches_canonical_bytes(self, bytes: &[u8]) -> bool {
                bytes.len() == $size
                    && bytes[$preimage..] == self.0
                    && derive_identity($domain, &bytes[..$preimage]) == self.0
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter
                    .debug_tuple(stringify!($name))
                    .field(&self.0)
                    .finish()
            }
        }
    };
}

identity_type!(
    CompilerExecutionReceiptPublicationIdentityV1,
    PUBLICATION_IDENTITY_DOMAIN,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_BYTES_V1,
    PUBLICATION_PREIMAGE_BYTES
);
identity_type!(
    CompilerExecutionReceiptPublicationAckIdentityV1,
    ACK_IDENTITY_DOMAIN,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_BYTES_V1,
    ACK_PREIMAGE_BYTES
);
identity_type!(
    CompilerExecutionReceiptCarriageIdentityV1,
    CARRIAGE_IDENTITY_DOMAIN,
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_BYTES_V1,
    CARRIAGE_PREIMAGE_BYTES
);

impl CompilerExecutionReceiptPublicationIdentityV1 {
    const fn from_bytes_for_protocol(bytes: [u8; SHA256_BYTES]) -> Self {
        Self(bytes)
    }
}

/// Immutable sidecar carrying one exact signed receipt and its issuer-owned bindings.
///
/// This record is authority-free. Its identity proves byte equality, not durable publication. A
/// protected Worker ledger must verify the receipt, commit the exact sidecar and rollback
/// transition, and construct a move-only durability result before issuer acknowledgment.
#[derive(Clone, Eq, PartialEq)]
pub struct CompilerExecutionReceiptPublicationV1 {
    record: codec::Publication,
    receipt: CompilerExecutionAttestationReceiptV1,
}
impl CompilerExecutionReceiptPublicationV1 {
    /// Constructs an inert sidecar; the caller must independently establish durable state.
    pub fn new(
        issuer_journal_identity: [u8; 32],
        compiler_occurrence_identity: [u8; 32],
        receipt: CompilerExecutionAttestationReceiptV1,
    ) -> Result<Self, CompilerExecutionReceiptPublicationErrorV1> {
        let record = codec::V1.publication(
            codec::Bindings {
                policy: *receipt.policy_identity().as_bytes(),
                journal: issuer_journal_identity,
                occurrence: compiler_occurrence_identity,
                receipt: *receipt.identity().as_bytes(),
            },
            receipt.canonical_bytes(),
        )?;
        Ok(Self { record, receipt })
    }
    /// Strictly decodes the nested signature before publication bindings.
    pub fn decode(bytes: &[u8]) -> Result<Self, CompilerExecutionReceiptPublicationErrorV1> {
        let parts = codec::V1.publication_parts(bytes)?;
        let receipt = CompilerExecutionAttestationReceiptV1::decode(parts.receipt)?;
        let record = codec::V1.finish_publication(
            parts,
            *receipt.policy_identity().as_bytes(),
            *receipt.identity().as_bytes(),
            receipt.canonical_bytes(),
            bytes,
        )?;
        Ok(Self { record, receipt })
    }
    pub const fn policy_identity(&self) -> CompilerExecutionIssuerPolicyIdentityV1 {
        CompilerExecutionIssuerPolicyIdentityV1::from_bytes_for_protocol(
            self.record.bindings.policy,
        )
    }
    pub const fn issuer_journal_identity(&self) -> [u8; 32] {
        self.record.bindings.journal
    }
    pub const fn compiler_occurrence_identity(&self) -> [u8; 32] {
        self.record.bindings.occurrence
    }
    pub const fn receipt_identity(&self) -> CompilerExecutionAttestationReceiptIdentityV1 {
        CompilerExecutionAttestationReceiptIdentityV1::from_bytes_for_protocol(
            self.record.bindings.receipt,
        )
    }
    pub const fn receipt(&self) -> &CompilerExecutionAttestationReceiptV1 {
        &self.receipt
    }
    pub const fn identity(&self) -> CompilerExecutionReceiptPublicationIdentityV1 {
        CompilerExecutionReceiptPublicationIdentityV1(self.record.wire.identity)
    }
    pub const fn canonical_bytes(&self) -> &[u8; COMPILER_EXECUTION_RECEIPT_PUBLICATION_BYTES_V1] {
        &self.record.wire.bytes
    }
    /// Checks every binding against independently retained issued state.
    pub fn matches_issued_record(
        &self,
        expected_policy_identity: CompilerExecutionIssuerPolicyIdentityV1,
        expected_issuer_journal_identity: [u8; 32],
        expected_compiler_occurrence_identity: [u8; 32],
        expected_receipt_identity: CompilerExecutionAttestationReceiptIdentityV1,
    ) -> Result<(), CompilerExecutionReceiptPublicationErrorV1> {
        self.record.bindings.matches(codec::Bindings {
            policy: *expected_policy_identity.as_bytes(),
            journal: expected_issuer_journal_identity,
            occurrence: expected_compiler_occurrence_identity,
            receipt: *expected_receipt_identity.as_bytes(),
        })
    }
    pub const fn proves_durable_publication(&self) -> bool {
        false
    }
    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }
}
impl fmt::Debug for CompilerExecutionReceiptPublicationV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompilerExecutionReceiptPublicationV1")
            .field("policy_identity", &self.policy_identity())
            .field("issuer_journal_identity", &self.issuer_journal_identity())
            .field(
                "compiler_occurrence_identity",
                &self.compiler_occurrence_identity(),
            )
            .field("receipt_identity", &self.receipt_identity())
            .field("identity", &self.identity())
            .finish_non_exhaustive()
    }
}

/// Inert claim that a protected Worker ledger durably consumed one exact sidecar.
///
/// The protected Worker ledger must be independently reacquired and matched before this claim can
/// produce an issuer ACK authority.
#[derive(Clone, Eq, PartialEq)]
pub struct CompilerExecutionReceiptPublicationAckV1 {
    record: codec::Ack,
}
impl CompilerExecutionReceiptPublicationAckV1 {
    /// Constructs an inert ACK claim; durable Worker state remains independently verified.
    pub fn new(
        publication: &CompilerExecutionReceiptPublicationV1,
        worker_ledger_record_identity: [u8; 32],
    ) -> Result<Self, CompilerExecutionReceiptPublicationErrorV1> {
        Ok(Self {
            record: codec::V1.ack(codec::AckFields {
                bindings: publication.record.bindings,
                publication: publication.record.wire.identity,
                worker: worker_ledger_record_identity,
                sequence: publication.receipt.sequence(),
                current: publication.receipt.next_rollback_anchor(),
            })?,
        })
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, CompilerExecutionReceiptPublicationErrorV1> {
        Ok(Self {
            record: codec::V1.decode_ack(bytes)?,
        })
    }
    pub fn matches_publication(
        &self,
        publication: &CompilerExecutionReceiptPublicationV1,
    ) -> Result<(), CompilerExecutionReceiptPublicationErrorV1> {
        self.record.matches(
            &publication.record,
            publication.receipt.sequence(),
            publication.receipt.next_rollback_anchor(),
        )
    }
    pub const fn policy_identity(&self) -> CompilerExecutionIssuerPolicyIdentityV1 {
        CompilerExecutionIssuerPolicyIdentityV1::from_bytes_for_protocol(
            self.record.fields.bindings.policy,
        )
    }
    pub const fn issuer_journal_identity(&self) -> [u8; 32] {
        self.record.fields.bindings.journal
    }
    pub const fn compiler_occurrence_identity(&self) -> [u8; 32] {
        self.record.fields.bindings.occurrence
    }
    pub const fn receipt_identity(&self) -> CompilerExecutionAttestationReceiptIdentityV1 {
        CompilerExecutionAttestationReceiptIdentityV1::from_bytes_for_protocol(
            self.record.fields.bindings.receipt,
        )
    }
    pub const fn publication_identity(&self) -> CompilerExecutionReceiptPublicationIdentityV1 {
        CompilerExecutionReceiptPublicationIdentityV1::from_bytes_for_protocol(
            self.record.fields.publication,
        )
    }
    pub const fn worker_ledger_record_identity(&self) -> [u8; 32] {
        self.record.fields.worker
    }
    pub const fn sequence(&self) -> u64 {
        self.record.fields.sequence
    }
    pub const fn current_rollback_anchor(&self) -> [u8; 32] {
        self.record.fields.current
    }
    pub const fn identity(&self) -> CompilerExecutionReceiptPublicationAckIdentityV1 {
        CompilerExecutionReceiptPublicationAckIdentityV1(self.record.wire.identity)
    }
    pub const fn canonical_bytes(
        &self,
    ) -> &[u8; COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_BYTES_V1] {
        &self.record.wire.bytes
    }
    /// Checks the claim against independently reacquired durable Worker state.
    pub fn matches_worker_ledger_record(
        &self,
        expected: [u8; 32],
    ) -> Result<(), CompilerExecutionReceiptPublicationErrorV1> {
        self.record.matches_worker(expected)
    }
    pub const fn proves_durable_publication(&self) -> bool {
        false
    }
    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }
}
impl fmt::Debug for CompilerExecutionReceiptPublicationAckV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompilerExecutionReceiptPublicationAckV1")
            .field("policy_identity", &self.policy_identity())
            .field("issuer_journal_identity", &self.issuer_journal_identity())
            .field(
                "compiler_occurrence_identity",
                &self.compiler_occurrence_identity(),
            )
            .field("receipt_identity", &self.receipt_identity())
            .field("publication_identity", &self.publication_identity())
            .field(
                "worker_ledger_record_identity",
                &self.worker_ledger_record_identity(),
            )
            .field("sequence", &self.sequence())
            .field("identity", &self.identity())
            .finish_non_exhaustive()
    }
}

/// Complete authority-free evidence needed to verify one published compiler receipt.
///
/// The record carries the caller-pinned policy, complete signed request, issuer publication
/// sidecar, and Worker publication ACK without projecting or reinterpreting any nested field. Its
/// constructor verifies the receipt against the carried policy and request and checks the exact
/// ACK relationship. A consumer must still compare the policy with protected configuration and
/// atomically enforce rollback before constructing compiler authority.
#[derive(Clone, Eq, PartialEq)]
pub struct CompilerExecutionReceiptCarriageV1 {
    policy: CompilerExecutionIssuerPolicyV1,
    request: CompilerExecutionAttestationRequestV1,
    publication: CompilerExecutionReceiptPublicationV1,
    acknowledgment: CompilerExecutionReceiptPublicationAckV1,
    identity: CompilerExecutionReceiptCarriageIdentityV1,
    canonical_bytes: [u8; COMPILER_EXECUTION_RECEIPT_CARRIAGE_BYTES_V1],
}

impl CompilerExecutionReceiptCarriageV1 {
    /// Constructs one exact internally consistent carriage record.
    pub fn new(
        policy: CompilerExecutionIssuerPolicyV1,
        request: CompilerExecutionAttestationRequestV1,
        publication: CompilerExecutionReceiptPublicationV1,
        acknowledgment: CompilerExecutionReceiptPublicationAckV1,
    ) -> Result<Self, CompilerExecutionReceiptPublicationErrorV1> {
        publication.receipt().clone().verify(
            &policy,
            &request,
            request.challenge().prior_rollback_anchor(),
        )?;
        acknowledgment.matches_publication(&publication)?;

        let record = codec::V1.carriage(
            policy.canonical_bytes(),
            request.canonical_bytes(),
            publication.canonical_bytes(),
            acknowledgment.canonical_bytes(),
        );
        Ok(Self {
            policy,
            request,
            publication,
            acknowledgment,
            identity: CompilerExecutionReceiptCarriageIdentityV1(record.identity),
            canonical_bytes: record.bytes,
        })
    }
    /// Strictly decodes every complete nested record and rechecks relationships.
    pub fn decode(bytes: &[u8]) -> Result<Self, CompilerExecutionReceiptPublicationErrorV1> {
        let parts = codec::V1.carriage_parts(bytes)?;
        let policy = CompilerExecutionIssuerPolicyV1::decode(parts.policy)?;
        let request = CompilerExecutionAttestationRequestV1::decode(parts.request)?;
        let publication = CompilerExecutionReceiptPublicationV1::decode(parts.publication)?;
        let acknowledgment = CompilerExecutionReceiptPublicationAckV1::decode(parts.ack)?;
        let decoded = Self::new(policy, request, publication, acknowledgment)?;
        if decoded.identity.0 != parts.identity || decoded.canonical_bytes.as_slice() != bytes {
            return Err(
                CompilerExecutionReceiptPublicationErrorV1::IdentityMismatch(
                    "compiler receipt carriage",
                ),
            );
        }
        Ok(decoded)
    }

    pub const fn policy(&self) -> &CompilerExecutionIssuerPolicyV1 {
        &self.policy
    }

    pub const fn request(&self) -> &CompilerExecutionAttestationRequestV1 {
        &self.request
    }

    pub const fn publication(&self) -> &CompilerExecutionReceiptPublicationV1 {
        &self.publication
    }

    pub const fn acknowledgment(&self) -> &CompilerExecutionReceiptPublicationAckV1 {
        &self.acknowledgment
    }

    pub const fn identity(&self) -> CompilerExecutionReceiptCarriageIdentityV1 {
        self.identity
    }

    pub const fn canonical_bytes(&self) -> &[u8; COMPILER_EXECUTION_RECEIPT_CARRIAGE_BYTES_V1] {
        &self.canonical_bytes
    }

    /// The carried policy must still be compared with protected verifier configuration.
    pub const fn requires_protected_policy_verification(&self) -> bool {
        true
    }

    /// Carriage bytes alone do not establish durable rollback currentness.
    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

impl fmt::Debug for CompilerExecutionReceiptCarriageV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilerExecutionReceiptCarriageV1")
            .field("policy_identity", &self.policy.identity())
            .field("request_identity", &self.request.identity())
            .field("publication_identity", &self.publication.identity())
            .field("acknowledgment_identity", &self.acknowledgment.identity())
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

pub(crate) fn encode_header_version(output: &mut [u8], magic: [u8; 8], version: u16) -> usize {
    let total_length = output.len();
    let mut offset = 0;
    put(output, &mut offset, &magic);
    put(output, &mut offset, &version.to_le_bytes());
    put(output, &mut offset, &0_u16.to_le_bytes());
    put(output, &mut offset, &(total_length as u64).to_le_bytes());
    put(output, &mut offset, &0_u32.to_le_bytes());
    debug_assert_eq!(offset, HEADER_BYTES);
    offset
}

pub(crate) fn decode_header_version(
    reader: &mut Reader<'_>,
    expected_magic: [u8; 8],
    expected_version: u16,
    actual_length: usize,
    field: &'static str,
) -> Result<(), CompilerExecutionReceiptPublicationErrorV1> {
    if reader.fixed::<8>()? != expected_magic {
        return Err(CompilerExecutionReceiptPublicationErrorV1::InvalidMagic(
            field,
        ));
    }
    let version = reader.u16()?;
    if version != expected_version {
        return Err(
            CompilerExecutionReceiptPublicationErrorV1::UnsupportedVersion { field, version },
        );
    }
    let flags = reader.u16()?;
    if flags != 0 {
        return Err(CompilerExecutionReceiptPublicationErrorV1::UnsupportedFlags { field, flags });
    }
    let declared = reader.u64()?;
    if declared != actual_length as u64 {
        return Err(
            CompilerExecutionReceiptPublicationErrorV1::DeclaredLengthMismatch {
                field,
                declared,
                actual: actual_length,
            },
        );
    }
    if reader.fixed::<4>()? != [0; 4] {
        return Err(CompilerExecutionReceiptPublicationErrorV1::NonzeroReserved);
    }
    Ok(())
}

pub(crate) fn require_identity(
    value: [u8; SHA256_BYTES],
    field: &'static str,
) -> Result<(), CompilerExecutionReceiptPublicationErrorV1> {
    if value == [0; SHA256_BYTES] {
        Err(CompilerExecutionReceiptPublicationErrorV1::ZeroValue(field))
    } else {
        Ok(())
    }
}

pub(crate) fn require_length(
    bytes: &[u8],
    expected: usize,
    field: &'static str,
) -> Result<(), CompilerExecutionReceiptPublicationErrorV1> {
    if bytes.len() == expected {
        Ok(())
    } else {
        Err(CompilerExecutionReceiptPublicationErrorV1::InvalidLength {
            field,
            expected,
            actual: bytes.len(),
        })
    }
}

pub(crate) fn derive_identity(domain: &[u8], bytes: &[u8]) -> [u8; SHA256_BYTES] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    digest.finalize().into()
}

pub(crate) fn put(output: &mut [u8], offset: &mut usize, value: &[u8]) {
    let end = *offset + value.len();
    output[*offset..end].copy_from_slice(value);
    *offset = end;
}

pub(crate) struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    pub(crate) const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    pub(crate) fn take(
        &mut self,
        length: usize,
    ) -> Result<&'a [u8], CompilerExecutionReceiptPublicationErrorV1> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(CompilerExecutionReceiptPublicationErrorV1::Truncated)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(CompilerExecutionReceiptPublicationErrorV1::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    pub(crate) fn fixed<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], CompilerExecutionReceiptPublicationErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| CompilerExecutionReceiptPublicationErrorV1::Truncated)
    }

    pub(crate) fn u16(&mut self) -> Result<u16, CompilerExecutionReceiptPublicationErrorV1> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }

    pub(crate) fn u64(&mut self) -> Result<u64, CompilerExecutionReceiptPublicationErrorV1> {
        Ok(u64::from_le_bytes(self.fixed()?))
    }
}

/// Strict canonical receipt-publication and ACK decoding failure.
#[derive(Debug)]
#[non_exhaustive]
pub enum CompilerExecutionReceiptPublicationErrorV1 {
    InvalidLength {
        field: &'static str,
        expected: usize,
        actual: usize,
    },
    InvalidMagic(&'static str),
    UnsupportedVersion {
        field: &'static str,
        version: u16,
    },
    UnsupportedFlags {
        field: &'static str,
        flags: u16,
    },
    DeclaredLengthMismatch {
        field: &'static str,
        declared: u64,
        actual: usize,
    },
    NonzeroReserved,
    ZeroValue(&'static str),
    PolicyMismatch,
    IssuerJournalMismatch,
    OccurrenceMismatch,
    ReceiptMismatch,
    PublicationMismatch,
    WorkerLedgerMismatch,
    SequenceMismatch,
    RollbackAnchorMismatch,
    IdentityMismatch(&'static str),
    TrailingBytes,
    Truncated,
    Attestation(CompilerExecutionAttestationErrorV1),
}

impl fmt::Display for CompilerExecutionReceiptPublicationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLength {
                field,
                expected,
                actual,
            } => write!(
                formatter,
                "{field} has {actual} bytes; expected exactly {expected}"
            ),
            Self::InvalidMagic(field) => write!(formatter, "invalid {field} magic"),
            Self::UnsupportedVersion { field, version } => {
                write!(formatter, "unsupported {field} version {version}")
            }
            Self::UnsupportedFlags { field, flags } => {
                write!(formatter, "unsupported {field} flags {flags:#06x}")
            }
            Self::DeclaredLengthMismatch {
                field,
                declared,
                actual,
            } => write!(
                formatter,
                "{field} declares {declared} bytes but contains {actual}"
            ),
            Self::NonzeroReserved => formatter.write_str("reserved bytes are nonzero"),
            Self::ZeroValue(field) => write!(formatter, "{field} is zero"),
            Self::PolicyMismatch => formatter.write_str("issuer policy identity mismatch"),
            Self::IssuerJournalMismatch => formatter.write_str("issuer journal identity mismatch"),
            Self::OccurrenceMismatch => {
                formatter.write_str("compiler occurrence identity mismatch")
            }
            Self::ReceiptMismatch => formatter.write_str("compiler receipt identity mismatch"),
            Self::PublicationMismatch => {
                formatter.write_str("compiler receipt publication identity mismatch")
            }
            Self::WorkerLedgerMismatch => formatter.write_str("Worker ledger identity mismatch"),
            Self::SequenceMismatch => formatter.write_str("compiler receipt sequence mismatch"),
            Self::RollbackAnchorMismatch => {
                formatter.write_str("Worker current rollback anchor mismatch")
            }
            Self::IdentityMismatch(field) => write!(formatter, "{field} identity mismatch"),
            Self::TrailingBytes => formatter.write_str("canonical record has trailing bytes"),
            Self::Truncated => formatter.write_str("canonical record is truncated"),
            Self::Attestation(error) => write!(formatter, "invalid compiler receipt: {error}"),
        }
    }
}

impl Error for CompilerExecutionReceiptPublicationErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Attestation(error) => Some(error),
            _ => None,
        }
    }
}

impl From<CompilerExecutionAttestationErrorV1> for CompilerExecutionReceiptPublicationErrorV1 {
    fn from(error: CompilerExecutionAttestationErrorV1) -> Self {
        Self::Attestation(error)
    }
}
