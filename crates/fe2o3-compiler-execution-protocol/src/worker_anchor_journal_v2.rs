//! Native source-bearing journal over the same external-anchor state machine.
use crate::{
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_TRANSACTION_BYTES_V2 as T,
    CompilerExecutionExternalAnchorTransactionV2 as Transaction,
    CompilerExecutionNativeJournalErrorV2 as TransactionError,
    CompilerExecutionWorkerAnchorJournalErrorV1 as FrameError,
    CompilerExecutionWorkerAnchorJournalStageV1 as Stage,
    attestation_resources::{CompilerExecutionAttestationStorageV2 as Storage, STRICT_VERIFY_WORK},
    worker_anchor_journal_codec as codec,
};
use fe2o3_external_anchor_protocol::{
    ANCHOR_CHALLENGE_WIRE_LEN_V1 as C, ANCHOR_TRANSITION_RECEIPT_BYTES_V1 as R, AnchorChallengeV1,
    AnchorProtocolErrorV1, AnchorTransitionReceiptV1, ChallengeKindV1, HashChainHeadV1,
    PinnedAnchorKeyV1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{fmt, mem::size_of};

pub const COMPILER_EXECUTION_WORKER_ANCHOR_JOURNAL_BYTES_V2: usize = 32 + T + C + R + 64;
const N: usize = COMPILER_EXECUTION_WORKER_ANCHOR_JOURNAL_BYTES_V2;
const SCHEMA: codec::Schema = codec::Schema {
    magic: *b"F2O3CAJ2",
    version: 2,
    domain: b"FE2O3/COMPILER-EXECUTION-WORKER-ANCHOR-JOURNAL/V2\0",
};
#[derive(Debug)]
pub enum CompilerExecutionWorkerAnchorJournalErrorV2 {
    Resource(Resource),
    Transaction(TransactionError),
    Frame(FrameError),
}
type Error = CompilerExecutionWorkerAnchorJournalErrorV2;
type Result<T> = std::result::Result<T, Error>;
impl From<Resource> for Error {
    fn from(e: Resource) -> Self {
        Self::Resource(e)
    }
}
impl From<TransactionError> for Error {
    fn from(e: TransactionError) -> Self {
        Self::Transaction(e)
    }
}
impl From<FrameError> for Error {
    fn from(e: FrameError) -> Self {
        Self::Frame(e)
    }
}
impl From<AnchorProtocolErrorV1> for Error {
    fn from(e: AnchorProtocolErrorV1) -> Self {
        Self::Frame(FrameError::Anchor(e))
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => fmt::Display::fmt(e, f),
            Self::Transaction(e) => fmt::Display::fmt(e, f),
            Self::Frame(e) => fmt::Display::fmt(e, f),
        }
    }
}
impl std::error::Error for Error {}

/// Inert, move-only native journal. A decoded journal is not proof of durability.
/// All constructors borrow inputs and return the full additional owner charge.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::CompilerExecutionWorkerAnchorJournalV2;
/// fn duplicate(value: CompilerExecutionWorkerAnchorJournalV2) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::{CompilerExecutionWorkerAnchorJournalV1, CompilerExecutionWorkerAnchorJournalV2};
/// fn convert(value: CompilerExecutionWorkerAnchorJournalV1) -> CompilerExecutionWorkerAnchorJournalV2 { value.into() }
/// ```
#[derive(Debug)]
pub struct CompilerExecutionWorkerAnchorJournalV2 {
    transaction: Transaction,
    challenge: AnchorChallengeV1,
    receipt: Option<AnchorTransitionReceiptV1>,
    worker: [u8; 32],
    stage: Stage,
    canonical: [u8; N],
}
type Journal = CompilerExecutionWorkerAnchorJournalV2;
impl Journal {
    pub const RETAINED_STORAGE: usize = size_of::<(Self, Storage)>();
    const WORK: usize = 64 * N + 8 * STRICT_VERIFY_WORK;
    const FRAME: usize = 8 * Self::RETAINED_STORAGE + 8192;

