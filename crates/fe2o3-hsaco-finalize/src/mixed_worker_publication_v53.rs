//! Exact mixed-contract facades over durable Worker replay and publication.
//! Artifact publication preserves conditional premises; it is not proof or launch admission.
use super::*;
use crate::PreparedFinalizedNominalWorkerHsacoV53;
type E = WorkerV3HsacoPublicationErrorV1;

/// Move-only nominal intent; cannot be passed to the descriptor-V1 persistence API.
#[derive(Debug)]
pub struct PreparedMixedWorkerPublicationV53 {
    prepared: PreparedProtectedWorkerV3HsacoPublicationV1,
}
impl PreparedMixedWorkerPublicationV53 {
    pub fn publication_intent(&self) -> SealedProtectedWorkerV3HsacoPublicationIntentV1 {
        self.prepared.intent
    }
    pub fn exact_finalized_hsaco(&self) -> &[u8] {
        self.prepared.exact_finalized_hsaco()
    }
    pub const fn grants_publication_authority(&self) -> bool {
        false
    }
    pub const fn grants_load_authority(&self) -> bool {
        false
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Independently reconstructed nominal lineage. Recovery is not compiler/proof authority.
/// ```compile_fail
/// use fe2o3_hsaco_finalize::RecoveredMixedWorkerPublicationV53;
/// fn duplicate(value: RecoveredMixedWorkerPublicationV53) { let _ = value.clone(); }
/// ```
#[derive(Debug)]
pub struct RecoveredMixedWorkerPublicationV53 {
    outcome: WorkerV3PublicationIntentOutcomeV1,
    pub(super) record: WorkerV3PublicationIntentRecordV1,
    pub(super) finalized: PreparedFinalizedNominalWorkerHsacoV53,
    pub(super) intent: SealedProtectedWorkerV3HsacoPublicationIntentV1,
}
impl RecoveredMixedWorkerPublicationV53 {
    pub const fn outcome(&self) -> WorkerV3PublicationIntentOutcomeV1 {
        self.outcome
    }
    pub const fn storage_record(&self) -> WorkerV3PublicationIntentRecordV1 {
        self.record
    }
    pub const fn publication_intent(&self) -> SealedProtectedWorkerV3HsacoPublicationIntentV1 {
        self.intent
    }
    pub fn exact_finalized_hsaco(&self) -> &[u8] {
        self.finalized.finalized().as_bytes()
    }
    pub fn finalized_evidence(&self) -> &PreparedFinalizedNominalWorkerHsacoV53 {
        &self.finalized
    }
    pub fn compiler_execution_subject_v1(
        &self,
    ) -> Result<InertCompilerExecutionSubjectV1, CompilerExecutionSubjectErrorV1> {
        let raw = self.finalized.raw();
        InertCompilerExecutionSubjectV1::from_replay_evidence(
            raw.attempt(),
            raw.handoff_slot(),
            raw.transaction_identity(),
            raw.outer_handoff(),
        )
    }
    pub fn publication_binding(
        &self,
        closure: CompilerClosureV2,
    ) -> Result<WorkerV3PublicationBindingV1, E> {
        RecoveredPublicationRef::MixedV53(self).publication_binding(closure)
    }
    pub const fn grants_publication_authority(&self) -> bool {
        false
    }
    pub const fn grants_load_authority(&self) -> bool {
        false
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Exact published nominal occurrence with its non-clone current-publication lease.
/// Publication does not authenticate compiler execution or grant GPU launch authority.
#[derive(Debug)]
pub struct PublishedMixedWorkerHsacoV53 {
    recovered: RecoveredMixedWorkerPublicationV53,
    publication: AttemptScopedHsacoPublicationResultV3,
}
impl PublishedMixedWorkerHsacoV53 {
    pub const fn recovered_evidence(&self) -> &RecoveredMixedWorkerPublicationV53 {
        &self.recovered
    }
    pub const fn publication_result(&self) -> &AttemptScopedHsacoPublicationResultV3 {
        &self.publication
    }
    pub const fn published_claim(&self) -> &DurablePublishedHsacoClaimV3 {
        self.publication.published_claim()
    }
    pub fn compiler_execution_subject_v1(
        &self,
    ) -> Result<InertCompilerExecutionSubjectV1, CompilerExecutionSubjectErrorV1> {
        self.recovered.compiler_execution_subject_v1()
    }
    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }
    pub const fn grants_proof_authority(&self) -> bool {
        false
    }
    pub const fn grants_load_authority(&self) -> bool {
        false
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
    pub fn into_load_envelope_parts_v1(
        self,
    ) -> Result<PublishedProtectedWorkerV3LoadEnvelopePartsV1, E> {
        let storage_record = self.recovered.record;
        let claim = self.publication.published_claim().clone();
        let current_lease = self.publication.into_current_lease();
        let replay =
            crate::prepare_mixed_worker_compact_finalizer_replay_v53(self.recovered.finalized)?
                .into_parts();
        Ok(PublishedProtectedWorkerV3LoadEnvelopePartsV1 {
            replay,
            storage_record,
            claim,
            current_lease,
        })
    }
}

pub fn prepare_mixed_worker_publication_v53(
    producer: &ProducerIdentity,
    finalized: PreparedFinalizedNominalWorkerHsacoV53,
) -> Result<PreparedMixedWorkerPublicationV53, E> {
    Ok(PreparedMixedWorkerPublicationV53 {
        prepared: prepare_versioned_publication(producer, FinalizedOwner::MixedV53(finalized))?,
    })
}
pub fn persist_prepared_mixed_worker_publication_v53(
    output_dir: &Path,
    producer: &ProducerIdentity,
    prepared: PreparedMixedWorkerPublicationV53,
) -> Result<RecoveredMixedWorkerPublicationV53, E> {
    validate_nominal_recovery(
        producer,
        persist_versioned_publication(output_dir, producer, prepared.prepared)?,
    )
}
pub fn recover_mixed_worker_publication_v53(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
) -> Result<RecoveredMixedWorkerPublicationV53, E> {
    validate_nominal_recovery(
        producer,
        recover_worker_v3_publication_intent_v1(output_dir, producer, attempt)?,
    )
}
pub fn publish_recovered_mixed_worker_hsaco_v53(
    output_dir: &Path,
    producer: &ProducerIdentity,
    compiler_closure: CompilerClosureV2,
    recovered: RecoveredMixedWorkerPublicationV53,
) -> Result<PublishedMixedWorkerHsacoV53, E> {
    let publication = publish_recovered_versioned(
        output_dir,
        producer,
        compiler_closure,
        RecoveredPublicationRef::MixedV53(&recovered),
    )?;
    Ok(PublishedMixedWorkerHsacoV53 {
        recovered,
        publication,
    })
}

fn validate_nominal_recovery(
    producer: &ProducerIdentity,
    recovered: RecoveredWorkerV3PublicationIntentV1,
) -> Result<RecoveredMixedWorkerPublicationV53, E> {
    let ValidatedRecoveredPublication {
        outcome,
        record,
        finalized,
        intent,
    } = validate_recovered_versioned(producer, recovered)?;
    Ok(RecoveredMixedWorkerPublicationV53 {
        outcome,
        record,
        finalized: finalized.into_mixed_v53()?,
        intent,
    })
}
