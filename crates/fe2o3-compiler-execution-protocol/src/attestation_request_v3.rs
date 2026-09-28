//! Complete native request. Subject decoding shares the caller's bounded ledger.
//!
//! The public constructor transfers the actual conditional subject:
//! ```
//! use fe2o3_compiler_execution_protocol::{CompilerExecutionAttestationChallengeV3 as C,
//!     CompilerExecutionAttestationRequestV3 as Q, CompilerExecutionAttestationErrorV3 as E,
//!     CompilerExecutionAttestationStorageV3 as S};
//! use fe2o3_artifact_transaction::InertCompilerExecutionSubjectV3;
//! use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
//! fn transfer(c: C, s: InertCompilerExecutionSubjectV3,
//!     b: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) -> Result<(Q, S), E> {
//!     Q::new(c, s, b)
//! }
//! ```
//! ```compile_fail
//! use fe2o3_compiler_execution_protocol::{CompilerExecutionAttestationChallengeV3 as C,
//!     CompilerExecutionAttestationRequestV3 as Q};
//! use fe2o3_artifact_transaction::InertCompilerExecutionSubjectV2;
//! use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
//! fn mix(c: C, s: InertCompilerExecutionSubjectV2,
//!     b: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) { let _ = Q::new(c, s, b); }
//! ```
//! ```compile_fail
//! use fe2o3_compiler_execution_protocol::{CompilerExecutionAttestationRequestV3 as Q3,
//!     CompilerExecutionAttestationRequestV2 as Q2};
//! fn downgrade(q: Q3) -> Q2 { q.into() }
//! ```
use crate::{
    CompilerExecutionAttestationErrorV1 as Framing, CompilerExecutionAttestationErrorV3 as Error,
    attestation_challenge_v3::{
        self as challenge, CompilerExecutionAttestationChallengeV3 as Challenge,
        CompilerExecutionSubjectBindingV3 as Binding,
    },
    attestation_request_codec as codec,
    attestation_resources::{self as resources, CompilerExecutionAttestationStorageV3 as Storage},
};
use fe2o3_artifact_transaction::{
    INERT_COMPILER_EXECUTION_SUBJECT_STORAGE_V3 as SUBJECT_STORAGE,
    INERT_COMPILER_EXECUTION_SUBJECT_WORK_V3 as SUBJECT_WORK,
    InertCompilerExecutionSubjectV3 as Subject,
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use std::{fmt, mem::size_of};

crate::attestation_request_adapter::attestation_request_adapter!(
    V3,
    "3",
    "5",
    COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V3,
    COMPILER_EXECUTION_ATTESTATION_REQUEST_WORK_V3,
    COMPILER_EXECUTION_ATTESTATION_REQUEST_DECODE_WORK_V3,
    COMPILER_EXECUTION_ATTESTATION_REQUEST_STORAGE_V3,
    COMPILER_EXECUTION_ATTESTATION_REQUEST_DECODE_STORAGE_V3,
    CompilerExecutionAttestationRequestIdentityV3,
    CompilerExecutionAttestationRequestV3
);
