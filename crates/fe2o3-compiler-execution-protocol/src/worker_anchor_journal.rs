#![forbid(unsafe_code)]

use std::{error::Error, fmt};

use crate::worker_anchor_journal_codec as codec;
use fe2o3_external_anchor_protocol::{
    ANCHOR_CHALLENGE_WIRE_LEN_V1, ANCHOR_TRANSITION_RECEIPT_BYTES_V1, AnchorChallengeV1,
    AnchorPositionV1, AnchorProtocolErrorV1, AnchorTransitionReceiptV1, ChallengeKindV1,
    HashChainHeadV1, PinnedAnchorKeyV1,
};

use crate::{
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_TRANSACTION_BYTES_V1,
    CompilerExecutionExternalAnchorTransactionErrorV1,
    CompilerExecutionExternalAnchorTransactionV1,
};

const SHA256_BYTES: usize = 32;
const HEADER_BYTES: usize = 24;
const STAGE_AND_RESERVED_BYTES: usize = 8;
const VERSION_V1: u16 = 1;
const JOURNAL_MAGIC: [u8; 8] = *b"F2O3CAJ1";
const JOURNAL_IDENTITY_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-WORKER-ANCHOR-JOURNAL/V1\0";
const SCHEMA: codec::Schema = codec::Schema {
    magic: JOURNAL_MAGIC,
    version: VERSION_V1,
    domain: JOURNAL_IDENTITY_DOMAIN,
};

const JOURNAL_PREIMAGE_BYTES: usize = HEADER_BYTES
    + STAGE_AND_RESERVED_BYTES
    + COMPILER_EXECUTION_EXTERNAL_ANCHOR_TRANSACTION_BYTES_V1
    + ANCHOR_CHALLENGE_WIRE_LEN_V1
    + ANCHOR_TRANSITION_RECEIPT_BYTES_V1
    + SHA256_BYTES;

/// Exact canonical byte length of one compiler Worker external-anchor journal record.
pub const COMPILER_EXECUTION_WORKER_ANCHOR_JOURNAL_BYTES_V1: usize =
    JOURNAL_PREIMAGE_BYTES + SHA256_BYTES;

/// Canonical phase of one compiler Worker external-anchor transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CompilerExecutionWorkerAnchorJournalStageV1 {
    PreparedAnchor = 1,
    AnchorCommitted = 2,
    Published = 3,
    Aborted = 4,
}

impl CompilerExecutionWorkerAnchorJournalStageV1 {
    pub(crate) fn decode(value: u8) -> Result<Self, CompilerExecutionWorkerAnchorJournalErrorV1> {
        match value {
            1 => Ok(Self::PreparedAnchor),
            2 => Ok(Self::AnchorCommitted),
            3 => Ok(Self::Published),
            4 => Ok(Self::Aborted),
            _ => Err(
                CompilerExecutionWorkerAnchorJournalErrorV1::InvalidEncoding(
                    "journal stage is unknown",
                ),
            ),
        }
    }
}

/// Domain-separated identity of one exact compiler Worker anchor-journal state.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerExecutionWorkerAnchorJournalIdentityV1([u8; SHA256_BYTES]);

impl CompilerExecutionWorkerAnchorJournalIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; SHA256_BYTES] {
        &self.0
    }

    pub fn matches_canonical_bytes(self, bytes: &[u8]) -> bool {
        bytes.len() == COMPILER_EXECUTION_WORKER_ANCHOR_JOURNAL_BYTES_V1
            && bytes[JOURNAL_PREIMAGE_BYTES..] == self.0
            && journal_identity(&bytes[..JOURNAL_PREIMAGE_BYTES]) == self.0
    }
}

impl fmt::Debug for CompilerExecutionWorkerAnchorJournalIdentityV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("CompilerExecutionWorkerAnchorJournalIdentityV1")
            .field(&self.0)
            .finish()
    }
}

