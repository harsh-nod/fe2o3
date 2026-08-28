//! Crash-safe Worker rollback ledger for protected compiler-execution receipts.

use std::error::Error;
use std::fmt;
use std::os::fd::OwnedFd;

use fe2o3_artifact_transaction::{
    InertCompilerExecutionSubjectV1, NoRetainedDurableDirectoryHooksV1,
    RetainedDurableDirectoryErrorV1, RetainedDurableDirectoryHooksV1, RetainedDurableDirectoryV1,
};
use fe2o3_runtime_protocol::{
    COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V1,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_BYTES_V1, CompilerExecutionAttestationErrorV1,
    CompilerExecutionAttestationRequestV1, CompilerExecutionCurrentRecordVerificationErrorV1,
    CompilerExecutionCurrentRecordVerificationV1, CompilerExecutionIssuerPolicyV1,
    CompilerExecutionReceiptCarriageV1, CompilerExecutionReceiptPublicationAckV1,
    CompilerExecutionReceiptPublicationErrorV1, CompilerExecutionReceiptPublicationV1,
};
use sha2::{Digest, Sha256};

const RECORD_MAGIC: [u8; 8] = *b"F2O3CEW1";
const RECORD_VERSION: u16 = 1;
const HEADER_BYTES: usize = 24;
const SHA256_BYTES: usize = 32;
const RECORD_PREIMAGE_BYTES: usize = HEADER_BYTES
    + SHA256_BYTES
    + 8
    + SHA256_BYTES
    + SHA256_BYTES
    + COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V1
    + COMPILER_EXECUTION_RECEIPT_PUBLICATION_BYTES_V1;
/// Exact byte length of one protected Worker compiler-receipt ledger record.
pub const COMPILER_EXECUTION_WORKER_LEDGER_RECORD_BYTES_V1: usize =
    RECORD_PREIMAGE_BYTES + SHA256_BYTES;

const RECORD_IDENTITY_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-WORKER-LEDGER-RECORD/V1\0";
const PROTECTED_POLICY_VERIFICATION_DOMAIN: &[u8] =
    b"FE2O3/PROTECTED-COMPILER-EXECUTION-POLICY-VERIFICATION/V1\0";
const PROTECTED_WORKER_LEDGER_VERIFICATION_DOMAIN: &[u8] =
    b"FE2O3/PROTECTED-COMPILER-EXECUTION-WORKER-LEDGER-VERIFICATION/V1\0";
const CANONICAL_RECORD: &str = "compiler-execution-worker-v1.state";
const REDO_RECORD: &str = "compiler-execution-worker-v1.redo";

#[derive(Clone)]
pub(crate) struct WorkerReceiptRecordV1 {
    sequence: u64,
    prior_rollback_anchor: [u8; SHA256_BYTES],
    current_rollback_anchor: [u8; SHA256_BYTES],
    request: CompilerExecutionAttestationRequestV1,
    publication: CompilerExecutionReceiptPublicationV1,
    identity: [u8; SHA256_BYTES],
    canonical: [u8; COMPILER_EXECUTION_WORKER_LEDGER_RECORD_BYTES_V1],
}

impl fmt::Debug for WorkerReceiptRecordV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WorkerReceiptRecordV1")
            .field("sequence", &self.sequence)
            .field("prior_rollback_anchor", &self.prior_rollback_anchor)
            .field("current_rollback_anchor", &self.current_rollback_anchor)
            .field("request_identity", &self.request.identity())
            .field("publication_identity", &self.publication.identity())
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

