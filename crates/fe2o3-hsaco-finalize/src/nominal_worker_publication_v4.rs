//! Structural V4 intent persistence/recovery through the existing strict Worker V3 store.
//! This is a prerequisite, not #272 closure. No publication authority bridge,
//! conditional proof receipt, load envelope or host admission is exposed here.
use super::*;
use crate::PreparedFinalizedNominalWorkerHsacoV4;
type E = WorkerV3HsacoPublicationErrorV1;

/// Move-only V4 intent, distinct from the V1 and V3 persistence owners.
///
/// ```compile_fail
/// use fe2o3_hsaco_finalize::{PreparedNominalWorkerPublicationV3, PreparedNominalWorkerPublicationV4};
/// fn downgrade(value: PreparedNominalWorkerPublicationV4) -> PreparedNominalWorkerPublicationV3 { value.into() }
/// ```
#[derive(Debug)]
pub struct PreparedNominalWorkerPublicationV4 {
    prepared: PreparedProtectedWorkerV3HsacoPublicationV1,
}
impl PreparedNominalWorkerPublicationV4 {
    pub fn publication_intent(&self) -> SealedProtectedWorkerV3HsacoPublicationIntentV1 {
        self.prepared.intent
    }
    pub fn exact_finalized_hsaco(&self) -> &[u8] {
        self.prepared.exact_finalized_hsaco()
    }
    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }
    pub const fn grants_proof_authority(&self) -> bool {
        false
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

/// Independently reconstructed structural V4 lineage with exact mandatory contracts.
/// It cannot enter the V1/V3 publication bridge or produce a host load envelope.
///
/// ```compile_fail
/// use fe2o3_hsaco_finalize::RecoveredNominalWorkerPublicationV4;
/// fn duplicate(value: RecoveredNominalWorkerPublicationV4) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_hsaco_finalize::{RecoveredNominalWorkerPublicationV3, RecoveredNominalWorkerPublicationV4};
/// fn downgrade(value: RecoveredNominalWorkerPublicationV4) -> RecoveredNominalWorkerPublicationV3 { value.into() }
/// ```
/// ```compile_fail
/// use fe2o3_hsaco_finalize::{RecoveredProtectedWorkerV3HsacoPublicationV1, RecoveredNominalWorkerPublicationV4};
/// fn downgrade(value: RecoveredNominalWorkerPublicationV4) -> RecoveredProtectedWorkerV3HsacoPublicationV1 { value.into() }
/// ```
/// ```compile_fail
/// use fe2o3_hsaco_finalize::{RecoveredNominalWorkerPublicationV4, CompilerClosureV2, publish_recovered_nominal_worker_hsaco_v3};
/// fn publish(path: &std::path::Path, producer: &fe2o3_artifact_transaction::ProducerIdentity,
///            closure: CompilerClosureV2, value: RecoveredNominalWorkerPublicationV4) {
///     let _ = publish_recovered_nominal_worker_hsaco_v3(path, producer, closure, value);
/// }
/// ```
/// ```compile_fail
/// use fe2o3_hsaco_finalize::RecoveredNominalWorkerPublicationV4;
/// fn load(value: RecoveredNominalWorkerPublicationV4) { let _ = value.into_load_envelope_parts_v1(); }
/// ```
#[derive(Debug)]
pub struct RecoveredNominalWorkerPublicationV4 {
    outcome: WorkerV3PublicationIntentOutcomeV1,
    record: WorkerV3PublicationIntentRecordV1,
    finalized: PreparedFinalizedNominalWorkerHsacoV4,
    intent: SealedProtectedWorkerV3HsacoPublicationIntentV1,
}
impl RecoveredNominalWorkerPublicationV4 {
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
    pub fn finalized_evidence(&self) -> &PreparedFinalizedNominalWorkerHsacoV4 {
        &self.finalized
    }
    pub const fn is_structural_only(&self) -> bool {
        true
    }
    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }
    pub const fn grants_proof_authority(&self) -> bool {
        false
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

/// Prepare inert durable components without issuing a publication capability.
pub fn prepare_nominal_worker_publication_v4(
    producer: &ProducerIdentity,
    finalized: PreparedFinalizedNominalWorkerHsacoV4,
) -> Result<PreparedNominalWorkerPublicationV4, E> {
    Ok(PreparedNominalWorkerPublicationV4 {
        prepared: prepare_versioned_publication(producer, FinalizedOwner::NominalV4(finalized))?,
    })
}

/// Persist and independently replay using the same storage and validator as V1/V3.
pub fn persist_prepared_nominal_worker_publication_v4(
    output_dir: &Path,
    producer: &ProducerIdentity,
    prepared: PreparedNominalWorkerPublicationV4,
) -> Result<RecoveredNominalWorkerPublicationV4, E> {
    validate_nominal_recovery_v4(
        producer,
        persist_versioned_publication(output_dir, producer, prepared.prepared)?,
    )
}

/// Reconstruct exact V4 custody; refuse V1/V3 owners and every schema substitution.
pub fn recover_nominal_worker_publication_v4(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
) -> Result<RecoveredNominalWorkerPublicationV4, E> {
    validate_nominal_recovery_v4(
        producer,
        recover_worker_v3_publication_intent_v1(output_dir, producer, attempt)?,
    )
}

fn validate_nominal_recovery_v4(
    producer: &ProducerIdentity,
    recovered: RecoveredWorkerV3PublicationIntentV1,
) -> Result<RecoveredNominalWorkerPublicationV4, E> {
    let ValidatedRecoveredPublication {
        outcome,
        record,
        finalized,
        intent,
    } = validate_recovered_versioned(producer, recovered)?;
    Ok(RecoveredNominalWorkerPublicationV4 {
        outcome,
        record,
        finalized: finalized.into_nominal_v4()?,
        intent,
    })
}
