//! Canonical evidence that a protected service reacquired one exact current Worker record.

use std::{error::Error, fmt};

use fe2o3_artifact_transaction::InertCompilerExecutionSubjectV1;
use sha2::{Digest, Sha256};

use crate::CompilerExecutionReceiptCarriageV1;

const MAGIC: [u8; 8] = *b"F2O3CEV1";
const VERSION: u16 = 1;
const HEADER_BYTES: usize = 24;
const SHA256_BYTES: usize = 32;
const IDENTITY_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-CURRENT-RECORD-VERIFICATION/V1\0";
const PREIMAGE_BYTES: usize = HEADER_BYTES + 9 * SHA256_BYTES + 8;

/// Exact byte length of one current-record verification result.
pub const COMPILER_EXECUTION_CURRENT_RECORD_VERIFICATION_BYTES_V1: usize =
    PREIMAGE_BYTES + SHA256_BYTES;

/// Domain-separated identity of one exact current-record verification result.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerExecutionCurrentRecordVerificationIdentityV1([u8; SHA256_BYTES]);

impl CompilerExecutionCurrentRecordVerificationIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; SHA256_BYTES] {
        &self.0
    }
}

impl fmt::Debug for CompilerExecutionCurrentRecordVerificationIdentityV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("CompilerExecutionCurrentRecordVerificationIdentityV1")
            .field(&self.0)
            .finish()
    }
}

/// Authority-free record of one protected policy comparison and Worker-ledger reacquisition.
///
/// Decoding proves canonical structure only. A caller must authenticate the service session and
/// compare every field with its exact request before treating the final two identities as
/// protected evidence.
#[derive(Clone, Eq, PartialEq)]
pub struct CompilerExecutionCurrentRecordVerificationV1 {
    fields: FieldsV1,
    identity: CompilerExecutionCurrentRecordVerificationIdentityV1,
    canonical_bytes: [u8; COMPILER_EXECUTION_CURRENT_RECORD_VERIFICATION_BYTES_V1],
}

impl CompilerExecutionCurrentRecordVerificationV1 {
    /// Constructs a descriptive record from one exact internally consistent carriage.
    ///
    /// The final two identities are meaningful only when a protected service derived them after
    /// independent policy comparison and exact durable record reacquisition. Construction alone
    /// grants no authority.
    pub fn new(
        subject: &InertCompilerExecutionSubjectV1,
        carriage: &CompilerExecutionReceiptCarriageV1,
        protected_policy_verification_identity: [u8; SHA256_BYTES],
        protected_worker_ledger_verification_identity: [u8; SHA256_BYTES],
    ) -> Result<Self, CompilerExecutionCurrentRecordVerificationErrorV1> {
        if carriage.request().subject() != subject {
            return Err(CompilerExecutionCurrentRecordVerificationErrorV1::SubjectMismatch);
        }
        Self::encode(FieldsV1 {
            policy_identity: *carriage.policy().identity().as_bytes(),
            subject_identity: *subject.identity().sha256(),
            carriage_identity: *carriage.identity().as_bytes(),
            issuer_journal_identity: carriage.acknowledgment().issuer_journal_identity(),
            worker_ledger_record_identity: carriage
                .acknowledgment()
                .worker_ledger_record_identity(),
            sequence: carriage.acknowledgment().sequence(),
            prior_rollback_anchor: carriage.publication().receipt().prior_rollback_anchor(),
            current_rollback_anchor: carriage.acknowledgment().current_rollback_anchor(),
            protected_policy_verification_identity,
            protected_worker_ledger_verification_identity,
        })
    }

    /// Strictly decodes one complete canonical result.
    pub fn decode(bytes: &[u8]) -> Result<Self, CompilerExecutionCurrentRecordVerificationErrorV1> {
        if bytes.len() != COMPILER_EXECUTION_CURRENT_RECORD_VERIFICATION_BYTES_V1 {
            return Err(
                CompilerExecutionCurrentRecordVerificationErrorV1::InvalidLength {
                    actual: bytes.len(),
                },
            );
        }
        let mut reader = Reader::new(bytes);
        if reader.fixed::<8>()? != MAGIC
            || reader.u16()? != VERSION
            || reader.u16()? != 0
            || reader.u64()? != COMPILER_EXECUTION_CURRENT_RECORD_VERIFICATION_BYTES_V1 as u64
            || reader.fixed::<4>()? != [0; 4]
        {
            return Err(CompilerExecutionCurrentRecordVerificationErrorV1::InvalidHeader);
        }
        let fields = FieldsV1 {
            policy_identity: reader.fixed()?,
            subject_identity: reader.fixed()?,
            carriage_identity: reader.fixed()?,
            issuer_journal_identity: reader.fixed()?,
            worker_ledger_record_identity: reader.fixed()?,
            sequence: reader.u64()?,
            prior_rollback_anchor: reader.fixed()?,
            current_rollback_anchor: reader.fixed()?,
            protected_policy_verification_identity: reader.fixed()?,
            protected_worker_ledger_verification_identity: reader.fixed()?,
        };
        let declared_identity = reader.fixed::<SHA256_BYTES>()?;
        if !reader.is_empty() {
            return Err(CompilerExecutionCurrentRecordVerificationErrorV1::TrailingBytes);
        }
        let decoded = Self::encode(fields)?;
        if declared_identity != decoded.identity.0 || decoded.canonical_bytes.as_slice() != bytes {
            return Err(CompilerExecutionCurrentRecordVerificationErrorV1::IdentityMismatch);
        }
        Ok(decoded)
    }