impl WorkerReceiptRecordV1 {
    fn new(
        policy: &CompilerExecutionIssuerPolicyV1,
        request: CompilerExecutionAttestationRequestV1,
        publication: CompilerExecutionReceiptPublicationV1,
        expected_sequence: u64,
        expected_prior_anchor: [u8; SHA256_BYTES],
    ) -> Result<Self, ProtectedCompilerExecutionWorkerLedgerErrorV1> {
        if publication.policy_identity() != policy.identity() {
            return Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::PolicyMismatch);
        }
        let receipt = publication.receipt();
        if receipt.sequence() != expected_sequence {
            return Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::SequenceMismatch);
        }
        receipt
            .clone()
            .verify(policy, &request, expected_prior_anchor)?;
        Self::encode(
            policy,
            expected_sequence,
            expected_prior_anchor,
            receipt.next_rollback_anchor(),
            request,
            publication,
        )
    }

    fn encode(
        policy: &CompilerExecutionIssuerPolicyV1,
        sequence: u64,
        prior_rollback_anchor: [u8; SHA256_BYTES],
        current_rollback_anchor: [u8; SHA256_BYTES],
        request: CompilerExecutionAttestationRequestV1,
        publication: CompilerExecutionReceiptPublicationV1,
    ) -> Result<Self, ProtectedCompilerExecutionWorkerLedgerErrorV1> {
        validate_position(sequence, prior_rollback_anchor, current_rollback_anchor)?;
        if publication.policy_identity() != policy.identity()
            || publication.receipt().sequence() != sequence
            || publication.receipt().prior_rollback_anchor() != prior_rollback_anchor
            || publication.receipt().next_rollback_anchor() != current_rollback_anchor
        {
            return Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::RecordMismatch);
        }
        publication
            .receipt()
            .clone()
            .verify(policy, &request, prior_rollback_anchor)?;

        let mut canonical = [0_u8; COMPILER_EXECUTION_WORKER_LEDGER_RECORD_BYTES_V1];
        let mut offset = encode_header(&mut canonical);
        put(&mut canonical, &mut offset, policy.identity().as_bytes());
        put(&mut canonical, &mut offset, &sequence.to_le_bytes());
        put(&mut canonical, &mut offset, &prior_rollback_anchor);
        put(&mut canonical, &mut offset, &current_rollback_anchor);
        put(&mut canonical, &mut offset, request.canonical_bytes());
        put(&mut canonical, &mut offset, publication.canonical_bytes());
        debug_assert_eq!(offset, RECORD_PREIMAGE_BYTES);
        let identity = record_digest(&canonical[..offset]);
        put(&mut canonical, &mut offset, &identity);
        debug_assert_eq!(offset, canonical.len());
        Ok(Self {
            sequence,
            prior_rollback_anchor,
            current_rollback_anchor,
            request,
            publication,
            identity,
            canonical,
        })
    }

    fn decode(
        bytes: &[u8],
        policy: &CompilerExecutionIssuerPolicyV1,
    ) -> Result<Self, ProtectedCompilerExecutionWorkerLedgerErrorV1> {
        if bytes.len() != COMPILER_EXECUTION_WORKER_LEDGER_RECORD_BYTES_V1 {
            return Err(
                ProtectedCompilerExecutionWorkerLedgerErrorV1::InvalidRecord(
                    "Worker receipt ledger record has the wrong byte length",
                ),
            );
        }
        let mut reader = Reader::new(bytes);
        decode_header(&mut reader)?;
        if reader.fixed::<SHA256_BYTES>()? != *policy.identity().as_bytes() {
            return Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::PolicyMismatch);
        }
        let sequence = reader.u64()?;
        let prior_rollback_anchor = reader.fixed::<SHA256_BYTES>()?;
        let current_rollback_anchor = reader.fixed::<SHA256_BYTES>()?;
        let request = CompilerExecutionAttestationRequestV1::decode(
            reader.take(COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V1)?,
        )?;
        let publication = CompilerExecutionReceiptPublicationV1::decode(
            reader.take(COMPILER_EXECUTION_RECEIPT_PUBLICATION_BYTES_V1)?,
        )?;
        let declared_identity = reader.fixed::<SHA256_BYTES>()?;
        if !reader.is_empty() {
            return Err(
                ProtectedCompilerExecutionWorkerLedgerErrorV1::InvalidRecord(
                    "Worker receipt ledger record has trailing bytes",
                ),
            );
        }
        let expected_identity = record_digest(&bytes[..RECORD_PREIMAGE_BYTES]);
        if declared_identity != expected_identity || declared_identity == [0; SHA256_BYTES] {
            return Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::IdentityMismatch);
        }
        let decoded = Self::encode(
            policy,
            sequence,
            prior_rollback_anchor,
            current_rollback_anchor,
            request,
            publication,
        )?;
        if decoded.identity != declared_identity || decoded.canonical.as_slice() != bytes {
            return Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::IdentityMismatch);
        }
        Ok(decoded)
    }

    fn is_legal_successor_of(&self, prior: &Self) -> bool {
        prior.sequence.checked_add(1) == Some(self.sequence)
            && self.prior_rollback_anchor == prior.current_rollback_anchor
    }

    pub(crate) fn acknowledgment(
        &self,
    ) -> Result<
        CompilerExecutionReceiptPublicationAckV1,
        ProtectedCompilerExecutionWorkerLedgerErrorV1,
    > {
        let ack = CompilerExecutionReceiptPublicationAckV1::new(&self.publication, self.identity)?;
        ack.matches_worker_ledger_record(self.identity)?;
        Ok(ack)
    }

    fn matches_input(
        &self,
        request: &CompilerExecutionAttestationRequestV1,
        publication: &CompilerExecutionReceiptPublicationV1,
    ) -> bool {
        &self.request == request && &self.publication == publication
    }

    pub(crate) const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub(crate) const fn prior_rollback_anchor(&self) -> [u8; SHA256_BYTES] {
        self.prior_rollback_anchor
    }

    pub(crate) const fn current_rollback_anchor(&self) -> [u8; SHA256_BYTES] {
        self.current_rollback_anchor
    }

    pub(crate) const fn request(&self) -> &CompilerExecutionAttestationRequestV1 {
        &self.request
    }

    pub(crate) const fn publication(&self) -> &CompilerExecutionReceiptPublicationV1 {
        &self.publication
    }
}

/// Move-only evidence formed only from an exact post-commit Worker record reacquisition.
pub(super) struct ReacquiredWorkerReceiptRecordV1 {
    acknowledgment: CompilerExecutionReceiptPublicationAckV1,
}

impl ReacquiredWorkerReceiptRecordV1 {
    pub(super) fn into_acknowledgment(self) -> CompilerExecutionReceiptPublicationAckV1 {
        self.acknowledgment
    }
}

pub(crate) struct WorkerReceiptLedgerV1 {
    store: RetainedDurableDirectoryV1,
    policy: CompilerExecutionIssuerPolicyV1,
    record: Option<WorkerReceiptRecordV1>,
    poisoned: bool,
}

impl WorkerReceiptLedgerV1 {
    pub(crate) fn recover(
        service_root: OwnedFd,
        policy: &CompilerExecutionIssuerPolicyV1,
    ) -> Result<Self, ProtectedCompilerExecutionWorkerLedgerErrorV1> {
        let mut hooks = NoRetainedDurableDirectoryHooksV1;
        Self::recover_with_hooks(service_root, policy, &mut hooks)
    }

