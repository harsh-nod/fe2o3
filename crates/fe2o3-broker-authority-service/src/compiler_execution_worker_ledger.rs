//! Crash-safe Worker rollback ledger for protected compiler-execution receipts.

use std::error::Error;
use std::fmt;
use std::os::fd::OwnedFd;

use fe2o3_artifact_transaction::{
    InertCompilerExecutionSubjectV1, NoRetainedDurableDirectoryHooksV1,
    RetainedDurableDirectoryErrorV1, RetainedDurableDirectoryHooksV1, RetainedDurableDirectoryV1,
};
use fe2o3_external_anchor_protocol::{
    ANCHOR_TRANSITION_RECEIPT_BYTES_V1, AnchorChallengeV1, AnchorPositionV1, AnchorProtocolErrorV1,
    AnchorTransitionReceiptV1, AnchoredStateV1, CallerNonceV1, ChallengeKindV1, HashChainHeadV1,
    PinnedAnchorKeyV1,
};
use fe2o3_runtime_protocol::{
    COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V1,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_BYTES_V1,
    COMPILER_EXECUTION_WORKER_ANCHOR_JOURNAL_BYTES_V1, CompilerExecutionAttestationErrorV1,
    CompilerExecutionAttestationRequestV1, CompilerExecutionCurrentRecordVerificationErrorV3,
    CompilerExecutionCurrentRecordVerificationV3,
    CompilerExecutionExternalAnchorTransactionErrorV1,
    CompilerExecutionExternalAnchorTransactionV1, CompilerExecutionIssuerPolicyV1,
    CompilerExecutionReceiptCarriageV1, CompilerExecutionReceiptPublicationAckV1,
    CompilerExecutionReceiptPublicationErrorV1, CompilerExecutionReceiptPublicationV1,
    CompilerExecutionWorkerAnchorJournalErrorV1, CompilerExecutionWorkerAnchorJournalStageV1,
    CompilerExecutionWorkerAnchorJournalV1,
};
use sha2::{Digest, Sha256};

const RECORD_MAGIC: [u8; 8] = *b"F2O3CEW2";
const RECORD_VERSION: u16 = 2;
const HEADER_BYTES: usize = 24;
const SHA256_BYTES: usize = 32;
const RECORD_PREIMAGE_BYTES: usize = HEADER_BYTES
    + SHA256_BYTES
    + 8
    + SHA256_BYTES
    + SHA256_BYTES
    + COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V1
    + COMPILER_EXECUTION_RECEIPT_PUBLICATION_BYTES_V1
    + ANCHOR_TRANSITION_RECEIPT_BYTES_V1;
/// Exact byte length of one protected Worker compiler-receipt ledger record.
pub const COMPILER_EXECUTION_WORKER_LEDGER_RECORD_BYTES_V2: usize =
    RECORD_PREIMAGE_BYTES + SHA256_BYTES;

const RECORD_IDENTITY_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-WORKER-LEDGER-RECORD/V2\0";
const PROTECTED_POLICY_VERIFICATION_DOMAIN: &[u8] =
    b"FE2O3/PROTECTED-COMPILER-EXECUTION-POLICY-VERIFICATION/V1\0";
const PROTECTED_WORKER_LEDGER_VERIFICATION_DOMAIN: &[u8] =
    b"FE2O3/PROTECTED-COMPILER-EXECUTION-WORKER-LEDGER-VERIFICATION/V1\0";
const CANONICAL_RECORD: &str = "compiler-execution-worker-v2.state";
const REDO_RECORD: &str = "compiler-execution-worker-v2.redo";
const LEGACY_V1_CANONICAL_RECORD: &str = "compiler-execution-worker-v1.state";
const LEGACY_V1_REDO_RECORD: &str = "compiler-execution-worker-v1.redo";
const LEGACY_V1_RECORD_BYTES: usize = 1690;
const ANCHOR_CANONICAL_RECORD: &str = "compiler-execution-worker-anchor-v1.state";
const ANCHOR_REDO_RECORD: &str = "compiler-execution-worker-anchor-v1.redo";

#[derive(Clone)]
pub(crate) struct WorkerReceiptRecordV2 {
    sequence: u64,
    prior_rollback_anchor: [u8; SHA256_BYTES],
    current_rollback_anchor: [u8; SHA256_BYTES],
    request: CompilerExecutionAttestationRequestV1,
    publication: CompilerExecutionReceiptPublicationV1,
    external_anchor_receipt: AnchorTransitionReceiptV1,
    identity: [u8; SHA256_BYTES],
    canonical: [u8; COMPILER_EXECUTION_WORKER_LEDGER_RECORD_BYTES_V2],
}

impl fmt::Debug for WorkerReceiptRecordV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WorkerReceiptRecordV2")
            .field("sequence", &self.sequence)
            .field("prior_rollback_anchor", &self.prior_rollback_anchor)
            .field("current_rollback_anchor", &self.current_rollback_anchor)
            .field("request_identity", &self.request.identity())
            .field("publication_identity", &self.publication.identity())
            .field(
                "external_anchor_receipt",
                &self.external_anchor_receipt.identity(),
            )
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

