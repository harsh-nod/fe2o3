//! Bounded planning and result records for an external GPU-kernel verifier.
//!
//! This crate constructs canonical proof requests and executes an evidence
//! recorder through a bounded, shell-free process boundary. The authenticated
//! Linux path measures Verus, solver, and recorder bytes into sealed executable
//! snapshots and returns descriptive, challenge-bound execution evidence. The
//! legacy planning path retains caller-supplied identities for compatibility.

mod artifact_record;
mod authenticated_execution;
mod executor;
mod model;
mod plan;
mod result;

pub use artifact_record::{
    ArtifactProofEvidenceV1, ArtifactRecordConversionError, ReviewedInvocationIdentityV1,
    canonical_invocation_digest, convert_to_artifact_proof_record,
};
pub use authenticated_execution::{
    AuthenticatedBindingField, AuthenticatedExecutionError, AuthenticatedExecutionProgramsV1,
    AuthenticatedResultError, AuthenticatedVerusExecutionEvidenceV1, BoundExecutionPayloadV1,
    DataOperation, ExecutableMeasurementV1, ExecutableOperation, ExecutableRole,
    MAX_EXECUTABLE_BYTES, execute_authenticated_verus,
};
pub use executor::{
    ExecutionError, ExecutionErrorKind, ExecutionLimits, ExecutionPath, ExecutionStage,
    ExecutionSuccess, MAX_CAPTURE_BYTES, OutputStream, ProcessOutput, execute_recorder,
};
pub use model::{
    AxiomPolicy, Configuration, ConfigurationEntry, CorrelationId, Digest, ExecutionTools,
    MAX_CONFIGURATION_ENTRIES, MAX_PROPERTIES, MAX_TEXT_BYTES, MAX_TRUSTED_ITEMS,
    MeasuredToolIdentity, ModelError, ProofOutcome, ProofProperty, ProofRequestV1,
    ProofTargetIdentity, Text, TrustedItem, VerificationModelIdentity,
};
pub use plan::{
    CommandSpec, InvocationPaths, InvocationPlan, MAX_PATH_BYTES, MAX_TIMEOUT_SECONDS, PlanError,
    VerifierPolicy, build_invocation_plan,
};
pub use result::{
    MAX_RESULT_BYTES, ProofResultV1, RecorderTermination, ResultError, parse_recorder_result,
};
