#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

mod codec;
mod compiler_transaction;
mod compiler_transaction_recorder;
mod error;
mod inputs;
mod model;

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
    ExactCompilerInvocationV1, ExactCompilerSourceClosureV1, ExactCompilerSourceFileV1,
    ExactCompilerToolV1, ExactSemanticLayoutWitnessV1, ExactWorkerToolV1, Gfx942CompilerTargetV1,
    MAX_COMPILER_TRANSACTION_SOURCE_BYTES_V1, MAX_COMPILER_TRANSACTION_SOURCE_FILE_BYTES_V1,
    MAX_SEALED_COMPILER_TRANSACTION_BYTES_V1, SEALED_COMPILER_TRANSACTION_MAGIC_V1,
    SEALED_COMPILER_TRANSACTION_VERSION_V1, SealedCompilerTransactionDecodeErrorV1,
    SealedCompilerTransactionIdentityV1, SealedCompilerTransactionV1,
};
pub use error::{EnvelopeDecodeError, EnvelopeValidationError, PublicationClaimFieldV1};
pub use inputs::{
    EnvelopeInputsDecodeError, EnvelopeInputsValidationError, MAX_WORKER_V2_ENVELOPE_INPUTS_BYTES,
    WORKER_V2_ENVELOPE_INPUTS_MAGIC, WORKER_V2_ENVELOPE_INPUTS_VERSION,
    WorkerV2EnvelopeInputsIdentityV1, WorkerV2EnvelopeInputsV1,
};
pub use model::{
    DescriptorLineageV1, ExactRawHsacoV1, MAX_WORKER_V2_LOAD_ENVELOPE_BYTES,
    MAX_WORKER_V2_PROOF_EVIDENCE_BYTES, MAX_WORKER_V2_RAW_HSACO_BYTES,
    WorkerV2LoadEnvelopeIdentityV1, WorkerV2LoadEnvelopeV1,
};