impl WorkerReceiptRecordV2 {
    fn new(
        policy: &CompilerExecutionIssuerPolicyV1,
        request: CompilerExecutionAttestationRequestV1,
        publication: CompilerExecutionReceiptPublicationV1,
        external_anchor_receipt: AnchorTransitionReceiptV1,
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
            external_anchor_receipt,
        )
    }

    fn encode(
        policy: &CompilerExecutionIssuerPolicyV1,
        sequence: u64,
        prior_rollback_anchor: [u8; SHA256_BYTES],
        current_rollback_anchor: [u8; SHA256_BYTES],
        request: CompilerExecutionAttestationRequestV1,
        publication: CompilerExecutionReceiptPublicationV1,
        external_anchor_receipt: AnchorTransitionReceiptV1,
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
        validate_external_anchor_receipt(
            policy,
            &request,
            &publication,
            sequence,
            &external_anchor_receipt,
        )?;

        let mut canonical = [0_u8; COMPILER_EXECUTION_WORKER_LEDGER_RECORD_BYTES_V2];
        let mut offset = encode_header(&mut canonical);
        put(&mut canonical, &mut offset, policy.identity().as_bytes());
        put(&mut canonical, &mut offset, &sequence.to_le_bytes());
        put(&mut canonical, &mut offset, &prior_rollback_anchor);
        put(&mut canonical, &mut offset, &current_rollback_anchor);
        put(&mut canonical, &mut offset, request.canonical_bytes());
        put(&mut canonical, &mut offset, publication.canonical_bytes());
        put(
            &mut canonical,
            &mut offset,
            external_anchor_receipt.canonical_bytes(),
        );
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
            external_anchor_receipt,
            identity,
            canonical,
        })
    }

    fn decode(
        bytes: &[u8],
        policy: &CompilerExecutionIssuerPolicyV1,
    ) -> Result<Self, ProtectedCompilerExecutionWorkerLedgerErrorV1> {
        if bytes.len() != COMPILER_EXECUTION_WORKER_LEDGER_RECORD_BYTES_V2 {
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
        let key = pinned_anchor_key(policy)?;
        let external_anchor_receipt = AnchorTransitionReceiptV1::decode(
            reader.take(ANCHOR_TRANSITION_RECEIPT_BYTES_V1)?,
            &key,
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
            external_anchor_receipt,
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
        external_anchor_receipt: &AnchorTransitionReceiptV1,
    ) -> bool {
        &self.request == request
            && &self.publication == publication
            && &self.external_anchor_receipt == external_anchor_receipt
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

    pub(crate) const fn external_anchor_receipt(&self) -> &AnchorTransitionReceiptV1 {
        &self.external_anchor_receipt
    }
}

/// Move-only evidence formed only from an exact post-commit Worker record reacquisition.
pub(super) struct ReacquiredWorkerReceiptRecordV2 {
    acknowledgment: CompilerExecutionReceiptPublicationAckV1,
}

impl ReacquiredWorkerReceiptRecordV2 {
    pub(super) fn into_acknowledgment(self) -> CompilerExecutionReceiptPublicationAckV1 {
        self.acknowledgment
    }
}

pub(crate) struct WorkerReceiptLedgerV1 {
    store: RetainedDurableDirectoryV1,
    policy: CompilerExecutionIssuerPolicyV1,
    record: Option<WorkerReceiptRecordV2>,
    anchor_journal: Option<CompilerExecutionWorkerAnchorJournalV1>,
    poisoned: bool,
}

pub(crate) enum WorkerExternalAnchorPublicationPlanV1 {
    Exchange(AnchorChallengeV1),
    CommitLocally,
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
        if store
            .read_private(LEGACY_V1_CANONICAL_RECORD, LEGACY_V1_RECORD_BYTES)?
            .is_some()
            || store
                .read_private(LEGACY_V1_REDO_RECORD, LEGACY_V1_RECORD_BYTES)?
                .is_some()
        {
            return Err(
                ProtectedCompilerExecutionWorkerLedgerErrorV1::InvalidRecord(
                    "legacy Worker V1 record requires explicit fail-closed migration",
                ),
            );
        }
        let canonical_bytes = store.read_private(
            CANONICAL_RECORD,
            COMPILER_EXECUTION_WORKER_LEDGER_RECORD_BYTES_V2,
        )?;
        let redo_bytes = store.read_private(
            REDO_RECORD,
            COMPILER_EXECUTION_WORKER_LEDGER_RECORD_BYTES_V2,
        )?;
        let record = match (canonical_bytes, redo_bytes) {
            (None, None) => None,
            (canonical, Some(redo_bytes)) => {
                let redo = WorkerReceiptRecordV2::decode(&redo_bytes, policy)?;
                let canonical_record = canonical
                    .as_deref()
                    .map(|bytes| WorkerReceiptRecordV2::decode(bytes, policy))
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
                    COMPILER_EXECUTION_WORKER_LEDGER_RECORD_BYTES_V2,
                    hooks,
                )?;
                Some(reacquire_exact(&store, policy, &redo)?)
            }
            (Some(canonical_bytes), None) => {
                let record = WorkerReceiptRecordV2::decode(&canonical_bytes, policy)?;
                let established = store.establish_recovered_record_durability(
                    CANONICAL_RECORD,
                    REDO_RECORD,
                    &canonical_bytes,
                    COMPILER_EXECUTION_WORKER_LEDGER_RECORD_BYTES_V2,
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
        let anchor_journal = recover_anchor_journal(&store, policy, record.as_ref(), hooks)?;
        Ok(Self {
            store,
            policy: policy.clone(),
            record,
            anchor_journal,
            poisoned: false,
        })
    }

    fn commit_publication_record_with_hooks(
        &mut self,
        request: CompilerExecutionAttestationRequestV1,
        publication: CompilerExecutionReceiptPublicationV1,
        external_anchor_receipt: AnchorTransitionReceiptV1,
        hooks: &mut impl RetainedDurableDirectoryHooksV1,
    ) -> Result<ReacquiredWorkerReceiptRecordV2, ProtectedCompilerExecutionWorkerLedgerErrorV1>
    {
        if self.poisoned {
            return Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::Poisoned);
        }
        if let Some(record) = self.record.as_ref()
            && record.matches_input(&request, &publication, &external_anchor_receipt)
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
        let next = WorkerReceiptRecordV2::new(
            &self.policy,
            request,
            publication,
            external_anchor_receipt,
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
            COMPILER_EXECUTION_WORKER_LEDGER_RECORD_BYTES_V2,
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

    /// Durably prepares or recovers one exact externally anchored publication.
    pub(crate) fn prepare_external_anchor_publication(
        &mut self,
        request: CompilerExecutionAttestationRequestV1,
        publication: CompilerExecutionReceiptPublicationV1,
    ) -> Result<WorkerExternalAnchorPublicationPlanV1, ProtectedCompilerExecutionWorkerLedgerErrorV1>
    {
        let mut hooks = NoRetainedDurableDirectoryHooksV1;
        self.prepare_external_anchor_publication_with_hooks(request, publication, &mut hooks)
    }

    #[cfg(test)]
    /// Durably prepares one exact externally anchored publication before returning its challenge.
    pub(crate) fn prepare_external_anchor(
        &mut self,
        request: CompilerExecutionAttestationRequestV1,
        publication: CompilerExecutionReceiptPublicationV1,
    ) -> Result<AnchorChallengeV1, ProtectedCompilerExecutionWorkerLedgerErrorV1> {
        let mut hooks = NoRetainedDurableDirectoryHooksV1;
        self.prepare_external_anchor_with_hooks(request, publication, &mut hooks)
    }

    #[cfg(test)]
    fn prepare_external_anchor_with_hooks(
        &mut self,
        request: CompilerExecutionAttestationRequestV1,
        publication: CompilerExecutionReceiptPublicationV1,
        hooks: &mut impl RetainedDurableDirectoryHooksV1,
    ) -> Result<AnchorChallengeV1, ProtectedCompilerExecutionWorkerLedgerErrorV1> {
        match self.prepare_external_anchor_publication_with_hooks(request, publication, hooks)? {
            WorkerExternalAnchorPublicationPlanV1::Exchange(challenge) => Ok(challenge),
            WorkerExternalAnchorPublicationPlanV1::CommitLocally => {
                Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::ExternalAnchorJournalActive)
            }
        }
    }

    fn prepare_external_anchor_publication_with_hooks(
        &mut self,
        request: CompilerExecutionAttestationRequestV1,
        publication: CompilerExecutionReceiptPublicationV1,
        hooks: &mut impl RetainedDurableDirectoryHooksV1,
    ) -> Result<WorkerExternalAnchorPublicationPlanV1, ProtectedCompilerExecutionWorkerLedgerErrorV1>
    {
        if self.poisoned {
            return Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::Poisoned);
        }
        let transaction = CompilerExecutionExternalAnchorTransactionV1::new(
            self.policy.clone(),
            request,
            publication,
        )?;
        if let Some(journal) = self.anchor_journal.as_ref()
            && journal.transaction() == &transaction
        {
            let reacquired = self.reacquire_anchor_journal(journal)?;
            return match reacquired.stage() {
                CompilerExecutionWorkerAnchorJournalStageV1::PreparedAnchor => Ok(
                    WorkerExternalAnchorPublicationPlanV1::Exchange(reacquired.challenge().clone()),
                ),
                CompilerExecutionWorkerAnchorJournalStageV1::AnchorCommitted
                | CompilerExecutionWorkerAnchorJournalStageV1::Published => {
                    Ok(WorkerExternalAnchorPublicationPlanV1::CommitLocally)
                }
                CompilerExecutionWorkerAnchorJournalStageV1::Aborted => {
                    Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::ExternalAnchorNotCommitted)
                }
            };
        }

        let stable = match self.anchor_journal.as_ref() {
            None if self.record.is_none() => {
                AnchoredStateV1::from_local_state(0, HashChainHeadV1::from_bytes([0; SHA256_BYTES]))
            }
            Some(journal)
                if journal.stage() == CompilerExecutionWorkerAnchorJournalStageV1::Published =>
            {
                AnchoredStateV1::from_local_state(
                    journal.transaction().sequence(),
                    journal.challenge().proposed_head(),
                )
            }
            Some(journal)
                if journal.stage() == CompilerExecutionWorkerAnchorJournalStageV1::Aborted =>
            {
                AnchoredStateV1::from_local_state(
                    journal.transaction().sequence() - 1,
                    journal.challenge().prior_head(),
                )
            }
            _ => {
                return Err(
                    ProtectedCompilerExecutionWorkerLedgerErrorV1::ExternalAnchorJournalActive,
                );
            }
        };
        let key = pinned_anchor_key(&self.policy)?;
        let nonce = generate_anchor_nonce()?;
        let prepared = stable.prepare(transaction.external_anchor_digest(), &key)?;
        let pending = prepared.begin_advance(CallerNonceV1::from_bytes(nonce), &key)?;
        let next = CompilerExecutionWorkerAnchorJournalV1::prepared(
            transaction,
            pending.challenge().clone(),
        )?;
        self.commit_anchor_journal_with_hooks(next, hooks)?;
        Ok(WorkerExternalAnchorPublicationPlanV1::Exchange(
            self.anchor_journal
                .as_ref()
                .expect("committed anchor journal is retained")
                .challenge()
                .clone(),
        ))
    }

    /// Verifies and durably records one observation for the exact persisted challenge.
    pub(crate) fn record_external_anchor_observation(
        &mut self,
        observation: &[u8],
    ) -> Result<
        CompilerExecutionWorkerAnchorJournalStageV1,
        ProtectedCompilerExecutionWorkerLedgerErrorV1,
    > {
        let mut hooks = NoRetainedDurableDirectoryHooksV1;
        self.record_external_anchor_observation_with_hooks(observation, &mut hooks)
    }

    fn record_external_anchor_observation_with_hooks(
        &mut self,
        observation: &[u8],
        hooks: &mut impl RetainedDurableDirectoryHooksV1,
    ) -> Result<
        CompilerExecutionWorkerAnchorJournalStageV1,
        ProtectedCompilerExecutionWorkerLedgerErrorV1,
    > {
        if self.poisoned {
            return Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::Poisoned);
        }
        let retained = self
            .anchor_journal
            .as_ref()
            .ok_or(ProtectedCompilerExecutionWorkerLedgerErrorV1::MissingAnchorJournal)?
            .clone();
        let key = pinned_anchor_key(&self.policy)?;
        let receipt =
            AnchorTransitionReceiptV1::new(retained.challenge().clone(), observation, &key)?;
        if retained.stage() != CompilerExecutionWorkerAnchorJournalStageV1::PreparedAnchor {
            if retained.receipt() != Some(&receipt) {
                return Err(
                    ProtectedCompilerExecutionWorkerLedgerErrorV1::ExternalAnchorObservationMismatch,
                );
            }
            let reacquired = self.reacquire_anchor_journal(&retained)?;
            return Ok(reacquired.stage());
        }
        let next = retained.record_anchor_receipt(receipt)?;
        let stage = next.stage();
        self.commit_anchor_journal_with_hooks(next, hooks)?;
        Ok(stage)
    }

    /// Commits the exact anchor-approved transaction and only then publishes its Worker ACK.
    pub(crate) fn commit_anchored_publication(
        &mut self,
    ) -> Result<ReacquiredWorkerReceiptRecordV2, ProtectedCompilerExecutionWorkerLedgerErrorV1>
    {
        let mut hooks = NoRetainedDurableDirectoryHooksV1;
        self.commit_anchored_publication_with_hooks(&mut hooks)
    }

    fn commit_anchored_publication_with_hooks(
        &mut self,
        hooks: &mut impl RetainedDurableDirectoryHooksV1,
    ) -> Result<ReacquiredWorkerReceiptRecordV2, ProtectedCompilerExecutionWorkerLedgerErrorV1>
    {
        if self.poisoned {
            return Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::Poisoned);
        }
        let committed = self
            .anchor_journal
            .as_ref()
            .ok_or(ProtectedCompilerExecutionWorkerLedgerErrorV1::MissingAnchorJournal)?
            .clone();
        if committed.stage() == CompilerExecutionWorkerAnchorJournalStageV1::Published {
            validate_anchor_journal_join(&self.policy, self.record.as_ref(), Some(&committed))?;
            let record = self
                .record
                .as_ref()
                .ok_or(ProtectedCompilerExecutionWorkerLedgerErrorV1::MissingCanonicalRecord)?;
            return witness(&self.reacquire(record)?);
        }
        if committed.stage() != CompilerExecutionWorkerAnchorJournalStageV1::AnchorCommitted {
            return Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::ExternalAnchorNotCommitted);
        }
        let external_anchor_receipt = committed
            .receipt()
            .ok_or(ProtectedCompilerExecutionWorkerLedgerErrorV1::ExternalAnchorReceiptMismatch)?
            .clone();
        let transaction = committed.transaction();
        let result = self.commit_publication_record_with_hooks(
            transaction.request().clone(),
            transaction.publication().clone(),
            external_anchor_receipt,
            hooks,
        )?;
        let worker_record_identity = self
            .record
            .as_ref()
            .ok_or(ProtectedCompilerExecutionWorkerLedgerErrorV1::MissingCanonicalRecord)?
            .identity;
        let published = committed.mark_published(worker_record_identity)?;
        self.commit_anchor_journal_with_hooks(published, hooks)?;
        validate_anchor_journal_join(
            &self.policy,
            self.record.as_ref(),
            self.anchor_journal.as_ref(),
        )?;
        Ok(result)
    }

    fn commit_anchor_journal_with_hooks(
        &mut self,
        next: CompilerExecutionWorkerAnchorJournalV1,
        hooks: &mut impl RetainedDurableDirectoryHooksV1,
    ) -> Result<(), ProtectedCompilerExecutionWorkerLedgerErrorV1> {
        let legal = self.anchor_journal.as_ref().map_or_else(
            || self.record.is_none() && next.is_genesis_prepared(),
            |prior| next.is_legal_successor_of(prior),
        );
        if !legal {
            return Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::IllegalAnchorSuccessor);
        }
        validate_anchor_journal_join(&self.policy, self.record.as_ref(), Some(&next))?;
        if let Err(error) = self.store.commit_record(
            ANCHOR_CANONICAL_RECORD,
            ANCHOR_REDO_RECORD,
            next.canonical_bytes(),
            COMPILER_EXECUTION_WORKER_ANCHOR_JOURNAL_BYTES_V1,
            hooks,
        ) {
            self.poisoned = true;
            return Err(error.into());
        }
        let reacquired = match self.reacquire_anchor_journal(&next) {
            Ok(record) => record,
            Err(error) => {
                self.poisoned = true;
                return Err(error);
            }
        };
        self.anchor_journal = Some(reacquired);
        Ok(())
    }

    fn reacquire_anchor_journal(
        &self,
        expected: &CompilerExecutionWorkerAnchorJournalV1,
    ) -> Result<CompilerExecutionWorkerAnchorJournalV1, ProtectedCompilerExecutionWorkerLedgerErrorV1>
    {
        reacquire_anchor_journal_exact(&self.store, expected)
    }

    #[cfg(test)]
    pub(crate) const fn anchor_journal(&self) -> Option<&CompilerExecutionWorkerAnchorJournalV1> {
        self.anchor_journal.as_ref()
    }

    fn reacquire(
        &self,
        expected: &WorkerReceiptRecordV2,
    ) -> Result<WorkerReceiptRecordV2, ProtectedCompilerExecutionWorkerLedgerErrorV1> {
        reacquire_exact(&self.store, &self.policy, expected)
    }

    pub(crate) const fn last_record(&self) -> Option<&WorkerReceiptRecordV2> {
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
        external_anchor_currentness_receipt: AnchorTransitionReceiptV1,
        verification_challenge: [u8; SHA256_BYTES],
    ) -> Result<
        CompilerExecutionCurrentRecordVerificationV3,
        ProtectedCompilerExecutionWorkerLedgerErrorV1,
    > {
        let (record, _) = self.reacquire_exact_current_carriage(expected_carriage)?;
        let expected_subject = expected_carriage.request().subject();
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
        CompilerExecutionCurrentRecordVerificationV3::new(
            expected_carriage,
            record.external_anchor_receipt().clone(),
            external_anchor_currentness_receipt,
            verification_challenge,
            protected_policy_verification_identity,
            protected_worker_ledger_verification_identity,
        )
        .map_err(Into::into)
    }

    /// Reacquires the retained commit and derives one exact client-bound recovery challenge.
    pub(crate) fn external_anchor_currentness_challenge(
        &self,
        expected_carriage: &CompilerExecutionReceiptCarriageV1,
        verification_challenge: [u8; SHA256_BYTES],
    ) -> Result<AnchorChallengeV1, ProtectedCompilerExecutionWorkerLedgerErrorV1> {
        let (record, _) = self.reacquire_exact_current_carriage(expected_carriage)?;
        CompilerExecutionCurrentRecordVerificationV3::external_anchor_currentness_challenge(
            expected_carriage,
            record.external_anchor_receipt(),
            verification_challenge,
        )
        .map_err(Into::into)
    }

    fn reacquire_exact_current_carriage(
        &self,
        expected_carriage: &CompilerExecutionReceiptCarriageV1,
    ) -> Result<
        (WorkerReceiptRecordV2, CompilerExecutionReceiptCarriageV1),
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
        Ok((record, reacquired_carriage))
    }

    fn reacquire_current(
        &self,
        expected_subject: &InertCompilerExecutionSubjectV1,
    ) -> Result<
        (WorkerReceiptRecordV2, CompilerExecutionReceiptCarriageV1),
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

