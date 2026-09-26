//! Structural V5 intent persistence/recovery through the existing strict Worker V3 store.
//! This is a prerequisite, not #272 closure. No publication authority bridge,
//! conditional proof receipt, load envelope or host admission is exposed here.
use super::*;
use crate::PreparedFinalizedNominalWorkerHsacoV5;
type E = WorkerV3HsacoPublicationErrorV1;

/// Move-only V5 intent, distinct from the V1, V3 and V4 persistence owners.
///
/// ```compile_fail
/// use fe2o3_hsaco_finalize::{PreparedNominalWorkerPublicationV4, PreparedNominalWorkerPublicationV5};
/// fn downgrade(value: PreparedNominalWorkerPublicationV5) -> PreparedNominalWorkerPublicationV4 { value.into() }
/// ```
#[derive(Debug)]
pub struct PreparedNominalWorkerPublicationV5 {
    prepared: PreparedProtectedWorkerV3HsacoPublicationV1,
}
impl PreparedNominalWorkerPublicationV5 {
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

/// Independently reconstructed structural V5 lineage with exact mandatory contracts.
/// It cannot enter the V1/V3 publication bridge or produce a host load envelope.
///
/// ```compile_fail
/// use fe2o3_hsaco_finalize::RecoveredNominalWorkerPublicationV5;
/// fn duplicate(value: RecoveredNominalWorkerPublicationV5) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_hsaco_finalize::{RecoveredNominalWorkerPublicationV4, RecoveredNominalWorkerPublicationV5};
/// fn downgrade(value: RecoveredNominalWorkerPublicationV5) -> RecoveredNominalWorkerPublicationV4 { value.into() }
/// ```
/// ```compile_fail
/// use fe2o3_hsaco_finalize::{RecoveredProtectedWorkerV3HsacoPublicationV1, RecoveredNominalWorkerPublicationV5};
/// fn downgrade(value: RecoveredNominalWorkerPublicationV5) -> RecoveredProtectedWorkerV3HsacoPublicationV1 { value.into() }
/// ```
/// ```compile_fail
/// use fe2o3_hsaco_finalize::{RecoveredNominalWorkerPublicationV5, CompilerClosureV2, publish_recovered_nominal_worker_hsaco_v3};
/// fn publish(path: &std::path::Path, producer: &fe2o3_artifact_transaction::ProducerIdentity,
///            closure: CompilerClosureV2, value: RecoveredNominalWorkerPublicationV5) {
///     let _ = publish_recovered_nominal_worker_hsaco_v3(path, producer, closure, value);
/// }
/// ```
/// ```compile_fail
/// use fe2o3_hsaco_finalize::RecoveredNominalWorkerPublicationV5;
/// fn load(value: RecoveredNominalWorkerPublicationV5) { let _ = value.into_load_envelope_parts_v1(); }
/// ```
/// ```compile_fail
/// use fe2o3_hsaco_finalize::RecoveredNominalWorkerPublicationV5;
/// fn authority(value: RecoveredNominalWorkerPublicationV5) { let _ = value.publication_binding(); }
/// ```
#[derive(Debug)]
pub struct RecoveredNominalWorkerPublicationV5 {
    outcome: WorkerV3PublicationIntentOutcomeV1,
    record: WorkerV3PublicationIntentRecordV1,
    finalized: PreparedFinalizedNominalWorkerHsacoV5,
    intent: SealedProtectedWorkerV3HsacoPublicationIntentV1,
}
impl RecoveredNominalWorkerPublicationV5 {
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
    pub fn finalized_evidence(&self) -> &PreparedFinalizedNominalWorkerHsacoV5 {
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
pub fn prepare_nominal_worker_publication_v5(
    producer: &ProducerIdentity,
    finalized: PreparedFinalizedNominalWorkerHsacoV5,
) -> Result<PreparedNominalWorkerPublicationV5, E> {
    Ok(PreparedNominalWorkerPublicationV5 {
        prepared: prepare_versioned_publication(producer, FinalizedOwner::NominalV5(finalized))?,
    })
}

/// Persist and independently replay using the same storage and validator as V1/V3/V4.
pub fn persist_prepared_nominal_worker_publication_v5(
    output_dir: &Path,
    producer: &ProducerIdentity,
    prepared: PreparedNominalWorkerPublicationV5,
) -> Result<RecoveredNominalWorkerPublicationV5, E> {
    validate_nominal_recovery_v5(
        producer,
        persist_versioned_publication(output_dir, producer, prepared.prepared)?,
    )
}

/// Reconstruct exact V5 custody; refuse V1/V3/V4 owners and every schema substitution.
pub fn recover_nominal_worker_publication_v5(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
) -> Result<RecoveredNominalWorkerPublicationV5, E> {
    validate_nominal_recovery_v5(
        producer,
        recover_worker_v3_publication_intent_v1(output_dir, producer, attempt)?,
    )
}

fn validate_nominal_recovery_v5(
    producer: &ProducerIdentity,
    recovered: RecoveredWorkerV3PublicationIntentV1,
) -> Result<RecoveredNominalWorkerPublicationV5, E> {
    let ValidatedRecoveredPublication {
        outcome,
        record,
        finalized,
        intent,
    } = validate_recovered_versioned(producer, recovered)?;
    Ok(RecoveredNominalWorkerPublicationV5 {
        outcome,
        record,
        finalized: finalized.into_nominal_v5()?,
        intent,
    })
}
