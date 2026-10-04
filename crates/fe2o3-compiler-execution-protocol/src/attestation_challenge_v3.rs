//! Nominal native subject binding and issuer challenge, without issuance rights.
use crate::{
    CompilerExecutionAttestationErrorV3 as Error,
    CompilerExecutionIssuerPolicyIdentityV3 as PolicyIdentity,
    CompilerExecutionIssuerPolicyV3 as Policy, attestation_request_codec as codec,
    attestation_resources::{self as resources, CompilerExecutionAttestationStorageV3 as Storage},
};
use fe2o3_artifact_transaction::{
    InertCompilerExecutionSubjectStorageV3 as SubjectStorage,
    InertCompilerExecutionSubjectV3 as Subject,
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use std::{fmt, mem::size_of};

crate::attestation_challenge_adapter::attestation_challenge_adapter!(
    V3,
    "3",
    "5",
    COMPILER_EXECUTION_ATTESTATION_CHALLENGE_BYTES_V3,
    COMPILER_EXECUTION_ATTESTATION_CHALLENGE_WORK_V3,
    COMPILER_EXECUTION_ATTESTATION_CHALLENGE_STORAGE_V3,
    CompilerExecutionSubjectBindingV3,
    CompilerExecutionAttestationChallengeIdentityV3,
    CompilerExecutionAttestationChallengeV3
);