    fn recover_with_hooks(
        service_root: OwnedFd,
        policy: &CompilerExecutionIssuerPolicyV1,
        hooks: &mut impl RetainedDurableDirectoryHooksV1,
    ) -> Result<Self, ProtectedCompilerExecutionWorkerLedgerErrorV1> {
        let store = RetainedDurableDirectoryV1::admit_service_owned(service_root)?;
        let canonical_bytes = store.read_private(
            CANONICAL_RECORD,
            COMPILER_EXECUTION_WORKER_LEDGER_RECORD_BYTES_V1,
        )?;
        let redo_bytes = store.read_private(
            REDO_RECORD,
            COMPILER_EXECUTION_WORKER_LEDGER_RECORD_BYTES_V1,
        )?;
        let record = match (canonical_bytes, redo_bytes) {
            (None, None) => None,
            (canonical, Some(redo_bytes)) => {
                let redo = WorkerReceiptRecordV1::decode(&redo_bytes, policy)?;
                let canonical_record = canonical
                    .as_deref()
                    .map(|bytes| WorkerReceiptRecordV1::decode(bytes, policy))
                    .transpose()?;
                let legal = canonical_record.as_ref().map_or_else(
                    || redo.sequence == 1 && redo.prior_rollback_anchor == [0; SHA256_BYTES],
                    |prior| redo.is_legal_successor_of(prior),
                );
                if !legal {
                    return Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::IllegalSuccessor);
                }
                store.promote_validated_redo(
                    CANONICAL_RECORD,
                    REDO_RECORD,
                    canonical.as_deref(),
                    &redo_bytes,
                    COMPILER_EXECUTION_WORKER_LEDGER_RECORD_BYTES_V1,
                    hooks,
                )?;
                Some(reacquire_exact(&store, policy, &redo)?)
            }
            (Some(canonical_bytes), None) => {
                let record = WorkerReceiptRecordV1::decode(&canonical_bytes, policy)?;
                let established = store.establish_recovered_record_durability(
                    CANONICAL_RECORD,
                    REDO_RECORD,
                    &canonical_bytes,
                    COMPILER_EXECUTION_WORKER_LEDGER_RECORD_BYTES_V1,
                    hooks,
                )?;
                if established != canonical_bytes {
                    return Err(
                        ProtectedCompilerExecutionWorkerLedgerErrorV1::ReacquiredRecordMismatch,
                    );
                }
                Some(reacquire_exact(&store, policy, &record)?)
            }
        };
        Ok(Self {
            store,
            policy: policy.clone(),
            record,
            poisoned: false,
        })
    }

    pub(crate) fn commit_publication(
        &mut self,
        request: CompilerExecutionAttestationRequestV1,
        publication: CompilerExecutionReceiptPublicationV1,
    ) -> Result<ReacquiredWorkerReceiptRecordV1, ProtectedCompilerExecutionWorkerLedgerErrorV1>
    {
        let mut hooks = NoRetainedDurableDirectoryHooksV1;
        self.commit_publication_with_hooks(request, publication, &mut hooks)
    }

    fn commit_publication_with_hooks(
        &mut self,
        request: CompilerExecutionAttestationRequestV1,
        publication: CompilerExecutionReceiptPublicationV1,
        hooks: &mut impl RetainedDurableDirectoryHooksV1,
    ) -> Result<ReacquiredWorkerReceiptRecordV1, ProtectedCompilerExecutionWorkerLedgerErrorV1>
    {
        if self.poisoned {
            return Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::Poisoned);
        }
        if let Some(record) = self.record.as_ref()
            && record.matches_input(&request, &publication)
        {
            let reacquired = self.reacquire(record)?;
            return witness(&reacquired);
        }

        let (expected_sequence, expected_prior_anchor) = match self.record.as_ref() {
            None => (1, [0; SHA256_BYTES]),
            Some(record) => (
                record
                    .sequence
                    .checked_add(1)
                    .ok_or(ProtectedCompilerExecutionWorkerLedgerErrorV1::SequenceExhausted)?,
                record.current_rollback_anchor,
            ),
        };
        let next = WorkerReceiptRecordV1::new(
            &self.policy,
            request,
            publication,
            expected_sequence,
            expected_prior_anchor,
        )?;
        if self
            .record
            .as_ref()
            .is_some_and(|record| !next.is_legal_successor_of(record))
        {
            return Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::IllegalSuccessor);
        }
        if let Err(error) = self.store.commit_record(
            CANONICAL_RECORD,
            REDO_RECORD,
            &next.canonical,
            COMPILER_EXECUTION_WORKER_LEDGER_RECORD_BYTES_V1,
            hooks,
        ) {
            self.poisoned = true;
            return Err(error.into());
        }
        let reacquired = match self.reacquire(&next) {
            Ok(record) => record,
            Err(error) => {
                self.poisoned = true;
                return Err(error);
            }
        };
        let result = witness(&reacquired)?;
        self.record = Some(reacquired);
        Ok(result)
    }

    fn reacquire(
        &self,
        expected: &WorkerReceiptRecordV1,
    ) -> Result<WorkerReceiptRecordV1, ProtectedCompilerExecutionWorkerLedgerErrorV1> {
        reacquire_exact(&self.store, &self.policy, expected)
    }

    pub(crate) const fn last_record(&self) -> Option<&WorkerReceiptRecordV1> {
        self.record.as_ref()
    }

    /// Reacquires the exact current durable record and reconstructs its complete inert carriage.
    pub(crate) fn recover_current_carriage(
        &self,
        expected_subject: &InertCompilerExecutionSubjectV1,
    ) -> Result<CompilerExecutionReceiptCarriageV1, ProtectedCompilerExecutionWorkerLedgerErrorV1>
    {
        let (_, carriage) = self.reacquire_current(expected_subject)?;
        Ok(carriage)
    }

    /// Reacquires and compares the complete exact current carriage under protected policy.
    pub(crate) fn verify_current_carriage(
        &self,
        expected_carriage: &CompilerExecutionReceiptCarriageV1,
    ) -> Result<
        CompilerExecutionCurrentRecordVerificationV1,
        ProtectedCompilerExecutionWorkerLedgerErrorV1,
    > {
        if expected_carriage.policy() != &self.policy {
            return Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::PolicyMismatch);
        }
        let expected_subject = expected_carriage.request().subject();
        let (record, reacquired_carriage) = self.reacquire_current(expected_subject)?;
        if &reacquired_carriage != expected_carriage
            || reacquired_carriage.canonical_bytes() != expected_carriage.canonical_bytes()
        {
            return Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::CarriageMismatch);
        }
        let protected_policy_verification_identity = verification_digest(
            PROTECTED_POLICY_VERIFICATION_DOMAIN,
            &[
                self.policy.canonical_bytes(),
                expected_subject.canonical_bytes(),
                expected_carriage.canonical_bytes(),
                &record.identity,
            ],
        );
        let protected_worker_ledger_verification_identity = verification_digest(
            PROTECTED_WORKER_LEDGER_VERIFICATION_DOMAIN,
            &[
                &record.canonical,
                expected_carriage.canonical_bytes(),
                &protected_policy_verification_identity,
            ],
        );
        CompilerExecutionCurrentRecordVerificationV1::new(
            expected_subject,
            expected_carriage,
            protected_policy_verification_identity,
            protected_worker_ledger_verification_identity,
        )
        .map_err(Into::into)
    }

    fn reacquire_current(
        &self,
        expected_subject: &InertCompilerExecutionSubjectV1,
    ) -> Result<
        (WorkerReceiptRecordV1, CompilerExecutionReceiptCarriageV1),
        ProtectedCompilerExecutionWorkerLedgerErrorV1,
    > {
        if self.poisoned {
            return Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::Poisoned);
        }
        let retained = self
            .record
            .as_ref()
            .ok_or(ProtectedCompilerExecutionWorkerLedgerErrorV1::MissingCanonicalRecord)?;
        let reacquired = self.reacquire(retained)?;
        if reacquired.request().subject() != expected_subject {
            return Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::SubjectMismatch);
        }
        let acknowledgment = reacquired.acknowledgment()?;
        let carriage = CompilerExecutionReceiptCarriageV1::new(
            self.policy.clone(),
            reacquired.request.clone(),
            reacquired.publication.clone(),
            acknowledgment,
        )
        .map_err(ProtectedCompilerExecutionWorkerLedgerErrorV1::from)?;
        Ok((reacquired, carriage))
    }
}