fn recover_anchor_journal(
    store: &RetainedDurableDirectoryV1,
    policy: &CompilerExecutionIssuerPolicyV1,
    current: Option<&WorkerReceiptRecordV2>,
    hooks: &mut impl RetainedDurableDirectoryHooksV1,
) -> Result<
    Option<CompilerExecutionWorkerAnchorJournalV1>,
    ProtectedCompilerExecutionWorkerLedgerErrorV1,
> {
    let canonical_bytes = store.read_private(
        ANCHOR_CANONICAL_RECORD,
        COMPILER_EXECUTION_WORKER_ANCHOR_JOURNAL_BYTES_V1,
    )?;
    let redo_bytes = store.read_private(
        ANCHOR_REDO_RECORD,
        COMPILER_EXECUTION_WORKER_ANCHOR_JOURNAL_BYTES_V1,
    )?;
    let journal = match (canonical_bytes, redo_bytes) {
        (None, None) => None,
        (canonical, Some(redo_bytes)) => {
            let redo = CompilerExecutionWorkerAnchorJournalV1::decode(&redo_bytes)?;
            let canonical_record = canonical
                .as_deref()
                .map(CompilerExecutionWorkerAnchorJournalV1::decode)
                .transpose()?;
            let legal = canonical_record.as_ref().map_or_else(
                || current.is_none() && redo.is_genesis_prepared(),
                |prior| redo.is_legal_successor_of(prior),
            );
            if !legal {
                return Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::IllegalAnchorSuccessor);
            }
            store.promote_validated_redo(
                ANCHOR_CANONICAL_RECORD,
                ANCHOR_REDO_RECORD,
                canonical.as_deref(),
                &redo_bytes,
                COMPILER_EXECUTION_WORKER_ANCHOR_JOURNAL_BYTES_V1,
                hooks,
            )?;
            Some(reacquire_anchor_journal_exact(store, &redo)?)
        }
        (Some(canonical_bytes), None) => {
            let record = CompilerExecutionWorkerAnchorJournalV1::decode(&canonical_bytes)?;
            let established = store.establish_recovered_record_durability(
                ANCHOR_CANONICAL_RECORD,
                ANCHOR_REDO_RECORD,
                &canonical_bytes,
                COMPILER_EXECUTION_WORKER_ANCHOR_JOURNAL_BYTES_V1,
                hooks,
            )?;
            if established != canonical_bytes {
                return Err(
                    ProtectedCompilerExecutionWorkerLedgerErrorV1::ReacquiredAnchorJournalMismatch,
                );
            }
            Some(reacquire_anchor_journal_exact(store, &record)?)
        }
    };
    validate_anchor_journal_join(policy, current, journal.as_ref())?;
    Ok(journal)
}

fn reacquire_anchor_journal_exact(
    store: &RetainedDurableDirectoryV1,
    expected: &CompilerExecutionWorkerAnchorJournalV1,
) -> Result<CompilerExecutionWorkerAnchorJournalV1, ProtectedCompilerExecutionWorkerLedgerErrorV1> {
    let bytes = store
        .read_private(
            ANCHOR_CANONICAL_RECORD,
            COMPILER_EXECUTION_WORKER_ANCHOR_JOURNAL_BYTES_V1,
        )?
        .ok_or(ProtectedCompilerExecutionWorkerLedgerErrorV1::MissingAnchorJournal)?;
    if bytes.as_slice() != expected.canonical_bytes() {
        return Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::ReacquiredAnchorJournalMismatch);
    }
    let reacquired = CompilerExecutionWorkerAnchorJournalV1::decode(&bytes)?;
    if reacquired.identity() != expected.identity()
        || reacquired.canonical_bytes() != expected.canonical_bytes()
    {
        return Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::ReacquiredAnchorJournalMismatch);
    }
    Ok(reacquired)
}

