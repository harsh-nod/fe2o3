#![forbid(unsafe_code)]

use std::{error::Error, fmt};

use sha2::{Digest, Sha256};

use crate::{
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_BYTES_V1, CompilerExecutionAttestationErrorV1,
    CompilerExecutionAttestationReceiptIdentityV1, CompilerExecutionAttestationReceiptV1,
    CompilerExecutionIssuerPolicyIdentityV1,
};

const SHA256_BYTES: usize = 32;
const HEADER_BYTES: usize = 24;
const VERSION_V1: u16 = 1;

const PUBLICATION_MAGIC: [u8; 8] = *b"F2O3CES1";
const ACK_MAGIC: [u8; 8] = *b"F2O3CEA1";
const PUBLICATION_IDENTITY_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-RECEIPT-PUBLICATION/V1\0";
const ACK_IDENTITY_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-RECEIPT-PUBLICATION-ACK/V1\0";

const PUBLICATION_PREIMAGE_BYTES: usize =
    HEADER_BYTES + (4 * SHA256_BYTES) + COMPILER_EXECUTION_ATTESTATION_RECEIPT_BYTES_V1;
/// Exact canonical byte length of one compiler-execution receipt publication sidecar V1.
pub const COMPILER_EXECUTION_RECEIPT_PUBLICATION_BYTES_V1: usize =
    PUBLICATION_PREIMAGE_BYTES + SHA256_BYTES;

const ACK_PREIMAGE_BYTES: usize = HEADER_BYTES + (6 * SHA256_BYTES) + 8 + SHA256_BYTES;
/// Exact canonical byte length of one compiler-execution receipt publication acknowledgment V1.
pub const COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_BYTES_V1: usize =
    ACK_PREIMAGE_BYTES + SHA256_BYTES;

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
    policy_identity: CompilerExecutionIssuerPolicyIdentityV1,
    issuer_journal_identity: [u8; SHA256_BYTES],
    compiler_occurrence_identity: [u8; SHA256_BYTES],
    receipt_identity: CompilerExecutionAttestationReceiptIdentityV1,
    receipt: CompilerExecutionAttestationReceiptV1,
    identity: CompilerExecutionReceiptPublicationIdentityV1,
    canonical_bytes: [u8; COMPILER_EXECUTION_RECEIPT_PUBLICATION_BYTES_V1],
}

impl CompilerExecutionReceiptPublicationV1 {
    /// Constructs the canonical sidecar for one exact issued journal record and occurrence.
    pub fn new(
        issuer_journal_identity: [u8; SHA256_BYTES],
        compiler_occurrence_identity: [u8; SHA256_BYTES],
        receipt: CompilerExecutionAttestationReceiptV1,
    ) -> Result<Self, CompilerExecutionReceiptPublicationErrorV1> {
        require_identity(issuer_journal_identity, "issuer journal")?;
        require_identity(compiler_occurrence_identity, "compiler occurrence")?;

        let mut bytes = [0_u8; COMPILER_EXECUTION_RECEIPT_PUBLICATION_BYTES_V1];
        let mut offset = encode_header(&mut bytes, PUBLICATION_MAGIC);
        put(
            &mut bytes,
            &mut offset,
            receipt.policy_identity().as_bytes(),
        );
        put(&mut bytes, &mut offset, &issuer_journal_identity);
        put(&mut bytes, &mut offset, &compiler_occurrence_identity);
        put(&mut bytes, &mut offset, receipt.identity().as_bytes());
        put(&mut bytes, &mut offset, receipt.canonical_bytes());
        debug_assert_eq!(offset, PUBLICATION_PREIMAGE_BYTES);
        let identity = CompilerExecutionReceiptPublicationIdentityV1(derive_identity(
            PUBLICATION_IDENTITY_DOMAIN,
            &bytes[..offset],
        ));
        put(&mut bytes, &mut offset, identity.as_bytes());
        debug_assert_eq!(offset, bytes.len());

        Ok(Self {
            policy_identity: receipt.policy_identity(),
            issuer_journal_identity,
            compiler_occurrence_identity,
            receipt_identity: receipt.identity(),
            receipt,
            identity,
            canonical_bytes: bytes,
        })
    }

