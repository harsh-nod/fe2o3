#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Target-neutral request and result contracts for the fe2o3 compiler driver.
//!
//! This crate contains no compiler implementation and grants no publication,
//! proof, load, dispatch, or launch authority. Caller-supplied identities and
//! payloads remain untrusted commitments at this boundary.

mod diagnostic;
mod identity;
mod output;
mod receipt;
mod request;
mod snapshot;

pub use diagnostic::{
    CanonicalDiagnosticV1, DiagnosticCodeErrorV1, DiagnosticCodeV1, DiagnosticMessageErrorV1,
    DiagnosticMessageV1, DiagnosticSeverityV1, MAX_DIAGNOSTIC_MESSAGE_BYTES_V1,
};
pub use identity::{
    CandidateFormatIdentityV1, CandidateIdentityV1, CompilerProfileIdentityV1,
    DiagnosticSubjectIdentityV1, IDENTITY_BYTES_V1, KernelInstanceIdentityV1,
    ObligationSetIdentityV1, PipelineConfigurationIdentityV1, RequestIdentityV1,
    SnapshotFormatIdentityV1, SnapshotIdentityV1, TargetProfileIdentityV1,
    TransformConfigurationIdentityV1, TransformIdentityV1,
};
pub use output::{
    CompileDispositionV1, CompileOutputErrorV1, CompileOutputV1, ExecutableCandidateErrorV1,
    ExecutableCandidateV1, MAX_EXECUTABLE_CANDIDATE_BYTES_V1, OutputResourceV1,
};
pub use receipt::{ReceiptOutcomeV1, StageReceiptErrorV1, StageReceiptV1};
pub use request::{
    CompileLimitFieldV1, CompileLimitsErrorV1, CompileLimitsV1, CompileRequestErrorV1,
    CompileRequestV1, MAX_DIAGNOSTICS_V1, MAX_STAGE_RECEIPTS_V1, MAX_STAGE_SNAPSHOTS_V1,
    MAX_TOTAL_SNAPSHOT_BYTES_V1, PipelineSelectorV1,
};
pub use snapshot::{
    CompilerStageV1, MAX_STAGE_SNAPSHOT_BYTES_V1, StageSnapshotErrorV1, StageSnapshotV1,
};

/// Schema version represented by the V1 Rust types in this crate.
pub const COMPILER_API_SCHEMA_VERSION_V1: u16 = 1;