fn validate_anchor_journal_join(
    policy: &CompilerExecutionIssuerPolicyV1,
    current: Option<&WorkerReceiptRecordV2>,
    journal: Option<&CompilerExecutionWorkerAnchorJournalV1>,
) -> Result<(), ProtectedCompilerExecutionWorkerLedgerErrorV1> {
    let Some(journal) = journal else {
        return if current.is_none() {
            Ok(())
        } else {
            Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::MissingAnchorJournal)
        };
    };
    let transaction = journal.transaction();
    if transaction.policy() != policy {
        return Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::PolicyMismatch);
    }
    if journal.stage() == CompilerExecutionWorkerAnchorJournalStageV1::Published {
        let current = current
            .ok_or(ProtectedCompilerExecutionWorkerLedgerErrorV1::AnchorJournalRecordMismatch)?;
        if journal.worker_record_identity() != current.identity
            || !worker_record_matches_committed_journal(current, journal)
        {
            return Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::AnchorJournalRecordMismatch);
        }
        return Ok(());
    }

    if journal.stage() == CompilerExecutionWorkerAnchorJournalStageV1::AnchorCommitted
        && current.is_some_and(|record| worker_record_matches_committed_journal(record, journal))
    {
        return Ok(());
    }

    let pending_matches = current.map_or_else(
        || {
            transaction.sequence() == 1
                && transaction.prior_rollback_anchor() == [0; SHA256_BYTES]
                && journal.challenge().prior_head()
                    == HashChainHeadV1::from_bytes([0; SHA256_BYTES])
        },
        |record| {
            record.sequence.checked_add(1) == Some(transaction.sequence())
                && transaction.prior_rollback_anchor() == record.current_rollback_anchor
                && journal.challenge().prior_head()
                    == record.external_anchor_receipt().challenge().proposed_head()
        },
    );
    if !pending_matches {
        return Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::AnchorJournalRecordMismatch);
    }
    Ok(())
}

fn worker_record_matches_transaction(
    record: &WorkerReceiptRecordV2,
    transaction: &CompilerExecutionExternalAnchorTransactionV1,
) -> bool {
    record.sequence == transaction.sequence()
        && record.prior_rollback_anchor == transaction.prior_rollback_anchor()
        && record.current_rollback_anchor == transaction.current_rollback_anchor()
        && &record.request == transaction.request()
        && &record.publication == transaction.publication()
}

fn worker_record_matches_committed_journal(
    record: &WorkerReceiptRecordV2,
    journal: &CompilerExecutionWorkerAnchorJournalV1,
) -> bool {
    worker_record_matches_transaction(record, journal.transaction())
        && journal.receipt() == Some(record.external_anchor_receipt())
}

fn validate_external_anchor_receipt(
    policy: &CompilerExecutionIssuerPolicyV1,
    request: &CompilerExecutionAttestationRequestV1,
    publication: &CompilerExecutionReceiptPublicationV1,
    sequence: u64,
    receipt: &AnchorTransitionReceiptV1,
) -> Result<(), ProtectedCompilerExecutionWorkerLedgerErrorV1> {
    let transaction = CompilerExecutionExternalAnchorTransactionV1::new(
        policy.clone(),
        request.clone(),
        publication.clone(),
    )?;
    let key = pinned_anchor_key(policy)?;
    let reverified = AnchorTransitionReceiptV1::decode(receipt.canonical_bytes(), &key)?;
    let challenge = receipt.challenge();
    if reverified != *receipt
        || receipt.position() != AnchorPositionV1::Proposed
        || challenge.kind() != ChallengeKindV1::Advance
        || challenge.expected_sequence() != sequence
        || challenge.transaction() != transaction.external_anchor_digest()
        || challenge.anchor_key_identity() != key.identity()
        || ((sequence == 1)
            != (challenge.prior_head() == HashChainHeadV1::from_bytes([0; SHA256_BYTES])))
    {
        return Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::ExternalAnchorReceiptMismatch);
    }
    Ok(())
}

fn pinned_anchor_key(
    policy: &CompilerExecutionIssuerPolicyV1,
) -> Result<PinnedAnchorKeyV1, ProtectedCompilerExecutionWorkerLedgerErrorV1> {
    PinnedAnchorKeyV1::from_bytes(*policy.external_anchor_verifying_key()).map_err(Into::into)
}

fn generate_anchor_nonce()
-> Result<[u8; SHA256_BYTES], ProtectedCompilerExecutionWorkerLedgerErrorV1> {
    const MAX_ATTEMPTS: usize = 4;
    for _ in 0..MAX_ATTEMPTS {
        let mut nonce = [0_u8; SHA256_BYTES];
        let mut filled = 0;
        while filled < nonce.len() {
            let count = rustix::rand::getrandom(
                &mut nonce[filled..],
                rustix::rand::GetRandomFlags::empty(),
            )
            .map_err(|error| {
                ProtectedCompilerExecutionWorkerLedgerErrorV1::Entropy(
                    std::io::Error::from_raw_os_error(error.raw_os_error()),
                )
            })?;
            if count == 0 {
                return Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::Entropy(
                    std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "getrandom returned no compiler anchor nonce bytes",
                    ),
                ));
            }
            filled += count;
        }
        if nonce != [0; SHA256_BYTES] {
            return Ok(nonce);
        }
    }
    Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::Entropy(
        std::io::Error::other("getrandom repeatedly returned a zero compiler anchor nonce"),
    ))
}

fn reacquire_exact(
    store: &RetainedDurableDirectoryV1,
    policy: &CompilerExecutionIssuerPolicyV1,
    expected: &WorkerReceiptRecordV2,
) -> Result<WorkerReceiptRecordV2, ProtectedCompilerExecutionWorkerLedgerErrorV1> {
    let bytes = store
        .read_private(
            CANONICAL_RECORD,
            COMPILER_EXECUTION_WORKER_LEDGER_RECORD_BYTES_V2,
        )?
        .ok_or(ProtectedCompilerExecutionWorkerLedgerErrorV1::MissingCanonicalRecord)?;
    if bytes.as_slice() != expected.canonical {
        return Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::ReacquiredRecordMismatch);
    }
    let reacquired = WorkerReceiptRecordV2::decode(&bytes, policy)?;
    if reacquired.identity != expected.identity || reacquired.canonical != expected.canonical {
        return Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::ReacquiredRecordMismatch);
    }
    Ok(reacquired)
}