    fn encode(fields: FieldsV1) -> Result<Self, CompilerExecutionCurrentRecordVerificationErrorV1> {
        fields.validate()?;
        let mut canonical_bytes = [0_u8; COMPILER_EXECUTION_CURRENT_RECORD_VERIFICATION_BYTES_V1];
        let mut offset = 0;
        put(&mut canonical_bytes, &mut offset, &MAGIC);
        put(&mut canonical_bytes, &mut offset, &VERSION.to_le_bytes());
        put(&mut canonical_bytes, &mut offset, &0_u16.to_le_bytes());
        put(
            &mut canonical_bytes,
            &mut offset,
            &(COMPILER_EXECUTION_CURRENT_RECORD_VERIFICATION_BYTES_V1 as u64).to_le_bytes(),
        );
        put(&mut canonical_bytes, &mut offset, &0_u32.to_le_bytes());
        put(&mut canonical_bytes, &mut offset, &fields.policy_identity);
        put(&mut canonical_bytes, &mut offset, &fields.subject_identity);
        put(&mut canonical_bytes, &mut offset, &fields.carriage_identity);
        put(
            &mut canonical_bytes,
            &mut offset,
            &fields.issuer_journal_identity,
        );
        put(
            &mut canonical_bytes,
            &mut offset,
            &fields.worker_ledger_record_identity,
        );
        put(
            &mut canonical_bytes,
            &mut offset,
            &fields.sequence.to_le_bytes(),
        );
        put(
            &mut canonical_bytes,
            &mut offset,
            &fields.prior_rollback_anchor,
        );
        put(
            &mut canonical_bytes,
            &mut offset,
            &fields.current_rollback_anchor,
        );
        put(
            &mut canonical_bytes,
            &mut offset,
            &fields.protected_policy_verification_identity,
        );
        put(
            &mut canonical_bytes,
            &mut offset,
            &fields.protected_worker_ledger_verification_identity,
        );
        debug_assert_eq!(offset, PREIMAGE_BYTES);
        let identity = CompilerExecutionCurrentRecordVerificationIdentityV1(derive_identity(
            &canonical_bytes[..offset],
        ));
        put(&mut canonical_bytes, &mut offset, &identity.0);
        debug_assert_eq!(offset, canonical_bytes.len());
        Ok(Self {
            fields,
            identity,
            canonical_bytes,
        })
    }

    pub const fn policy_identity(&self) -> [u8; SHA256_BYTES] {
        self.fields.policy_identity
    }

    pub const fn subject_identity(&self) -> [u8; SHA256_BYTES] {
        self.fields.subject_identity
    }

    pub const fn carriage_identity(&self) -> [u8; SHA256_BYTES] {
        self.fields.carriage_identity
    }

    pub const fn issuer_journal_identity(&self) -> [u8; SHA256_BYTES] {
        self.fields.issuer_journal_identity
    }

    pub const fn worker_ledger_record_identity(&self) -> [u8; SHA256_BYTES] {
        self.fields.worker_ledger_record_identity
    }

    pub const fn sequence(&self) -> u64 {
        self.fields.sequence
    }

    pub const fn prior_rollback_anchor(&self) -> [u8; SHA256_BYTES] {
        self.fields.prior_rollback_anchor
    }

    pub const fn current_rollback_anchor(&self) -> [u8; SHA256_BYTES] {
        self.fields.current_rollback_anchor
    }

    pub const fn protected_policy_verification_identity(&self) -> [u8; SHA256_BYTES] {
        self.fields.protected_policy_verification_identity
    }

    pub const fn protected_worker_ledger_verification_identity(&self) -> [u8; SHA256_BYTES] {
        self.fields.protected_worker_ledger_verification_identity
    }