/// Fixed, inert local journal record for one externally anchored Worker publication.
///
/// The complete compiler transaction and exact advance challenge are present in every stage.
/// `PreparedAnchor` carries no receipt. `AnchorCommitted` and `Published` carry a receipt
/// observing the proposed external position; `Aborted` carries one observing the prior position.
/// Only `Published` binds a nonzero final Worker-record identity.
#[derive(Clone, Eq, PartialEq)]
pub struct CompilerExecutionWorkerAnchorJournalV1 {
    stage: CompilerExecutionWorkerAnchorJournalStageV1,
    transaction: CompilerExecutionExternalAnchorTransactionV1,
    challenge: AnchorChallengeV1,
    receipt: Option<AnchorTransitionReceiptV1>,
    worker_record_identity: [u8; SHA256_BYTES],
    identity: CompilerExecutionWorkerAnchorJournalIdentityV1,
    canonical_bytes: [u8; COMPILER_EXECUTION_WORKER_ANCHOR_JOURNAL_BYTES_V1],
}

impl CompilerExecutionWorkerAnchorJournalV1 {
    /// Forms the state that must be durable before challenge bytes leave the service boundary.
    pub fn prepared(
        transaction: CompilerExecutionExternalAnchorTransactionV1,
        challenge: AnchorChallengeV1,
    ) -> Result<Self, CompilerExecutionWorkerAnchorJournalErrorV1> {
        Self::encode(
            CompilerExecutionWorkerAnchorJournalStageV1::PreparedAnchor,
            transaction,
            challenge,
            None,
            [0; SHA256_BYTES],
        )
    }

    /// Records one exact signed external observation and selects commit or abort deterministically.
    pub fn record_anchor_receipt(
        self,
        receipt: AnchorTransitionReceiptV1,
    ) -> Result<Self, CompilerExecutionWorkerAnchorJournalErrorV1> {
        if self.stage != CompilerExecutionWorkerAnchorJournalStageV1::PreparedAnchor {
            return Err(CompilerExecutionWorkerAnchorJournalErrorV1::IllegalTransition);
        }
        let stage = match receipt.position() {
            AnchorPositionV1::Prior => CompilerExecutionWorkerAnchorJournalStageV1::Aborted,
            AnchorPositionV1::Proposed => {
                CompilerExecutionWorkerAnchorJournalStageV1::AnchorCommitted
            }
        };
        Self::encode(
            stage,
            self.transaction,
            self.challenge,
            Some(receipt),
            [0; SHA256_BYTES],
        )
    }

    /// Binds the already committed external transition to the final durable Worker record.
    pub fn mark_published(
        self,
        worker_record_identity: [u8; SHA256_BYTES],
    ) -> Result<Self, CompilerExecutionWorkerAnchorJournalErrorV1> {
        if self.stage != CompilerExecutionWorkerAnchorJournalStageV1::AnchorCommitted
            || worker_record_identity == [0; SHA256_BYTES]
        {
            return Err(CompilerExecutionWorkerAnchorJournalErrorV1::IllegalTransition);
        }
        Self::encode(
            CompilerExecutionWorkerAnchorJournalStageV1::Published,
            self.transaction,
            self.challenge,
            self.receipt,
            worker_record_identity,
        )
    }

    /// Strictly decodes and revalidates one complete canonical journal record.
    pub fn decode(bytes: &[u8]) -> Result<Self, CompilerExecutionWorkerAnchorJournalErrorV1> {
        if bytes.len() != COMPILER_EXECUTION_WORKER_ANCHOR_JOURNAL_BYTES_V1 {
            return Err(CompilerExecutionWorkerAnchorJournalErrorV1::InvalidLength {
                expected: COMPILER_EXECUTION_WORKER_ANCHOR_JOURNAL_BYTES_V1,
                actual: bytes.len(),
            });
        }
        let parts = SCHEMA.decode(
            bytes,
            COMPILER_EXECUTION_EXTERNAL_ANCHOR_TRANSACTION_BYTES_V1,
        )?;
        let transaction = CompilerExecutionExternalAnchorTransactionV1::decode(parts.transaction)?;
        let key = pinned_anchor_key(&transaction)?;
        let receipt = if parts.stage == CompilerExecutionWorkerAnchorJournalStageV1::PreparedAnchor
        {
            if parts.receipt != [0; ANCHOR_TRANSITION_RECEIPT_BYTES_V1] {
                return Err(CompilerExecutionWorkerAnchorJournalErrorV1::StagePayloadMismatch);
            }
            None
        } else {
            Some(AnchorTransitionReceiptV1::decode(parts.receipt, &key)?)
        };
        let decoded = Self::encode(
            parts.stage,
            transaction,
            parts.challenge,
            receipt,
            parts.worker,
        )?;
        if decoded.canonical_bytes.as_slice() != bytes {
            return Err(CompilerExecutionWorkerAnchorJournalErrorV1::IdentityMismatch);
        }
        Ok(decoded)
    }

