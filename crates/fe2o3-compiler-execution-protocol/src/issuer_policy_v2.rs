//! SubjectV2 policy admission. Decoding is not provisioning or execution proof.
use crate::{
    CompilerExecutionAttestationErrorV1, CompilerExecutionIssuerMeasurementV1 as Measurement,
    attestation_resources::{self as resources, CompilerExecutionAttestationStorageV2 as Storage},
    issuer_policy_codec as codec,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{error::Error, fmt, mem::size_of};

use fe2o3_artifact_transaction::CompilerExecutionSubjectErrorV2 as SubjectError;
crate::issuer_policy_adapter::issuer_policy_adapter!(
    V2,
    "2",
    "4",
    COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V2,
    COMPILER_EXECUTION_ISSUER_POLICY_WORK_V2,
    COMPILER_EXECUTION_ISSUER_POLICY_STORAGE_V2,
    CompilerExecutionIssuerPolicyIdentityV2,
    CompilerExecutionIssuerPolicyV2,
    CompilerExecutionAttestationErrorV2
);
