#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

mod codec;
mod compiler_transaction;
mod error;
mod model;

pub use codec::{WORKER_V2_LOAD_ENVELOPE_MAGIC, WORKER_V2_LOAD_ENVELOPE_VERSION};
pub use compiler_transaction::{
    BackendInvocationIdentityV1, COMPILER_TRANSACTION_EVIDENCE_MAGIC_V1,
    COMPILER_TRANSACTION_EVIDENCE_VERSION_V1, CompilerSourceClosureV1, CompilerSourceDependencyV1,
    CompilerTransactionDecodeErrorV1, CompilerTransactionEvidenceCapsuleV1,
    CompilerTransactionEvidenceIdentityV1, CompilerTransactionEvidencePartsV1,
    CompilerTransactionValidationErrorV1, KernelIrIdentityV1,
    MAX_COMPILER_TRANSACTION_DEPENDENCIES_V1, MAX_COMPILER_TRANSACTION_EVIDENCE_BYTES_V1,
    MAX_COMPILER_TRANSACTION_FEATURES_V1, RustcInvocationIdentityV1, SemanticWitnessIdentityV1,
    SourceClosureIdentityV1, SourceRootIdentityV1, WorkerV2EnvelopeIdentityV1,
};
pub use error::{EnvelopeDecodeError, EnvelopeValidationError, PublicationClaimFieldV1};
pub use model::{
    DescriptorLineageV1, ExactRawHsacoV1, MAX_WORKER_V2_LOAD_ENVELOPE_BYTES,
    MAX_WORKER_V2_PROOF_EVIDENCE_BYTES, MAX_WORKER_V2_RAW_HSACO_BYTES, WorkerV2LoadEnvelopeV1,
};