    fn encode(
        stage: CompilerExecutionWorkerAnchorJournalStageV1,
        transaction: CompilerExecutionExternalAnchorTransactionV1,
        challenge: AnchorChallengeV1,
        receipt: Option<AnchorTransitionReceiptV1>,
        worker_record_identity: [u8; SHA256_BYTES],
    ) -> Result<Self, CompilerExecutionWorkerAnchorJournalErrorV1> {
        validate_challenge(&transaction, &challenge)?;
        validate_stage_payload(
            stage,
            &transaction,
            &challenge,
            receipt.as_ref(),
            worker_record_identity,
        )?;

        let canonical_bytes = SCHEMA.encode::<COMPILER_EXECUTION_WORKER_ANCHOR_JOURNAL_BYTES_V1>(
            stage,
            transaction.canonical_bytes(),
            &challenge,
            receipt.as_ref(),
            worker_record_identity,
        )?;
        let identity = CompilerExecutionWorkerAnchorJournalIdentityV1(journal_identity(
            &canonical_bytes[..JOURNAL_PREIMAGE_BYTES],
        ));
        Ok(Self {
            stage,
            transaction,
            challenge,
            receipt,
            worker_record_identity,
            identity,
            canonical_bytes,
        })
    }

    /// Checks one exact legal durable-journal successor without granting authority.
    pub fn is_legal_successor_of(&self, prior: &Self) -> bool {
        codec::successor(prior.position(), self.position())
    }

    fn position(&self) -> codec::Position<'_> {
        codec::Position {
            stage: self.stage,
            transaction: self.transaction.canonical_bytes(),
            policy: self.transaction.policy().canonical_bytes(),
            sequence: self.transaction.sequence(),
            prior: self.transaction.prior_rollback_anchor(),
            current: self.transaction.current_rollback_anchor(),
            challenge: &self.challenge,
            receipt: self.receipt.as_ref(),
            worker: self.worker_record_identity,
        }
    }

    pub const fn stage(&self) -> CompilerExecutionWorkerAnchorJournalStageV1 {
        self.stage
    }

    pub const fn transaction(&self) -> &CompilerExecutionExternalAnchorTransactionV1 {
        &self.transaction
    }

    pub const fn challenge(&self) -> &AnchorChallengeV1 {
        &self.challenge
    }

    pub const fn receipt(&self) -> Option<&AnchorTransitionReceiptV1> {
        self.receipt.as_ref()
    }

    pub const fn worker_record_identity(&self) -> [u8; SHA256_BYTES] {
        self.worker_record_identity
    }

    pub const fn identity(&self) -> CompilerExecutionWorkerAnchorJournalIdentityV1 {
        self.identity
    }

    pub const fn canonical_bytes(
        &self,
    ) -> &[u8; COMPILER_EXECUTION_WORKER_ANCHOR_JOURNAL_BYTES_V1] {
        &self.canonical_bytes
    }

    pub fn is_genesis_prepared(&self) -> bool {
        self.stage == CompilerExecutionWorkerAnchorJournalStageV1::PreparedAnchor
            && self.transaction.sequence() == 1
            && self.transaction.prior_rollback_anchor() == [0; SHA256_BYTES]
            && self.challenge.expected_sequence() == 1
            && self.challenge.prior_head() == HashChainHeadV1::from_bytes([0; SHA256_BYTES])
    }

    pub const fn grants_authority(&self) -> bool {
        false
    }
}

impl fmt::Debug for CompilerExecutionWorkerAnchorJournalV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilerExecutionWorkerAnchorJournalV1")
            .field("stage", &self.stage)
            .field("transaction_identity", &self.transaction.identity())
            .field("sequence", &self.transaction.sequence())
            .field(
                "receipt_identity",
                &self.receipt.as_ref().map(|value| value.identity()),
            )
            .field("worker_record_identity", &self.worker_record_identity)
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