    pub fn prepared(
        t: &Transaction,
        c: &AnchorChallengeV1,
        b: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        Self::build(Stage::PreparedAnchor, t, c, None, [0; 32], b)
    }
    pub fn record_anchor_receipt(
        &self,
        r: &AnchorTransitionReceiptV1,
        b: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        if self.stage != Stage::PreparedAnchor {
            return Err(FrameError::IllegalTransition.into());
        }
        let stage = match r.position() {
            fe2o3_external_anchor_protocol::AnchorPositionV1::Prior => Stage::Aborted,
            fe2o3_external_anchor_protocol::AnchorPositionV1::Proposed => Stage::AnchorCommitted,
        };
        Self::build(
            stage,
            &self.transaction,
            &self.challenge,
            Some(r),
            [0; 32],
            b,
        )
    }
    pub fn mark_published(&self, worker: [u8; 32], b: &mut Budget<'_>) -> Result<(Self, Storage)> {
        if self.stage != Stage::AnchorCommitted || worker == [0; 32] {
            return Err(FrameError::IllegalTransition.into());
        }
        Self::build(
            Stage::Published,
            &self.transaction,
            &self.challenge,
            self.receipt.as_ref(),
            worker,
            b,
        )
    }
    fn build(
        stage: Stage,
        t: &Transaction,
        c: &AnchorChallengeV1,
        r: Option<&AnchorTransitionReceiptV1>,
        worker: [u8; 32],
        b: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        let floor = t.retained_storage()
            + size_of::<AnchorChallengeV1>()
            + r.map_or(0, |_| size_of::<AnchorTransitionReceiptV1>());
        b.with_prepaid_scope(floor, 8, Self::WORK, Self::FRAME, |b| {
            let key = PinnedAnchorKeyV1::from_bytes(*t.policy().external_anchor_verifying_key())?;
            if c.kind() != ChallengeKindV1::Advance
                || c.expected_sequence() != t.sequence()
                || c.transaction() != t.external_anchor_digest(b)?
                || c.anchor_key_identity() != key.identity()
                || (t.sequence() == 1 && c.prior_head() != HashChainHeadV1::from_bytes([0; 32]))
            {
                return Err(FrameError::ChallengeMismatch.into());
            }
            codec::validate_payload(stage, c, r, worker, &key)?;
            let canonical = SCHEMA.encode(stage, t.canonical_bytes(), c, r, worker)?;
            let (transaction, storage) = Transaction::decode(t.canonical_bytes(), b)?;
            b.reserve_storage(storage.additional_storage())?;
            Ok((
                Self {
                    transaction,
                    challenge: c.clone(),
                    receipt: r.cloned(),
                    worker,
                    stage,
                    canonical,
                },
                Storage(Self::RETAINED_STORAGE),
            ))
        })
    }
    pub fn decode(bytes: &[u8], b: &mut Budget<'_>) -> Result<(Self, Storage)> {
        b.with_prepaid_scope(bytes.len().min(N), 8, Self::WORK, Self::FRAME, |b| {
            let parts = SCHEMA.decode(bytes, T)?;
            let (t, storage) = Transaction::decode(parts.transaction, b)?;
            b.reserve_storage(storage.additional_storage())?;
            b.reserve_storage(
                size_of::<AnchorChallengeV1>() + size_of::<AnchorTransitionReceiptV1>(),
            )?;
            let key = PinnedAnchorKeyV1::from_bytes(*t.policy().external_anchor_verifying_key())?;
            let receipt = if parts.stage == Stage::PreparedAnchor {
                if parts.receipt != [0; R] {
                    return Err(FrameError::StagePayloadMismatch.into());
                }
                None
            } else {
                Some(AnchorTransitionReceiptV1::decode(parts.receipt, &key)?)
            };
            let (record, _) = Self::build(
                parts.stage,
                &t,
                &parts.challenge,
                receipt.as_ref(),
                parts.worker,
                b,
            )?;
            if record.canonical.as_slice() != bytes {
                return Err(FrameError::IdentityMismatch.into());
            }
            Ok((record, Storage(Self::RETAINED_STORAGE)))
        })
    }
    pub fn is_legal_successor_of(&self, prior: &Self, b: &mut Budget<'_>) -> Result<bool> {
        b.with_prepaid_scope(2 * Self::RETAINED_STORAGE, 8, 16 * N, 256, |_| {
            Ok(codec::successor(prior.position(), self.position()))
        })
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
            worker: self.worker,
        }
    }
    pub fn is_genesis_prepared(&self) -> bool {
        self.stage == Stage::PreparedAnchor
            && self.transaction.sequence() == 1
            && self.transaction.prior_rollback_anchor() == [0; 32]
            && self.challenge.prior_head() == HashChainHeadV1::from_bytes([0; 32])
    }
    pub const fn stage(&self) -> Stage {
        self.stage
    }
    pub const fn transaction(&self) -> &Transaction {
        &self.transaction
    }
    pub const fn challenge(&self) -> &AnchorChallengeV1 {
        &self.challenge
    }
    pub const fn receipt(&self) -> Option<&AnchorTransitionReceiptV1> {
        self.receipt.as_ref()
    }
    pub const fn worker_record_identity(&self) -> [u8; 32] {
        self.worker
    }
    pub const fn canonical_bytes(&self) -> &[u8; N] {
        &self.canonical
    }
    pub const fn retained_storage(&self) -> usize {
        Self::RETAINED_STORAGE
    }
}
