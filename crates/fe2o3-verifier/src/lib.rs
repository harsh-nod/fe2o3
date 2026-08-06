//! Bounded planning and result records for an external GPU-kernel verifier.
//!
//! This crate never executes a process. It constructs an argv vector and a
//! canonical request document, then validates the bounded output of a separate
//! evidence recorder. Tool measurements are caller-supplied evidence, not
//! measurements performed or authenticated by this crate.

mod model;
mod plan;
mod result;

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