    pub const fn identity(&self) -> CompilerExecutionCurrentRecordVerificationIdentityV1 {
        self.identity
    }

    pub const fn canonical_bytes(
        &self,
    ) -> &[u8; COMPILER_EXECUTION_CURRENT_RECORD_VERIFICATION_BYTES_V1] {
        &self.canonical_bytes
    }

    pub const fn grants_authority(&self) -> bool {
        false
    }
}

impl fmt::Debug for CompilerExecutionCurrentRecordVerificationV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilerExecutionCurrentRecordVerificationV1")
            .field("policy_identity", &self.fields.policy_identity)
            .field("subject_identity", &self.fields.subject_identity)
            .field("carriage_identity", &self.fields.carriage_identity)
            .field(
                "worker_ledger_record_identity",
                &self.fields.worker_ledger_record_identity,
            )
            .field("sequence", &self.fields.sequence)
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct FieldsV1 {
    policy_identity: [u8; SHA256_BYTES],
    subject_identity: [u8; SHA256_BYTES],
    carriage_identity: [u8; SHA256_BYTES],
    issuer_journal_identity: [u8; SHA256_BYTES],
    worker_ledger_record_identity: [u8; SHA256_BYTES],
    sequence: u64,
    prior_rollback_anchor: [u8; SHA256_BYTES],
    current_rollback_anchor: [u8; SHA256_BYTES],
    protected_policy_verification_identity: [u8; SHA256_BYTES],
    protected_worker_ledger_verification_identity: [u8; SHA256_BYTES],
}

impl FieldsV1 {
    fn validate(self) -> Result<(), CompilerExecutionCurrentRecordVerificationErrorV1> {
        if [
            self.policy_identity,
            self.subject_identity,
            self.carriage_identity,
            self.issuer_journal_identity,
            self.worker_ledger_record_identity,
            self.current_rollback_anchor,
            self.protected_policy_verification_identity,
            self.protected_worker_ledger_verification_identity,
        ]
        .contains(&[0; SHA256_BYTES])
        {
            return Err(CompilerExecutionCurrentRecordVerificationErrorV1::ZeroIdentity);
        }
        if self.sequence == 0
            || (self.sequence == 1) != (self.prior_rollback_anchor == [0; SHA256_BYTES])
        {
            return Err(CompilerExecutionCurrentRecordVerificationErrorV1::InvalidPosition);
        }
        Ok(())
    }
}

fn derive_identity(bytes: &[u8]) -> [u8; SHA256_BYTES] {
    let mut digest = Sha256::new();
    digest.update(IDENTITY_DOMAIN);
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
    ) -> Result<&'a [u8], CompilerExecutionCurrentRecordVerificationErrorV1> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(CompilerExecutionCurrentRecordVerificationErrorV1::Truncated)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(CompilerExecutionCurrentRecordVerificationErrorV1::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn fixed<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], CompilerExecutionCurrentRecordVerificationErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| CompilerExecutionCurrentRecordVerificationErrorV1::Truncated)
    }

    fn u16(&mut self) -> Result<u16, CompilerExecutionCurrentRecordVerificationErrorV1> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }

    fn u64(&mut self) -> Result<u64, CompilerExecutionCurrentRecordVerificationErrorV1> {
        Ok(u64::from_le_bytes(self.fixed()?))
    }

    const fn is_empty(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompilerExecutionCurrentRecordVerificationErrorV1 {
    InvalidLength { actual: usize },
    InvalidHeader,
    Truncated,
    TrailingBytes,
    SubjectMismatch,
    ZeroIdentity,
    InvalidPosition,
    IdentityMismatch,
}

impl fmt::Display for CompilerExecutionCurrentRecordVerificationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLength { actual } => {
                write!(
                    formatter,
                    "current-record verification has invalid length {actual}"
                )
            }
            Self::InvalidHeader => {
                formatter.write_str("current-record verification header is invalid")
            }
            Self::Truncated => formatter.write_str("current-record verification is truncated"),
            Self::TrailingBytes => {
                formatter.write_str("current-record verification has trailing bytes")
            }
            Self::SubjectMismatch => {
                formatter.write_str("current-record verification subject differs from carriage")
            }
            Self::ZeroIdentity => {
                formatter.write_str("current-record verification contains a zero identity")
            }
            Self::InvalidPosition => {
                formatter.write_str("current-record verification rollback position is invalid")
            }
            Self::IdentityMismatch => {
                formatter.write_str("current-record verification identity mismatch")
            }
        }
    }
}

impl Error for CompilerExecutionCurrentRecordVerificationErrorV1 {}