    /// Strictly decodes one exact canonical sidecar and verifies the nested receipt signature.
    pub fn decode(bytes: &[u8]) -> Result<Self, CompilerExecutionReceiptPublicationErrorV1> {
        require_length(
            bytes,
            COMPILER_EXECUTION_RECEIPT_PUBLICATION_BYTES_V1,
            "receipt publication",
        )?;
        let mut reader = Reader::new(bytes);
        decode_header(
            &mut reader,
            PUBLICATION_MAGIC,
            bytes.len(),
            "receipt publication",
        )?;
        let policy_identity = reader.fixed::<32>()?;
        let issuer_journal_identity = reader.fixed::<32>()?;
        let compiler_occurrence_identity = reader.fixed::<32>()?;
        let receipt_identity = reader.fixed::<32>()?;
        let receipt = CompilerExecutionAttestationReceiptV1::decode(
            reader.take(COMPILER_EXECUTION_ATTESTATION_RECEIPT_BYTES_V1)?,
        )?;
        let declared_identity = reader.fixed::<32>()?;
        if !reader.is_empty() {
            return Err(CompilerExecutionReceiptPublicationErrorV1::TrailingBytes);
        }
        require_identity(issuer_journal_identity, "issuer journal")?;
        require_identity(compiler_occurrence_identity, "compiler occurrence")?;
        if policy_identity != *receipt.policy_identity().as_bytes() {
            return Err(CompilerExecutionReceiptPublicationErrorV1::PolicyMismatch);
        }
        if receipt_identity != *receipt.identity().as_bytes() {
            return Err(CompilerExecutionReceiptPublicationErrorV1::ReceiptMismatch);
        }
        let decoded = Self::new(
            issuer_journal_identity,
            compiler_occurrence_identity,
            receipt,
        )?;
        if declared_identity != *decoded.identity.as_bytes()
            || decoded.canonical_bytes.as_slice() != bytes
        {
            return Err(
                CompilerExecutionReceiptPublicationErrorV1::IdentityMismatch("receipt publication"),
            );
        }
        Ok(decoded)
    }

    pub const fn policy_identity(&self) -> CompilerExecutionIssuerPolicyIdentityV1 {
        self.policy_identity
    }

    pub const fn issuer_journal_identity(&self) -> [u8; SHA256_BYTES] {
        self.issuer_journal_identity
    }

    pub const fn compiler_occurrence_identity(&self) -> [u8; SHA256_BYTES] {
        self.compiler_occurrence_identity
    }

    pub const fn receipt_identity(&self) -> CompilerExecutionAttestationReceiptIdentityV1 {
        self.receipt_identity
    }

    pub const fn receipt(&self) -> &CompilerExecutionAttestationReceiptV1 {
        &self.receipt
    }

    pub const fn identity(&self) -> CompilerExecutionReceiptPublicationIdentityV1 {
        self.identity
    }

    pub const fn canonical_bytes(&self) -> &[u8; COMPILER_EXECUTION_RECEIPT_PUBLICATION_BYTES_V1] {
        &self.canonical_bytes
    }

    /// Checks every issuer-owned binding against independently retained issued state.
    pub fn matches_issued_record(
        &self,
        expected_policy_identity: CompilerExecutionIssuerPolicyIdentityV1,
        expected_issuer_journal_identity: [u8; SHA256_BYTES],
        expected_compiler_occurrence_identity: [u8; SHA256_BYTES],
        expected_receipt_identity: CompilerExecutionAttestationReceiptIdentityV1,
    ) -> Result<(), CompilerExecutionReceiptPublicationErrorV1> {
        if self.policy_identity != expected_policy_identity {
            return Err(CompilerExecutionReceiptPublicationErrorV1::PolicyMismatch);
        }
        if self.issuer_journal_identity != expected_issuer_journal_identity {
            return Err(CompilerExecutionReceiptPublicationErrorV1::IssuerJournalMismatch);
        }
        if self.compiler_occurrence_identity != expected_compiler_occurrence_identity {
            return Err(CompilerExecutionReceiptPublicationErrorV1::OccurrenceMismatch);
        }
        if self.receipt_identity != expected_receipt_identity {
            return Err(CompilerExecutionReceiptPublicationErrorV1::ReceiptMismatch);
        }
        Ok(())
    }

    pub const fn proves_durable_publication(&self) -> bool {
        false
    }

    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }
}