fn reacquire_exact(
    store: &RetainedDurableDirectoryV1,
    policy: &CompilerExecutionIssuerPolicyV1,
    expected: &WorkerReceiptRecordV1,
) -> Result<WorkerReceiptRecordV1, ProtectedCompilerExecutionWorkerLedgerErrorV1> {
    let bytes = store
        .read_private(
            CANONICAL_RECORD,
            COMPILER_EXECUTION_WORKER_LEDGER_RECORD_BYTES_V1,
        )?
        .ok_or(ProtectedCompilerExecutionWorkerLedgerErrorV1::MissingCanonicalRecord)?;
    if bytes.as_slice() != expected.canonical {
        return Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::ReacquiredRecordMismatch);
    }
    let reacquired = WorkerReceiptRecordV1::decode(&bytes, policy)?;
    if reacquired.identity != expected.identity || reacquired.canonical != expected.canonical {
        return Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::ReacquiredRecordMismatch);
    }
    Ok(reacquired)
}

fn witness(
    record: &WorkerReceiptRecordV1,
) -> Result<ReacquiredWorkerReceiptRecordV1, ProtectedCompilerExecutionWorkerLedgerErrorV1> {
    Ok(ReacquiredWorkerReceiptRecordV1 {
        acknowledgment: record.acknowledgment()?,
    })
}

fn validate_position(
    sequence: u64,
    prior_rollback_anchor: [u8; SHA256_BYTES],
    current_rollback_anchor: [u8; SHA256_BYTES],
) -> Result<(), ProtectedCompilerExecutionWorkerLedgerErrorV1> {
    if sequence == 0
        || current_rollback_anchor == [0; SHA256_BYTES]
        || (sequence == 1) != (prior_rollback_anchor == [0; SHA256_BYTES])
    {
        return Err(
            ProtectedCompilerExecutionWorkerLedgerErrorV1::InvalidRecord(
                "Worker receipt ledger has a noncanonical rollback position",
            ),
        );
    }
    Ok(())
}

fn encode_header(output: &mut [u8]) -> usize {
    let mut offset = 0;
    put(output, &mut offset, &RECORD_MAGIC);
    put(output, &mut offset, &RECORD_VERSION.to_le_bytes());
    put(output, &mut offset, &0_u16.to_le_bytes());
    put(
        output,
        &mut offset,
        &(COMPILER_EXECUTION_WORKER_LEDGER_RECORD_BYTES_V1 as u64).to_le_bytes(),
    );
    put(output, &mut offset, &0_u32.to_le_bytes());
    offset
}

fn decode_header(
    reader: &mut Reader<'_>,
) -> Result<(), ProtectedCompilerExecutionWorkerLedgerErrorV1> {
    if reader.fixed::<8>()? != RECORD_MAGIC
        || reader.u16()? != RECORD_VERSION
        || reader.u16()? != 0
        || reader.u64()? != COMPILER_EXECUTION_WORKER_LEDGER_RECORD_BYTES_V1 as u64
        || reader.fixed::<4>()? != [0; 4]
    {
        return Err(
            ProtectedCompilerExecutionWorkerLedgerErrorV1::InvalidRecord(
                "Worker receipt ledger header is not canonical",
            ),
        );
    }
    Ok(())
}

fn put(output: &mut [u8], offset: &mut usize, value: &[u8]) {
    let end = *offset + value.len();
    output[*offset..end].copy_from_slice(value);
    *offset = end;
}

fn record_digest(bytes: &[u8]) -> [u8; SHA256_BYTES] {
    let mut digest = Sha256::new();
    digest.update(RECORD_IDENTITY_DOMAIN);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    digest.finalize().into()
}

