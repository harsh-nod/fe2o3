//! Complete native request. Subject decoding shares the caller's bounded ledger.
use crate::{
    CompilerExecutionAttestationErrorV1 as Framing, CompilerExecutionAttestationErrorV2 as Error,
    attestation_challenge_v2::{
        self as challenge, CompilerExecutionAttestationChallengeV2 as Challenge,
        CompilerExecutionSubjectBindingV2 as Binding,
    },
    attestation_request_codec as codec,
    attestation_resources::{self as resources, CompilerExecutionAttestationStorageV2 as Storage},
};
use fe2o3_artifact_transaction::{
    INERT_COMPILER_EXECUTION_SUBJECT_STORAGE_V2 as SUBJECT_STORAGE,
    INERT_COMPILER_EXECUTION_SUBJECT_WORK_V2 as SUBJECT_WORK,
    InertCompilerExecutionSubjectV2 as Subject,
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use std::{fmt, mem::size_of};

crate::attestation_request_adapter::attestation_request_adapter!(
    V2,
    "2",
    "4",
    COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V2,
    COMPILER_EXECUTION_ATTESTATION_REQUEST_WORK_V2,
    COMPILER_EXECUTION_ATTESTATION_REQUEST_DECODE_WORK_V2,
    COMPILER_EXECUTION_ATTESTATION_REQUEST_STORAGE_V2,
    COMPILER_EXECUTION_ATTESTATION_REQUEST_DECODE_STORAGE_V2,
    CompilerExecutionAttestationRequestIdentityV2,
    CompilerExecutionAttestationRequestV2
);