impl fmt::Debug for CompilerExecutionReceiptPublicationV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilerExecutionReceiptPublicationV1")
            .field("policy_identity", &self.policy_identity)
            .field("issuer_journal_identity", &self.issuer_journal_identity)
            .field(
                "compiler_occurrence_identity",
                &self.compiler_occurrence_identity,
            )
            .field("receipt_identity", &self.receipt_identity)
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

/// Inert claim that a protected Worker ledger durably consumed one exact sidecar.
///
/// The protected Worker ledger must be independently reacquired and matched before this claim can
/// produce an issuer ACK authority.
#[derive(Clone, Eq, PartialEq)]
pub struct CompilerExecutionReceiptPublicationAckV1 {
    policy_identity: CompilerExecutionIssuerPolicyIdentityV1,
    issuer_journal_identity: [u8; SHA256_BYTES],
    compiler_occurrence_identity: [u8; SHA256_BYTES],
    receipt_identity: CompilerExecutionAttestationReceiptIdentityV1,
    publication_identity: CompilerExecutionReceiptPublicationIdentityV1,
    worker_ledger_record_identity: [u8; SHA256_BYTES],
    sequence: u64,
    current_rollback_anchor: [u8; SHA256_BYTES],
    identity: CompilerExecutionReceiptPublicationAckIdentityV1,
    canonical_bytes: [u8; COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_BYTES_V1],
}

impl CompilerExecutionReceiptPublicationAckV1 {
    /// Constructs an inert ACK claim after a Worker ledger reports its exact durable record.
    pub fn new(
        publication: &CompilerExecutionReceiptPublicationV1,
        worker_ledger_record_identity: [u8; SHA256_BYTES],
    ) -> Result<Self, CompilerExecutionReceiptPublicationErrorV1> {
        require_identity(worker_ledger_record_identity, "Worker ledger record")?;
        let sequence = publication.receipt.sequence();
        if sequence == 0 {
            return Err(CompilerExecutionReceiptPublicationErrorV1::ZeroValue(
                "receipt sequence",
            ));
        }
        let current_rollback_anchor = publication.receipt.next_rollback_anchor();
        require_identity(current_rollback_anchor, "current rollback anchor")?;

        let mut bytes = [0_u8; COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_BYTES_V1];
        let mut offset = encode_header(&mut bytes, ACK_MAGIC);
        put(
            &mut bytes,
            &mut offset,
            publication.policy_identity.as_bytes(),
        );
        put(
            &mut bytes,
            &mut offset,
            &publication.issuer_journal_identity,
        );
        put(
            &mut bytes,
            &mut offset,
            &publication.compiler_occurrence_identity,
        );
        put(
            &mut bytes,
            &mut offset,
            publication.receipt_identity.as_bytes(),
        );
        put(&mut bytes, &mut offset, publication.identity.as_bytes());
        put(&mut bytes, &mut offset, &worker_ledger_record_identity);
        put(&mut bytes, &mut offset, &sequence.to_le_bytes());
        put(&mut bytes, &mut offset, &current_rollback_anchor);
        debug_assert_eq!(offset, ACK_PREIMAGE_BYTES);
        let identity = CompilerExecutionReceiptPublicationAckIdentityV1(derive_identity(
            ACK_IDENTITY_DOMAIN,
            &bytes[..offset],
        ));
        put(&mut bytes, &mut offset, identity.as_bytes());
        debug_assert_eq!(offset, bytes.len());

        Ok(Self {
            policy_identity: publication.policy_identity,
            issuer_journal_identity: publication.issuer_journal_identity,
            compiler_occurrence_identity: publication.compiler_occurrence_identity,
            receipt_identity: publication.receipt_identity,
            publication_identity: publication.identity,
            worker_ledger_record_identity,
            sequence,
            current_rollback_anchor,
            identity,
            canonical_bytes: bytes,
        })
    }