fn verification_digest(domain: &[u8], parts: &[&[u8]]) -> [u8; SHA256_BYTES] {
    let mut digest = Sha256::new();
    digest.update(domain);
    for part in parts {
        digest.update((part.len() as u64).to_le_bytes());
        digest.update(part);
    }
    digest.finalize().into()
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
    ) -> Result<&'a [u8], ProtectedCompilerExecutionWorkerLedgerErrorV1> {
        let end = self.offset.checked_add(length).ok_or(
            ProtectedCompilerExecutionWorkerLedgerErrorV1::InvalidRecord(
                "Worker receipt ledger offset overflow",
            ),
        )?;
        let value = self.bytes.get(self.offset..end).ok_or(
            ProtectedCompilerExecutionWorkerLedgerErrorV1::InvalidRecord(
                "Worker receipt ledger is truncated",
            ),
        )?;
        self.offset = end;
        Ok(value)
    }

    fn fixed<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], ProtectedCompilerExecutionWorkerLedgerErrorV1> {
        self.take(N)?.try_into().map_err(|_| {
            ProtectedCompilerExecutionWorkerLedgerErrorV1::InvalidRecord(
                "Worker receipt ledger is truncated",
            )
        })
    }

    fn u16(&mut self) -> Result<u16, ProtectedCompilerExecutionWorkerLedgerErrorV1> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }

    fn u64(&mut self) -> Result<u64, ProtectedCompilerExecutionWorkerLedgerErrorV1> {
        Ok(u64::from_le_bytes(self.fixed()?))
    }

    const fn is_empty(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

/// Protected Worker compiler-receipt ledger failure.
#[derive(Debug)]
pub enum ProtectedCompilerExecutionWorkerLedgerErrorV1 {
    Durable(RetainedDurableDirectoryErrorV1),
    Attestation(CompilerExecutionAttestationErrorV1),
    Publication(CompilerExecutionReceiptPublicationErrorV1),
    InvalidRecord(&'static str),
    PolicyMismatch,
    SequenceMismatch,
    RecordMismatch,
    SubjectMismatch,
    IdentityMismatch,
    IllegalSuccessor,
    SequenceExhausted,
    MissingCanonicalRecord,
    ReacquiredRecordMismatch,
    CarriageMismatch,
    Verification(CompilerExecutionCurrentRecordVerificationErrorV1),
    Poisoned,
}

impl fmt::Display for ProtectedCompilerExecutionWorkerLedgerErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Durable(error) => write!(formatter, "Worker receipt durability failed: {error}"),
            Self::Attestation(error) => {
                write!(formatter, "Worker receipt verification failed: {error}")
            }
            Self::Publication(error) => {
                write!(formatter, "Worker receipt publication failed: {error}")
            }
            Self::InvalidRecord(reason) => {
                write!(formatter, "invalid Worker receipt ledger record: {reason}")
            }
            Self::PolicyMismatch => formatter.write_str("Worker receipt policy mismatch"),
            Self::SequenceMismatch => formatter.write_str("Worker receipt sequence mismatch"),
            Self::RecordMismatch => formatter.write_str("Worker receipt record fields disagree"),
            Self::SubjectMismatch => {
                formatter.write_str("Worker receipt compiler subject mismatch")
            }
            Self::IdentityMismatch => {
                formatter.write_str("Worker receipt record identity mismatch")
            }
            Self::IllegalSuccessor => {
                formatter.write_str("Worker receipt ledger has an illegal successor")
            }
            Self::SequenceExhausted => {
                formatter.write_str("Worker receipt ledger sequence is exhausted")
            }
            Self::MissingCanonicalRecord => {
                formatter.write_str("Worker receipt canonical record is missing")
            }
            Self::ReacquiredRecordMismatch => {
                formatter.write_str("reacquired Worker receipt record changed")
            }
            Self::CarriageMismatch => {
                formatter.write_str("reacquired Worker receipt carriage changed")
            }
            Self::Verification(error) => {
                write!(
                    formatter,
                    "Worker receipt currentness record failed: {error}"
                )
            }
            Self::Poisoned => {
                formatter.write_str("Worker receipt ledger is poisoned and requires restart")
            }
        }
    }
}

impl Error for ProtectedCompilerExecutionWorkerLedgerErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Durable(error) => Some(error),
            Self::Attestation(error) => Some(error),
            Self::Publication(error) => Some(error),
            Self::Verification(error) => Some(error),
            _ => None,
        }
    }
}

impl From<RetainedDurableDirectoryErrorV1> for ProtectedCompilerExecutionWorkerLedgerErrorV1 {
    fn from(error: RetainedDurableDirectoryErrorV1) -> Self {
        Self::Durable(error)
    }
}

impl From<CompilerExecutionAttestationErrorV1> for ProtectedCompilerExecutionWorkerLedgerErrorV1 {
    fn from(error: CompilerExecutionAttestationErrorV1) -> Self {
        Self::Attestation(error)
    }
}

impl From<CompilerExecutionReceiptPublicationErrorV1>
    for ProtectedCompilerExecutionWorkerLedgerErrorV1
{
    fn from(error: CompilerExecutionReceiptPublicationErrorV1) -> Self {
        Self::Publication(error)
    }
}

impl From<CompilerExecutionCurrentRecordVerificationErrorV1>
    for ProtectedCompilerExecutionWorkerLedgerErrorV1
{
    fn from(error: CompilerExecutionCurrentRecordVerificationErrorV1) -> Self {
        Self::Verification(error)
    }
}

#[cfg(test)]
mod tests {
    use std::fs::{self, File};
    use std::io;
    use std::os::unix::fs::PermissionsExt;

    use ed25519_dalek::{SigningKey, VerifyingKey};
    use fe2o3_artifact_transaction::{
        INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1, INERT_COMPILER_EXECUTION_SUBJECT_MAGIC_V1,
        INERT_COMPILER_EXECUTION_SUBJECT_VERSION_V1, InertCompilerExecutionSubjectV1,
        RetainedDurableFaultTimingV1, RetainedDurableRecordBoundaryV1,
    };
    use fe2o3_runtime_protocol::{
        CompilerExecutionAttestationChallengeV1, CompilerExecutionAttestationReceiptV1,
        CompilerExecutionIssuerMeasurementV1,
    };
    use tempfile::TempDir;

    use super::*;

