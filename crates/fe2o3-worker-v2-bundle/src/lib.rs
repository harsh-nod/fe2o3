#![deny(unsafe_code)]
#![doc = include_str!("../README.md")]

mod application_handoff;
mod codec;
mod compiler_transaction;
mod compiler_transaction_recorder;
mod error;
mod inputs;
mod load_envelope_name;
mod model;
mod protected_transcript_v2;
mod protected_v2;

pub use application_handoff::{
    ApplicationHandoffProtocolErrorV1, MAX_WORKER_V2_ARTIFACT_DIRECTORY_ENTRIES_V1,
    WORKER_V2_APPLICATION_ARTIFACT_DIR_FD_ENV_V1, WORKER_V2_APPLICATION_ENVELOPE_FD_ENV_V1,
    WORKER_V2_APPLICATION_HANDOFF_ACK_BYTES_V1, WORKER_V2_APPLICATION_HANDOFF_ACK_FD_ENV_V1,
    WORKER_V2_APPLICATION_HANDOFF_CHALLENGE_ENV_V1,
    WORKER_V2_APPLICATION_HANDOFF_COMMITMENT_ENV_V1, WorkerV2ApplicationHandoffAckV1,
    WorkerV2ApplicationHandoffChallengeV1, WorkerV2ApplicationHandoffCommitmentV1,
    WorkerV2ApplicationHandoffExpectationV1, WorkerV2ApplicationIdentityV1,
};
pub use codec::{WORKER_V2_LOAD_ENVELOPE_MAGIC, WORKER_V2_LOAD_ENVELOPE_VERSION};
pub use compiler_transaction::{
    CALLER_MEASURED_IDENTITY_ALGORITHM_V2, COMPILER_TRANSACTION_EVIDENCE_MAGIC_V2,
    COMPILER_TRANSACTION_EVIDENCE_VERSION_V2, CallerMeasuredBackendInvocationIdentityV2,
    CallerMeasuredKernelIrIdentityV2, CallerMeasuredSemanticWitnessIdentityV2,
    CallerMeasuredSourceDependencyV2, CallerMeasuredSourceRootIdentityV2, CompilerSourceClosureV2,
    CompilerTransactionDecodeErrorV2, CompilerTransactionEvidenceCapsuleV2,
    CompilerTransactionEvidenceIdentityV2, CompilerTransactionEvidencePartsV2,
    CompilerTransactionValidationErrorV2, MAX_COMPILER_TRANSACTION_DEPENDENCIES_V2,
    MAX_COMPILER_TRANSACTION_EVIDENCE_BYTES_V2, MAX_COMPILER_TRANSACTION_FEATURES_V2,
    SourceClosureIdentityV2,
};
pub use compiler_transaction_recorder::{
    AlphaZetaSemanticLayoutWitnessesV1, AuthenticatedCompilerTransactionExecutionReceiptV1,
    AuthenticatedCompilerTransactionResultV1, CompilerTransactionCheckpointV1,
    CompilerTransactionContentIdentityV1, CompilerTransactionMeasurementsV1,
    CompilerTransactionRecorderErrorV1, CompilerTransactionRecorderV1, CompilerTransactionStageV1,
    ExactCompilerInvocationV1, ExactCompilerInvocationV2, ExactCompilerSourceClosureV1,
    ExactCompilerSourceFileV1, ExactCompilerToolV1, ExactSemanticLayoutWitnessV1,
    ExactWorkerToolV1, Gfx942CompilerTargetV1, MAX_COMPILER_TRANSACTION_SOURCE_BYTES_V1,
    MAX_COMPILER_TRANSACTION_SOURCE_FILE_BYTES_V1, MAX_SEALED_COMPILER_TRANSACTION_BYTES_V1,
    SEALED_COMPILER_TRANSACTION_MAGIC_V1, SEALED_COMPILER_TRANSACTION_VERSION_V1,
    ScalarGemmV1SemanticLayoutWitnessV1, SealedCompilerTransactionDecodeErrorV1,
    SealedCompilerTransactionIdentityV1, SealedCompilerTransactionV1,
};
pub use error::{EnvelopeDecodeError, EnvelopeValidationError, PublicationClaimFieldV1};
pub use inputs::{
    EnvelopeInputsDecodeError, EnvelopeInputsValidationError, MAX_WORKER_V2_ENVELOPE_INPUTS_BYTES,
    WORKER_V2_ENVELOPE_INPUTS_MAGIC, WORKER_V2_ENVELOPE_INPUTS_VERSION,
    WorkerV2EnvelopeInputsIdentityV1, WorkerV2EnvelopeInputsV1,
};
pub use load_envelope_name::{
    WORKER_V2_LOAD_ENVELOPE_NAME_PREFIX_V1, WORKER_V2_LOAD_ENVELOPE_NAME_SUFFIX_V1,
    worker_v2_load_envelope_name_v1,
};
pub use model::{
    DescriptorLineageV1, ExactRawHsacoV1, MAX_WORKER_V2_LOAD_ENVELOPE_BYTES,
    MAX_WORKER_V2_PROOF_EVIDENCE_BYTES, MAX_WORKER_V2_RAW_HSACO_BYTES,
    WorkerV2LoadEnvelopeIdentityV1, WorkerV2LoadEnvelopeV1,
};
pub use protected_transcript_v2::{
    WorkerV2ProducerBindingV2, WorkerV2ProtectedInspectionRouteV2,
    WorkerV2ProtectedInspectionTranscriptIdentityV2, WorkerV2ProtectedInspectionTranscriptV2,
    WorkerV2PublicationIntentTranscriptIdentityV2, WorkerV2PublicationIntentTranscriptV2,
    WorkerV2TranscriptValidationErrorV2,
};
pub use protected_v2::{
    MAX_WORKER_V2_FINAL_ARTIFACT_EVIDENCE_BYTES_V2, MAX_WORKER_V2_LOAD_ENVELOPE_BYTES_V2,
    WORKER_V2_FINAL_ARTIFACT_EVIDENCE_MAGIC_V2, WORKER_V2_FINAL_ARTIFACT_EVIDENCE_VERSION_V2,
    WORKER_V2_LOAD_ENVELOPE_MAGIC_V2, WORKER_V2_LOAD_ENVELOPE_VERSION_V2, WorkerV2AbiIdentityV2,
    WorkerV2DescriptorIdentityV2, WorkerV2FinalArtifactDecodeErrorV2,
    WorkerV2FinalArtifactEvidenceIdentityV2, WorkerV2FinalArtifactEvidenceV2,
    WorkerV2FinalArtifactFieldV2, WorkerV2FinalArtifactValidationErrorV2,
    WorkerV2FinalBytesIdentityV2, WorkerV2LoadEnvelopeDecodeErrorV2,
    WorkerV2LoadEnvelopeIdentityV2, WorkerV2LoadEnvelopeV2, WorkerV2LoadEnvelopeValidationErrorV2,
    WorkerV2ProofOrInspectionIdentityV2, WorkerV2ProofOrInspectionKindV2,
    WorkerV2ResourceIdentityV2, WorkerV2SymbolIdentityV2, WorkerV2TargetIdentityV2,
};