    /// Strictly decodes one exact canonical ACK claim.
    pub fn decode(bytes: &[u8]) -> Result<Self, CompilerExecutionReceiptPublicationErrorV1> {
        require_length(
            bytes,
            COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_BYTES_V1,
            "receipt publication acknowledgment",
        )?;
        let mut reader = Reader::new(bytes);
        decode_header(
            &mut reader,
            ACK_MAGIC,
            bytes.len(),
            "receipt publication acknowledgment",
        )?;
        let policy_identity =
            CompilerExecutionIssuerPolicyIdentityV1::from_bytes_for_protocol(reader.fixed::<32>()?);
        let issuer_journal_identity = reader.fixed::<32>()?;
        let compiler_occurrence_identity = reader.fixed::<32>()?;
        let receipt_identity =
            CompilerExecutionAttestationReceiptIdentityV1::from_bytes_for_protocol(
                reader.fixed::<32>()?,
            );
        let publication_identity =
            CompilerExecutionReceiptPublicationIdentityV1::from_bytes_for_protocol(
                reader.fixed::<32>()?,
            );
        let worker_ledger_record_identity = reader.fixed::<32>()?;
        let sequence = reader.u64()?;
        let current_rollback_anchor = reader.fixed::<32>()?;
        let declared_identity = reader.fixed::<32>()?;
        if !reader.is_empty() {
            return Err(CompilerExecutionReceiptPublicationErrorV1::TrailingBytes);
        }
        require_identity(*policy_identity.as_bytes(), "issuer policy")?;
        require_identity(issuer_journal_identity, "issuer journal")?;
        require_identity(compiler_occurrence_identity, "compiler occurrence")?;
        require_identity(*receipt_identity.as_bytes(), "receipt")?;
        require_identity(*publication_identity.as_bytes(), "receipt publication")?;
        require_identity(worker_ledger_record_identity, "Worker ledger record")?;
        if sequence == 0 {
            return Err(CompilerExecutionReceiptPublicationErrorV1::ZeroValue(
                "receipt sequence",
            ));
        }
        require_identity(current_rollback_anchor, "current rollback anchor")?;

        let mut canonical = [0_u8; COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_BYTES_V1];
        canonical[..ACK_PREIMAGE_BYTES].copy_from_slice(&bytes[..ACK_PREIMAGE_BYTES]);
        let identity = CompilerExecutionReceiptPublicationAckIdentityV1(derive_identity(
            ACK_IDENTITY_DOMAIN,
            &canonical[..ACK_PREIMAGE_BYTES],
        ));
        canonical[ACK_PREIMAGE_BYTES..].copy_from_slice(identity.as_bytes());
        if declared_identity != *identity.as_bytes() || canonical.as_slice() != bytes {
            return Err(
                CompilerExecutionReceiptPublicationErrorV1::IdentityMismatch(
                    "receipt publication acknowledgment",
                ),
            );
        }

        Ok(Self {
            policy_identity,
            issuer_journal_identity,
            compiler_occurrence_identity,
            receipt_identity,
            publication_identity,
            worker_ledger_record_identity,
            sequence,
            current_rollback_anchor,
            identity,
            canonical_bytes: canonical,
        })
    }

    /// Checks every sidecar-derived field against independently decoded sidecar bytes.
    pub fn matches_publication(
        &self,
        publication: &CompilerExecutionReceiptPublicationV1,
    ) -> Result<(), CompilerExecutionReceiptPublicationErrorV1> {
        if self.policy_identity != publication.policy_identity {
            return Err(CompilerExecutionReceiptPublicationErrorV1::PolicyMismatch);
        }
        if self.issuer_journal_identity != publication.issuer_journal_identity {
            return Err(CompilerExecutionReceiptPublicationErrorV1::IssuerJournalMismatch);
        }
        if self.compiler_occurrence_identity != publication.compiler_occurrence_identity {
            return Err(CompilerExecutionReceiptPublicationErrorV1::OccurrenceMismatch);
        }
        if self.receipt_identity != publication.receipt_identity {
            return Err(CompilerExecutionReceiptPublicationErrorV1::ReceiptMismatch);
        }
        if self.publication_identity != publication.identity {
            return Err(CompilerExecutionReceiptPublicationErrorV1::PublicationMismatch);
        }
        if self.sequence != publication.receipt.sequence() {
            return Err(CompilerExecutionReceiptPublicationErrorV1::SequenceMismatch);
        }
        if self.current_rollback_anchor != publication.receipt.next_rollback_anchor() {
            return Err(CompilerExecutionReceiptPublicationErrorV1::RollbackAnchorMismatch);
        }
        Ok(())
    }

    pub const fn policy_identity(&self) -> CompilerExecutionIssuerPolicyIdentityV1 {
        self.policy_identity
    }

    pub const fn issuer_journal_identity(&self) -> [u8; SHA256_BYTES] {
        self.issuer_journal_identity
    }