    const SUBJECT_IDENTITY_DOMAIN: &[u8] = b"FE2O3/INERT-COMPILER-EXECUTION-SUBJECT/V1\0";
    const COMPILER_CLOSURE_IDENTITY_DOMAIN: &[u8] = b"fe2o3-compiler-closure-identity-v2\0";
    const RECORD_BOUNDARIES: [RetainedDurableRecordBoundaryV1; 7] = [
        RetainedDurableRecordBoundaryV1::CreateTemp,
        RetainedDurableRecordBoundaryV1::WriteTemp,
        RetainedDurableRecordBoundaryV1::SyncTemp,
        RetainedDurableRecordBoundaryV1::RenameTempToRedo,
        RetainedDurableRecordBoundaryV1::SyncRedoName,
        RetainedDurableRecordBoundaryV1::RenameRedoToCanonical,
        RetainedDurableRecordBoundaryV1::SyncCanonicalName,
    ];
    const FAULT_TIMINGS: [RetainedDurableFaultTimingV1; 2] = [
        RetainedDurableFaultTimingV1::Before,
        RetainedDurableFaultTimingV1::After,
    ];

    struct Fixture {
        directory: TempDir,
        signing_key: SigningKey,
        policy: CompilerExecutionIssuerPolicyV1,
        subject: InertCompilerExecutionSubjectV1,
    }

    impl Fixture {
        fn new() -> Self {
            let directory = TempDir::new().unwrap();
            fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
            let signing_key = SigningKey::from_bytes(&[0x51; 32]);
            let policy = policy(&signing_key.verifying_key(), 7);
            Self {
                directory,
                signing_key,
                policy,
                subject: subject(0x20),
            }
        }

        fn root(&self) -> OwnedFd {
            File::open(self.directory.path()).unwrap().into()
        }

        fn entry(
            &self,
            sequence: u64,
            prior_rollback_anchor: [u8; SHA256_BYTES],
            seed: u8,
        ) -> (
            CompilerExecutionAttestationRequestV1,
            CompilerExecutionReceiptPublicationV1,
        ) {
            let challenge = CompilerExecutionAttestationChallengeV1::new(
                &self.policy,
                &self.subject,
                [seed; SHA256_BYTES],
                sequence,
                prior_rollback_anchor,
            )
            .unwrap();
            let request =
                CompilerExecutionAttestationRequestV1::new(challenge, self.subject.clone())
                    .unwrap();
            let receipt = CompilerExecutionAttestationReceiptV1::issue(
                &self.policy,
                &request,
                &self.signing_key,
            )
            .unwrap();
            let publication = CompilerExecutionReceiptPublicationV1::new(
                [seed.wrapping_add(1); SHA256_BYTES],
                [seed.wrapping_add(2); SHA256_BYTES],
                receipt,
            )
            .unwrap();
            (request, publication)
        }
    }

    struct RecordFault {
        boundary: RetainedDurableRecordBoundaryV1,
        timing: RetainedDurableFaultTimingV1,
        fired: bool,
    }

    impl RecordFault {
        const fn new(
            boundary: RetainedDurableRecordBoundaryV1,
            timing: RetainedDurableFaultTimingV1,
        ) -> Self {
            Self {
                boundary,
                timing,
                fired: false,
            }
        }
    }

