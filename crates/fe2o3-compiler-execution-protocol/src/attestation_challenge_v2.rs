//! Nominal native subject binding and issuer challenge, without issuance rights.
use crate::{
    CompilerExecutionAttestationErrorV2 as Error,
    CompilerExecutionIssuerPolicyIdentityV2 as PolicyIdentity,
    CompilerExecutionIssuerPolicyV2 as Policy, attestation_request_codec as codec,
    attestation_resources::{self as resources, CompilerExecutionAttestationStorageV2 as Storage},
};
use fe2o3_artifact_transaction::{
    InertCompilerExecutionSubjectStorageV2 as SubjectStorage,
    InertCompilerExecutionSubjectV2 as Subject,
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use std::{fmt, mem::size_of};

crate::attestation_challenge_adapter::attestation_challenge_adapter!(
    V2,
    "2",
    "4",
    COMPILER_EXECUTION_ATTESTATION_CHALLENGE_BYTES_V2,
    COMPILER_EXECUTION_ATTESTATION_CHALLENGE_WORK_V2,
    COMPILER_EXECUTION_ATTESTATION_CHALLENGE_STORAGE_V2,
    CompilerExecutionSubjectBindingV2,
    CompilerExecutionAttestationChallengeIdentityV2,
    CompilerExecutionAttestationChallengeV2
);