    pub const fn compiler_occurrence_identity(&self) -> [u8; SHA256_BYTES] {
        self.compiler_occurrence_identity
    }

    pub const fn receipt_identity(&self) -> CompilerExecutionAttestationReceiptIdentityV1 {
        self.receipt_identity
    }

    pub const fn publication_identity(&self) -> CompilerExecutionReceiptPublicationIdentityV1 {
        self.publication_identity
    }

    pub const fn worker_ledger_record_identity(&self) -> [u8; SHA256_BYTES] {
        self.worker_ledger_record_identity
    }

    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub const fn current_rollback_anchor(&self) -> [u8; SHA256_BYTES] {
        self.current_rollback_anchor
    }

    pub const fn identity(&self) -> CompilerExecutionReceiptPublicationAckIdentityV1 {
        self.identity
    }

    pub const fn canonical_bytes(
        &self,
    ) -> &[u8; COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_BYTES_V1] {
        &self.canonical_bytes
    }

    /// Checks the claim against one independently reacquired durable Worker ledger record.
    pub fn matches_worker_ledger_record(
        &self,
        expected: [u8; SHA256_BYTES],
    ) -> Result<(), CompilerExecutionReceiptPublicationErrorV1> {
        if self.worker_ledger_record_identity == expected {
            Ok(())
        } else {
            Err(CompilerExecutionReceiptPublicationErrorV1::WorkerLedgerMismatch)
        }
    }

    pub const fn proves_durable_publication(&self) -> bool {
        false
    }

    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }
}

impl fmt::Debug for CompilerExecutionReceiptPublicationAckV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilerExecutionReceiptPublicationAckV1")
            .field("policy_identity", &self.policy_identity)
            .field("issuer_journal_identity", &self.issuer_journal_identity)
            .field(
                "compiler_occurrence_identity",
                &self.compiler_occurrence_identity,
            )
            .field("receipt_identity", &self.receipt_identity)
            .field("publication_identity", &self.publication_identity)
            .field(
                "worker_ledger_record_identity",
                &self.worker_ledger_record_identity,
            )
            .field("sequence", &self.sequence)
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

fn encode_header(output: &mut [u8], magic: [u8; 8]) -> usize {
    let total_length = output.len();
    let mut offset = 0;
    put(output, &mut offset, &magic);
    put(output, &mut offset, &VERSION_V1.to_le_bytes());
    put(output, &mut offset, &0_u16.to_le_bytes());
    put(output, &mut offset, &(total_length as u64).to_le_bytes());
    put(output, &mut offset, &0_u32.to_le_bytes());
    debug_assert_eq!(offset, HEADER_BYTES);
    offset
}

fn decode_header(
    reader: &mut Reader<'_>,
    expected_magic: [u8; 8],
    actual_length: usize,
    field: &'static str,
) -> Result<(), CompilerExecutionReceiptPublicationErrorV1> {
    if reader.fixed::<8>()? != expected_magic {
        return Err(CompilerExecutionReceiptPublicationErrorV1::InvalidMagic(
            field,
        ));
    }
    let version = reader.u16()?;
    if version != VERSION_V1 {
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

fn require_identity(
    value: [u8; SHA256_BYTES],
    field: &'static str,
) -> Result<(), CompilerExecutionReceiptPublicationErrorV1> {
    if value == [0; SHA256_BYTES] {
        Err(CompilerExecutionReceiptPublicationErrorV1::ZeroValue(field))
    } else {
        Ok(())
    }
}

fn require_length(
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

fn derive_identity(domain: &[u8], bytes: &[u8]) -> [u8; SHA256_BYTES] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    digest.finalize().into()
}

fn put(output: &mut [u8], offset: &mut usize, value: &[u8]) {
    let end = *offset + value.len();
    output[*offset..end].copy_from_slice(value);
    *offset = end;
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(
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

    fn fixed<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], CompilerExecutionReceiptPublicationErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| CompilerExecutionReceiptPublicationErrorV1::Truncated)
    }

    fn u16(&mut self) -> Result<u16, CompilerExecutionReceiptPublicationErrorV1> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }

    fn u64(&mut self) -> Result<u64, CompilerExecutionReceiptPublicationErrorV1> {
        Ok(u64::from_le_bytes(self.fixed()?))
    }

    fn is_empty(&self) -> bool {
        self.offset == self.bytes.len()
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