    impl RetainedDurableDirectoryHooksV1 for RecordFault {
        fn record(
            &mut self,
            boundary: RetainedDurableRecordBoundaryV1,
            timing: RetainedDurableFaultTimingV1,
        ) -> io::Result<()> {
            if boundary == self.boundary && timing == self.timing {
                self.fired = true;
                Err(io::Error::other("injected Worker receipt ledger crash"))
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn exact_record_round_trips_and_reacquires_before_ack() {
        assert_eq!(COMPILER_EXECUTION_WORKER_LEDGER_RECORD_BYTES_V1, 1690);
        let fixture = Fixture::new();
        let (request, publication) = fixture.entry(1, [0; SHA256_BYTES], 0x71);
        let mut ledger = WorkerReceiptLedgerV1::recover(fixture.root(), &fixture.policy).unwrap();
        assert!(ledger.last_record().is_none());
        let witness = ledger
            .commit_publication(request.clone(), publication.clone())
            .unwrap();
        let ack = witness.into_acknowledgment();
        let record = ledger.last_record().unwrap();
        assert_eq!(record.sequence, 1);
        assert_eq!(record.prior_rollback_anchor, [0; SHA256_BYTES]);
        assert_eq!(
            record.current_rollback_anchor,
            publication.receipt().next_rollback_anchor()
        );
        assert_eq!(record.request, request);
        assert_eq!(record.publication, publication);
        assert_eq!(ack.worker_ledger_record_identity(), record.identity);
        ack.matches_publication(&record.publication).unwrap();
        ack.matches_worker_ledger_record(record.identity).unwrap();
        assert_eq!(
            &record.canonical[RECORD_PREIMAGE_BYTES..],
            record.identity.as_slice()
        );
        let carriage = ledger.recover_current_carriage(&fixture.subject).unwrap();
        assert_eq!(carriage.policy(), &fixture.policy);
        assert_eq!(carriage.request(), &request);
        assert_eq!(carriage.publication(), &publication);
        assert_eq!(carriage.acknowledgment(), &ack);
        let verification = ledger.verify_current_carriage(&carriage).unwrap();
        assert_eq!(
            verification.policy_identity(),
            *fixture.policy.identity().as_bytes()
        );
        assert_eq!(
            verification.subject_identity(),
            *fixture.subject.identity().sha256()
        );
        assert_eq!(
            verification.carriage_identity(),
            *carriage.identity().as_bytes()
        );
        assert_eq!(
            verification.worker_ledger_record_identity(),
            record.identity
        );
        assert_eq!(verification.sequence(), 1);
        assert_eq!(verification.prior_rollback_anchor(), [0; SHA256_BYTES]);
        assert_eq!(
            verification.current_rollback_anchor(),
            record.current_rollback_anchor
        );
        assert_ne!(
            verification.protected_policy_verification_identity(),
            [0; 32]
        );
        assert_ne!(
            verification.protected_worker_ledger_verification_identity(),
            [0; 32]
        );
        assert_ne!(
            verification.protected_policy_verification_identity(),
            verification.protected_worker_ledger_verification_identity()
        );
        assert!(!verification.grants_authority());
        assert_eq!(
            CompilerExecutionCurrentRecordVerificationV1::decode(verification.canonical_bytes())
                .unwrap(),
            verification
        );
        assert!(matches!(
            ledger.recover_current_carriage(&subject(0x21)),
            Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::SubjectMismatch)
        ));

        let canonical = record.canonical;
        drop(ledger);
        let mut recovered =
            WorkerReceiptLedgerV1::recover(fixture.root(), &fixture.policy).unwrap();
        assert_eq!(recovered.last_record().unwrap().canonical, canonical);
        let replay = recovered
            .commit_publication(request.clone(), publication.clone())
            .unwrap()
            .into_acknowledgment();
        assert_eq!(replay, ack);
        let restarted = recovered
            .recover_current_carriage(&fixture.subject)
            .unwrap();
        assert_eq!(restarted.request(), &request);
        assert_eq!(restarted.publication(), &publication);
        assert_eq!(restarted.acknowledgment(), &ack);
    }

    #[test]
    fn exact_replay_performs_no_record_mutation() {
        let fixture = Fixture::new();
        let (request, publication) = fixture.entry(1, [0; SHA256_BYTES], 0x72);
        let mut ledger = WorkerReceiptLedgerV1::recover(fixture.root(), &fixture.policy).unwrap();
        ledger
            .commit_publication(request.clone(), publication.clone())
            .unwrap();
        let mut fault = RecordFault::new(
            RetainedDurableRecordBoundaryV1::CreateTemp,
            RetainedDurableFaultTimingV1::Before,
        );
        ledger
            .commit_publication_with_hooks(request, publication, &mut fault)
            .unwrap();
        assert!(!fault.fired);
    }

    #[test]
    fn rollback_chain_advances_once_and_rejects_replay_or_substitution() {
        let fixture = Fixture::new();
        let (first_request, first_publication) = fixture.entry(1, [0; 32], 0x73);
        let first_anchor = first_publication.receipt().next_rollback_anchor();
        let (second_request, second_publication) = fixture.entry(2, first_anchor, 0x74);
        let mut ledger = WorkerReceiptLedgerV1::recover(fixture.root(), &fixture.policy).unwrap();
        ledger
            .commit_publication(first_request.clone(), first_publication.clone())
            .unwrap();
        let stale_carriage = ledger.recover_current_carriage(&fixture.subject).unwrap();
        ledger
            .commit_publication(second_request.clone(), second_publication.clone())
            .unwrap();
        assert_eq!(ledger.last_record().unwrap().sequence, 2);

        assert!(matches!(
            ledger.commit_publication(first_request, first_publication),
            Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::SequenceMismatch)
        ));
        let substituted = CompilerExecutionReceiptPublicationV1::new(
            [0x91; SHA256_BYTES],
            second_publication.compiler_occurrence_identity(),
            second_publication.receipt().clone(),
        )
        .unwrap();
        assert!(matches!(
            ledger.commit_publication(second_request, substituted),
            Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::SequenceMismatch)
        ));
        assert_eq!(
            ledger.last_record().unwrap().publication,
            second_publication
        );
        assert!(matches!(
            ledger.verify_current_carriage(&stale_carriage),
            Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::CarriageMismatch)
        ));
    }

    #[test]
    fn every_record_byte_mutation_and_wrong_length_rejects() {
        let fixture = Fixture::new();
        let (request, publication) = fixture.entry(1, [0; 32], 0x75);
        let record =
            WorkerReceiptRecordV1::new(&fixture.policy, request, publication, 1, [0; 32]).unwrap();
        for index in 0..record.canonical.len() {
            let mut mutated = record.canonical;
            mutated[index] ^= 0x80;
            assert!(
                WorkerReceiptRecordV1::decode(&mutated, &fixture.policy).is_err(),
                "mutation at byte {index} was accepted"
            );
        }
        assert!(
            WorkerReceiptRecordV1::decode(
                &record.canonical[..record.canonical.len() - 1],
                &fixture.policy,
            )
            .is_err()
        );
        let mut extended = record.canonical.to_vec();
        extended.push(0);
        assert!(WorkerReceiptRecordV1::decode(&extended, &fixture.policy).is_err());
    }

    #[test]
    fn wrong_policy_and_valid_non_successor_redo_fail_closed() {
        let fixture = Fixture::new();
        let (first_request, first_publication) = fixture.entry(1, [0; 32], 0x76);
        let first_anchor = first_publication.receipt().next_rollback_anchor();
        let mut ledger = WorkerReceiptLedgerV1::recover(fixture.root(), &fixture.policy).unwrap();
        ledger
            .commit_publication(first_request, first_publication)
            .unwrap();

        let wrong_key = SigningKey::from_bytes(&[0x52; SHA256_BYTES]);
        let wrong_policy = policy(&wrong_key.verifying_key(), 8);
        assert!(matches!(
            WorkerReceiptLedgerV1::recover(fixture.root(), &wrong_policy),
            Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::PolicyMismatch)
        ));

        let (third_request, third_publication) = fixture.entry(3, first_anchor, 0x77);
        let third = WorkerReceiptRecordV1::new(
            &fixture.policy,
            third_request,
            third_publication,
            3,
            first_anchor,
        )
        .unwrap();
        ledger
            .store
            .stage_record_redo(
                REDO_RECORD,
                &third.canonical,
                COMPILER_EXECUTION_WORKER_LEDGER_RECORD_BYTES_V1,
                &mut NoRetainedDurableDirectoryHooksV1,
            )
            .unwrap();
        drop(ledger);
        assert!(matches!(
            WorkerReceiptLedgerV1::recover(fixture.root(), &fixture.policy),
            Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::IllegalSuccessor)
        ));
    }

    #[test]
    fn every_first_commit_crash_boundary_recovers_empty_or_exact_successor() {
        for boundary in RECORD_BOUNDARIES {
            for timing in FAULT_TIMINGS {
                let fixture = Fixture::new();
                let (request, publication) = fixture.entry(1, [0; 32], 0x78);
                let expected = WorkerReceiptRecordV1::new(
                    &fixture.policy,
                    request.clone(),
                    publication.clone(),
                    1,
                    [0; 32],
                )
                .unwrap();
                let mut ledger =
                    WorkerReceiptLedgerV1::recover(fixture.root(), &fixture.policy).unwrap();
                let mut fault = RecordFault::new(boundary, timing);
                assert!(
                    ledger
                        .commit_publication_with_hooks(request, publication, &mut fault)
                        .is_err(),
                    "first commit unexpectedly succeeded at {boundary:?}/{timing:?}"
                );
                assert!(fault.fired, "fault did not fire at {boundary:?}/{timing:?}");
                assert!(ledger.poisoned);
                drop(ledger);

                let recovered =
                    WorkerReceiptLedgerV1::recover(fixture.root(), &fixture.policy).unwrap();
                assert!(
                    recovered.last_record().is_none()
                        || recovered.last_record().unwrap().canonical == expected.canonical,
                    "first commit recovered a third state at {boundary:?}/{timing:?}"
                );
            }
        }
    }

    #[test]
    fn every_successor_crash_boundary_recovers_only_prior_or_successor() {
        for boundary in RECORD_BOUNDARIES {
            for timing in FAULT_TIMINGS {
                let fixture = Fixture::new();
                let (first_request, first_publication) = fixture.entry(1, [0; 32], 0x79);
                let first_anchor = first_publication.receipt().next_rollback_anchor();
                let (second_request, second_publication) = fixture.entry(2, first_anchor, 0x7a);
                let mut ledger =
                    WorkerReceiptLedgerV1::recover(fixture.root(), &fixture.policy).unwrap();
                ledger
                    .commit_publication(first_request, first_publication)
                    .unwrap();
                let prior = ledger.last_record().unwrap().canonical;
                let successor = WorkerReceiptRecordV1::new(
                    &fixture.policy,
                    second_request.clone(),
                    second_publication.clone(),
                    2,
                    first_anchor,
                )
                .unwrap()
                .canonical;
                let mut fault = RecordFault::new(boundary, timing);
                assert!(
                    ledger
                        .commit_publication_with_hooks(
                            second_request,
                            second_publication,
                            &mut fault,
                        )
                        .is_err(),
                    "successor unexpectedly succeeded at {boundary:?}/{timing:?}"
                );
                assert!(fault.fired, "fault did not fire at {boundary:?}/{timing:?}");
                drop(ledger);

                let recovered =
                    WorkerReceiptLedgerV1::recover(fixture.root(), &fixture.policy).unwrap();
                let recovered = recovered.last_record().unwrap().canonical;
                assert!(
                    recovered == prior || recovered == successor,
                    "successor recovered a third state at {boundary:?}/{timing:?}"
                );
            }
        }
    }

    fn policy(verifying_key: &VerifyingKey, generation: u64) -> CompilerExecutionIssuerPolicyV1 {
        CompilerExecutionIssuerPolicyV1::new(
            generation,
            CompilerExecutionIssuerMeasurementV1::new([0x61; SHA256_BYTES], 12_345).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([0x62; SHA256_BYTES], 67_890).unwrap(),
            verifying_key.to_bytes(),
            SigningKey::from_bytes(&[0xa1; 32])
                .verifying_key()
                .to_bytes(),
        )
        .unwrap()
    }

    fn subject(seed: u8) -> InertCompilerExecutionSubjectV1 {
        let closure_pins = [
            [seed; 32],
            [seed + 1; 32],
            [seed + 2; 32],
            [seed + 3; 32],
            [seed + 4; 32],
            [seed + 5; 32],
        ];
        let mut closure_digest = Sha256::new();
        closure_digest.update(COMPILER_CLOSURE_IDENTITY_DOMAIN);
        closure_digest.update(1_u16.to_le_bytes());
        for pin in closure_pins {
            closure_digest.update(pin);
        }
        let closure_identity: [u8; 32] = closure_digest.finalize().into();
        let mut bytes = [0_u8; INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1];
        let mut offset = 0;
        put(
            &mut bytes,
            &mut offset,
            &INERT_COMPILER_EXECUTION_SUBJECT_MAGIC_V1,
        );
        put(
            &mut bytes,
            &mut offset,
            &INERT_COMPILER_EXECUTION_SUBJECT_VERSION_V1.to_le_bytes(),
        );
        put(&mut bytes, &mut offset, &0_u16.to_le_bytes());
        put(
            &mut bytes,
            &mut offset,
            &(INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1 as u64).to_le_bytes(),
        );
        put(&mut bytes, &mut offset, &0_u32.to_le_bytes());
        put(&mut bytes, &mut offset, &9_u64.to_le_bytes());
        put(&mut bytes, &mut offset, &[seed + 6; 16]);
        put(&mut bytes, &mut offset, &[seed + 7; 32]);
        bytes[offset] = 0;
        offset += 8;
        put(&mut bytes, &mut offset, &[seed + 8; 32]);
        put(&mut bytes, &mut offset, &[seed + 9; 32]);
        for pin in closure_pins {
            put(&mut bytes, &mut offset, &pin);
        }
        put(&mut bytes, &mut offset, &1_u16.to_le_bytes());
        put(&mut bytes, &mut offset, &closure_identity);
        for axis in 0_u8..7 {
            put(&mut bytes, &mut offset, &[seed + 10 + axis; 32]);
            put(
                &mut bytes,
                &mut offset,
                &(1_000_u64 + u64::from(axis)).to_le_bytes(),
            );
        }
        let identity = subject_digest(&bytes[..offset]);
        put(&mut bytes, &mut offset, &identity);
        assert_eq!(offset, bytes.len());
        InertCompilerExecutionSubjectV1::decode(&bytes).unwrap()
    }

    fn subject_digest(bytes: &[u8]) -> [u8; SHA256_BYTES] {
        let mut digest = Sha256::new();
        digest.update(SUBJECT_IDENTITY_DOMAIN);
        digest.update((bytes.len() as u64).to_le_bytes());
        digest.update(bytes);
        digest.finalize().into()
    }
}
