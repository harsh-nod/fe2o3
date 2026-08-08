#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

mod codec;
mod compiler_transaction;
mod error;
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
pub use error::{EnvelopeDecodeError, EnvelopeValidationError, PublicationClaimFieldV1};
pub use model::{
    DescriptorLineageV1, ExactRawHsacoV1, MAX_WORKER_V2_LOAD_ENVELOPE_BYTES,
    MAX_WORKER_V2_PROOF_EVIDENCE_BYTES, MAX_WORKER_V2_RAW_HSACO_BYTES, WorkerV2LoadEnvelopeV1,
};