fn witness(
    record: &WorkerReceiptRecordV2,
) -> Result<ReacquiredWorkerReceiptRecordV2, ProtectedCompilerExecutionWorkerLedgerErrorV1> {
    Ok(ReacquiredWorkerReceiptRecordV2 {
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
        &(COMPILER_EXECUTION_WORKER_LEDGER_RECORD_BYTES_V2 as u64).to_le_bytes(),
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
        || reader.u64()? != COMPILER_EXECUTION_WORKER_LEDGER_RECORD_BYTES_V2 as u64
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
    AnchorProtocol(AnchorProtocolErrorV1),
    AnchorTransaction(CompilerExecutionExternalAnchorTransactionErrorV1),
    AnchorJournal(CompilerExecutionWorkerAnchorJournalErrorV1),
    Entropy(std::io::Error),
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
    MissingAnchorJournal,
    ReacquiredAnchorJournalMismatch,
    AnchorJournalRecordMismatch,
    IllegalAnchorSuccessor,
    ExternalAnchorJournalActive,
    ExternalAnchorObservationMismatch,
    ExternalAnchorReceiptMismatch,
    ExternalAnchorNotCommitted,
    CarriageMismatch,
    Verification(CompilerExecutionCurrentRecordVerificationErrorV3),
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
            Self::AnchorProtocol(error) => {
                write!(formatter, "Worker external-anchor protocol failed: {error}")
            }
            Self::AnchorTransaction(error) => {
                write!(
                    formatter,
                    "Worker external-anchor transaction failed: {error}"
                )
            }
            Self::AnchorJournal(error) => {
                write!(formatter, "Worker external-anchor journal failed: {error}")
            }
            Self::Entropy(error) => {
                write!(
                    formatter,
                    "Worker external-anchor nonce generation failed: {error}"
                )
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
            Self::MissingAnchorJournal => {
                formatter.write_str("Worker external-anchor journal is missing")
            }
            Self::ReacquiredAnchorJournalMismatch => {
                formatter.write_str("reacquired Worker external-anchor journal changed")
            }
            Self::AnchorJournalRecordMismatch => formatter.write_str(
                "Worker external-anchor journal disagrees with the receipt ledger record",
            ),
            Self::IllegalAnchorSuccessor => {
                formatter.write_str("Worker external-anchor journal has an illegal successor")
            }
            Self::ExternalAnchorJournalActive => {
                formatter.write_str("Worker external-anchor journal has an active transaction")
            }
            Self::ExternalAnchorObservationMismatch => {
                formatter.write_str("Worker external-anchor observation changed across retry")
            }
            Self::ExternalAnchorReceiptMismatch => formatter.write_str(
                "Worker external-anchor receipt does not bind the exact proposed publication",
            ),
            Self::ExternalAnchorNotCommitted => {
                formatter.write_str("Worker external-anchor transaction has not been committed")
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
            Self::AnchorProtocol(error) => Some(error),
            Self::AnchorTransaction(error) => Some(error),
            Self::AnchorJournal(error) => Some(error),
            Self::Entropy(error) => Some(error),
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

impl From<AnchorProtocolErrorV1> for ProtectedCompilerExecutionWorkerLedgerErrorV1 {
    fn from(error: AnchorProtocolErrorV1) -> Self {
        Self::AnchorProtocol(error)
    }
}

impl From<CompilerExecutionExternalAnchorTransactionErrorV1>
    for ProtectedCompilerExecutionWorkerLedgerErrorV1
{
    fn from(error: CompilerExecutionExternalAnchorTransactionErrorV1) -> Self {
        Self::AnchorTransaction(error)
    }
}

impl From<CompilerExecutionWorkerAnchorJournalErrorV1>
    for ProtectedCompilerExecutionWorkerLedgerErrorV1
{
    fn from(error: CompilerExecutionWorkerAnchorJournalErrorV1) -> Self {
        Self::AnchorJournal(error)
    }
}

impl From<CompilerExecutionCurrentRecordVerificationErrorV3>
    for ProtectedCompilerExecutionWorkerLedgerErrorV1
{
    fn from(error: CompilerExecutionCurrentRecordVerificationErrorV3) -> Self {
        Self::Verification(error)
    }
}

#[cfg(test)]
mod tests {
    use std::fs::{self, File};
    use std::io;
    use std::os::unix::fs::PermissionsExt;

    use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
    use fe2o3_artifact_transaction::{
        INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1, INERT_COMPILER_EXECUTION_SUBJECT_MAGIC_V1,
        INERT_COMPILER_EXECUTION_SUBJECT_VERSION_V1, InertCompilerExecutionSubjectV1,
        RetainedDurableFaultTimingV1, RetainedDurableRecordBoundaryV1,
    };
    use fe2o3_external_anchor_protocol::{
        ANCHOR_OBSERVATION_WIRE_LEN_V1, AnchorPositionV1, UnsignedAnchorObservationV1,
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
        anchor_signing_key: SigningKey,
        policy: CompilerExecutionIssuerPolicyV1,
        subject: InertCompilerExecutionSubjectV1,
    }

    impl Fixture {
        fn new() -> Self {
            let directory = TempDir::new().unwrap();
            fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
            let signing_key = SigningKey::from_bytes(&[0x51; 32]);
            let anchor_signing_key = SigningKey::from_bytes(&[0xa1; 32]);
            let policy = policy(
                &signing_key.verifying_key(),
                &anchor_signing_key.verifying_key(),
                7,
            );
            Self {
                directory,
                signing_key,
                anchor_signing_key,
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

        fn anchor_observation(
            &self,
            challenge: &AnchorChallengeV1,
            position: AnchorPositionV1,
        ) -> [u8; ANCHOR_OBSERVATION_WIRE_LEN_V1] {
            let unsigned = UnsignedAnchorObservationV1::from_challenge(challenge, position);
            let signature = self
                .anchor_signing_key
                .sign(&unsigned.signing_bytes())
                .to_bytes();
            unsigned.attach_signature(signature)
        }

        fn external_anchor_receipt(
            &self,
            request: &CompilerExecutionAttestationRequestV1,
            publication: &CompilerExecutionReceiptPublicationV1,
            prior_external_head: [u8; SHA256_BYTES],
            nonce: [u8; SHA256_BYTES],
        ) -> AnchorTransitionReceiptV1 {
            let transaction = CompilerExecutionExternalAnchorTransactionV1::new(
                self.policy.clone(),
                request.clone(),
                publication.clone(),
            )
            .unwrap();
            let key =
                PinnedAnchorKeyV1::from_bytes(self.anchor_signing_key.verifying_key().to_bytes())
                    .unwrap();
            let stable = AnchoredStateV1::from_local_state(
                transaction.sequence() - 1,
                HashChainHeadV1::from_bytes(prior_external_head),
            );
            let prepared = stable
                .prepare(transaction.external_anchor_digest(), &key)
                .unwrap();
            let pending = prepared
                .begin_advance(CallerNonceV1::from_bytes(nonce), &key)
                .unwrap();
            let observation =
                self.anchor_observation(pending.challenge(), AnchorPositionV1::Proposed);
            AnchorTransitionReceiptV1::new(pending.challenge().clone(), &observation, &key).unwrap()
        }

        fn external_anchor_currentness_receipt(
            &self,
            carriage: &CompilerExecutionReceiptCarriageV1,
            commit_receipt: &AnchorTransitionReceiptV1,
            verification_challenge: [u8; SHA256_BYTES],
            position: AnchorPositionV1,
        ) -> AnchorTransitionReceiptV1 {
            let challenge = CompilerExecutionCurrentRecordVerificationV3::external_anchor_currentness_challenge(
                carriage,
                commit_receipt,
                verification_challenge,
            )
            .unwrap();
            let key =
                PinnedAnchorKeyV1::from_bytes(self.anchor_signing_key.verifying_key().to_bytes())
                    .unwrap();
            let observation = self.anchor_observation(&challenge, position);
            AnchorTransitionReceiptV1::new(challenge, &observation, &key).unwrap()
        }

        fn commit_publication(
            &self,
            ledger: &mut WorkerReceiptLedgerV1,
            request: CompilerExecutionAttestationRequestV1,
            publication: CompilerExecutionReceiptPublicationV1,
        ) -> Result<ReacquiredWorkerReceiptRecordV2, ProtectedCompilerExecutionWorkerLedgerErrorV1>
        {
            match ledger.prepare_external_anchor_publication(request, publication)? {
                WorkerExternalAnchorPublicationPlanV1::Exchange(challenge) => {
                    let observation =
                        self.anchor_observation(&challenge, AnchorPositionV1::Proposed);
                    ledger.record_external_anchor_observation(&observation)?;
                }
                WorkerExternalAnchorPublicationPlanV1::CommitLocally => {}
            }
            ledger.commit_anchored_publication()
        }

        fn replay_publication_with_hooks(
            &self,
            ledger: &mut WorkerReceiptLedgerV1,
            request: CompilerExecutionAttestationRequestV1,
            publication: CompilerExecutionReceiptPublicationV1,
            hooks: &mut impl RetainedDurableDirectoryHooksV1,
        ) -> Result<ReacquiredWorkerReceiptRecordV2, ProtectedCompilerExecutionWorkerLedgerErrorV1>
        {
            match ledger.prepare_external_anchor_publication_with_hooks(
                request,
                publication,
                hooks,
            )? {
                WorkerExternalAnchorPublicationPlanV1::Exchange(_) => {
                    panic!("published replay unexpectedly requested another anchor exchange")
                }
                WorkerExternalAnchorPublicationPlanV1::CommitLocally => {
                    ledger.commit_anchored_publication_with_hooks(hooks)
                }
            }
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
        assert_eq!(COMPILER_EXECUTION_WORKER_LEDGER_RECORD_BYTES_V2, 2218);
        let fixture = Fixture::new();
        let (request, publication) = fixture.entry(1, [0; SHA256_BYTES], 0x71);
        let mut ledger = WorkerReceiptLedgerV1::recover(fixture.root(), &fixture.policy).unwrap();
        assert!(ledger.last_record().is_none());
        let witness = fixture
            .commit_publication(&mut ledger, request.clone(), publication.clone())
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
        assert_eq!(
            Some(record.external_anchor_receipt()),
            ledger.anchor_journal().unwrap().receipt()
        );
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
        let verification_challenge = [0xb1; SHA256_BYTES];
        let external_currentness_challenge = ledger
            .external_anchor_currentness_challenge(&carriage, verification_challenge)
            .unwrap();
        assert_eq!(
            external_currentness_challenge.kind(),
            ChallengeKindV1::Recover
        );
        let prior_currentness_receipt = fixture.external_anchor_currentness_receipt(
            &carriage,
            record.external_anchor_receipt(),
            verification_challenge,
            AnchorPositionV1::Prior,
        );
        assert!(matches!(
            ledger.verify_current_carriage(
                &carriage,
                prior_currentness_receipt,
                verification_challenge,
            ),
            Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::Verification(
                CompilerExecutionCurrentRecordVerificationErrorV3::ExternalAnchorCurrentnessReceiptMismatch
            ))
        ));
        let external_currentness_receipt = fixture.external_anchor_currentness_receipt(
            &carriage,
            record.external_anchor_receipt(),
            verification_challenge,
            AnchorPositionV1::Proposed,
        );
        let verification = ledger
            .verify_current_carriage(
                &carriage,
                external_currentness_receipt.clone(),
                verification_challenge,
            )
            .unwrap();
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
        assert_eq!(
            verification.external_anchor_commit_receipt(),
            record.external_anchor_receipt()
        );
        assert_eq!(
            verification.external_rollback_verification_identity(),
            *external_currentness_receipt.identity().as_bytes()
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
            CompilerExecutionCurrentRecordVerificationV3::decode(verification.canonical_bytes())
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
        let replay = fixture
            .commit_publication(&mut recovered, request.clone(), publication.clone())
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
        fixture
            .commit_publication(&mut ledger, request.clone(), publication.clone())
            .unwrap();
        let mut fault = RecordFault::new(
            RetainedDurableRecordBoundaryV1::CreateTemp,
            RetainedDurableFaultTimingV1::Before,
        );
        fixture
            .replay_publication_with_hooks(&mut ledger, request, publication, &mut fault)
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
        fixture
            .commit_publication(
                &mut ledger,
                first_request.clone(),
                first_publication.clone(),
            )
            .unwrap();
        let stale_carriage = ledger.recover_current_carriage(&fixture.subject).unwrap();
        fixture
            .commit_publication(
                &mut ledger,
                second_request.clone(),
                second_publication.clone(),
            )
            .unwrap();
        assert_eq!(ledger.last_record().unwrap().sequence, 2);
        let current = ledger.last_record().unwrap().canonical;

        assert!(
            fixture
                .commit_publication(&mut ledger, first_request, first_publication)
                .is_err()
        );
        let substituted = CompilerExecutionReceiptPublicationV1::new(
            [0x91; SHA256_BYTES],
            second_publication.compiler_occurrence_identity(),
            second_publication.receipt().clone(),
        )
        .unwrap();
        assert!(
            fixture
                .commit_publication(&mut ledger, second_request, substituted)
                .is_err()
        );
        assert_eq!(ledger.last_record().unwrap().canonical, current);
        assert_eq!(
            ledger.last_record().unwrap().publication,
            second_publication
        );
        let current_carriage = ledger.recover_current_carriage(&fixture.subject).unwrap();
        let verification_challenge = [0xb2; SHA256_BYTES];
        assert!(matches!(
            ledger.external_anchor_currentness_challenge(&stale_carriage, verification_challenge,),
            Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::CarriageMismatch)
        ));
        let currentness_receipt = fixture.external_anchor_currentness_receipt(
            &current_carriage,
            ledger.last_record().unwrap().external_anchor_receipt(),
            verification_challenge,
            AnchorPositionV1::Proposed,
        );
        assert!(matches!(
            ledger.verify_current_carriage(
                &stale_carriage,
                currentness_receipt,
                verification_challenge,
            ),
            Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::CarriageMismatch)
        ));
    }

    #[test]
    fn every_record_byte_mutation_and_wrong_length_rejects() {
        let fixture = Fixture::new();
        let (request, publication) = fixture.entry(1, [0; 32], 0x75);
        let external_anchor_receipt =
            fixture.external_anchor_receipt(&request, &publication, [0; 32], [0x75; 32]);
        let record = WorkerReceiptRecordV2::new(
            &fixture.policy,
            request,
            publication,
            external_anchor_receipt,
            1,
            [0; 32],
        )
        .unwrap();
        for index in 0..record.canonical.len() {
            let mut mutated = record.canonical;
            mutated[index] ^= 0x80;
            assert!(
                WorkerReceiptRecordV2::decode(&mutated, &fixture.policy).is_err(),
                "mutation at byte {index} was accepted"
            );
        }
        assert!(
            WorkerReceiptRecordV2::decode(
                &record.canonical[..record.canonical.len() - 1],
                &fixture.policy,
            )
            .is_err()
        );
        let mut extended = record.canonical.to_vec();
        extended.push(0);
        assert!(WorkerReceiptRecordV2::decode(&extended, &fixture.policy).is_err());
    }

    #[test]
    fn legacy_v1_worker_files_require_explicit_migration() {
        for name in [LEGACY_V1_CANONICAL_RECORD, LEGACY_V1_REDO_RECORD] {
            let fixture = Fixture::new();
            let path = fixture.directory.path().join(name);
            fs::write(&path, vec![0_u8; LEGACY_V1_RECORD_BYTES]).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();

            assert!(matches!(
                WorkerReceiptLedgerV1::recover(fixture.root(), &fixture.policy),
                Err(
                    ProtectedCompilerExecutionWorkerLedgerErrorV1::InvalidRecord(
                        "legacy Worker V1 record requires explicit fail-closed migration"
                    )
                )
            ));
        }
    }

    #[test]
    fn v2_worker_record_without_anchor_journal_fails_closed() {
        let fixture = Fixture::new();
        let (request, publication) = fixture.entry(1, [0; 32], 0x75);
        let receipt = fixture.external_anchor_receipt(&request, &publication, [0; 32], [0xd0; 32]);
        let mut ledger = WorkerReceiptLedgerV1::recover(fixture.root(), &fixture.policy).unwrap();
        ledger
            .commit_publication_record_with_hooks(
                request,
                publication,
                receipt,
                &mut NoRetainedDurableDirectoryHooksV1,
            )
            .unwrap();
        drop(ledger);

        assert!(matches!(
            WorkerReceiptLedgerV1::recover(fixture.root(), &fixture.policy),
            Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::MissingAnchorJournal)
        ));
    }

    #[test]
    fn non_genesis_record_rejects_a_zero_external_prior_head() {
        let fixture = Fixture::new();
        let prior_rollback_anchor = [0xd1; SHA256_BYTES];
        let (request, publication) = fixture.entry(2, prior_rollback_anchor, 0x75);
        let receipt = fixture.external_anchor_receipt(&request, &publication, [0; 32], [0xd2; 32]);

        assert!(matches!(
            WorkerReceiptRecordV2::new(
                &fixture.policy,
                request,
                publication,
                receipt,
                2,
                prior_rollback_anchor,
            ),
            Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::ExternalAnchorReceiptMismatch)
        ));
    }

    #[test]
    fn alternate_valid_anchor_receipt_cannot_join_the_durable_journal() {
        let fixture = Fixture::new();
        let (request, publication) = fixture.entry(1, [0; 32], 0x76);
        let mut ledger = WorkerReceiptLedgerV1::recover(fixture.root(), &fixture.policy).unwrap();
        let challenge = ledger
            .prepare_external_anchor(request.clone(), publication.clone())
            .unwrap();
        let observation = fixture.anchor_observation(&challenge, AnchorPositionV1::Proposed);
        ledger
            .record_external_anchor_observation(&observation)
            .unwrap();
        let retained_receipt = ledger.anchor_journal().unwrap().receipt().unwrap().clone();
        let alternate_receipt =
            fixture.external_anchor_receipt(&request, &publication, [0; 32], [0xd1; 32]);
        assert_ne!(alternate_receipt, retained_receipt);

        ledger
            .commit_publication_record_with_hooks(
                request,
                publication,
                alternate_receipt,
                &mut NoRetainedDurableDirectoryHooksV1,
            )
            .unwrap();
        drop(ledger);

        assert!(matches!(
            WorkerReceiptLedgerV1::recover(fixture.root(), &fixture.policy),
            Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::AnchorJournalRecordMismatch)
        ));
    }

    #[test]
    fn current_anchor_receipt_survives_successor_prepare_abort_and_replacement() {
        let fixture = Fixture::new();
        let (first_request, first_publication) = fixture.entry(1, [0; 32], 0x77);
        let first_anchor = first_publication.receipt().next_rollback_anchor();
        let mut ledger = WorkerReceiptLedgerV1::recover(fixture.root(), &fixture.policy).unwrap();
        fixture
            .commit_publication(
                &mut ledger,
                first_request.clone(),
                first_publication.clone(),
            )
            .unwrap();
        let current_record = ledger.last_record().unwrap().canonical;
        let current_receipt = ledger
            .last_record()
            .unwrap()
            .external_anchor_receipt()
            .clone();

        let (second_request, second_publication) = fixture.entry(2, first_anchor, 0x78);
        let second_challenge = ledger
            .prepare_external_anchor(second_request, second_publication)
            .unwrap();
        assert_eq!(ledger.last_record().unwrap().canonical, current_record);
        assert_eq!(
            ledger.last_record().unwrap().external_anchor_receipt(),
            &current_receipt
        );
        assert_eq!(
            ledger
                .recover_current_carriage(&fixture.subject)
                .unwrap()
                .publication(),
            &first_publication
        );

        drop(ledger);
        let mut recovered =
            WorkerReceiptLedgerV1::recover(fixture.root(), &fixture.policy).unwrap();
        assert_eq!(recovered.last_record().unwrap().canonical, current_record);
        assert_eq!(
            recovered.last_record().unwrap().external_anchor_receipt(),
            &current_receipt
        );
        let abort = fixture.anchor_observation(&second_challenge, AnchorPositionV1::Prior);
        assert_eq!(
            recovered
                .record_external_anchor_observation(&abort)
                .unwrap(),
            CompilerExecutionWorkerAnchorJournalStageV1::Aborted
        );

        let (replacement_request, replacement_publication) = fixture.entry(2, first_anchor, 0x79);
        recovered
            .prepare_external_anchor(replacement_request, replacement_publication)
            .unwrap();
        assert_eq!(recovered.last_record().unwrap().canonical, current_record);
        assert_eq!(
            recovered.last_record().unwrap().external_anchor_receipt(),
            &current_receipt
        );
    }

    #[test]
    fn successor_journal_must_continue_the_embedded_external_head() {
        let fixture = Fixture::new();
        let (first_request, first_publication) = fixture.entry(1, [0; 32], 0x7a);
        let first_anchor = first_publication.receipt().next_rollback_anchor();
        let mut ledger = WorkerReceiptLedgerV1::recover(fixture.root(), &fixture.policy).unwrap();
        fixture
            .commit_publication(&mut ledger, first_request, first_publication)
            .unwrap();

        let (second_request, second_publication) = fixture.entry(2, first_anchor, 0x7b);
        let transaction = CompilerExecutionExternalAnchorTransactionV1::new(
            fixture.policy.clone(),
            second_request,
            second_publication,
        )
        .unwrap();
        let key =
            PinnedAnchorKeyV1::from_bytes(fixture.anchor_signing_key.verifying_key().to_bytes())
                .unwrap();
        let wrong_prior_head = HashChainHeadV1::from_bytes([0xe1; SHA256_BYTES]);
        assert_ne!(
            wrong_prior_head,
            ledger
                .last_record()
                .unwrap()
                .external_anchor_receipt()
                .challenge()
                .proposed_head()
        );
        let prepared = AnchoredStateV1::from_local_state(1, wrong_prior_head)
            .prepare(transaction.external_anchor_digest(), &key)
            .unwrap();
        let pending = prepared
            .begin_advance(CallerNonceV1::from_bytes([0xe2; SHA256_BYTES]), &key)
            .unwrap();
        let substituted = CompilerExecutionWorkerAnchorJournalV1::prepared(
            transaction,
            pending.challenge().clone(),
        )
        .unwrap();
        ledger
            .store
            .commit_record(
                ANCHOR_CANONICAL_RECORD,
                ANCHOR_REDO_RECORD,
                substituted.canonical_bytes(),
                COMPILER_EXECUTION_WORKER_ANCHOR_JOURNAL_BYTES_V1,
                &mut NoRetainedDurableDirectoryHooksV1,
            )
            .unwrap();
        drop(ledger);

        assert!(matches!(
            WorkerReceiptLedgerV1::recover(fixture.root(), &fixture.policy),
            Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::AnchorJournalRecordMismatch)
        ));
    }

    #[test]
    fn wrong_policy_and_valid_non_successor_redo_fail_closed() {
        let fixture = Fixture::new();
        let (first_request, first_publication) = fixture.entry(1, [0; 32], 0x76);
        let first_anchor = first_publication.receipt().next_rollback_anchor();
        let mut ledger = WorkerReceiptLedgerV1::recover(fixture.root(), &fixture.policy).unwrap();
        fixture
            .commit_publication(&mut ledger, first_request, first_publication)
            .unwrap();

        let wrong_key = SigningKey::from_bytes(&[0x52; SHA256_BYTES]);
        let wrong_policy = policy(
            &wrong_key.verifying_key(),
            &fixture.anchor_signing_key.verifying_key(),
            8,
        );
        assert!(matches!(
            WorkerReceiptLedgerV1::recover(fixture.root(), &wrong_policy),
            Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::PolicyMismatch)
        ));

        let (third_request, third_publication) = fixture.entry(3, first_anchor, 0x77);
        let third_external_anchor_receipt = fixture.external_anchor_receipt(
            &third_request,
            &third_publication,
            [0x77; 32],
            [0x78; 32],
        );
        let third = WorkerReceiptRecordV2::new(
            &fixture.policy,
            third_request,
            third_publication,
            third_external_anchor_receipt,
            3,
            first_anchor,
        )
        .unwrap();
        ledger
            .store
            .stage_record_redo(
                REDO_RECORD,
                &third.canonical,
                COMPILER_EXECUTION_WORKER_LEDGER_RECORD_BYTES_V2,
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
                let mut ledger =
                    WorkerReceiptLedgerV1::recover(fixture.root(), &fixture.policy).unwrap();
                let challenge = ledger
                    .prepare_external_anchor(request.clone(), publication.clone())
                    .unwrap();
                let observation =
                    fixture.anchor_observation(&challenge, AnchorPositionV1::Proposed);
                ledger
                    .record_external_anchor_observation(&observation)
                    .unwrap();
                let external_anchor_receipt =
                    ledger.anchor_journal().unwrap().receipt().unwrap().clone();
                let expected = WorkerReceiptRecordV2::new(
                    &fixture.policy,
                    request.clone(),
                    publication.clone(),
                    external_anchor_receipt,
                    1,
                    [0; 32],
                )
                .unwrap();
                let mut fault = RecordFault::new(boundary, timing);
                assert!(
                    ledger
                        .commit_anchored_publication_with_hooks(&mut fault)
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
                fixture
                    .commit_publication(&mut ledger, first_request, first_publication)
                    .unwrap();
                let prior = ledger.last_record().unwrap().canonical;
                let challenge = ledger
                    .prepare_external_anchor(second_request.clone(), second_publication.clone())
                    .unwrap();
                let observation =
                    fixture.anchor_observation(&challenge, AnchorPositionV1::Proposed);
                ledger
                    .record_external_anchor_observation(&observation)
                    .unwrap();
                let external_anchor_receipt =
                    ledger.anchor_journal().unwrap().receipt().unwrap().clone();
                let successor = WorkerReceiptRecordV2::new(
                    &fixture.policy,
                    second_request.clone(),
                    second_publication.clone(),
                    external_anchor_receipt,
                    2,
                    first_anchor,
                )
                .unwrap()
                .canonical;
                let mut fault = RecordFault::new(boundary, timing);
                assert!(
                    ledger
                        .commit_anchored_publication_with_hooks(&mut fault)
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

    #[test]
    fn anchored_publication_restarts_and_replays_exactly_before_ack() {
        let fixture = Fixture::new();
        let (request, publication) = fixture.entry(1, [0; 32], 0x81);
        let mut ledger = WorkerReceiptLedgerV1::recover(fixture.root(), &fixture.policy).unwrap();
        let challenge = ledger
            .prepare_external_anchor(request.clone(), publication.clone())
            .unwrap();
        let prepared = ledger.anchor_journal().unwrap().clone();
        assert_eq!(
            prepared.stage(),
            CompilerExecutionWorkerAnchorJournalStageV1::PreparedAnchor
        );
        assert_eq!(prepared.transaction().request(), &request);
        assert_eq!(prepared.transaction().publication(), &publication);
        assert_eq!(prepared.challenge(), &challenge);
        assert!(ledger.last_record().is_none());
        assert!(matches!(
            ledger
                .prepare_external_anchor_publication(request.clone(), publication.clone())
                .unwrap(),
            WorkerExternalAnchorPublicationPlanV1::Exchange(replayed) if replayed == challenge
        ));

        drop(ledger);
        let mut recovered =
            WorkerReceiptLedgerV1::recover(fixture.root(), &fixture.policy).unwrap();
        assert_eq!(recovered.anchor_journal().unwrap(), &prepared);
        let mut no_write = RecordFault::new(
            RetainedDurableRecordBoundaryV1::CreateTemp,
            RetainedDurableFaultTimingV1::Before,
        );
        let replayed_challenge = recovered
            .prepare_external_anchor_with_hooks(request.clone(), publication.clone(), &mut no_write)
            .unwrap();
        assert_eq!(replayed_challenge, challenge);
        assert!(!no_write.fired);

        let observation = fixture.anchor_observation(&challenge, AnchorPositionV1::Proposed);
        assert_eq!(
            recovered
                .record_external_anchor_observation(&observation)
                .unwrap(),
            CompilerExecutionWorkerAnchorJournalStageV1::AnchorCommitted
        );
        assert!(matches!(
            recovered
                .prepare_external_anchor_publication(request.clone(), publication.clone())
                .unwrap(),
            WorkerExternalAnchorPublicationPlanV1::CommitLocally
        ));
        assert!(recovered.last_record().is_none());
        let conflicting = fixture.anchor_observation(&challenge, AnchorPositionV1::Prior);
        assert!(matches!(
            recovered.record_external_anchor_observation(&conflicting),
            Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::ExternalAnchorObservationMismatch)
        ));

        let acknowledgment = recovered
            .commit_anchored_publication()
            .unwrap()
            .into_acknowledgment();
        let worker_identity = recovered.last_record().unwrap().identity;
        let published = recovered.anchor_journal().unwrap().clone();
        assert_eq!(
            published.stage(),
            CompilerExecutionWorkerAnchorJournalStageV1::Published
        );
        assert_eq!(published.worker_record_identity(), worker_identity);
        assert_eq!(
            acknowledgment.worker_ledger_record_identity(),
            worker_identity
        );
        assert_eq!(
            published.receipt().unwrap().position(),
            AnchorPositionV1::Proposed
        );
        assert_eq!(
            recovered.last_record().unwrap().external_anchor_receipt(),
            published.receipt().unwrap()
        );

        drop(recovered);
        let mut restarted =
            WorkerReceiptLedgerV1::recover(fixture.root(), &fixture.policy).unwrap();
        assert_eq!(restarted.anchor_journal().unwrap(), &published);
        assert!(matches!(
            restarted
                .prepare_external_anchor_publication(request.clone(), publication.clone())
                .unwrap(),
            WorkerExternalAnchorPublicationPlanV1::CommitLocally
        ));
        assert_eq!(
            restarted
                .record_external_anchor_observation(&observation)
                .unwrap(),
            CompilerExecutionWorkerAnchorJournalStageV1::Published
        );
        let replayed_acknowledgment = restarted
            .commit_anchored_publication()
            .unwrap()
            .into_acknowledgment();
        assert_eq!(replayed_acknowledgment, acknowledgment);
    }

    #[test]
    fn external_anchor_abort_restarts_and_permits_only_a_replacement() {
        let fixture = Fixture::new();
        let (request, publication) = fixture.entry(1, [0; 32], 0x82);
        let mut ledger = WorkerReceiptLedgerV1::recover(fixture.root(), &fixture.policy).unwrap();
        let challenge = ledger
            .prepare_external_anchor(request.clone(), publication.clone())
            .unwrap();
        let observation = fixture.anchor_observation(&challenge, AnchorPositionV1::Prior);
        assert_eq!(
            ledger
                .record_external_anchor_observation(&observation)
                .unwrap(),
            CompilerExecutionWorkerAnchorJournalStageV1::Aborted
        );
        assert!(matches!(
            ledger.commit_anchored_publication(),
            Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::ExternalAnchorNotCommitted)
        ));
        assert!(ledger.last_record().is_none());
        assert!(matches!(
            ledger.prepare_external_anchor_publication(request, publication),
            Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::ExternalAnchorNotCommitted)
        ));
        let aborted = ledger.anchor_journal().unwrap().clone();

        drop(ledger);
        let mut recovered =
            WorkerReceiptLedgerV1::recover(fixture.root(), &fixture.policy).unwrap();
        assert_eq!(recovered.anchor_journal().unwrap(), &aborted);
        assert_eq!(
            recovered
                .record_external_anchor_observation(&observation)
                .unwrap(),
            CompilerExecutionWorkerAnchorJournalStageV1::Aborted
        );
        let (replacement_request, replacement_publication) = fixture.entry(1, [0; 32], 0x83);
        let replacement_challenge = recovered
            .prepare_external_anchor(replacement_request.clone(), replacement_publication.clone())
            .unwrap();
        assert_ne!(replacement_challenge, challenge);
        let replacement = recovered.anchor_journal().unwrap();
        assert_eq!(
            replacement.stage(),
            CompilerExecutionWorkerAnchorJournalStageV1::PreparedAnchor
        );
        assert_eq!(replacement.transaction().request(), &replacement_request);
        assert_eq!(
            replacement.transaction().publication(),
            &replacement_publication
        );
        assert_eq!(replacement.challenge().prior_head(), challenge.prior_head());
    }

    #[test]
    fn anchor_only_recovery_rejects_policy_substitution() {
        let fixture = Fixture::new();
        let (request, publication) = fixture.entry(1, [0; 32], 0x84);
        let mut ledger = WorkerReceiptLedgerV1::recover(fixture.root(), &fixture.policy).unwrap();
        ledger
            .prepare_external_anchor(request, publication)
            .unwrap();
        drop(ledger);

        let other_anchor = SigningKey::from_bytes(&[0xa2; 32]);
        let substituted_policy = policy(
            &fixture.signing_key.verifying_key(),
            &other_anchor.verifying_key(),
            7,
        );
        assert!(matches!(
            WorkerReceiptLedgerV1::recover(fixture.root(), &substituted_policy),
            Err(ProtectedCompilerExecutionWorkerLedgerErrorV1::PolicyMismatch)
        ));
    }

    #[test]
    fn every_anchor_prepare_crash_recovers_empty_or_the_exact_preparation() {
        for boundary in RECORD_BOUNDARIES {
            for timing in FAULT_TIMINGS {
                let fixture = Fixture::new();
                let (request, publication) = fixture.entry(1, [0; 32], 0x85);
                let mut ledger =
                    WorkerReceiptLedgerV1::recover(fixture.root(), &fixture.policy).unwrap();
                let mut fault = RecordFault::new(boundary, timing);
                assert!(
                    ledger
                        .prepare_external_anchor_with_hooks(
                            request.clone(),
                            publication.clone(),
                            &mut fault,
                        )
                        .is_err(),
                    "anchor preparation unexpectedly succeeded at {boundary:?}/{timing:?}"
                );
                assert!(fault.fired, "fault did not fire at {boundary:?}/{timing:?}");
                assert!(ledger.poisoned);
                drop(ledger);

                let mut recovered =
                    WorkerReceiptLedgerV1::recover(fixture.root(), &fixture.policy).unwrap();
                assert!(recovered.last_record().is_none());
                if let Some(prepared) = recovered.anchor_journal().cloned() {
                    assert_eq!(
                        prepared.stage(),
                        CompilerExecutionWorkerAnchorJournalStageV1::PreparedAnchor
                    );
                    assert_eq!(prepared.transaction().request(), &request);
                    assert_eq!(prepared.transaction().publication(), &publication);
                    let challenge = prepared.challenge().clone();
                    let mut no_write = RecordFault::new(
                        RetainedDurableRecordBoundaryV1::CreateTemp,
                        RetainedDurableFaultTimingV1::Before,
                    );
                    assert_eq!(
                        recovered
                            .prepare_external_anchor_with_hooks(
                                request.clone(),
                                publication.clone(),
                                &mut no_write,
                            )
                            .unwrap(),
                        challenge
                    );
                    assert!(!no_write.fired);
                }
            }
        }
    }

    #[test]
    fn every_anchor_observation_crash_recovers_only_adjacent_stages() {
        for (position, expected_stage) in [
            (
                AnchorPositionV1::Proposed,
                CompilerExecutionWorkerAnchorJournalStageV1::AnchorCommitted,
            ),
            (
                AnchorPositionV1::Prior,
                CompilerExecutionWorkerAnchorJournalStageV1::Aborted,
            ),
        ] {
            for boundary in RECORD_BOUNDARIES {
                for timing in FAULT_TIMINGS {
                    let fixture = Fixture::new();
                    let (request, publication) = fixture.entry(1, [0; 32], 0x86);
                    let mut ledger =
                        WorkerReceiptLedgerV1::recover(fixture.root(), &fixture.policy).unwrap();
                    let challenge = ledger
                        .prepare_external_anchor(request, publication)
                        .unwrap();
                    let observation = fixture.anchor_observation(&challenge, position);
                    let mut fault = RecordFault::new(boundary, timing);
                    assert!(
                        ledger
                            .record_external_anchor_observation_with_hooks(
                                &observation,
                                &mut fault,
                            )
                            .is_err(),
                        "anchor observation unexpectedly succeeded at {position:?}/{boundary:?}/{timing:?}"
                    );
                    assert!(fault.fired, "fault did not fire at {boundary:?}/{timing:?}");
                    drop(ledger);

                    let mut recovered =
                        WorkerReceiptLedgerV1::recover(fixture.root(), &fixture.policy).unwrap();
                    let recovered_stage = recovered.anchor_journal().unwrap().stage();
                    assert!(
                        recovered_stage
                            == CompilerExecutionWorkerAnchorJournalStageV1::PreparedAnchor
                            || recovered_stage == expected_stage,
                        "anchor observation recovered a third stage at {position:?}/{boundary:?}/{timing:?}"
                    );
                    assert_eq!(
                        recovered
                            .record_external_anchor_observation(&observation)
                            .unwrap(),
                        expected_stage
                    );
                    assert!(recovered.last_record().is_none());
                }
            }
        }
    }

    #[test]
    fn every_worker_record_crash_after_anchor_commit_finishes_exactly_once() {
        for boundary in RECORD_BOUNDARIES {
            for timing in FAULT_TIMINGS {
                let fixture = Fixture::new();
                let (request, publication) = fixture.entry(1, [0; 32], 0x87);
                let mut ledger =
                    WorkerReceiptLedgerV1::recover(fixture.root(), &fixture.policy).unwrap();
                let challenge = ledger
                    .prepare_external_anchor(request, publication)
                    .unwrap();
                let observation =
                    fixture.anchor_observation(&challenge, AnchorPositionV1::Proposed);
                ledger
                    .record_external_anchor_observation(&observation)
                    .unwrap();
                let committed = ledger.anchor_journal().unwrap().clone();
                let mut fault = RecordFault::new(boundary, timing);
                assert!(
                    ledger
                        .commit_anchored_publication_with_hooks(&mut fault)
                        .is_err(),
                    "Worker publication unexpectedly succeeded at {boundary:?}/{timing:?}"
                );
                assert!(fault.fired, "fault did not fire at {boundary:?}/{timing:?}");
                drop(ledger);

                let mut recovered =
                    WorkerReceiptLedgerV1::recover(fixture.root(), &fixture.policy).unwrap();
                assert_eq!(recovered.anchor_journal().unwrap(), &committed);
                let acknowledgment = recovered
                    .commit_anchored_publication()
                    .unwrap()
                    .into_acknowledgment();
                let record = recovered.last_record().unwrap();
                assert_eq!(
                    acknowledgment.worker_ledger_record_identity(),
                    record.identity
                );
                assert_eq!(
                    recovered.anchor_journal().unwrap().stage(),
                    CompilerExecutionWorkerAnchorJournalStageV1::Published
                );
            }
        }
    }

    #[test]
    fn every_published_journal_crash_finishes_exactly_once() {
        for boundary in RECORD_BOUNDARIES {
            for timing in FAULT_TIMINGS {
                let fixture = Fixture::new();
                let (request, publication) = fixture.entry(1, [0; 32], 0x88);
                let mut ledger =
                    WorkerReceiptLedgerV1::recover(fixture.root(), &fixture.policy).unwrap();
                let challenge = ledger
                    .prepare_external_anchor(request.clone(), publication.clone())
                    .unwrap();
                let observation =
                    fixture.anchor_observation(&challenge, AnchorPositionV1::Proposed);
                ledger
                    .record_external_anchor_observation(&observation)
                    .unwrap();
                let committed = ledger.anchor_journal().unwrap().clone();
                let external_anchor_receipt = committed.receipt().unwrap().clone();
                ledger
                    .commit_publication_record_with_hooks(
                        request,
                        publication,
                        external_anchor_receipt,
                        &mut NoRetainedDurableDirectoryHooksV1,
                    )
                    .unwrap();
                let published = committed
                    .clone()
                    .mark_published(ledger.last_record().unwrap().identity)
                    .unwrap();
                let mut fault = RecordFault::new(boundary, timing);
                assert!(
                    ledger
                        .commit_anchor_journal_with_hooks(published.clone(), &mut fault)
                        .is_err(),
                    "published journal unexpectedly succeeded at {boundary:?}/{timing:?}"
                );
                assert!(fault.fired, "fault did not fire at {boundary:?}/{timing:?}");
                drop(ledger);

                let mut recovered =
                    WorkerReceiptLedgerV1::recover(fixture.root(), &fixture.policy).unwrap();
                let recovered_journal = recovered.anchor_journal().unwrap();
                assert!(
                    recovered_journal == &committed || recovered_journal == &published,
                    "published journal recovered a third state at {boundary:?}/{timing:?}"
                );
                let acknowledgment = recovered
                    .commit_anchored_publication()
                    .unwrap()
                    .into_acknowledgment();
                let record = recovered.last_record().unwrap();
                assert_eq!(
                    acknowledgment.worker_ledger_record_identity(),
                    record.identity
                );
                assert_eq!(recovered.anchor_journal().unwrap(), &published);
            }
        }
    }

    fn policy(
        verifying_key: &VerifyingKey,
        anchor_verifying_key: &VerifyingKey,
        generation: u64,
    ) -> CompilerExecutionIssuerPolicyV1 {
        CompilerExecutionIssuerPolicyV1::new(
            generation,
            CompilerExecutionIssuerMeasurementV1::new([0x61; SHA256_BYTES], 12_345).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([0x62; SHA256_BYTES], 67_890).unwrap(),
            verifying_key.to_bytes(),
            anchor_verifying_key.to_bytes(),
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