fn validate_challenge(
    transaction: &CompilerExecutionExternalAnchorTransactionV1,
    challenge: &AnchorChallengeV1,
) -> Result<(), CompilerExecutionWorkerAnchorJournalErrorV1> {
    let key = pinned_anchor_key(transaction)?;
    if challenge.kind() != ChallengeKindV1::Advance
        || challenge.expected_sequence() != transaction.sequence()
        || challenge.transaction() != transaction.external_anchor_digest()
        || challenge.anchor_key_identity() != key.identity()
    {
        return Err(CompilerExecutionWorkerAnchorJournalErrorV1::ChallengeMismatch);
    }
    if transaction.sequence() == 1
        && challenge.prior_head() != HashChainHeadV1::from_bytes([0; SHA256_BYTES])
    {
        return Err(CompilerExecutionWorkerAnchorJournalErrorV1::ChallengeMismatch);
    }
    Ok(())
}

fn validate_stage_payload(
    stage: CompilerExecutionWorkerAnchorJournalStageV1,
    transaction: &CompilerExecutionExternalAnchorTransactionV1,
    challenge: &AnchorChallengeV1,
    receipt: Option<&AnchorTransitionReceiptV1>,
    worker_record_identity: [u8; SHA256_BYTES],
) -> Result<(), CompilerExecutionWorkerAnchorJournalErrorV1> {
    codec::validate_payload(
        stage,
        challenge,
        receipt,
        worker_record_identity,
        &pinned_anchor_key(transaction)?,
    )
}

fn pinned_anchor_key(
    transaction: &CompilerExecutionExternalAnchorTransactionV1,
) -> Result<PinnedAnchorKeyV1, CompilerExecutionWorkerAnchorJournalErrorV1> {
    PinnedAnchorKeyV1::from_bytes(*transaction.policy().external_anchor_verifying_key())
        .map_err(Into::into)
}

fn journal_identity(bytes: &[u8]) -> [u8; SHA256_BYTES] {
    SCHEMA.identity(bytes)
}

/// Failure while constructing or decoding one compiler Worker anchor-journal state.
#[derive(Debug)]
#[non_exhaustive]
pub enum CompilerExecutionWorkerAnchorJournalErrorV1 {
    InvalidLength { expected: usize, actual: usize },
    InvalidEncoding(&'static str),
    Transaction(CompilerExecutionExternalAnchorTransactionErrorV1),
    Anchor(AnchorProtocolErrorV1),
    ChallengeMismatch,
    ReceiptMismatch,
    StagePayloadMismatch,
    IllegalTransition,
    IdentityMismatch,
}

impl fmt::Display for CompilerExecutionWorkerAnchorJournalErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLength { expected, actual } => write!(
                formatter,
                "compiler Worker anchor-journal length mismatch: expected {expected}, got {actual}"
            ),
            Self::InvalidEncoding(reason) => {
                write!(
                    formatter,
                    "invalid compiler Worker anchor journal: {reason}"
                )
            }
            Self::Transaction(error) => write!(formatter, "compiler anchor transaction: {error}"),
            Self::Anchor(error) => write!(formatter, "external anchor: {error}"),
            Self::ChallengeMismatch => {
                formatter.write_str("compiler Worker anchor challenge mismatch")
            }
            Self::ReceiptMismatch => formatter.write_str("compiler Worker anchor receipt mismatch"),
            Self::StagePayloadMismatch => {
                formatter.write_str("compiler Worker anchor stage and payload disagree")
            }
            Self::IllegalTransition => {
                formatter.write_str("illegal compiler Worker anchor-journal transition")
            }
            Self::IdentityMismatch => {
                formatter.write_str("compiler Worker anchor-journal identity mismatch")
            }
        }
    }
}

impl Error for CompilerExecutionWorkerAnchorJournalErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Transaction(error) => Some(error),
            Self::Anchor(error) => Some(error),
            _ => None,
        }
    }
}

impl From<CompilerExecutionExternalAnchorTransactionErrorV1>
    for CompilerExecutionWorkerAnchorJournalErrorV1
{
    fn from(error: CompilerExecutionExternalAnchorTransactionErrorV1) -> Self {
        Self::Transaction(error)
    }
}

impl From<AnchorProtocolErrorV1> for CompilerExecutionWorkerAnchorJournalErrorV1 {
    fn from(error: AnchorProtocolErrorV1) -> Self {
        Self::Anchor(error)
    }
}
