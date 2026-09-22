//! Typed nominal facades over the existing durable replay and publication gate.
use super::*;
use crate::PreparedFinalizedNominalWorkerHsacoV3;
type E = WorkerV3HsacoPublicationErrorV1;

/// Move-only nominal intent; cannot be passed to the descriptor-V1 persistence API.
#[derive(Debug)]
pub struct PreparedNominalWorkerPublicationV3 {
    prepared: PreparedProtectedWorkerV3HsacoPublicationV1,
}
impl PreparedNominalWorkerPublicationV3 {
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
/// use fe2o3_hsaco_finalize::RecoveredNominalWorkerPublicationV3;
/// fn duplicate(value: RecoveredNominalWorkerPublicationV3) { let _ = value.clone(); }
/// ```
#[derive(Debug)]
pub struct RecoveredNominalWorkerPublicationV3 {
    outcome: WorkerV3PublicationIntentOutcomeV1,
    record: WorkerV3PublicationIntentRecordV1,
    finalized: PreparedFinalizedNominalWorkerHsacoV3,
    intent: SealedProtectedWorkerV3HsacoPublicationIntentV1,
}
impl RecoveredNominalWorkerPublicationV3 {
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
    pub fn finalized_evidence(&self) -> &PreparedFinalizedNominalWorkerHsacoV3 {
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
        RecoveredPublicationRef::NominalV3(self).publication_binding(closure)
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
pub struct PublishedNominalWorkerHsacoV3 {
    recovered: RecoveredNominalWorkerPublicationV3,
    publication: AttemptScopedHsacoPublicationResultV3,
}
impl PublishedNominalWorkerHsacoV3 {
    pub const fn recovered_evidence(&self) -> &RecoveredNominalWorkerPublicationV3 {
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
            crate::prepare_nominal_worker_compact_finalizer_replay_v3(self.recovered.finalized)?
                .into_parts();
        Ok(PublishedProtectedWorkerV3LoadEnvelopePartsV1 {
            replay,
            storage_record,
            claim,
            current_lease,
        })
    }
}

pub fn prepare_nominal_worker_publication_v3(
    producer: &ProducerIdentity,
    finalized: PreparedFinalizedNominalWorkerHsacoV3,
) -> Result<PreparedNominalWorkerPublicationV3, E> {
    Ok(PreparedNominalWorkerPublicationV3 {
        prepared: prepare_versioned_publication(producer, FinalizedOwner::NominalV3(finalized))?,
    })
}
pub fn persist_prepared_nominal_worker_publication_v3(
    output_dir: &Path,
    producer: &ProducerIdentity,
    prepared: PreparedNominalWorkerPublicationV3,
) -> Result<RecoveredNominalWorkerPublicationV3, E> {
    validate_nominal_recovery(
        producer,
        persist_versioned_publication(output_dir, producer, prepared.prepared)?,
    )
}
pub fn recover_nominal_worker_publication_v3(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
) -> Result<RecoveredNominalWorkerPublicationV3, E> {
    validate_nominal_recovery(
        producer,
        recover_worker_v3_publication_intent_v1(output_dir, producer, attempt)?,
    )
}
pub fn publish_recovered_nominal_worker_hsaco_v3(
    output_dir: &Path,
    producer: &ProducerIdentity,
    compiler_closure: CompilerClosureV2,
    recovered: RecoveredNominalWorkerPublicationV3,
) -> Result<PublishedNominalWorkerHsacoV3, E> {
    let publication = publish_recovered_versioned(
        output_dir,
        producer,
        compiler_closure,
        RecoveredPublicationRef::NominalV3(&recovered),
    )?;
    Ok(PublishedNominalWorkerHsacoV3 {
        recovered,
        publication,
    })
}

fn validate_nominal_recovery(
    producer: &ProducerIdentity,
    recovered: RecoveredWorkerV3PublicationIntentV1,
) -> Result<RecoveredNominalWorkerPublicationV3, E> {
    let ValidatedRecoveredPublication {
        outcome,
        record,
        finalized,
        intent,
    } = validate_recovered_versioned(producer, recovered)?;
    Ok(RecoveredNominalWorkerPublicationV3 {
        outcome,
        record,
        finalized: finalized.into_nominal()?,
        intent,
    })
}

/// Only complete typed recovery owners reach the one publication authority bridge.
#[derive(Clone, Copy)]
pub(super) enum RecoveredPublicationRef<'a> {
    V1(&'a RecoveredProtectedWorkerV3HsacoPublicationV1),
    NominalV3(&'a RecoveredNominalWorkerPublicationV3),
}
impl<'a> RecoveredPublicationRef<'a> {
    pub(super) fn intent(self) -> SealedProtectedWorkerV3HsacoPublicationIntentV1 {
        match self {
            Self::V1(v) => v.intent,
            Self::NominalV3(v) => v.intent,
        }
    }
    fn record(self) -> WorkerV3PublicationIntentRecordV1 {
        match self {
            Self::V1(v) => v.record,
            Self::NominalV3(v) => v.record,
        }
    }
    pub(super) fn finalized(self) -> FinalizedRef<'a> {
        match self {
            Self::V1(v) => FinalizedRef::V1(&v.finalized),
            Self::NominalV3(v) => FinalizedRef::NominalV3(&v.finalized),
        }
    }
    pub(super) fn publication_binding(
        self,
        compiler_closure: CompilerClosureV2,
    ) -> Result<WorkerV3PublicationBindingV1, E> {
        if compiler_closure
            != self
                .finalized()
                .raw()
                .binding_expectation()
                .compiler_closure()
        {
            return Err(E::CompilerClosureMismatch);
        }
        let intent = self.intent();
        let raw = intent.raw_output_identity();
        let finalized = intent.finalized_output_identity();
        WorkerV3PublicationBindingV1::new(
            compiler_closure,
            self.record().identity().as_bytes(),
            *intent.finalization_identity().as_bytes(),
            *intent.source_evidence_identity().as_bytes(),
            *intent.binding_identity().as_bytes(),
            *intent.raw_inspection_identity().as_bytes(),
            *raw.sha256(),
            raw.byte_len(),
            *finalized.sha256(),
            finalized.byte_len(),
        )
        .map_err(E::PublicationBinding)
    }
}
